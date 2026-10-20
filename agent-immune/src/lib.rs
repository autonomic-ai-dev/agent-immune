pub mod config;
pub mod firecracker;
pub mod jetstream_consumer;
pub mod leak_check;
pub mod mcp_server;
pub mod sandbox;
pub mod scanner;
pub mod seccomp;
pub mod serve;
pub mod update;
pub mod wasm_sandbox;

use anyhow::{Context, Result};
use std::path::Path;

const APPROVAL_TIMEOUT_SECS: &str = "300";

pub async fn run_scan(path: &Path) -> Result<()> {
    let pkgs = scanner::parse_manifest(path)?;
    if pkgs.is_empty() {
        println!("{{ \"packages\": [], \"vulnerabilities\": [] }}");
        return Ok(());
    }
    println!(
        "Scanning {} dependencies from {}",
        pkgs.len(),
        path.display()
    );
    let results = scanner::query_osv(&pkgs).await?;
    println!("{}", serde_json::to_string_pretty(&results)?);
    if results.blocking {
        request_human_approval(path, &results).await?;
    }
    Ok(())
}

async fn request_human_approval(path: &Path, results: &scanner::ScanResult) -> Result<()> {
    let blocking = results
        .vulnerabilities
        .iter()
        .filter(|v| matches!(v.severity.as_str(), "CRITICAL" | "HIGH" | "UNKNOWN"))
        .count();
    let id = format!("scan-{}", uuid::Uuid::now_v7());
    let summary = format!("{blocking} blocking advisories in {}", path.display());
    eprintln!("awaiting approval {id}: run `agent-mouth approvals approve {id}` to allow");
    let status = tokio::process::Command::new("agent-mouth")
        .args([
            "approvals",
            "request",
            "--id",
            &id,
            "--summary",
            &summary,
            "--timeout-secs",
            APPROVAL_TIMEOUT_SECS,
        ])
        .status()
        .await
        .context("agent-mouth unavailable; blocking scan results without an approval")?;
    if status.success() {
        eprintln!("scan blocked by policy; approved by a human ({id})");
        return Ok(());
    }
    anyhow::bail!(
        "scan failed: critical, high or unknown-severity vulnerabilities found; approval {id} not granted"
    );
}
