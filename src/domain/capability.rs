use serde::{Deserialize, Serialize};

/// A feature exposed by an infrastructure adapter.
///
/// Some Bitbucket Server/Data Center installations provide features through
/// version-specific APIs or installed applications. Capability discovery keeps
/// those differences out of application and transport code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Workspaces,
    Repositories,
    PullRequests,
    RepositoryContent,
}
