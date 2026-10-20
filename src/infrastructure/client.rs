use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use reqwest::{
    RequestBuilder, StatusCode,
    header::{CONTENT_TYPE, HeaderMap, RETRY_AFTER},
    multipart::{Form, Part},
    redirect::Policy,
};
use serde::de::DeserializeOwned;
use thiserror::Error as ThisError;
use tokio::time::sleep;
use tracing::{debug, warn};
use url::Url;

use crate::{
    application::{
        AddPullRequestCommentCommand, BitbucketCatalog, CapabilityProvider, CreateBranchCommand,
        CreateCommitCommand, CreatePullRequestCommand, PullRequests, RepositoryContent,
    },
    config::{Authentication, RetryPolicy, Settings},
    domain::{
        Branch, Capability, Commit, CommitAuthor, CommitId, CommitParent, Error, FileContent, Page,
        PageRequest, PullRequest, PullRequestComment, PullRequestDiff, PullRequestParticipant,
        PullRequestRef, Repository, RepositoryPath, RepositorySlug, Result, User, Workspace,
        WorkspaceKey,
    },
};

use super::dto::{
    BranchDto, CommentDto, CommentParentDto, CommitDto, CreateBranchDto, CreateCommentDto,
    CreateProjectRefDto, CreatePullRequestDto, CreatePullRequestRefDto, CreateRepositoryRefDto,
    CreateReviewerDto, CreateUserRefDto, PageDto, ParticipantDto, ProjectDto, PullRequestDto,
    RepositoryDto, error_message,
};

const API_SEGMENTS: [&str; 3] = ["rest", "api", "1.0"];

#[derive(Clone)]
pub struct BitbucketClient {
    http: reqwest::Client,
    base_url: Url,
    authentication: Authentication,
    retry: RetryPolicy,
    max_response_bytes: usize,
}

impl BitbucketClient {
    /// Builds a reusable client and connection pool from validated settings.
    ///
    /// # Errors
    ///
    /// Returns [`ClientBuildError`] when a CA file cannot be read, contains an
    /// invalid certificate, or the underlying HTTP client cannot be built.
    pub async fn new(settings: Settings) -> std::result::Result<Self, ClientBuildError> {
        if !settings.tls_verify {
            warn!("TLS certificate verification is disabled");
        }
        let mut builder = reqwest::Client::builder()
            .timeout(settings.timeout)
            .connect_timeout(settings.connect_timeout)
            .pool_max_idle_per_host(settings.pool_max_idle_per_host)
            .user_agent(&settings.user_agent)
            .redirect(Policy::none())
            .danger_accept_invalid_certs(!settings.tls_verify);

        if let Some(path) = settings.ca_file.as_ref() {
            let pem = tokio::fs::read(path)
                .await
                .map_err(|source| ClientBuildError::ReadCa {
                    path: path.clone(),
                    source,
                })?;
            let certificate =
                reqwest::Certificate::from_pem(&pem).map_err(ClientBuildError::InvalidCa)?;
            builder = builder.add_root_certificate(certificate);
        }

        let http = builder.build().map_err(ClientBuildError::Build)?;
        Ok(Self {
            http,
            base_url: settings.base_url,
            authentication: settings.authentication,
            retry: settings.retry,
            max_response_bytes: settings.max_response_bytes,
        })
    }

