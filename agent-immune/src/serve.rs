use axum::{Json, Router, extract::State, routing::get};
use std::sync::Arc;

use crate::config::Config;

pub struct AppState {
    pub config: Config,
}

pub async fn start(config: Config) -> anyhow::Result<()> {
    tracing::info!("Starting agent-immune daemon...");

    if config.nats.jetstream_consumer {
        let url = config.nats.url.clone();
        let network_blackhole = config.sandbox.network_blackhole;
        tokio::spawn(async move {
            if let Err(e) =
                crate::jetstream_consumer::run_sandbox_consumer(&url, network_blackhole).await
            {
                tracing::error!(error = %e, "JetStream sandbox consumer stopped");
            }
        });
    }

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
        "nats_url": state.config.nats.url,
    }))
}
