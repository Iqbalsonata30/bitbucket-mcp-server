use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bitbucket_mcp_server::{
    application::{
        AddPullRequestCommentCommand, BitbucketCatalog, CreateBranchCommand, CreateCommitCommand,
        CreatePullRequestCommand, PullRequests, RepositoryContent,
    },
    config::{Authentication, RetryPolicy, SecretString, Settings},
    domain::{
        BranchName, CommitId, Error, PageRequest, RepositoryPath, RepositorySlug, WorkspaceKey,
    },
    infrastructure::BitbucketClient,
};
use url::Url;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{body_json, body_string_contains, header, method, path, query_param},
};

fn commit_json() -> serde_json::Value {
    serde_json::json!({
        "id": "abcdef0123456789",
        "displayId": "abcdef0",
        "author": { "name": "Author", "emailAddress": "author@example.test" },
        "authorTimestamp": 1000,
        "committer": { "name": "Committer", "emailAddress": "committer@example.test" },
        "committerTimestamp": 1001,
        "message": "Commit message",
        "parents": [{ "id": "parent0123456789", "displayId": "parent0" }]
    })
}

fn participant_json() -> serde_json::Value {
    serde_json::json!({
        "user": {
            "name": "reviewer",
            "displayName": "Review User",
            "emailAddress": "reviewer@example.test"
        },
        "role": "REVIEWER",
        "approved": true,
        "status": "APPROVED"
    })
}

fn pull_request_json() -> serde_json::Value {
    serde_json::json!({
        "id": 42,
        "version": 3,
        "title": "Feature",
        "description": "Description",
        "state": "OPEN",
        "open": true,
        "closed": false,
        "fromRef": { "id": "refs/heads/feature", "displayId": "feature", "latestCommit": "source123" },
        "toRef": { "id": "refs/heads/main", "displayId": "main", "latestCommit": "target123" },
        "author": {
            "user": {
                "name": "author",
                "displayName": "PR Author",
                "emailAddress": "author@example.test"
            },
            "role": "AUTHOR",
            "approved": false,
            "status": "UNAPPROVED"
        },
        "reviewers": [participant_json()],
        "createdDate": 1000,
        "updatedDate": 2000
    })
}

fn comment_json() -> serde_json::Value {
    serde_json::json!({
        "id": 7,
        "version": 1,
        "text": "Looks good",
        "author": {
            "name": "reviewer",
            "displayName": "Review User",
            "emailAddress": "reviewer@example.test"
        },
        "createdDate": 1000,
        "updatedDate": 1000,
        "state": "OPEN",
        "severity": "NORMAL"
    })
}

fn settings(server: &MockServer, authentication: Authentication) -> Settings {
    Settings {
        base_url: Url::parse(&format!("{}/bitbucket/", server.uri())).expect("valid mock URL"),
        authentication,
        timeout: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        tls_verify: true,
        ca_file: None,
        retry: RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        },
        max_response_bytes: 64 * 1024,
        pool_max_idle_per_host: 2,
        user_agent: "bitbucket-mcp-server-test".to_owned(),
    }
}

fn token_authentication() -> Authentication {
    Authentication::Token {
        token: SecretString::new("test-token".to_owned()),
    }
}

#[tokio::test]
async fn lists_workspaces_with_bearer_auth_and_server_pagination() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects"))
        .and(query_param("start", "40"))
        .and(query_param("limit", "10"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": 1,
            "limit": 10,
            "isLastPage": false,
            "start": 40,
            "nextPageStart": 73,
            "values": [{
                "key": "PRJ",
                "name": "Project",
                "description": "Description",
                "public": false
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let page = client
        .list_workspaces(PageRequest::new(40, 10).expect("valid page"))
        .await
        .expect("request succeeds");

    assert_eq!(page.next_start, Some(73));
    assert_eq!(page.values[0].key.as_str(), "PRJ");
}

#[tokio::test]
async fn gets_repository_with_basic_auth_and_maps_dto() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects/PRJ/repos/example"))
        .and(header("authorization", "Basic dXNlcjpwYXNz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "slug": "example",
            "name": "Example",
            "description": null,
            "public": false,
            "archived": true,
            "defaultBranch": { "displayId": "main" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let authentication = Authentication::Basic {
        username: "user".to_owned(),
        password: SecretString::new("pass".to_owned()),
    };
    let client = BitbucketClient::new(settings(&server, authentication))
        .await
        .expect("client builds");
    let workspace = WorkspaceKey::parse("PRJ").expect("valid workspace");
    let repository = RepositorySlug::parse("example").expect("valid repository");
    let result = client
        .get_repository(&workspace, &repository)
        .await
        .expect("request succeeds");

    assert!(result.archived);
    assert_eq!(result.default_branch.as_deref(), Some("main"));
}

#[derive(Clone)]
struct TransientThenSuccess {
    calls: Arc<AtomicUsize>,
}

impl Respond for TransientThenSuccess {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(503)
        } else {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "key": "PRJ",
                "name": "Project",
                "public": false
            }))
        }
    }
}