    fn endpoint(&self, resource_segments: &[&str]) -> Result<Url> {
        let mut url = self.base_url.clone();
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| Error::ValidationError {
                field: "base_url",
                message: "cannot be used as a hierarchical URL".to_owned(),
            })?;
        segments
            .pop_if_empty()
            .extend(API_SEGMENTS)
            .extend(resource_segments);
        drop(segments);
        Ok(url)
    }

    fn authenticate(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.authentication {
            Authentication::Token { token } => request.bearer_auth(token.expose()),
            Authentication::Basic { username, password } => {
                request.basic_auth(username, Some(password.expose()))
            }
        }
    }

    async fn get<T>(
        &self,
        url: Url,
        page: Option<PageRequest>,
        context: ResourceContext<'_>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut attempt = 1;
        loop {
            let mut request = self.http.get(url.clone());
            if let Some(page) = page {
                request = request.query(&[("start", page.start), ("limit", u64::from(page.limit))]);
            }
            let result = self.authenticate(request).send().await;
            match result {
                Ok(response) => {
                    let status = response.status();
                    if is_retryable_status(status) && attempt < self.retry.max_attempts {
                        let delay = retry_delay(&self.retry, attempt, response.headers());
                        warn!(attempt, %status, ?delay, "retrying transient Bitbucket response");
                        sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    let body = read_bounded(response, self.max_response_bytes).await?;
                    if !status.is_success() {
                        return Err(map_status(status, &body, context, ConflictContext::Generic));
                    }
                    return serde_json::from_slice(&body).map_err(|error| Error::UpstreamError {
                        message: format!("invalid JSON response for HTTP {status}: {error}"),
                    });
                }
                Err(error)
                    if is_retryable_transport(&error) && attempt < self.retry.max_attempts =>
                {
                    let delay = retry_delay(&self.retry, attempt, &HeaderMap::new());
                    warn!(
                        attempt,
                        ?delay,
                        "retrying transient Bitbucket network failure"
                    );
                    sleep(delay).await;
                    attempt += 1;
                }
                Err(error) => {
                    return Err(Error::NetworkError {
                        message: error.to_string(),
                    });
                }
            }
        }
    }

    async fn send_json<T>(
        &self,
        request: RequestBuilder,
        context: ResourceContext<'_>,
        conflict: ConflictContext<'_>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self
            .authenticate(request)
            .send()
            .await
            .map_err(|error| network_error(&error))?;
        let status = response.status();
        let body = read_bounded(response, self.max_response_bytes).await?;
        if !status.is_success() {
            return Err(map_status(status, &body, context, conflict));
        }
        serde_json::from_slice(&body).map_err(|error| Error::UpstreamError {
            message: format!("invalid JSON response for HTTP {status}: {error}"),
        })
    }

    async fn get_query<T>(
        &self,
        url: Url,
        query: &[(&str, String)],
        context: ResourceContext<'_>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut attempt = 1;
        loop {
            let request = self.http.get(url.clone()).query(query);
            match self.authenticate(request).send().await {
                Ok(response) => {
                    let status = response.status();
                    if is_retryable_status(status) && attempt < self.retry.max_attempts {
                        sleep(retry_delay(&self.retry, attempt, response.headers())).await;
                        attempt += 1;
                        continue;
                    }
                    let body = read_bounded(response, self.max_response_bytes).await?;
                    if !status.is_success() {
                        return Err(map_status(status, &body, context, ConflictContext::Generic));
                    }
                    return serde_json::from_slice(&body).map_err(|error| Error::UpstreamError {
                        message: format!("invalid JSON response for HTTP {status}: {error}"),
                    });
                }
                Err(error)
                    if is_retryable_transport(&error) && attempt < self.retry.max_attempts =>
                {
                    sleep(retry_delay(&self.retry, attempt, &HeaderMap::new())).await;
                    attempt += 1;
                }
                Err(error) => return Err(network_error(&error)),
            }
        }
    }

    async fn get_raw(
        &self,
        url: Url,
        query: &[(&str, String)],
        context: ResourceContext<'_>,
        limit: usize,
    ) -> Result<(Option<String>, Vec<u8>)> {
        let mut attempt = 1;
        loop {
            let request = self.http.get(url.clone()).query(query);
            match self.authenticate(request).send().await {
                Ok(response) => {
                    let status = response.status();
                    if is_retryable_status(status) && attempt < self.retry.max_attempts {
                        let delay = retry_delay(&self.retry, attempt, response.headers());
                        sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    let media_type = response
                        .headers()
                        .get(CONTENT_TYPE)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let body = read_bounded(response, limit.min(self.max_response_bytes)).await?;
                    if !status.is_success() {
                        return Err(map_status(status, &body, context, ConflictContext::Generic));
                    }
                    return Ok((media_type, body));
                }
                Err(error)
                    if is_retryable_transport(&error) && attempt < self.retry.max_attempts =>
                {
                    sleep(retry_delay(&self.retry, attempt, &HeaderMap::new())).await;
                    attempt += 1;
                }
                Err(error) => return Err(network_error(&error)),
            }
        }
    }

    fn pull_requests_endpoint(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        suffix: &[&str],
    ) -> Result<Url> {
        let mut segments = vec![
            "projects",
            workspace.as_str(),
            "repos",
            repository.as_str(),
            "pull-requests",
        ];
        segments.extend_from_slice(suffix);
        self.endpoint(&segments)
    }
}

#[async_trait]
impl BitbucketCatalog for BitbucketClient {
    async fn list_workspaces(&self, page: PageRequest) -> Result<Page<Workspace>> {
        let url = self.endpoint(&["projects"])?;
        let dto: PageDto<ProjectDto> = self.get(url, Some(page), ResourceContext::Generic).await?;
        let values = dto
            .values
            .into_iter()
            .map(project_from_dto)
            .collect::<Result<Vec<_>>>()?;
        Ok(Page::new(values, dto.next_page_start))
    }

    async fn get_workspace(&self, key: &WorkspaceKey) -> Result<Workspace> {
        let url = self.endpoint(&["projects", key.as_str()])?;
        let dto = self
            .get(url, None, ResourceContext::Workspace { key: key.as_str() })
            .await?;
        project_from_dto(dto)
    }

    async fn list_repositories(
        &self,
        workspace: &WorkspaceKey,
        page: PageRequest,
    ) -> Result<Page<Repository>> {
        let url = self.endpoint(&["projects", workspace.as_str(), "repos"])?;
        let dto: PageDto<RepositoryDto> = self
            .get(
                url,
                Some(page),
                ResourceContext::Workspace {
                    key: workspace.as_str(),
                },
            )
            .await?;
        let values = dto
            .values
            .into_iter()
            .map(|repository| repository_from_dto(repository, workspace))
            .collect::<Result<Vec<_>>>()?;
        Ok(Page::new(values, dto.next_page_start))
    }

    async fn get_repository(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
    ) -> Result<Repository> {
        let url = self.endpoint(&["projects", workspace.as_str(), "repos", repository.as_str()])?;
        let dto = self
            .get(
                url,
                None,
                ResourceContext::Repository {
                    workspace: workspace.as_str(),
                    repository: repository.as_str(),
                },
            )
            .await?;
        repository_from_dto(dto, workspace)
    }
}

#[async_trait]
impl RepositoryContent for BitbucketClient {
    async fn list_branches(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        page: PageRequest,
    ) -> Result<Page<Branch>> {
        let url = self.endpoint(&[
            "projects",
            workspace.as_str(),
            "repos",
            repository.as_str(),
            "branches",
        ])?;
        let dto: PageDto<BranchDto> = self
            .get(
                url,
                Some(page),
                ResourceContext::Repository {
                    workspace: workspace.as_str(),
                    repository: repository.as_str(),
                },
            )
            .await?;
        let values = dto
            .values
            .into_iter()
            .map(branch_from_dto)
            .collect::<Result<Vec<_>>>()?;
        Ok(Page::new(values, dto.next_page_start))
    }

    async fn create_branch(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        command: &CreateBranchCommand,
    ) -> Result<Branch> {
        let url = self.endpoint(&[
            "projects",
            workspace.as_str(),
            "repos",
            repository.as_str(),
            "branches",
        ])?;
        let dto = CreateBranchDto {
            name: command.name.as_str(),
            start_point: &command.start_point,
            message: command.message.as_deref(),
        };
        let branch = self
            .send_json(
                self.http.post(url).json(&dto),
                ResourceContext::Repository {
                    workspace: workspace.as_str(),
                    repository: repository.as_str(),
                },
                ConflictContext::Branch {
                    branch: command.name.as_str(),
                },
            )
            .await?;
        branch_from_dto(branch)
    }

    async fn get_commit(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        commit: &CommitId,
    ) -> Result<Commit> {
        let url = self.endpoint(&[
            "projects",
            workspace.as_str(),
            "repos",
            repository.as_str(),
            "commits",
            commit.as_str(),
        ])?;
        let dto = self
            .get(
                url,
                None,
                ResourceContext::Commit {
                    commit: commit.as_str(),
                },
            )
            .await?;
        commit_from_dto(dto)
    }

    async fn create_commit(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        command: &CreateCommitCommand,
    ) -> Result<Commit> {
        let mut segments = vec![
            "projects",
            workspace.as_str(),
            "repos",
            repository.as_str(),
            "browse",
        ];
        segments.extend(command.path.segments());
        let url = self.endpoint(&segments)?;
        let file_name = command.path.segments().last().unwrap_or("content");
        let mut form = Form::new()
            .text("branch", command.branch.as_str().to_owned())
            .part(
                "content",
                Part::bytes(command.content.clone()).file_name(file_name.to_owned()),
            );
        if let Some(message) = &command.message {
            form = form.text("message", message.clone());
        }
        if let Some(source_commit) = &command.source_commit {
            form = form.text("sourceCommitId", source_commit.as_str().to_owned());
        }
        if let Some(source_branch) = &command.source_branch {
            form = form.text("sourceBranch", source_branch.as_str().to_owned());
        }
        let dto = self
            .send_json(
                self.http.put(url).multipart(form),
                ResourceContext::Repository {
                    workspace: workspace.as_str(),
                    repository: repository.as_str(),
                },
                ConflictContext::Commit,
            )
            .await?;
        commit_from_dto(dto)
    }

    async fn read_file(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        path: &RepositoryPath,
        revision: Option<&str>,
        max_bytes: usize,
    ) -> Result<FileContent> {
        if max_bytes == 0 {
            return Err(Error::ValidationError {
                field: "max_bytes",
                message: "must be greater than zero".to_owned(),
            });
        }
        let mut segments = vec![
            "projects",
            workspace.as_str(),
            "repos",
            repository.as_str(),
            "raw",
        ];
        segments.extend(path.segments());
        let url = self.endpoint(&segments)?;
        let query = revision
            .map(|revision| vec![("at", revision.to_owned())])
            .unwrap_or_default();
        let (media_type, bytes) = self
            .get_raw(
                url,
                &query,
                ResourceContext::File {
                    path: path.as_str(),
                },
                max_bytes,
            )
            .await?;
        Ok(FileContent {
            path: path.clone(),
            revision: revision.map(str::to_owned),
            media_type,
            bytes,
        })
    }
}

#[async_trait]
impl PullRequests for BitbucketClient {
    async fn create_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        command: &CreatePullRequestCommand,
    ) -> Result<PullRequest> {
        let url = self.pull_requests_endpoint(workspace, repository, &[])?;
        let request = CreatePullRequestDto {
            title: &command.title,
            description: command.description.as_deref(),
            from_ref: CreatePullRequestRefDto {
                id: &command.from_ref,
                repository: CreateRepositoryRefDto {
                    slug: command.source_repository.as_str(),
                    project: CreateProjectRefDto {
                        key: command.source_workspace.as_str(),
                    },
                },
            },
            to_ref: CreatePullRequestRefDto {
                id: &command.to_ref,
                repository: CreateRepositoryRefDto {
                    slug: repository.as_str(),
                    project: CreateProjectRefDto {
                        key: workspace.as_str(),
                    },
                },
            },
            reviewers: command
                .reviewers
                .iter()
                .map(|name| CreateReviewerDto {
                    user: CreateUserRefDto { name },
                })
                .collect(),
        };
        let dto = self
            .send_json(
                self.http.post(url).json(&request),
                ResourceContext::Repository {
                    workspace: workspace.as_str(),
                    repository: repository.as_str(),
                },
                ConflictContext::PullRequest,
            )
            .await?;
        Ok(pull_request_from_dto(dto))
    }

    async fn list_pull_requests(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        state: Option<&str>,
        page: PageRequest,
    ) -> Result<Page<PullRequest>> {
        let url = self.pull_requests_endpoint(workspace, repository, &[])?;
        let mut query = vec![
            ("start", page.start.to_string()),
            ("limit", page.limit.to_string()),
        ];
        if let Some(state) = state {
            query.push(("state", state.to_owned()));
        }
        let dto: PageDto<PullRequestDto> = self
            .get_query(
                url,
                &query,
                ResourceContext::Repository {
                    workspace: workspace.as_str(),
                    repository: repository.as_str(),
                },
            )
            .await?;
        let values = dto.values.into_iter().map(pull_request_from_dto).collect();
        Ok(Page::new(values, dto.next_page_start))
    }

    async fn get_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
    ) -> Result<PullRequest> {
        let id_string = id.to_string();
        let url = self.pull_requests_endpoint(workspace, repository, &[&id_string])?;
        let dto = self
            .get(url, None, ResourceContext::PullRequest { id })
            .await?;
        Ok(pull_request_from_dto(dto))
    }

    async fn approve_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
    ) -> Result<PullRequestParticipant> {
        let id_string = id.to_string();
        let url = self.pull_requests_endpoint(workspace, repository, &[&id_string, "approve"])?;
        let dto = self
            .send_json(
                self.http.post(url),
                ResourceContext::PullRequest { id },
                ConflictContext::PullRequest,
            )
            .await?;
        Ok(participant_from_dto(dto))
    }

    async fn merge_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        version: u64,
    ) -> Result<PullRequest> {
        let id_string = id.to_string();
        let url = self.pull_requests_endpoint(workspace, repository, &[&id_string, "merge"])?;
        let dto = self
            .send_json(
                self.http.post(url).query(&[("version", version)]),
                ResourceContext::PullRequest { id },
                ConflictContext::Merge,
            )
            .await?;
        Ok(pull_request_from_dto(dto))
    }

    async fn get_pull_request_diff(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        context_lines: Option<u32>,
        max_bytes: usize,
    ) -> Result<PullRequestDiff> {
        if max_bytes == 0 {
            return Err(Error::ValidationError {
                field: "max_bytes",
                message: "must be greater than zero".to_owned(),
            });
        }
        let diff_id = format!("{id}.diff");
        let url = self.pull_requests_endpoint(workspace, repository, &[&diff_id])?;
        let query = context_lines
            .map(|lines| vec![("contextLines", lines.to_string())])
            .unwrap_or_default();
        let (_, bytes) = self
            .get_raw(url, &query, ResourceContext::PullRequest { id }, max_bytes)
            .await?;
        let text = String::from_utf8(bytes).map_err(|error| Error::UpstreamError {
            message: format!("pull request diff is not valid UTF-8: {error}"),
        })?;
        Ok(PullRequestDiff {
            pull_request_id: id,
            text,
        })
    }

    async fn add_pull_request_comment(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        command: &AddPullRequestCommentCommand,
    ) -> Result<PullRequestComment> {
        let id_string = id.to_string();
        let url = self.pull_requests_endpoint(workspace, repository, &[&id_string, "comments"])?;
        let request = CreateCommentDto {
            text: &command.text,
            parent: command.parent_id.map(|id| CommentParentDto { id }),
        };
        let dto = self
            .send_json(
                self.http.post(url).json(&request),
                ResourceContext::PullRequest { id },
                ConflictContext::PullRequest,
            )
            .await?;
        Ok(comment_from_dto(dto))
    }

    async fn list_pull_request_comments(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        path: Option<&str>,
        page: PageRequest,
    ) -> Result<Page<PullRequestComment>> {
        let id_string = id.to_string();
        let url = self.pull_requests_endpoint(workspace, repository, &[&id_string, "comments"])?;
        let mut query = vec![
            ("start", page.start.to_string()),
            ("limit", page.limit.to_string()),
        ];
        if let Some(path) = path {
            query.push(("path", path.to_owned()));
        }
        let dto: PageDto<CommentDto> = self
            .get_query(url, &query, ResourceContext::PullRequest { id })
            .await?;
        Ok(Page::new(
            dto.values.into_iter().map(comment_from_dto).collect(),
            dto.next_page_start,
        ))
    }
}

