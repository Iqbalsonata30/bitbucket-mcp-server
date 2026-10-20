use async_trait::async_trait;

use crate::domain::{
    Branch, BranchName, Capability, Commit, CommitId, FileContent, Page, PageRequest, PullRequest,
    PullRequestComment, PullRequestDiff, PullRequestParticipant, Repository, RepositoryPath,
    RepositorySlug, Result, Workspace, WorkspaceKey,
};

/// Outbound application port implemented by Bitbucket infrastructure adapters.
#[async_trait]
pub trait BitbucketCatalog: Send + Sync {
    async fn list_workspaces(&self, page: PageRequest) -> Result<Page<Workspace>>;

    async fn get_workspace(&self, key: &WorkspaceKey) -> Result<Workspace>;

    async fn list_repositories(
        &self,
        workspace: &WorkspaceKey,
        page: PageRequest,
    ) -> Result<Page<Repository>>;

    async fn get_repository(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
    ) -> Result<Repository>;
}

pub trait CapabilityProvider: Send + Sync {
    fn supports(&self, capability: Capability) -> bool;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateBranchCommand {
    pub name: BranchName,
    pub start_point: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateCommitCommand {
    pub branch: BranchName,
    pub path: RepositoryPath,
    pub content: Vec<u8>,
    pub message: Option<String>,
    pub source_commit: Option<CommitId>,
    pub source_branch: Option<BranchName>,
}

#[async_trait]
pub trait RepositoryContent: Send + Sync {
    async fn list_branches(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        page: PageRequest,
    ) -> Result<Page<Branch>>;

    async fn create_branch(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        command: &CreateBranchCommand,
    ) -> Result<Branch>;

    async fn get_commit(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        commit: &CommitId,
    ) -> Result<Commit>;

    async fn create_commit(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        command: &CreateCommitCommand,
    ) -> Result<Commit>;

    async fn read_file(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        path: &RepositoryPath,
        revision: Option<&str>,
        max_bytes: usize,
    ) -> Result<FileContent>;
}

pub trait RepositoryPlatform: BitbucketCatalog + RepositoryContent {}

impl<T> RepositoryPlatform for T where T: BitbucketCatalog + RepositoryContent {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePullRequestCommand {
    pub title: String,
    pub description: Option<String>,
    pub from_ref: String,
    pub to_ref: String,
    pub source_workspace: WorkspaceKey,
    pub source_repository: RepositorySlug,
    pub reviewers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddPullRequestCommentCommand {
    pub text: String,
    pub parent_id: Option<u64>,
}

#[async_trait]
pub trait PullRequests: Send + Sync {
    async fn create_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        command: &CreatePullRequestCommand,
    ) -> Result<PullRequest>;

    async fn list_pull_requests(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        state: Option<&str>,
        page: PageRequest,
    ) -> Result<Page<PullRequest>>;

    async fn get_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
    ) -> Result<PullRequest>;

    async fn approve_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
    ) -> Result<PullRequestParticipant>;

    async fn merge_pull_request(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        version: u64,
    ) -> Result<PullRequest>;

    async fn get_pull_request_diff(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        context_lines: Option<u32>,
        max_bytes: usize,
    ) -> Result<PullRequestDiff>;

    async fn add_pull_request_comment(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        command: &AddPullRequestCommentCommand,
    ) -> Result<PullRequestComment>;

    async fn list_pull_request_comments(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
        id: u64,
        path: Option<&str>,
        page: PageRequest,
    ) -> Result<Page<PullRequestComment>>;
}

pub trait BitbucketPlatform: RepositoryPlatform + PullRequests {}

impl<T> BitbucketPlatform for T where T: RepositoryPlatform + PullRequests {}
