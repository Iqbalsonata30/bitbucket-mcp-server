use serde::{Deserialize, Serialize};

use super::{BranchName, CommitId, RepositoryPath, RepositorySlug, WorkspaceKey};

/// Client-independent representation of a Bitbucket project/workspace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub key: WorkspaceKey,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Repository {
    pub workspace: WorkspaceKey,
    pub slug: RepositorySlug,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
    pub archived: bool,
    pub default_branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub name: BranchName,
    pub latest_commit: CommitId,
    pub default: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitParent {
    pub id: CommitId,
    pub display_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Commit {
    pub id: CommitId,
    pub display_id: String,
    pub message: String,
    pub author: CommitAuthor,
    pub author_timestamp: i64,
    pub committer: CommitAuthor,
    pub committer_timestamp: i64,
    pub parents: Vec<CommitParent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileContent {
    pub path: RepositoryPath,
    pub revision: Option<String>,
    pub media_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestRef {
    pub id: String,
    pub display_id: String,
    pub latest_commit: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestParticipant {
    pub user: User,
    pub role: String,
    pub approved: bool,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub version: u64,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub open: bool,
    pub closed: bool,
    pub from_ref: PullRequestRef,
    pub to_ref: PullRequestRef,
    pub author: User,
    pub reviewers: Vec<PullRequestParticipant>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestComment {
    pub id: u64,
    pub version: u64,
    pub text: String,
    pub author: User,
    pub created_at: i64,
    pub updated_at: i64,
    pub state: Option<String>,
    pub severity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestDiff {
    pub pull_request_id: u64,
    pub text: String,
}
