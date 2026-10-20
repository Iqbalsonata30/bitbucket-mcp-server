#![no_main]

use bitbucket_mcp_server::domain::RepositoryPath;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &str| {
    if let Ok(path) = RepositoryPath::parse(input) {
        assert!(!path.as_str().is_empty());
        assert!(!path.as_str().starts_with('/'));
        assert!(
            !path
                .segments()
                .any(|segment| segment.is_empty() || segment == "..")
        );
    }
});
