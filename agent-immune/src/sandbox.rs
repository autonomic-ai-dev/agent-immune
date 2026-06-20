use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::warn;

use agent_body_core::{ExecuteResult, SandboxExecute};

#[derive(Debug, Clone)]
pub struct SandboxOptions {
    pub network_blackhole: bool,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        Self {
            network_blackhole: true,
        }
    }
}

/// Run a command in an isolated subprocess with optional network egress blackhole.
pub async fn run_isolated(job: &SandboxExecute, options: &SandboxOptions) -> ExecuteResult {
    let cwd = job
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    let output = if options.network_blackhole {
        run_with_network_blackhole(&job.command, cwd).await
    } else {
        run_plain(&job.command, cwd).await
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

/// Run a local script file or inline command through the sandbox executor.
pub async fn run_script(path: &Path, options: &SandboxOptions) -> ExecuteResult {
    let command = if path.is_file() {
        format!("sh {}", shell_escape(path))
    } else {
        return ExecuteResult {
            msg_id: uuid::Uuid::now_v7().to_string(),
            job_id: uuid::Uuid::now_v7().to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("script not found: {}", path.display()),
            success: false,
        };
    };

    let job = SandboxExecute {
        msg_id: uuid::Uuid::now_v7().to_string(),
        job_id: uuid::Uuid::now_v7().to_string(),
        command,
        cwd: path.parent().map(|p| p.display().to_string()),
    };
    run_isolated(&job, options).await
}

async fn run_plain(command: &str, cwd: &Path) -> std::io::Result<std::process::Output> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    } else {
        Command::new("sh")
            .args(["-c", command])
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    }
}

async fn run_with_network_blackhole(
    command: &str,
    cwd: &Path,
) -> std::io::Result<std::process::Output> {
    #[cfg(target_os = "linux")]
    {
        tracing::info!("sandbox: network blackhole via unshare -n");
        return Command::new("unshare")
            .args(["-n", "sh", "-c", command])
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
    }

    #[cfg(not(target_os = "linux"))]
    {
        warn!("network blackhole requires Linux (unshare -n); running env-isolated subprocess");
        run_plain(command, cwd).await
    }
}

fn shell_escape(path: &Path) -> String {
    let s = path.display().to_string();
    if s.contains(' ') || s.contains('\'') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_escape_quotes_spaces() {
        let p = PathBuf::from("/tmp/my script.sh");
        assert!(shell_escape(&p).starts_with('\''));
    }
}