#[tokio::test]
async fn retries_transient_get_response_then_succeeds() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects/PRJ"))
        .respond_with(TransientThenSuccess {
            calls: Arc::clone(&calls),
        })
        .expect(2)
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let workspace = WorkspaceKey::parse("PRJ").expect("valid workspace");
    let result = client
        .get_workspace(&workspace)
        .await
        .expect("retry succeeds");

    assert_eq!(result.key, workspace);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn maps_repository_not_found_to_domain_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects/PRJ/repos/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errors": [{ "message": "Repository does not exist" }]
        })))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let error = client
        .get_repository(
            &WorkspaceKey::parse("PRJ").expect("valid workspace"),
            &RepositorySlug::parse("missing").expect("valid repository"),
        )
        .await
        .expect_err("404 must fail");

    assert_eq!(
        error,
        Error::RepositoryNotFound {
            workspace: "PRJ".to_owned(),
            repository: "missing".to_owned(),
        }
    );
}

#[tokio::test]
async fn rejects_malformed_success_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects/PRJ"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let error = client
        .get_workspace(&WorkspaceKey::parse("PRJ").expect("valid workspace"))
        .await
        .expect_err("malformed JSON must fail");

    assert!(matches!(error, Error::UpstreamError { .. }));
}

#[tokio::test]
async fn rejects_response_larger_than_configured_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects/PRJ"))
        .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(128)))
        .mount(&server)
        .await;

    let mut client_settings = settings(&server, token_authentication());
    client_settings.max_response_bytes = 32;
    let client = BitbucketClient::new(client_settings)
        .await
        .expect("client builds");
    let error = client
        .get_workspace(&WorkspaceKey::parse("PRJ").expect("valid workspace"))
        .await
        .expect_err("oversized body must fail");

    assert_eq!(
        error,
        Error::UpstreamError {
            message: "response body exceeds configured limit of 32 bytes".to_owned(),
        }
    );
}

#[tokio::test]
async fn lists_branches_and_preserves_next_page_start() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/branches",
        ))
        .and(query_param("start", "5"))
        .and(query_param("limit", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "nextPageStart": 19,
            "values": [{
                "id": "refs/heads/main",
                "displayId": "main",
                "latestCommit": "abcdef0123456789",
                "isDefault": true
            }]
        })))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let result = client
        .list_branches(
            &WorkspaceKey::parse("PRJ").expect("workspace"),
            &RepositorySlug::parse("example").expect("repository"),
            PageRequest::new(5, 2).expect("page"),
        )
        .await
        .expect("branches returned");

    assert_eq!(result.next_start, Some(19));
    assert!(result.values[0].default);
}

#[tokio::test]
async fn creates_branch_with_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/branches",
        ))
        .and(body_json(serde_json::json!({
            "name": "feature/test",
            "startPoint": "abcdef",
            "message": "Create branch"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "refs/heads/feature/test",
            "displayId": "feature/test",
            "latestCommit": "abcdef",
            "isDefault": false
        })))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let result = client
        .create_branch(
            &WorkspaceKey::parse("PRJ").expect("workspace"),
            &RepositorySlug::parse("example").expect("repository"),
            &CreateBranchCommand {
                name: BranchName::parse("feature/test").expect("branch"),
                start_point: "abcdef".to_owned(),
                message: Some("Create branch".to_owned()),
            },
        )
        .await
        .expect("branch created");

    assert_eq!(result.name.as_str(), "feature/test");
}

