pub mod config;
pub mod firecracker;
pub mod jetstream_consumer;
pub mod leak_check;
pub mod sandbox;
pub mod scanner;
pub mod seccomp;
pub mod mcp_server;
pub mod serve;

use anyhow::Result;
use std::path::Path;

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
    Ok(())
}