impl CapabilityProvider for BitbucketClient {
    fn supports(&self, capability: Capability) -> bool {
        matches!(
            capability,
            Capability::Workspaces
                | Capability::Repositories
                | Capability::RepositoryContent
                | Capability::PullRequests
        )
    }
}

#[derive(Debug, ThisError)]
pub enum ClientBuildError {
    #[error("failed to read CA certificate {path}: {source}")]
    ReadCa {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("CA file does not contain a valid PEM certificate: {0}")]
    InvalidCa(reqwest::Error),

    #[error("failed to build HTTP client: {0}")]
    Build(reqwest::Error),
}

#[derive(Clone, Copy)]
enum ResourceContext<'a> {
    Generic,
    Workspace {
        key: &'a str,
    },
    Repository {
        workspace: &'a str,
        repository: &'a str,
    },
    Commit {
        commit: &'a str,
    },
    File {
        path: &'a str,
    },
    PullRequest {
        id: u64,
    },
}

#[derive(Clone, Copy)]
enum ConflictContext<'a> {
    Generic,
    Branch { branch: &'a str },
    Commit,
    PullRequest,
    Merge,
}

fn project_from_dto(dto: ProjectDto) -> Result<Workspace> {
    Ok(Workspace {
        key: WorkspaceKey::parse(dto.key)?,
        name: dto.name,
        description: dto.description,
        public: dto.public,
    })
}