#[tokio::test]
async fn gets_commit_and_maps_people_and_parents() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/commits/abcdef",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(commit_json()))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let result = client
        .get_commit(
            &WorkspaceKey::parse("PRJ").expect("workspace"),
            &RepositorySlug::parse("example").expect("repository"),
            &CommitId::parse("abcdef").expect("commit"),
        )
        .await
        .expect("commit returned");

    assert_eq!(result.author.email.as_deref(), Some("author@example.test"));
    assert_eq!(result.parents[0].display_id, "parent0");
}

#[tokio::test]
async fn creates_single_file_commit_as_multipart() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path(
            "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/browse/src/lib.rs",
        ))
        .and(body_string_contains("name=\"branch\""))
        .and(body_string_contains("feature/test"))
        .and(body_string_contains("name=\"sourceCommitId\""))
        .and(body_string_contains("previous"))
        .and(body_string_contains("fn main() {}"))
        .respond_with(ResponseTemplate::new(200).set_body_json(commit_json()))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let result = client
        .create_commit(
            &WorkspaceKey::parse("PRJ").expect("workspace"),
            &RepositorySlug::parse("example").expect("repository"),
            &CreateCommitCommand {
                branch: BranchName::parse("feature/test").expect("branch"),
                path: RepositoryPath::parse("src/lib.rs").expect("path"),
                content: b"fn main() {}".to_vec(),
                message: Some("Update file".to_owned()),
                source_commit: Some(CommitId::parse("previous").expect("source commit")),
                source_branch: None,
            },
        )
        .await
        .expect("commit created");

    assert_eq!(result.id.as_str(), "abcdef0123456789");
}

#[tokio::test]
async fn reads_raw_file_at_revision() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/raw/config/app.toml",
        ))
        .and(query_param("at", "refs/heads/main"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain")
                .set_body_string("enabled = true"),
        )
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let result = client
        .read_file(
            &WorkspaceKey::parse("PRJ").expect("workspace"),
            &RepositorySlug::parse("example").expect("repository"),
            &RepositoryPath::parse("config/app.toml").expect("path"),
            Some("refs/heads/main"),
            1024,
        )
        .await
        .expect("file returned");

    assert_eq!(result.bytes, b"enabled = true");
    assert_eq!(result.media_type.as_deref(), Some("text/plain"));
}

