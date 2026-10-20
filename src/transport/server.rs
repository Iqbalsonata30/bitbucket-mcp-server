use std::{sync::Arc, time::Instant};

use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, CancelledNotificationParam, Implementation,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo,
    },
    service::{NotificationContext, QuitReason, RequestContext, RoleServer, ServerInitializeError},
    transport::stdio,
};
use serde_json::json;
use thiserror::Error as ThisError;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use crate::{application::BitbucketPlatform, domain::Error};

use super::{MetricsRecorder, NoopMetrics, RequestMetric, pull_request_tools, tools};

/// MCP server state. Tool-specific application dependencies are added to this
/// state without leaking them into the protocol transport.
pub struct McpServer<C> {
    catalog: C,
    metrics: Arc<dyn MetricsRecorder>,
}

impl<C> McpServer<C> {
    #[must_use]
    pub fn new(catalog: C) -> Self {
        Self {
            catalog,
            metrics: Arc::new(NoopMetrics),
        }
    }

    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsRecorder>) -> Self {
        self.metrics = metrics;
        self
    }

    #[must_use]
    pub fn catalog(&self) -> &C {
        &self.catalog
    }
}

impl<C> ServerHandler for McpServer<C>
where
    C: BitbucketPlatform + 'static,
{
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Use the advertised Bitbucket tools. Availability depends on the configured self-hosted installation.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, ErrorData> {
        let started = Instant::now();
        let request_id = context.id.to_string();
        let span = info_span!(
            "mcp_request",
            request_id,
            method = "tools/list",
            protocol_version = tracing::field::Empty,
        );
        async move {
            if let Some(version) = context.protocol_version() {
                tracing::Span::current().record("protocol_version", version.as_str());
            }
            if context.ct.is_cancelled() {
                self.metrics.record_request(&RequestMetric {
                    method: "tools/list",
                    tool: None,
                    outcome: "cancelled",
                    duration: started.elapsed(),
                });
                info!(
                    outcome = "cancelled",
                    elapsed_ms = started.elapsed().as_millis()
                );
                return Err(ErrorData::internal_error("request cancelled", None));
            }

            let mut definitions = tools::definitions();
            definitions.extend(pull_request_tools::definitions());
            let result = ListToolsResult::with_all_items(definitions);
            self.metrics.record_request(&RequestMetric {
                method: "tools/list",
                tool: None,
                outcome: "success",
                duration: started.elapsed(),
            });
            info!(
                outcome = "success",
                elapsed_ms = started.elapsed().as_millis()
            );
            Ok(result)
        }
        .instrument(span)
        .await
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        let started = Instant::now();
        let request_id = context.id.to_string();
        let tool_name = request.name.to_string();
        let span = info_span!(
            "mcp_request",
            request_id,
            method = "tools/call",
            tool_name,
            protocol_version = tracing::field::Empty,
        );
        async move {
            if let Some(version) = context.protocol_version() {
                tracing::Span::current().record("protocol_version", version.as_str());
            }
            if context.ct.is_cancelled() {
                self.metrics.record_request(&RequestMetric {
                    method: "tools/call",
                    tool: Some(&tool_name),
                    outcome: "cancelled",
                    duration: started.elapsed(),
                });
                info!(
                    outcome = "cancelled",
                    elapsed_ms = started.elapsed().as_millis()
                );
                return Err(ErrorData::internal_error("request cancelled", None));
            }

            let result = tokio::select! {
                () = context.ct.cancelled() => {
                    self.metrics.record_request(&RequestMetric {
                        method: "tools/call",
                        tool: Some(&tool_name),
                        outcome: "cancelled",
                        duration: started.elapsed(),
                    });
                    info!(outcome = "cancelled", elapsed_ms = started.elapsed().as_millis());
                    return Err(ErrorData::internal_error("request cancelled", None));
                }
                result = async {
                    if tools::contains(request.name.as_ref()) {
                        tools::call(&self.catalog, request).await
                    } else {
                        pull_request_tools::call(&self.catalog, request).await
                    }
                } => result,
            };
            let outcome = if result.is_ok() {
                "completed"
            } else {
                "protocol_error"
            };
            self.metrics.record_request(&RequestMetric {
                method: "tools/call",
                tool: Some(&tool_name),
                outcome,
                duration: started.elapsed(),
            });
            info!(outcome, elapsed_ms = started.elapsed().as_millis());
            result
        }
        .instrument(span)
        .await
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        tools::definitions()
            .into_iter()
            .chain(pull_request_tools::definitions())
            .find(|tool| tool.name.as_ref() == name)
    }

    async fn on_cancelled(
        &self,
        notification: CancelledNotificationParam,
        _context: NotificationContext<RoleServer>,
    ) {
        info!(
            request_id = notification.request_id.as_ref().map(ToString::to_string),
            reason = notification.reason,
            "MCP request cancellation received"
        );
    }
}

