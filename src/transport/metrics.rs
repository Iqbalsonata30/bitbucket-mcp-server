use std::time::Duration;

/// Backend-neutral measurement emitted after an MCP request completes.
#[derive(Clone, Copy, Debug)]
pub struct RequestMetric<'a> {
    pub method: &'static str,
    pub tool: Option<&'a str>,
    pub outcome: &'static str,
    pub duration: Duration,
}

/// Hook for integrating a metrics backend without coupling transport code to it.
pub trait MetricsRecorder: Send + Sync + 'static {
    fn record_request(&self, metric: &RequestMetric<'_>);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopMetrics;

impl MetricsRecorder for NoopMetrics {
    fn record_request(&self, _metric: &RequestMetric<'_>) {}
}

/// Emits metric-shaped structured tracing events for log-based collectors.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingMetrics;

impl MetricsRecorder for TracingMetrics {
    fn record_request(&self, metric: &RequestMetric<'_>) {
        tracing::info!(
            target: "metrics",
            method = metric.method,
            tool = metric.tool,
            outcome = metric.outcome,
            duration_ms = metric.duration.as_millis(),
            "mcp_request_completed"
        );
    }
}
