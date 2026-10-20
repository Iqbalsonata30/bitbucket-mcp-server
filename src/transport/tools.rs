use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult, Tool},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{
    application::{
        CreateBranchCommand, CreateCommitCommand, RepositoryPlatform, content, repository,
        workspace,
    },
    domain::{
        Branch, BranchName, Commit, CommitId, FileContent, Page, PageRequest, Repository,
        RepositoryPath, RepositorySlug, Workspace, WorkspaceKey,
    },
};

use super::domain_error_result;

const DEFAULT_FILE_LIMIT: usize = 1024 * 1024;
const MAX_FILE_LIMIT: usize = 10 * 1024 * 1024;

pub(super) fn definitions() -> Vec<Tool> {
    vec![
        tool(
            "bitbucket_list_workspaces",
            "List Bitbucket projects exposed as workspaces.",
            paged_schema(Map::new(), &[]),
        ),
        tool(
            "bitbucket_get_workspace",
            "Get one Bitbucket workspace/project.",
            object_schema(
                properties(&[("workspace", string("Workspace/project key"))]),
                &["workspace"],
            ),
        ),
        tool(
            "bitbucket_list_repositories",
            "List repositories in a workspace.",
            paged_schema(
                properties(&[("workspace", string("Workspace/project key"))]),
                &["workspace"],
            ),
        ),
        tool(
            "bitbucket_get_repository",
            "Get one repository.",
            repository_schema(Map::new(), &[]),
        ),
        tool(
            "bitbucket_get_branches",
            "List branches in a repository.",
            paged_schema(repository_properties(), &["workspace", "repository"]),
        ),
        tool(
            "bitbucket_create_branch",
            "Create a branch from a commit or resolvable ref.",
            repository_schema(
                properties(&[
                    ("name", string("New branch name")),
                    (
                        "start_point",
                        string("Commit ID or resolvable starting point"),
                    ),
                    ("message", string("Optional branch creation message")),
                ]),
                &["name", "start_point"],
            ),
        ),
        tool(
            "bitbucket_get_commit",
            "Get a commit by commit ID.",
            repository_schema(
                properties(&[("commit", string("Commit hash"))]),
                &["commit"],
            ),
        ),
        tool(
            "bitbucket_create_commit",
            "Create one commit that creates or replaces one file. Bitbucket Data Center does not provide atomic multi-file commits through its core REST API.",
            repository_schema(
                properties(&[
                    ("branch", string("Existing target branch")),
                    ("path", string("Repository-relative file path")),
                    ("content", string("Complete UTF-8 file content")),
                    ("message", string("Optional commit message")),
                    (
                        "source_commit",
                        string("Current commit ID for optimistic concurrency when updating"),
                    ),
                    (
                        "source_branch",
                        string("Starting branch when creating the target branch"),
                    ),
                ]),
                &["branch", "path", "content"],
            ),
        ),
        tool(
            "bitbucket_read_file",
            "Read a bounded raw file from a repository. Binary data is returned as base64.",
            repository_schema(
                properties(&[
                    ("path", string("Repository-relative file path")),
                    ("revision", string("Optional commit or fully qualified ref")),
                    (
                        "max_bytes",
                        json!({
                            "type": "integer",
                            "minimum": 1,
                            "maximum": MAX_FILE_LIMIT,
                            "default": DEFAULT_FILE_LIMIT,
                            "description": "Maximum response bytes"
                        }),
                    ),
                ]),
                &["path"],
            ),
        ),
    ]
}

pub(super) fn contains(name: &str) -> bool {
    definitions().iter().any(|tool| tool.name.as_ref() == name)
}

