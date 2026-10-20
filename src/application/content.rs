use crate::domain::{
    Branch, Commit, CommitId, FileContent, Page, PageRequest, RepositoryPath, RepositorySlug,
    Result, WorkspaceKey,
};

use super::{CreateBranchCommand, CreateCommitCommand, RepositoryContent};

/// Lists a page of branches through the repository-content port.
///
/// # Errors
///
/// Returns any domain error produced by the configured adapter.
pub async fn list_branches<C: RepositoryContent>(
    content: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    page: PageRequest,
) -> Result<Page<Branch>> {
    content.list_branches(workspace, repository, page).await
}

/// Creates a branch through the repository-content port.
///
/// # Errors
///
/// Returns any domain error produced by the configured adapter.
pub async fn create_branch<C: RepositoryContent>(
    content: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    command: &CreateBranchCommand,
) -> Result<Branch> {
    content.create_branch(workspace, repository, command).await
}

/// Gets one commit through the repository-content port.
///
/// # Errors
///
/// Returns any domain error produced by the configured adapter.
pub async fn get_commit<C: RepositoryContent>(
    content: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    commit: &CommitId,
) -> Result<Commit> {
    content.get_commit(workspace, repository, commit).await
}

/// Creates a single-file commit through the repository-content port.
///
/// # Errors
///
/// Returns any domain error produced by the configured adapter.
pub async fn create_commit<C: RepositoryContent>(
    content: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    command: &CreateCommitCommand,
) -> Result<Commit> {
    content.create_commit(workspace, repository, command).await
}

/// Reads one bounded raw file through the repository-content port.
///
/// # Errors
///
/// Returns any domain error produced by the configured adapter.
pub async fn read_file<C: RepositoryContent>(
    content: &C,
    workspace: &WorkspaceKey,
    repository: &RepositorySlug,
    path: &RepositoryPath,
    revision: Option<&str>,
    max_bytes: usize,
) -> Result<FileContent> {
    content
        .read_file(workspace, repository, path, revision, max_bytes)
        .await
}
