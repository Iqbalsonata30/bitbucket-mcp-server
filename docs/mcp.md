# MCP Transport and Compliance

The server uses the official Model Context Protocol Rust SDK, pinned to
`rmcp 2.2.0`. It advertises and negotiates the stable MCP protocol version
`2025-11-25`; prerelease SDKs and draft protocol versions are not enabled.

Official references:

- [MCP specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)
- [Standard I/O transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- [MCP Rust SDK](https://github.com/modelcontextprotocol/rust-sdk/tree/rmcp-v2.2.0)

## Standard I/O

Standard input and output carry newline-delimited UTF-8 JSON-RPC 2.0 messages.
There are no `Content-Length` headers. Standard output is reserved exclusively
for MCP protocol messages. `tracing` writes diagnostics to standard error with
ANSI output disabled.

Closing standard input terminates the service cleanly. Initialization, version
negotiation, `notifications/initialized`, `tools/list`, `tools/call`, and
`notifications/cancelled` are dispatched by the official SDK.

## Capabilities

The server advertises the standard `tools` capability without proprietary MCP
extensions. Workspace, repository/content, and pull-request tools use closed
JSON object schemas and reject unknown fields.

Unknown tool names return JSON-RPC `METHOD_NOT_FOUND` (`-32601`). Malformed tool
arguments return `INVALID_PARAMS` (`-32602`).

## Tool Errors

Protocol/routing failures use JSON-RPC errors. Failures produced after a valid
tool has been routed use an ordinary `tools/call` result with `isError: true` and
structured content:

```json
{
  "code": "repository_not_found",
  "message": "repository not found: PRJ/repo"
}
```

This distinction lets any compliant MCP client expose actionable Bitbucket
failures while retaining standard JSON-RPC behavior for protocol problems.

## Cancellation

Each request receives the SDK cancellation token associated with its JSON-RPC
request ID. The SDK cancels that token when a matching
`notifications/cancelled` message arrives and suppresses a late response.
Handlers check the token before work and future long-running tool use cases must
select on it while awaiting Bitbucket operations.

## Observability

Tool list/call spans include:

- JSON-RPC request ID
- MCP method and tool name where applicable
- Negotiated protocol version
- Outcome
- Execution time in milliseconds

Cancellation notifications are logged with request ID and reason. Tool
arguments and results are not logged.

`MetricsRecorder` is a backend-neutral hook receiving method, tool, outcome, and
duration. The production composition root installs `TracingMetrics`, allowing a
log-based collector to aggregate metrics without coupling the MCP transport to
Prometheus, OpenTelemetry, or an HTTP endpoint.