#[expect(
    clippy::too_many_lines,
    reason = "the dispatcher keeps one small, explicit branch per independently specified MCP tool"
)]
pub(super) async fn call<C>(
    platform: &C,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData>
where
    C: RepositoryPlatform,
{
    let name = request.name.as_ref();
    match name {
        "bitbucket_list_workspaces" => {
            let input: PageInput = parse_arguments(request.arguments)?;
            execute(workspace::ListWorkspaces::new(platform).execute(input.page()?)).await
        }
        "bitbucket_get_workspace" => {
            let input: WorkspaceInput = parse_arguments(request.arguments)?;
            let key = WorkspaceKey::parse(input.workspace).map_err(invalid_domain_input)?;
            execute(workspace::GetWorkspace::new(platform).execute(&key)).await
        }
        "bitbucket_list_repositories" => {
            let input: WorkspacePageInput = parse_arguments(request.arguments)?;
            let key = WorkspaceKey::parse(input.workspace).map_err(invalid_domain_input)?;
            execute(repository::ListRepositories::new(platform).execute(&key, input.page.page()?))
                .await
        }
        "bitbucket_get_repository" => {
            let input: RepositoryInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.identifiers()?;
            execute(repository::GetRepository::new(platform).execute(&workspace, &repository)).await
        }
        "bitbucket_get_branches" => {
            let input: RepositoryPageInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            execute(content::list_branches(
                platform,
                &workspace,
                &repository,
                input.page.page()?,
            ))
            .await
        }
        "bitbucket_create_branch" => {
            let input: CreateBranchInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            let command = CreateBranchCommand {
                name: BranchName::parse(input.name).map_err(invalid_domain_input)?,
                start_point: input.start_point,
                message: input.message,
            };
            execute(content::create_branch(
                platform,
                &workspace,
                &repository,
                &command,
            ))
            .await
        }
        "bitbucket_get_commit" => {
            let input: GetCommitInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            let commit = CommitId::parse(input.commit).map_err(invalid_domain_input)?;
            execute(content::get_commit(
                platform,
                &workspace,
                &repository,
                &commit,
            ))
            .await
        }
        "bitbucket_create_commit" => {
            let input: CreateCommitInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            let command = CreateCommitCommand {
                branch: BranchName::parse(input.branch).map_err(invalid_domain_input)?,
                path: RepositoryPath::parse(input.path).map_err(invalid_domain_input)?,
                content: input.content.into_bytes(),
                message: input.message,
                source_commit: input
                    .source_commit
                    .map(CommitId::parse)
                    .transpose()
                    .map_err(invalid_domain_input)?,
                source_branch: input
                    .source_branch
                    .map(BranchName::parse)
                    .transpose()
                    .map_err(invalid_domain_input)?,
            };
            execute(content::create_commit(
                platform,
                &workspace,
                &repository,
                &command,
            ))
            .await
        }
        "bitbucket_read_file" => {
            let input: ReadFileInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            let path = RepositoryPath::parse(input.path).map_err(invalid_domain_input)?;
            let max_bytes = input.max_bytes.unwrap_or(DEFAULT_FILE_LIMIT);
            if !(1..=MAX_FILE_LIMIT).contains(&max_bytes) {
                return Err(ErrorData::invalid_params(
                    format!("max_bytes must be between 1 and {MAX_FILE_LIMIT}"),
                    None,
                ));
            }
            execute(content::read_file(
                platform,
                &workspace,
                &repository,
                &path,
                input.revision.as_deref(),
                max_bytes,
            ))
            .await
        }
        _ => Err(ErrorData::new(
            rmcp::model::ErrorCode::METHOD_NOT_FOUND,
            format!("unknown tool: {name}"),
            None,
        )),
    }
}

async fn execute<T, F>(future: F) -> Result<CallToolResult, ErrorData>
where
    T: IntoToolResponse,
    F: Future<Output = crate::domain::Result<T>>,
{
    match future.await {
        Ok(value) => Ok(CallToolResult::structured(value.into_response())),
        Err(error) => Ok(domain_error_result(&error)),
    }
}

trait IntoToolResponse {
    fn into_response(self) -> Value;
}

impl IntoToolResponse for Workspace {
    fn into_response(self) -> Value {
        json!(WorkspaceResponse::from(self))
    }
}

impl IntoToolResponse for Repository {
    fn into_response(self) -> Value {
        json!(RepositoryResponse::from(self))
    }
}

impl IntoToolResponse for Branch {
    fn into_response(self) -> Value {
        json!(BranchResponse::from(self))
    }
}

impl IntoToolResponse for Commit {
    fn into_response(self) -> Value {
        json!(CommitResponse::from(self))
    }
}

impl IntoToolResponse for FileContent {
    fn into_response(self) -> Value {
        json!(FileResponse::from(self))
    }
}

impl IntoToolResponse for Page<Workspace> {
    fn into_response(self) -> Value {
        json!(PageResponse {
            values: self
                .values
                .into_iter()
                .map(WorkspaceResponse::from)
                .collect(),
            next_start: self.next_start,
        })
    }
}

impl IntoToolResponse for Page<Repository> {
    fn into_response(self) -> Value {
        json!(PageResponse {
            values: self
                .values
                .into_iter()
                .map(RepositoryResponse::from)
                .collect(),
            next_start: self.next_start,
        })
    }
}

impl IntoToolResponse for Page<Branch> {
    fn into_response(self) -> Value {
        json!(PageResponse {
            values: self.values.into_iter().map(BranchResponse::from).collect(),
            next_start: self.next_start,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageInput {
    #[serde(default)]
    start: u64,
    #[serde(default = "default_page_limit")]
    limit: u32,
}

impl PageInput {
    fn page(&self) -> Result<PageRequest, ErrorData> {
        PageRequest::new(self.start, self.limit).map_err(invalid_domain_input)
    }
}

impl Default for PageInput {
    fn default() -> Self {
        Self {
            start: 0,
            limit: default_page_limit(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceInput {
    workspace: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspacePageInput {
    workspace: String,
    #[serde(flatten)]
    page: PageInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryInput {
    workspace: String,
    repository: String,
}

impl RepositoryInput {
    fn identifiers(&self) -> Result<(WorkspaceKey, RepositorySlug), ErrorData> {
        Ok((
            WorkspaceKey::parse(&self.workspace).map_err(invalid_domain_input)?,
            RepositorySlug::parse(&self.repository).map_err(invalid_domain_input)?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryPageInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    #[serde(flatten)]
    page: PageInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBranchInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    name: String,
    start_point: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetCommitInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    commit: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCommitInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    branch: String,
    path: String,
    content: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    source_commit: Option<String>,
    #[serde(default)]
    source_branch: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadFileInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    path: String,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[derive(Serialize)]
struct PageResponse<T> {
    values: Vec<T>,
    next_start: Option<u64>,
}

#[derive(Serialize)]
struct WorkspaceResponse {
    key: String,
    name: String,
    description: Option<String>,
    public: bool,
}

impl From<Workspace> for WorkspaceResponse {
    fn from(value: Workspace) -> Self {
        Self {
            key: value.key.to_string(),
            name: value.name,
            description: value.description,
            public: value.public,
        }
    }
}

#[derive(Serialize)]
struct RepositoryResponse {
    workspace: String,
    slug: String,
    name: String,
    description: Option<String>,
    public: bool,
    archived: bool,
    default_branch: Option<String>,
}

impl From<Repository> for RepositoryResponse {
    fn from(value: Repository) -> Self {
        Self {
            workspace: value.workspace.to_string(),
            slug: value.slug.to_string(),
            name: value.name,
            description: value.description,
            public: value.public,
            archived: value.archived,
            default_branch: value.default_branch,
        }
    }
}

#[derive(Serialize)]
struct BranchResponse {
    id: String,
    name: String,
    latest_commit: String,
    default: bool,
}

impl From<Branch> for BranchResponse {
    fn from(value: Branch) -> Self {
        Self {
            id: value.id,
            name: value.name.to_string(),
            latest_commit: value.latest_commit.to_string(),
            default: value.default,
        }
    }
}

#[derive(Serialize)]
struct CommitResponse {
    id: String,
    display_id: String,
    message: String,
    author_name: String,
    author_email: Option<String>,
    author_timestamp: i64,
    committer_name: String,
    committer_email: Option<String>,
    committer_timestamp: i64,
    parent_ids: Vec<String>,
}

impl From<Commit> for CommitResponse {
    fn from(value: Commit) -> Self {
        Self {
            id: value.id.to_string(),
            display_id: value.display_id,
            message: value.message,
            author_name: value.author.name,
            author_email: value.author.email,
            author_timestamp: value.author_timestamp,
            committer_name: value.committer.name,
            committer_email: value.committer.email,
            committer_timestamp: value.committer_timestamp,
            parent_ids: value
                .parents
                .into_iter()
                .map(|parent| parent.id.to_string())
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct FileResponse {
    path: String,
    revision: Option<String>,
    media_type: Option<String>,
    size: usize,
    encoding: &'static str,
    content: String,
}

impl From<FileContent> for FileResponse {
    fn from(value: FileContent) -> Self {
        let size = value.bytes.len();
        match String::from_utf8(value.bytes) {
            Ok(content) => Self {
                path: value.path.to_string(),
                revision: value.revision,
                media_type: value.media_type,
                size,
                encoding: "utf-8",
                content,
            },
            Err(error) => Self {
                path: value.path.to_string(),
                revision: value.revision,
                media_type: value.media_type,
                size,
                encoding: "base64",
                content: BASE64.encode(error.into_bytes()),
            },
        }
    }
}

fn parse_arguments<T: DeserializeOwned>(
    arguments: Option<Map<String, Value>>,
) -> Result<T, ErrorData> {
    serde_json::from_value(Value::Object(arguments.unwrap_or_default()))
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "this function is passed directly to Result::map_err"
)]
fn invalid_domain_input(error: crate::domain::Error) -> ErrorData {
    ErrorData::invalid_params(error.to_string(), None)
}

fn default_page_limit() -> u32 {
    25
}

fn tool(name: &'static str, description: &'static str, schema: Map<String, Value>) -> Tool {
    Tool::new(name, description, Arc::new(schema))
}

fn string(description: &'static str) -> Value {
    json!({ "type": "string", "minLength": 1, "description": description })
}

fn properties(entries: &[(&'static str, Value)]) -> Map<String, Value> {
    entries
        .iter()
        .map(|(name, schema)| ((*name).to_owned(), schema.clone()))
        .collect()
}

fn repository_properties() -> Map<String, Value> {
    properties(&[
        ("workspace", string("Workspace/project key")),
        ("repository", string("Repository slug")),
    ])
}

fn object_schema(properties: Map<String, Value>, required: &[&str]) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("object".to_owned()));
    schema.insert("properties".to_owned(), Value::Object(properties));
    schema.insert("required".to_owned(), json!(required));
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema
}

fn paged_schema(mut properties: Map<String, Value>, required: &[&str]) -> Map<String, Value> {
    properties.insert(
        "start".to_owned(),
        json!({ "type": "integer", "minimum": 0, "default": 0 }),
    );
    properties.insert(
        "limit".to_owned(),
        json!({ "type": "integer", "minimum": 1, "maximum": 1000, "default": 25 }),
    );
    object_schema(properties, required)
}

fn repository_schema(mut extra: Map<String, Value>, extra_required: &[&str]) -> Map<String, Value> {
    let mut properties = repository_properties();
    properties.append(&mut extra);
    let mut required = vec!["workspace", "repository"];
    required.extend_from_slice(extra_required);
    object_schema(properties, &required)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_phase_four_tool_has_a_closed_object_schema() {
        let tools = definitions();
        assert_eq!(tools.len(), 9);
        for tool in tools {
            assert_eq!(tool.input_schema["type"], "object");
            assert_eq!(tool.input_schema["additionalProperties"], false);
        }
    }

    #[test]
    fn binary_file_response_uses_base64() {
        let response = FileResponse::from(FileContent {
            path: RepositoryPath::parse("binary.dat").expect("valid path"),
            revision: None,
            media_type: Some("application/octet-stream".to_owned()),
            bytes: vec![0xff, 0x00],
        });
        assert_eq!(response.encoding, "base64");
        assert_eq!(response.content, "/wA=");
    }
}
