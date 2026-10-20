use bitbucket_mcp_server::{
    config::SecretString,
    domain::{MAX_PAGE_SIZE, PageRequest, RepositoryPath},
};
use proptest::prelude::*;

proptest! {
    #[test]
    fn valid_page_limits_round_trip(start in any::<u64>(), limit in 1_u32..=MAX_PAGE_SIZE) {
        let page = PageRequest::new(start, limit).expect("generated page is valid");
        prop_assert_eq!(page.start, start);
        prop_assert_eq!(page.limit, limit);
    }

    #[test]
    fn invalid_page_limits_are_rejected(limit in (MAX_PAGE_SIZE + 1)..=u32::MAX) {
        prop_assert!(PageRequest::new(0, limit).is_err());
    }

    #[test]
    fn accepted_repository_paths_round_trip(segments in prop::collection::vec("[a-zA-Z0-9_.-]{1,24}", 1..12)) {
        prop_assume!(segments.iter().all(|segment| segment != ".."));
        let input = segments.join("/");
        let path = RepositoryPath::parse(&input).expect("generated path is valid");
        prop_assert_eq!(path.as_str(), input);
    }

    #[test]
    fn secrets_never_appear_in_debug(secret in "[a-zA-Z0-9]{8,64}") {
        let debug = format!("{:?}", SecretString::new(secret.clone()));
        prop_assert_eq!(debug.as_str(), "[REDACTED]");
        prop_assert!(!debug.contains(&secret));
    }
}
