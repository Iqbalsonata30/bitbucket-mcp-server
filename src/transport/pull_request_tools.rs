use std::sync::Arc;

use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult, ErrorCode, Tool},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::{
    application::{
        AddPullRequestCommentCommand, BitbucketPlatform, CreatePullRequestCommand, pull_request,
    },
    domain::{
        Page, PageRequest, PullRequest, PullRequestComment, PullRequestDiff,
        PullRequestParticipant, RepositorySlug, WorkspaceKey,
    },
};

use super::domain_error_result;

const DEFAULT_DIFF_LIMIT: usize = 2 * 1024 * 1024;
const MAX_DIFF_LIMIT: usize = 10 * 1024 * 1024;

#[expect(
    clippy::too_many_lines,
    reason = "the declarative registry keeps all pull-request tool contracts visible together"
)]
pub(super) fn definitions() -> Vec<Tool> {
    vec![
        tool(
            "bitbucket_create_pull_request",
            "Create a pull request in a Bitbucket repository.",
            repository_schema(
                properties(&[
                    ("title", string("Pull request title")),
                    ("description", string("Optional pull request description")),
                    (
                        "from_ref",
                        string("Source branch or tag, preferably refs/heads/..."),
                    ),
                    ("to_ref", string("Target branch, preferably refs/heads/...")),
                    (
                        "source_workspace",
                        string("Optional source workspace for a fork"),
                    ),
                    (
                        "source_repository",
                        string("Optional source repository for a fork"),
                    ),
                    (
                        "reviewers",
                        json!({
                            "type": "array",
                            "items": { "type": "string", "minLength": 1 },
                            "default": []
                        }),
                    ),
                ]),
                &["title", "from_ref", "to_ref"],
            ),
        ),
        tool(
            "bitbucket_list_pull_requests",
            "List pull requests in a repository.",
            paged_repository_schema(
                properties(&[(
                    "state",
                    json!({
                        "type": "string",
                        "enum": ["OPEN", "DECLINED", "MERGED", "ALL"],
                        "description": "Pull request state; defaults to OPEN"
                    }),
                )]),
                &[],
            ),
        ),
        tool(
            "bitbucket_get_pull_request",
            "Get one pull request.",
            pull_request_schema(Map::new(), &[]),
        ),
        tool(
            "bitbucket_approve_pull_request",
            "Approve a pull request as the authenticated user.",
            pull_request_schema(Map::new(), &[]),
        ),
        tool(
            "bitbucket_merge_pull_request",
            "Merge a pull request using its current optimistic-lock version.",
            pull_request_schema(
                properties(&[(
                    "version",
                    json!({ "type": "integer", "minimum": 0, "description": "Current pull request version" }),
                )]),
                &["version"],
            ),
        ),
        tool(
            "bitbucket_get_pull_request_diff",
            "Get a bounded raw unified diff for a pull request.",
            pull_request_schema(
                properties(&[
                    (
                        "context_lines",
                        json!({ "type": "integer", "minimum": 0, "maximum": 1000 }),
                    ),
                    (
                        "max_bytes",
                        json!({
                            "type": "integer",
                            "minimum": 1,
                            "maximum": MAX_DIFF_LIMIT,
                            "default": DEFAULT_DIFF_LIMIT
                        }),
                    ),
                ]),
                &[],
            ),
        ),
        tool(
            "bitbucket_comment_pull_request",
            "Add a general comment or reply to a pull request.",
            pull_request_schema(
                properties(&[
                    ("text", string("Comment text")),
                    (
                        "parent_id",
                        json!({ "type": "integer", "minimum": 1, "description": "Optional parent comment ID" }),
                    ),
                ]),
                &["text"],
            ),
        ),
        tool(
            "bitbucket_list_pull_request_comments",
            "List pull request comments, optionally restricted to a repository path.",
            paged_pull_request_schema(
                properties(&[("path", string("Optional repository path"))]),
                &[],
            ),
        ),
    ]
}

