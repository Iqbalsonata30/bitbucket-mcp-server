use crate::domain::{
    Page, PageRequest, PullRequest, PullRequestComment, PullRequestDiff, PullRequestParticipant,
    RepositorySlug, Result, WorkspaceKey,
};

use super::{AddPullRequestCommentCommand, CreatePullRequestCommand, PullRequests};

/// Creates a pull request.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn create<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    command: &CreatePullRequestCommand,
) -> Result<PullRequest> {
    port.create_pull_request(workspace, repository, command)
        .await
}

/// Lists pull requests.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn list<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    state: Option<&str>,
    page: PageRequest,
) -> Result<Page<PullRequest>> {
    port.list_pull_requests(workspace, repository, state, page)
        .await
}

/// Gets a pull request.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn get<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    id: u64,
) -> Result<PullRequest> {
    port.get_pull_request(workspace, repository, id).await
}

/// Approves a pull request as the authenticated user.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn approve<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    id: u64,
) -> Result<PullRequestParticipant> {
    port.approve_pull_request(workspace, repository, id).await
}

/// Merges a pull request using its current version.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn merge<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    id: u64,
    version: u64,
) -> Result<PullRequest> {
    port.merge_pull_request(workspace, repository, id, version)
        .await
}

/// Gets a bounded raw pull-request diff.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn diff<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    id: u64,
    context_lines: Option<u32>,
    max_bytes: usize,
) -> Result<PullRequestDiff> {
    port.get_pull_request_diff(workspace, repository, id, context_lines, max_bytes)
        .await
}

/// Adds a pull-request comment.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn comment<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    id: u64,
    command: &AddPullRequestCommentCommand,
) -> Result<PullRequestComment> {
    port.add_pull_request_comment(workspace, repository, id, command)
        .await
}

/// Lists pull-request comments.
///
/// # Errors
/// Returns any domain error produced by the configured adapter.
pub async fn comments<C: PullRequests>(
    port: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    id: u64,
    path: Option<&str>,
    page: PageRequest,
) -> Result<Page<PullRequestComment>> {
    port.list_pull_request_comments(workspace, repository, id, path, page)
        .await
}
