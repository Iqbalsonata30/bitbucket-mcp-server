use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Stable failures understood by application use cases and MCP error mapping.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("authentication failed: {message}")]
    AuthenticationError { message: String },

    #[error("workspace not found: {key}")]
    WorkspaceNotFound { key: String },

    #[error("repository not found: {workspace}/{repository}")]
    RepositoryNotFound {
        workspace: String,
        repository: String,
    },

    #[error("commit not found: {commit}")]
    CommitNotFound { commit: String },

    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("commit conflict: {message}")]
    CommitConflict { message: String },

    #[error("pull request not found: {id}")]
    PullRequestNotFound { id: u64 },

    #[error("pull request conflict: {message}")]
    PullRequestConflict { message: String },

    #[error("merge conflict: {message}")]
    MergeConflict { message: String },

    #[error("branch already exists: {branch}")]
    BranchAlreadyExists { branch: String },

    #[error("invalid {field}: {message}")]
    ValidationError {
        field: &'static str,
        message: String,
    },

    #[error("network request failed: {message}")]
    NetworkError { message: String },

    #[error("Bitbucket returned an unexpected response: {message}")]
    UpstreamError { message: String },

    #[error("capability is unavailable on this Bitbucket installation: {capability}")]
    UnsupportedCapability { capability: &'static str },
}
