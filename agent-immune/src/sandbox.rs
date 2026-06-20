use std::path::Path;
use tokio::process::Command;

use agent_body_core::{ExecuteResult, SandboxExecute};

/// Run a command in an isolated subprocess (Phase 3 MVP — Firecracker deferred).
pub async fn run_isolated(job: &SandboxExecute) -> ExecuteResult {
    let cwd = job
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &job.command])
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .await
    } else {
        Command::new("sh")
            .args(["-c", &job.command])
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .await
    };

    match output {
        Ok(out) => ExecuteResult {
            msg_id: format!("{}-result", job.msg_id),
            job_id: job.job_id.clone(),
            exit_code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into(),
            stderr: String::from_utf8_lossy(&out.stderr).into(),
            success: out.status.success(),
        },
        Err(e) => ExecuteResult {
            msg_id: format!("{}-result", job.msg_id),
            job_id: job.job_id.clone(),
            exit_code: -1,
            stdout: String::new(),
            stderr: e.to_string(),
            success: false,
        },
    }
}
