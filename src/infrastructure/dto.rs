use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageDto<T> {
    pub values: Vec<T>,
    #[serde(default)]
    pub next_page_start: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectDto {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RepositoryDto {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub default_branch: Option<RefDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RefDto {
    pub display_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BranchDto {
    pub id: String,
    pub display_id: String,
    pub latest_commit: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateBranchDto<'a> {
    pub name: &'a str,
    pub start_point: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommitDto {
    pub id: String,
    pub display_id: String,
    pub author: CommitAuthorDto,
    pub author_timestamp: i64,
    pub committer: CommitAuthorDto,
    pub committer_timestamp: i64,
    pub message: String,
    #[serde(default)]
    pub parents: Vec<CommitParentDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommitAuthorDto {
    pub name: String,
    #[serde(default)]
    pub email_address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommitParentDto {
    pub id: String,
    pub display_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullRequestDto {
    pub id: u64,
    pub version: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: String,
    pub open: bool,
    pub closed: bool,
    pub from_ref: PullRequestRefDto,
    pub to_ref: PullRequestRefDto,
    pub author: ParticipantDto,
    #[serde(default)]
    pub reviewers: Vec<ParticipantDto>,
    pub created_date: i64,
    pub updated_date: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullRequestRefDto {
    pub id: String,
    pub display_id: String,
    pub latest_commit: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ParticipantDto {
    pub user: UserDto,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UserDto {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub email_address: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreatePullRequestDto<'a> {
    pub title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    pub from_ref: CreatePullRequestRefDto<'a>,
    pub to_ref: CreatePullRequestRefDto<'a>,
    pub reviewers: Vec<CreateReviewerDto<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreatePullRequestRefDto<'a> {
    pub id: &'a str,
    pub repository: CreateRepositoryRefDto<'a>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreateRepositoryRefDto<'a> {
    pub slug: &'a str,
    pub project: CreateProjectRefDto<'a>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreateProjectRefDto<'a> {
    pub key: &'a str,
}

#[derive(Debug, Serialize)]
pub(super) struct CreateReviewerDto<'a> {
    pub user: CreateUserRefDto<'a>,
}

#[derive(Debug, Serialize)]
pub(super) struct CreateUserRefDto<'a> {
    pub name: &'a str,
}

#[derive(Debug, Serialize)]
pub(super) struct CreateCommentDto<'a> {
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<CommentParentDto>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct CommentParentDto {
    pub id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommentDto {
    pub id: u64,
    pub version: u64,
    pub text: String,
    pub author: UserDto,
    pub created_date: i64,
    pub updated_date: i64,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ErrorEnvelope {
    #[serde(default)]
    pub errors: Vec<ErrorDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ErrorDto {
    #[serde(default)]
    pub context: Option<String>,
    pub message: String,
}

pub(super) fn error_message(body: &[u8]) -> Option<String> {
    let envelope: ErrorEnvelope = serde_json::from_slice(body).ok()?;
    let mut messages = envelope
        .errors
        .into_iter()
        .map(|error| match error.context {
            Some(context) => format!("{context}: {}", error.message),
            None => error.message,
        });
    let first = messages.next()?;
    Some(messages.fold(first, |mut combined, message| {
        combined.push_str("; ");
        combined.push_str(&message);
        combined
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_bitbucket_error_messages() {
        let body = br#"{"errors":[{"context":"name","message":"required"},{"message":"invalid"}]}"#;
        assert_eq!(
            error_message(body).as_deref(),
            Some("name: required; invalid")
        );
    }
}
