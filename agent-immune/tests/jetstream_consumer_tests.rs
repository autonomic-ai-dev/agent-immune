use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use agent_body_core::nats::subjects;
use agent_body_core::{ExecuteResult, SandboxExecute, STREAM_NAME, STREAM_SUBJECT_WILDCARD};
use async_nats::jetstream::{self, stream::StorageType};
use futures::StreamExt;

struct BrokerProcess {
    child: Child,
}

impl BrokerProcess {
    fn start(port: u16, store_dir: &std::path::Path) -> Self {
        let child = Command::new("nats-server")
            .args(["-js", "-a", "127.0.0.1", "-p"])
            .arg(port.to_string())
            .arg("-sd")
            .arg(store_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("nats-server must be on PATH");
        Self { child }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for BrokerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn wait_until_ready(url: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if async_nats::connect(url).await.is_ok() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "nats-server at {url} never became ready"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn sandbox_job_is_executed_and_its_result_published() {
    let store = tempfile::tempdir().unwrap();
    let port = free_port();
    let broker = BrokerProcess::start(port, store.path());
    let url = format!("nats://127.0.0.1:{port}");
    println!("nats-server pid={} url={url}", broker.pid());
    wait_until_ready(&url).await;

    unsafe {
        std::env::set_var("AUTONOMIC_NATS_URL", &url);
    }

    let client = async_nats::connect(&url).await.unwrap();
    let js = jetstream::new(client.clone());
    js.get_or_create_stream(jetstream::stream::Config {
        name: STREAM_NAME.to_string(),
        subjects: vec![STREAM_SUBJECT_WILDCARD.to_string()],
        storage: StorageType::File,
        ..Default::default()
    })
    .await
    .unwrap();

    let mut results = client.subscribe(subjects::EXECUTE_RESULT).await.unwrap();

    let options = agent_immune::sandbox::SandboxOptions::default();
    let consumer = tokio::spawn(async move {
        agent_immune::jetstream_consumer::run_sandbox_consumer(&url, &options).await
    });

    let job = SandboxExecute {
        msg_id: "sandbox-1".into(),
        job_id: "sandbox-1".into(),
        command: "echo hello".into(),
        cwd: None,
        memory_mb: None,
        cpu_cores: None,
        backend: None,
        wasm_path: None,
        fuel_limit: None,
        memory_limit_bytes: None,
    };
    js.publish(
        subjects::EXECUTE_SANDBOX.to_string(),
        serde_json::to_vec(&job).unwrap().into(),
    )
    .await
    .unwrap()
    .await
    .unwrap();

    let message = tokio::time::timeout(Duration::from_secs(20), results.next())
        .await
        .expect("no sandbox result within 20s")
        .expect("result subscription closed");
    let result: ExecuteResult = serde_json::from_slice(&message.payload).unwrap();

    consumer.abort();

    assert_eq!(result.job_id, "sandbox-1");
    assert!(result.success);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "hello");
}