#[tokio::test]
async fn creates_lists_and_gets_pull_requests() {
    let server = MockServer::start().await;
    let endpoint = "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/pull-requests";
    Mock::given(method("POST"))
        .and(path(endpoint))
        .and(body_json(serde_json::json!({
            "title": "Feature",
            "description": "Description",
            "fromRef": {
                "id": "refs/heads/feature",
                "repository": { "slug": "example", "project": { "key": "PRJ" } }
            },
            "toRef": {
                "id": "refs/heads/main",
                "repository": { "slug": "example", "project": { "key": "PRJ" } }
            },
            "reviewers": [{ "user": { "name": "reviewer" } }]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(pull_request_json()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(endpoint))
        .and(query_param("state", "OPEN"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [pull_request_json()],
            "nextPageStart": 50
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{endpoint}/42")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pull_request_json()))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let workspace = WorkspaceKey::parse("PRJ").expect("workspace");
    let repository = RepositorySlug::parse("example").expect("repository");
    let created = client
        .create_pull_request(
            &workspace,
            &repository,
            &CreatePullRequestCommand {
                title: "Feature".to_owned(),
                description: Some("Description".to_owned()),
                from_ref: "refs/heads/feature".to_owned(),
                to_ref: "refs/heads/main".to_owned(),
                source_workspace: workspace.clone(),
                source_repository: repository.clone(),
                reviewers: vec!["reviewer".to_owned()],
            },
        )
        .await
        .expect("PR created");
    let listed = client
        .list_pull_requests(
            &workspace,
            &repository,
            Some("OPEN"),
            PageRequest::default(),
        )
        .await
        .expect("PRs listed");
    let fetched = client
        .get_pull_request(&workspace, &repository, 42)
        .await
        .expect("PR fetched");

    assert_eq!(created.id, 42);
    assert_eq!(listed.next_start, Some(50));
    assert_eq!(fetched.reviewers[0].user.name, "reviewer");
}

#[tokio::test]
async fn approves_and_merges_pull_request() {
    let server = MockServer::start().await;
    let endpoint = "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/pull-requests/42";
    Mock::given(method("POST"))
        .and(path(format!("{endpoint}/approve")))
        .respond_with(ResponseTemplate::new(200).set_body_json(participant_json()))
        .mount(&server)
        .await;
    let mut merged = pull_request_json();
    merged["state"] = serde_json::json!("MERGED");
    merged["open"] = serde_json::json!(false);
    merged["closed"] = serde_json::json!(true);
    Mock::given(method("POST"))
        .and(path(format!("{endpoint}/merge")))
        .and(query_param("version", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(merged))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let workspace = WorkspaceKey::parse("PRJ").expect("workspace");
    let repository = RepositorySlug::parse("example").expect("repository");
    let participant = client
        .approve_pull_request(&workspace, &repository, 42)
        .await
        .expect("PR approved");
    let result = client
        .merge_pull_request(&workspace, &repository, 42, 3)
        .await
        .expect("PR merged");

    assert!(participant.approved);
    assert_eq!(result.state, "MERGED");
}

#[tokio::test]
async fn gets_bounded_raw_pull_request_diff() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/pull-requests/42.diff",
        ))
        .and(query_param("contextLines", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_string("diff --git a/a b/a"))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let result = client
        .get_pull_request_diff(
            &WorkspaceKey::parse("PRJ").expect("workspace"),
            &RepositorySlug::parse("example").expect("repository"),
            42,
            Some(5),
            1024,
        )
        .await
        .expect("diff returned");

    assert!(result.text.starts_with("diff --git"));
}

#[tokio::test]
async fn adds_and_lists_pull_request_comments() {
    let server = MockServer::start().await;
    let endpoint = "/bitbucket/rest/api/1.0/projects/PRJ/repos/example/pull-requests/42/comments";
    Mock::given(method("POST"))
        .and(path(endpoint))
        .and(body_json(serde_json::json!({
            "text": "Looks good",
            "parent": { "id": 5 }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(comment_json()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(endpoint))
        .and(query_param("path", "src/lib.rs"))
        .and(query_param("start", "0"))
        .and(query_param("limit", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "values": [comment_json()]
        })))
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let workspace = WorkspaceKey::parse("PRJ").expect("workspace");
    let repository = RepositorySlug::parse("example").expect("repository");
    let comment = client
        .add_pull_request_comment(
            &workspace,
            &repository,
            42,
            &AddPullRequestCommentCommand {
                text: "Looks good".to_owned(),
                parent_id: Some(5),
            },
        )
        .await
        .expect("comment added");
    let comments = client
        .list_pull_request_comments(
            &workspace,
            &repository,
            42,
            Some("src/lib.rs"),
            PageRequest::default(),
        )
        .await
        .expect("comments listed");

    assert_eq!(comment.id, 7);
    assert_eq!(comments.values[0].text, "Looks good");
}

#[tokio::test]
async fn reuses_client_safely_across_concurrent_requests() {
    const REQUESTS: usize = 64;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bitbucket/rest/api/1.0/projects/PRJ"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "key": "PRJ",
            "name": "Project",
            "public": false
        })))
        .expect(REQUESTS as u64)
        .mount(&server)
        .await;

    let client = BitbucketClient::new(settings(&server, token_authentication()))
        .await
        .expect("client builds");
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..REQUESTS {
        let client = client.clone();
        tasks.spawn(async move {
            client
                .get_workspace(&WorkspaceKey::parse("PRJ").expect("workspace"))
                .await
        });
    }

    while let Some(result) = tasks.join_next().await {
        let workspace = result.expect("task completes").expect("request succeeds");
        assert_eq!(workspace.key.as_str(), "PRJ");
    }
}
