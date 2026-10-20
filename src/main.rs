use std::{path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use bitbucket_mcp_server::{
    config::Settings,
    infrastructure::BitbucketClient,
    transport::{McpServer, TracingMetrics, serve_stdio_with_cancellation},
};
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(long, env = "BITBUCKET_CONFIG_FILE")]
    config: Option<PathBuf>,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            error!(%error, "failed to create async runtime");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(run());
    let exit_code = match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(%error, "server terminated with an error");
            ExitCode::FAILURE
        }
    };
    runtime.shutdown_timeout(Duration::from_secs(1));
    exit_code
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let settings = Settings::load(args.config.as_deref())?;
    let client = BitbucketClient::new(settings).await?;
    info!(
        protocol_version = "2025-11-25",
        transport = "stdio",
        "starting Bitbucket MCP server"
    );
    let cancellation = CancellationToken::new();
    install_shutdown_handler(cancellation.clone())?;
    let server = McpServer::new(client).with_metrics(Arc::new(TracingMetrics));
    let reason = serve_stdio_with_cancellation(server, cancellation).await?;
    info!(?reason, "Bitbucket MCP server stopped");
    Ok(())
}

#[cfg(unix)]
fn install_shutdown_handler(cancellation: CancellationToken) -> std::io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::spawn(async move {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    error!(%error, "failed to receive shutdown signal");
                    return;
                }
            }
            _ = terminate.recv() => {}
        }
        info!("shutdown signal received");
        cancellation.cancel();
    });
    Ok(())
}

#[cfg(not(unix))]
fn install_shutdown_handler(cancellation: CancellationToken) -> std::io::Result<()> {
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("shutdown signal received");
                cancellation.cancel();
            }
            Err(error) => error!(%error, "failed to receive shutdown signal"),
        }
    });
    Ok(())
}