#[expect(
    clippy::too_many_lines,
    reason = "the dispatcher keeps one explicit branch per independently specified pull-request tool"
)]
pub(super) async fn call<C>(
    platform: &C,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData>
where
    C: BitbucketPlatform,
{
    match request.name.as_ref() {
        "bitbucket_create_pull_request" => {
            let input: CreateInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            let source_workspace = input
                .source_workspace
                .as_deref()
                .map(WorkspaceKey::parse)
                .transpose()
                .map_err(invalid_input)?
                .unwrap_or_else(|| workspace.clone());
            let source_repository = input
                .source_repository
                .as_deref()
                .map(RepositorySlug::parse)
                .transpose()
                .map_err(invalid_input)?
                .unwrap_or_else(|| repository.clone());
            let command = CreatePullRequestCommand {
                title: input.title,
                description: input.description,
                from_ref: input.from_ref,
                to_ref: input.to_ref,
                source_workspace,
                source_repository,
                reviewers: input.reviewers,
            };
            execute(pull_request::create(
                platform,
                &workspace,
                &repository,
                &command,
            ))
            .await
        }
        "bitbucket_list_pull_requests" => {
            let input: ListInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            execute(pull_request::list(
                platform,
                &workspace,
                &repository,
                input.state.as_deref(),
                input.page.page()?,
            ))
            .await
        }
        "bitbucket_get_pull_request" => {
            let input: PullRequestInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            execute(pull_request::get(
                platform,
                &workspace,
                &repository,
                input.pull_request_id,
            ))
            .await
        }
        "bitbucket_approve_pull_request" => {
            let input: PullRequestInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.repository.identifiers()?;
            execute(pull_request::approve(
                platform,
                &workspace,
                &repository,
                input.pull_request_id,
            ))
            .await
        }
        "bitbucket_merge_pull_request" => {
            let input: MergeInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.pull_request.repository.identifiers()?;
            execute(pull_request::merge(
                platform,
                &workspace,
                &repository,
                input.pull_request.pull_request_id,
                input.version,
            ))
            .await
        }
        "bitbucket_get_pull_request_diff" => {
            let input: DiffInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.pull_request.repository.identifiers()?;
            let max_bytes = input.max_bytes.unwrap_or(DEFAULT_DIFF_LIMIT);
            validate_limit(max_bytes)?;
            execute(pull_request::diff(
                platform,
                &workspace,
                &repository,
                input.pull_request.pull_request_id,
                input.context_lines,
                max_bytes,
            ))
            .await
        }
        "bitbucket_comment_pull_request" => {
            let input: CommentInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.pull_request.repository.identifiers()?;
            let command = AddPullRequestCommentCommand {
                text: input.text,
                parent_id: input.parent_id,
            };
            execute(pull_request::comment(
                platform,
                &workspace,
                &repository,
                input.pull_request.pull_request_id,
                &command,
            ))
            .await
        }
        "bitbucket_list_pull_request_comments" => {
            let input: CommentsInput = parse_arguments(request.arguments)?;
            let (workspace, repository) = input.pull_request.repository.identifiers()?;
            execute(pull_request::comments(
                platform,
                &workspace,
                &repository,
                input.pull_request.pull_request_id,
                input.path.as_deref(),
                input.page.page()?,
            ))
            .await
        }
        name => Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            format!("unknown tool: {name}"),
            None,
        )),
    }
}

async fn execute<T, F>(future: F) -> Result<CallToolResult, ErrorData>
where
    T: IntoResponse,
    F: Future<Output = crate::domain::Result<T>>,
{
    match future.await {
        Ok(value) => Ok(CallToolResult::structured(value.into_response())),
        Err(error) => Ok(domain_error_result(&error)),
    }
}

trait IntoResponse {
    fn into_response(self) -> Value;
}

impl IntoResponse for PullRequest {
    fn into_response(self) -> Value {
        json!(PullRequestResponse::from(self))
    }
}

