use std::sync::Arc;

use anyhow::Context;
use mewcode_server::store::fs::{FsStore, resolve_data_dir};
use mewcode_server::{AppState, config::ServerConfig};
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let config = ServerConfig::load()?;
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log)))
        .with(fmt::layer().with_target(true))
        .init();

    let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("MEWCODE_HOST/MEWCODE_PORT must form a valid SocketAddr");

    // A create/write failure here aborts startup; we deliberately do not fall
    // back to an in-memory store.
    let data_dir = resolve_data_dir().context("failed to resolve mewcode data directory")?;
    let store = Arc::new(
        FsStore::new(data_dir.clone())
            .with_context(|| format!("failed to open session store at {}", data_dir.display()))?,
    );
    tracing::info!(data_dir = %data_dir.display(), "session store ready");

    let tracer = mewcode_engine::LangfuseTracer::from_env().map(Arc::new);
    match &tracer {
        Some(t) => {
            // Validate the keys/host against Langfuse without blocking startup.
            // Logs the project on success, or the reason traces won't be recorded.
            let t = t.clone();
            tokio::spawn(async move {
                match tokio::time::timeout(std::time::Duration::from_secs(10), t.health_check())
                    .await
                {
                    Ok(Ok(project)) => {
                        tracing::info!(project = %project, "Langfuse tracing enabled")
                    }
                    Ok(Err(e)) => tracing::warn!(
                        error = %e,
                        "Langfuse keys are set but the self-check failed; traces will not be recorded"
                    ),
                    Err(_) => tracing::warn!("Langfuse self-check timed out after 10s"),
                }
            });
        }
        None => tracing::info!(
            "Langfuse tracing disabled (set LANGFUSE_PUBLIC_KEY and LANGFUSE_SECRET_KEY to enable)"
        ),
    }
    let state = AppState::new(config.clone(), store).with_tracer(tracer);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "mewcode server listening");
    let app = mewcode_server::build_app(state);
    axum::serve(listener, app).await?;
    Ok(())
}