fn repository_from_dto(dto: RepositoryDto, workspace: &WorkspaceKey) -> Result<Repository> {
    Ok(Repository {
        workspace: workspace.clone(),
        slug: RepositorySlug::parse(dto.slug)?,
        name: dto.name,
        description: dto.description,
        public: dto.public,
        archived: dto.archived,
        default_branch: dto.default_branch.map(|branch| branch.display_id),
    })
}

fn branch_from_dto(dto: BranchDto) -> Result<Branch> {
    Ok(Branch {
        id: dto.id,
        name: crate::domain::BranchName::parse(dto.display_id)?,
        latest_commit: CommitId::parse(dto.latest_commit)?,
        default: dto.is_default,
    })
}

fn commit_from_dto(dto: CommitDto) -> Result<Commit> {
    Ok(Commit {
        id: CommitId::parse(dto.id)?,
        display_id: dto.display_id,
        message: dto.message,
        author: CommitAuthor {
            name: dto.author.name,
            email: dto.author.email_address,
        },
        author_timestamp: dto.author_timestamp,
        committer: CommitAuthor {
            name: dto.committer.name,
            email: dto.committer.email_address,
        },
        committer_timestamp: dto.committer_timestamp,
        parents: dto
            .parents
            .into_iter()
            .map(|parent| {
                Ok(CommitParent {
                    id: CommitId::parse(parent.id)?,
                    display_id: parent.display_id,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn user_from_dto(dto: super::dto::UserDto) -> User {
    User {
        name: dto.name,
        display_name: dto.display_name,
        email: dto.email_address,
    }
}

fn participant_from_dto(dto: ParticipantDto) -> PullRequestParticipant {
    PullRequestParticipant {
        user: user_from_dto(dto.user),
        role: dto.role,
        approved: dto.approved,
        status: dto.status,
    }
}

fn pull_request_from_dto(dto: PullRequestDto) -> PullRequest {
    PullRequest {
        id: dto.id,
        version: dto.version,
        title: dto.title,
        description: dto.description,
        state: dto.state,
        open: dto.open,
        closed: dto.closed,
        from_ref: PullRequestRef {
            id: dto.from_ref.id,
            display_id: dto.from_ref.display_id,
            latest_commit: dto.from_ref.latest_commit,
        },
        to_ref: PullRequestRef {
            id: dto.to_ref.id,
            display_id: dto.to_ref.display_id,
            latest_commit: dto.to_ref.latest_commit,
        },
        author: user_from_dto(dto.author.user),
        reviewers: dto
            .reviewers
            .into_iter()
            .map(participant_from_dto)
            .collect(),
        created_at: dto.created_date,
        updated_at: dto.updated_date,
    }
}

fn comment_from_dto(dto: CommentDto) -> PullRequestComment {
    PullRequestComment {
        id: dto.id,
        version: dto.version,
        text: dto.text,
        author: user_from_dto(dto.author),
        created_at: dto.created_date,
        updated_at: dto.updated_date,
        state: dto.state,
        severity: dto.severity,
    }
}

fn map_status(
    status: StatusCode,
    body: &[u8],
    context: ResourceContext<'_>,
    conflict: ConflictContext<'_>,
) -> Error {
    let message = error_message(body).unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        StatusCode::UNAUTHORIZED => Error::AuthenticationError { message },
        StatusCode::BAD_REQUEST => Error::ValidationError {
            field: "request",
            message,
        },
        StatusCode::NOT_FOUND => match context {
            ResourceContext::Workspace { key } => Error::WorkspaceNotFound {
                key: key.to_owned(),
            },
            ResourceContext::Repository {
                workspace,
                repository,
            } => Error::RepositoryNotFound {
                workspace: workspace.to_owned(),
                repository: repository.to_owned(),
            },
            ResourceContext::Generic => Error::UpstreamError { message },
            ResourceContext::Commit { commit } => Error::CommitNotFound {
                commit: commit.to_owned(),
            },
            ResourceContext::File { path } => Error::FileNotFound {
                path: path.to_owned(),
            },
            ResourceContext::PullRequest { id } => Error::PullRequestNotFound { id },
        },
        StatusCode::CONFLICT => match conflict {
            ConflictContext::Branch { branch } => Error::BranchAlreadyExists {
                branch: branch.to_owned(),
            },
            ConflictContext::Commit => Error::CommitConflict { message },
            ConflictContext::PullRequest => Error::PullRequestConflict { message },
            ConflictContext::Merge => Error::MergeConflict { message },
            ConflictContext::Generic => Error::UpstreamError { message },
        },
        _ => Error::UpstreamError { message },
    }
}

fn network_error(error: &reqwest::Error) -> Error {
    Error::NetworkError {
        message: error.to_string(),
    }
}

async fn read_bounded(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|content_length| content_length > u64::try_from(limit).unwrap_or(u64::MAX))
    {
        return Err(response_too_large(limit));
    }

    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(limit),
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| Error::NetworkError {
            message: error.to_string(),
        })?
    {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(response_too_large(limit));
        }
        body.extend_from_slice(&chunk);
    }
    debug!(bytes = body.len(), "received Bitbucket response");
    Ok(body)
}

fn response_too_large(limit: usize) -> Error {
    Error::UpstreamError {
        message: format!("response body exceeds configured limit of {limit} bytes"),
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn is_retryable_transport(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

fn retry_delay(policy: &RetryPolicy, attempt: u32, headers: &HeaderMap) -> Duration {
    if let Some(delay) = headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
    {
        return delay.min(policy.max_delay);
    }

    let multiplier = 1_u128 << attempt.saturating_sub(1).min(20);
    let exponential = policy.base_delay.as_millis().saturating_mul(multiplier);
    let capped = exponential.min(policy.max_delay.as_millis());
    let capped = u64::try_from(capped).unwrap_or(u64::MAX);
    if capped == 0 {
        return Duration::ZERO;
    }
    let jittered = fastrand::u64((capped / 2)..=capped);
    Duration::from_millis(jittered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_is_bounded_by_policy() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, "30".parse().expect("valid header"));
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_secs(2),
        };
        assert_eq!(retry_delay(&policy, 1, &headers), Duration::from_secs(2));
    }

    #[test]
    fn endpoint_preserves_context_path() {
        let client = BitbucketClient {
            http: reqwest::Client::new(),
            base_url: Url::parse("https://example.test/bitbucket/").expect("valid URL"),
            authentication: Authentication::Token {
                token: crate::config::SecretString::new("secret".to_owned()),
            },
            retry: RetryPolicy {
                max_attempts: 1,
                base_delay: Duration::ZERO,
                max_delay: Duration::ZERO,
            },
            max_response_bytes: 1024,
        };

        assert_eq!(
            client
                .endpoint(&["projects"])
                .expect("valid endpoint")
                .as_str(),
            "https://example.test/bitbucket/rest/api/1.0/projects"
        );
    }
}