impl IntoResponse for PullRequestParticipant {
    fn into_response(self) -> Value {
        json!(ParticipantResponse::from(self))
    }
}

impl IntoResponse for PullRequestComment {
    fn into_response(self) -> Value {
        json!(CommentResponse::from(self))
    }
}

impl IntoResponse for PullRequestDiff {
    fn into_response(self) -> Value {
        json!({ "pull_request_id": self.pull_request_id, "diff": self.text })
    }
}

impl IntoResponse for Page<PullRequest> {
    fn into_response(self) -> Value {
        json!({
            "values": self.values.into_iter().map(PullRequestResponse::from).collect::<Vec<_>>(),
            "next_start": self.next_start
        })
    }
}

impl IntoResponse for Page<PullRequestComment> {
    fn into_response(self) -> Value {
        json!({
            "values": self.values.into_iter().map(CommentResponse::from).collect::<Vec<_>>(),
            "next_start": self.next_start
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryInput {
    workspace: String,
    repository: String,
}

impl RepositoryInput {
    fn identifiers(&self) -> Result<(WorkspaceKey, RepositorySlug), ErrorData> {
        Ok((
            WorkspaceKey::parse(&self.workspace).map_err(invalid_input)?,
            RepositorySlug::parse(&self.repository).map_err(invalid_input)?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageInput {
    #[serde(default)]
    start: u64,
    #[serde(default = "default_page_limit")]
    limit: u32,
}

impl PageInput {
    fn page(&self) -> Result<PageRequest, ErrorData> {
        PageRequest::new(self.start, self.limit).map_err(invalid_input)
    }
}

impl Default for PageInput {
    fn default() -> Self {
        Self {
            start: 0,
            limit: default_page_limit(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    title: String,
    #[serde(default)]
    description: Option<String>,
    from_ref: String,
    to_ref: String,
    #[serde(default)]
    source_workspace: Option<String>,
    #[serde(default)]
    source_repository: Option<String>,
    #[serde(default)]
    reviewers: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    #[serde(flatten)]
    page: PageInput,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PullRequestInput {
    #[serde(flatten)]
    repository: RepositoryInput,
    pull_request_id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MergeInput {
    #[serde(flatten)]
    pull_request: PullRequestInput,
    version: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiffInput {
    #[serde(flatten)]
    pull_request: PullRequestInput,
    #[serde(default)]
    context_lines: Option<u32>,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommentInput {
    #[serde(flatten)]
    pull_request: PullRequestInput,
    text: String,
    #[serde(default)]
    parent_id: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommentsInput {
    #[serde(flatten)]
    pull_request: PullRequestInput,
    #[serde(flatten)]
    page: PageInput,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Serialize)]
struct PullRequestResponse {
    id: u64,
    version: u64,
    title: String,
    description: Option<String>,
    state: String,
    open: bool,
    closed: bool,
    from_ref: String,
    to_ref: String,
    author: String,
    reviewers: Vec<ParticipantResponse>,
    created_at: i64,
    updated_at: i64,
}

impl From<PullRequest> for PullRequestResponse {
    fn from(value: PullRequest) -> Self {
        Self {
            id: value.id,
            version: value.version,
            title: value.title,
            description: value.description,
            state: value.state,
            open: value.open,
            closed: value.closed,
            from_ref: value.from_ref.id,
            to_ref: value.to_ref.id,
            author: value.author.display_name,
            reviewers: value
                .reviewers
                .into_iter()
                .map(ParticipantResponse::from)
                .collect(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Serialize)]
struct ParticipantResponse {
    username: String,
    display_name: String,
    role: String,
    approved: bool,
    status: String,
}

impl From<PullRequestParticipant> for ParticipantResponse {
    fn from(value: PullRequestParticipant) -> Self {
        Self {
            username: value.user.name,
            display_name: value.user.display_name,
            role: value.role,
            approved: value.approved,
            status: value.status,
        }
    }
}

#[derive(Serialize)]
struct CommentResponse {
    id: u64,
    version: u64,
    text: String,
    author: String,
    created_at: i64,
    updated_at: i64,
    state: Option<String>,
    severity: Option<String>,
}

impl From<PullRequestComment> for CommentResponse {
    fn from(value: PullRequestComment) -> Self {
        Self {
            id: value.id,
            version: value.version,
            text: value.text,
            author: value.author.display_name,
            created_at: value.created_at,
            updated_at: value.updated_at,
            state: value.state,
            severity: value.severity,
        }
    }
}

fn parse_arguments<T: DeserializeOwned>(
    arguments: Option<Map<String, Value>>,
) -> Result<T, ErrorData> {
    serde_json::from_value(Value::Object(arguments.unwrap_or_default()))
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "this function is passed directly to Result::map_err"
)]
fn invalid_input(error: crate::domain::Error) -> ErrorData {
    ErrorData::invalid_params(error.to_string(), None)
}

fn validate_limit(limit: usize) -> Result<(), ErrorData> {
    if !(1..=MAX_DIFF_LIMIT).contains(&limit) {
        return Err(ErrorData::invalid_params(
            format!("max_bytes must be between 1 and {MAX_DIFF_LIMIT}"),
            None,
        ));
    }
    Ok(())
}

fn default_page_limit() -> u32 {
    25
}

fn tool(name: &'static str, description: &'static str, schema: Map<String, Value>) -> Tool {
    Tool::new(name, description, Arc::new(schema))
}

fn string(description: &'static str) -> Value {
    json!({ "type": "string", "minLength": 1, "description": description })
}

fn properties(entries: &[(&'static str, Value)]) -> Map<String, Value> {
    entries
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
}

fn repository_properties() -> Map<String, Value> {
    properties(&[
        ("workspace", string("Workspace/project key")),
        ("repository", string("Repository slug")),
    ])
}

fn object_schema(properties: Map<String, Value>, required: &[&str]) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("object".to_owned()));
    schema.insert("properties".to_owned(), Value::Object(properties));
    schema.insert("required".to_owned(), json!(required));
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema
}

fn repository_schema(mut extra: Map<String, Value>, extra_required: &[&str]) -> Map<String, Value> {
    let mut properties = repository_properties();
    properties.append(&mut extra);
    let mut required = vec!["workspace", "repository"];
    required.extend_from_slice(extra_required);
    object_schema(properties, &required)
}

fn pull_request_schema(
    mut extra: Map<String, Value>,
    extra_required: &[&str],
) -> Map<String, Value> {
    extra.insert(
        "pull_request_id".to_owned(),
        json!({ "type": "integer", "minimum": 1 }),
    );
    let mut required = vec!["pull_request_id"];
    required.extend_from_slice(extra_required);
    repository_schema(extra, &required)
}

fn paged_repository_schema(
    mut extra: Map<String, Value>,
    extra_required: &[&str],
) -> Map<String, Value> {
    add_pagination(&mut extra);
    repository_schema(extra, extra_required)
}

fn paged_pull_request_schema(
    mut extra: Map<String, Value>,
    extra_required: &[&str],
) -> Map<String, Value> {
    add_pagination(&mut extra);
    pull_request_schema(extra, extra_required)
}

fn add_pagination(properties: &mut Map<String, Value>) {
    properties.insert(
        "start".to_owned(),
        json!({ "type": "integer", "minimum": 0, "default": 0 }),
    );
    properties.insert(
        "limit".to_owned(),
        json!({ "type": "integer", "minimum": 1, "maximum": 1000, "default": 25 }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_phase_five_tool_has_a_closed_object_schema() {
        let tools = definitions();
        assert_eq!(tools.len(), 8);
        for tool in tools {
            assert_eq!(tool.input_schema["type"], "object");
            assert_eq!(tool.input_schema["additionalProperties"], false);
        }
    }
}
