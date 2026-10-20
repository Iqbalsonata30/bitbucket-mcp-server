use std::{fmt, sync::Arc};

use serde::{Deserialize, Serialize};

use super::{Error, Result};

macro_rules! identifier {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Parses and validates an identifier.
            ///
            /// # Errors
            ///
            /// Returns [`Error::ValidationError`] when the value is empty or
            /// exceeds the supported length.
            pub fn parse(value: impl AsRef<str>) -> Result<Self> {
                let value = value.as_ref().trim();
                if value.is_empty() {
                    return Err(Error::ValidationError {
                        field: $field,
                        message: "must not be empty".to_owned(),
                    });
                }
                if value.len() > 255 {
                    return Err(Error::ValidationError {
                        field: $field,
                        message: "must not exceed 255 bytes".to_owned(),
                    });
                }
                Ok(Self(Arc::from(value)))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;

            fn try_from(value: String) -> Result<Self> {
                Self::parse(value)
            }
        }
    };
}

identifier!(WorkspaceKey, "workspace");
identifier!(RepositorySlug, "repository");
identifier!(BranchName, "branch");
identifier!(CommitId, "commit");

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepositoryPath(Arc<str>);

impl RepositoryPath {
    /// Parses a repository-relative path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ValidationError`] for empty, absolute, parent-traversal,
    /// NUL-containing, or excessively long paths.
    pub fn parse(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref().trim();
        if value.is_empty()
            || value.starts_with('/')
            || value.contains('\0')
            || value
                .split('/')
                .any(|segment| segment.is_empty() || segment == "..")
            || value.len() > 4_096
        {
            return Err(Error::ValidationError {
                field: "path",
                message:
                    "must be a non-empty repository-relative path without empty or '..' segments"
                        .to_owned(),
            });
        }
        Ok(Self(Arc::from(value)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl fmt::Display for RepositoryPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_trims_input() {
        let key = WorkspaceKey::parse("  PROJ  ").expect("valid key");
        assert_eq!(key.as_str(), "PROJ");
    }

    #[test]
    fn identifier_rejects_empty_input() {
        let error = RepositorySlug::parse("  ").expect_err("empty slug must fail");
        assert!(matches!(error, Error::ValidationError { .. }));
    }

    #[test]
    fn repository_path_rejects_parent_traversal() {
        assert!(RepositoryPath::parse("src/../secret").is_err());
        assert!(RepositoryPath::parse("src/lib.rs").is_ok());
    }
}
