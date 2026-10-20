use serde::{Deserialize, Serialize};

use super::{Error, Result};

pub const DEFAULT_PAGE_SIZE: u32 = 25;
pub const MAX_PAGE_SIZE: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageRequest {
    pub start: u64,
    pub limit: u32,
}

impl PageRequest {
    /// Creates a bounded page request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ValidationError`] when `limit` is outside the allowed
    /// range.
    pub fn new(start: u64, limit: u32) -> Result<Self> {
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(Error::ValidationError {
                field: "limit",
                message: format!("must be between 1 and {MAX_PAGE_SIZE}"),
            });
        }
        Ok(Self { start, limit })
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            start: 0,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub values: Vec<T>,
    pub next_start: Option<u64>,
}

impl<T> Page<T> {
    #[must_use]
    pub fn new(values: Vec<T>, next_start: Option<u64>) -> Self {
        Self { values, next_start }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_is_bounded() {
        assert!(PageRequest::new(0, 1).is_ok());
        assert!(PageRequest::new(0, MAX_PAGE_SIZE).is_ok());
        assert!(PageRequest::new(0, 0).is_err());
        assert!(PageRequest::new(0, MAX_PAGE_SIZE + 1).is_err());
    }
}
