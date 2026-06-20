use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::AckPolicy, stream::StorageType};
use futures::StreamExt;
use std::time::Duration;
use tracing::{error, info, warn};

use agent_body_core::nats::subjects;
use agent_body_core::{ExecuteResult, SandboxExecute, STREAM_NAME, STREAM_SUBJECT_WILDCARD};

async fn connect_js(url: &str) -> Result<jetstream::Context> {
    let client = async_nats::connect(url)
        .await
        .context("connect to NATS")?;
    let js = jetstream::new(client);
    js.get_or_create_stream(jetstream::stream::Config {
        name: STREAM_NAME.to_string(),
        subjects: vec![STREAM_SUBJECT_WILDCARD.to_string()],
        storage: StorageType::File,
        duplicate_window: agent_body_core::default_duplicate_window(),
        max_age: Duration::from_secs(7 * 24 * 3600),
        ..Default::default()
    })
    .await
    .context("ensure AUTONOMIC stream")?;
    js.create_consumer_on_stream(
        jetstream::consumer::pull::Config {
            durable_name: Some("immune-sandbox".into()),
            filter_subject: subjects::EXECUTE_SANDBOX.into(),
            ack_policy: AckPolicy::Explicit,
            ack_wait: agent_body_core::default_ack_wait(),
            ..Default::default()
        },
        STREAM_NAME,
    )
    .await
    .ok();
    Ok(js)
}

async fn publish_result(js: &jetstream::Context, result: &ExecuteResult) -> Result<()> {
    let mut headers = async_nats::HeaderMap::new();
    headers.insert("Nats-Msg-Id", result.msg_id.as_str());
    let bytes = serde_json::to_vec(result)?;
    js.publish_with_headers(
        subjects::EXECUTE_RESULT.to_string(),
        headers,
        bytes.into(),
    )
        .await?
        .await
        .context("publish execute result")?;
    Ok(())
}

pub async fn run_sandbox_consumer(url: &str, network_blackhole: bool) -> Result<()> {
    let js = connect_js(url).await?;
    let consumer = js
        .get_consumer_from_stream("immune-sandbox", STREAM_NAME)
        .await
        .context("get immune-sandbox consumer")?;
    let mut messages = consumer
        .fetch()
        .max_messages(1)
        .messages()
        .await
        .context("fetch sandbox jobs")?;

    info!(
        "agent-immune JetStream consumer active on {}",
        subjects::EXECUTE_SANDBOX
    );

    while let Some(msg) = messages.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "sandbox consumer fetch error");
                continue;
            }
        };

        let job: SandboxExecute = match serde_json::from_slice(&msg.payload) {
            Ok(j) => j,
            Err(e) => {
                error!(error = %e, "invalid sandbox payload");
                let _ = msg.ack().await;
                continue;
            }
        };

        let options = crate::sandbox::SandboxOptions {
            network_blackhole,
        };
        let result = crate::sandbox::run_isolated(&job, &options).await;
        if let Err(e) = publish_result(&js, &result).await {
            warn!(error = %e, "failed to publish execute result");
        }
        if let Err(e) = msg.ack().await {
            warn!(error = %e, "failed to ack sandbox job");
        }
    }

    Ok(())
}
