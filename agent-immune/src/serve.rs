use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

use crate::config::Config;

pub struct AppState {
    pub config: Config,
}

pub async fn start(config: Config) -> anyhow::Result<()> {
    tracing::info!("Starting agent-immune daemon...");

    if config.nats.jetstream_consumer {
        let url = config.nats.url.clone();
        let options = crate::sandbox::SandboxOptions::from(&config.sandbox);
        tokio::spawn(async move {
            if let Err(e) = crate::jetstream_consumer::run_sandbox_consumer(&url, &options).await {
                tracing::error!(error = %e, "JetStream sandbox consumer stopped");
            }
        });
    }

    let mcp_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::mcp_server::ImmuneMcp::run(mcp_config).await {
            tracing::error!(error = %e, "MCP server stopped");
        }
    });

    let port = config.server.port;
    let state = Arc::new(AppState { config });
    let app = Router::new()
        .route("/health", get(health))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("HTTP server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "jetstream_consumer": state.config.nats.jetstream_consumer,
        "network_blackhole": state.config.sandbox.network_blackhole,
        "sandbox_backend": state.config.sandbox.backend,
        "seccomp": state.config.sandbox.seccomp,
        "firecracker_ready": crate::firecracker::is_available(),
        "nats_url": state.config.nats.url,
    }))
}
