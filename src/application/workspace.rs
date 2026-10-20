use crate::domain::{Page, PageRequest, Result, Workspace, WorkspaceKey};

use super::BitbucketCatalog;

pub struct ListWorkspaces<'a, C> {
    catalog: &'a C,
}

impl<'a, C> ListWorkspaces<'a, C>
where
    C: BitbucketCatalog,
{
    #[must_use]
    pub fn new(catalog: &'a C) -> Self {
        Self { catalog }
    }

    /// Lists one bounded page of workspaces.
    ///
    /// # Errors
    ///
    /// Returns any domain error produced by the catalog adapter.
    pub async fn execute(&self, page: PageRequest) -> Result<Page<Workspace>> {
        self.catalog.list_workspaces(page).await
    }
}

pub struct GetWorkspace<'a, C> {
    catalog: &'a C,
}

impl<'a, C> GetWorkspace<'a, C>
where
    C: BitbucketCatalog,
{
    #[must_use]
    pub fn new(catalog: &'a C) -> Self {
        Self { catalog }
    }

    /// Gets a workspace by its validated key.
    ///
    /// # Errors
    ///
    /// Returns any domain error produced by the catalog adapter.
    pub async fn execute(&self, key: &WorkspaceKey) -> Result<Workspace> {
        self.catalog.get_workspace(key).await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::{
        application::BitbucketCatalog,
        domain::{Repository, RepositorySlug},
    };

    struct FakeCatalog {
        workspace: Workspace,
    }

    #[async_trait]
    impl BitbucketCatalog for FakeCatalog {
        async fn list_workspaces(&self, _page: PageRequest) -> Result<Page<Workspace>> {
            Ok(Page::new(vec![self.workspace.clone()], None))
        }

        async fn get_workspace(&self, _key: &WorkspaceKey) -> Result<Workspace> {
            Ok(self.workspace.clone())
        }

        async fn list_repositories(
            &self,
            _workspace: &WorkspaceKey,
            _page: PageRequest,
        ) -> Result<Page<Repository>> {
            unreachable!("not used by this use-case test")
        }

        async fn get_repository(
            &self,
            _workspace: &WorkspaceKey,
            _repository: &RepositorySlug,
        ) -> Result<Repository> {
            unreachable!("not used by this use-case test")
        }
    }

    #[tokio::test]
    async fn list_use_case_delegates_without_transport_dependencies() {
        let catalog = FakeCatalog {
            workspace: Workspace {
                key: WorkspaceKey::parse("PROJ").expect("valid key"),
                name: "Project".to_owned(),
                description: None,
                public: false,
            },
        };

        let page = ListWorkspaces::new(&catalog)
            .execute(PageRequest::default())
            .await
            .expect("fake catalog succeeds");

        assert_eq!(page.values, vec![catalog.workspace]);
    }
}
