//! Specification-compliant Model Context Protocol adapters.

mod metrics;
mod pull_request_tools;
mod server;
mod tools;

pub use metrics::{MetricsRecorder, NoopMetrics, RequestMetric, TracingMetrics};
pub use server::{
    McpServer, StdioServerError, domain_error_result, serve_stdio, serve_stdio_with_cancellation,
};