/// Converts an application/domain failure into an MCP tool-level error.
///
/// Tool-level errors remain ordinary `tools/call` results so compliant clients
/// can expose actionable details to users and models.
#[must_use]
pub fn domain_error_result(error: &Error) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "code": domain_error_code(error),
        "message": error.to_string(),
    }))
}

/// Serves MCP over newline-delimited JSON-RPC on standard input/output.
///
/// # Errors
///
/// Returns [`StdioServerError`] when initialization or the service task fails.
pub async fn serve_stdio<C>(server: McpServer<C>) -> Result<QuitReason, StdioServerError>
where
    C: BitbucketPlatform + 'static,
{
    serve_stdio_with_cancellation(server, CancellationToken::new()).await
}

/// Serves MCP over stdio until the transport closes or cancellation is requested.
///
/// # Errors
///
/// Returns [`StdioServerError`] when initialization or the service task fails.
pub async fn serve_stdio_with_cancellation<C>(
    server: McpServer<C>,
    cancellation: CancellationToken,
) -> Result<QuitReason, StdioServerError>
where
    C: BitbucketPlatform + 'static,
{
    let service = server
        .serve_with_ct(stdio(), cancellation)
        .await
        .map_err(|error| StdioServerError::Initialize(Box::new(error)))?;
    service.waiting().await.map_err(StdioServerError::Join)
}

#[derive(Debug, ThisError)]
pub enum StdioServerError {
    #[error("failed to initialize MCP stdio service: {0}")]
    Initialize(Box<ServerInitializeError>),

    #[error("MCP stdio service task failed: {0}")]
    Join(tokio::task::JoinError),
}

fn domain_error_code(error: &Error) -> &'static str {
    match error {
        Error::AuthenticationError { .. } => "authentication_error",
        Error::WorkspaceNotFound { .. } => "workspace_not_found",
        Error::RepositoryNotFound { .. } => "repository_not_found",
        Error::CommitNotFound { .. } => "commit_not_found",
        Error::FileNotFound { .. } => "file_not_found",
        Error::CommitConflict { .. } => "commit_conflict",
        Error::PullRequestNotFound { .. } => "pull_request_not_found",
        Error::PullRequestConflict { .. } => "pull_request_conflict",
        Error::MergeConflict { .. } => "merge_conflict",
        Error::BranchAlreadyExists { .. } => "branch_already_exists",
        Error::ValidationError { .. } => "validation_error",
        Error::NetworkError { .. } => "network_error",
        Error::UpstreamError { .. } => "upstream_error",
        Error::UnsupportedCapability { .. } => "unsupported_capability",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_error_becomes_structured_tool_error() {
        let result = domain_error_result(&Error::RepositoryNotFound {
            workspace: "PRJ".to_owned(),
            repository: "repo".to_owned(),
        });

        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content,
            Some(json!({
                "code": "repository_not_found",
                "message": "repository not found: PRJ/repo"
            }))
        );
    }
}
