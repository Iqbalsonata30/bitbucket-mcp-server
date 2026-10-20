use crate::domain::{Page, PageRequest, Repository, RepositorySlug, Result, WorkspaceKey};

use super::BitbucketCatalog;

pub struct ListRepositories<'a, C> {
    catalog: &'a C,
}

impl<'a, C> ListRepositories<'a, C>
where
    C: BitbucketCatalog,
{
    #[must_use]
    pub fn new(catalog: &'a C) -> Self {
        Self { catalog }
    }

    /// Lists one bounded page of repositories in a workspace.
    ///
    /// # Errors
    ///
    /// Returns any domain error produced by the catalog adapter.
    pub async fn execute(
        &self,
        workspace: &WorkspaceKey,
        page: PageRequest,
    ) -> Result<Page<Repository>> {
        self.catalog.list_repositories(workspace, page).await
    }
}

pub struct GetRepository<'a, C> {
    catalog: &'a C,
}

impl<'a, C> GetRepository<'a, C>
where
    C: BitbucketCatalog,
{
    #[must_use]
    pub fn new(catalog: &'a C) -> Self {
        Self { catalog }
    }

    /// Gets a repository by workspace and slug.
    ///
    /// # Errors
    ///
    /// Returns any domain error produced by the catalog adapter.
    pub async fn execute(
        &self,
        workspace: &WorkspaceKey,
        repository: &RepositorySlug,
    ) -> Result<Repository> {
        self.catalog.get_repository(workspace, repository).await
    }
}
