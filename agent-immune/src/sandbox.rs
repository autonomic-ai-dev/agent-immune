use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::warn;

use agent_body_core::{ExecuteResult, SandboxExecute};

use crate::config::SandboxConfig as ImmuneSandboxConfig;

#[derive(Debug, Clone)]
pub struct SandboxOptions {
    pub network_blackhole: bool,
    pub backend: String,
    pub seccomp: bool,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        Self {
            network_blackhole: true,
            backend: "subprocess".into(),
            seccomp: true,
        }
    }
}

impl From<&ImmuneSandboxConfig> for SandboxOptions {
    fn from(cfg: &ImmuneSandboxConfig) -> Self {
        Self {
            network_blackhole: cfg.network_blackhole,
            backend: cfg.backend.clone(),
            seccomp: cfg.seccomp,
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

    let backend = job
        .backend
        .as_deref()
        .unwrap_or(options.backend.as_str());
    let memory_mb = job.memory_mb.unwrap_or(256);
    let cpu_cores = job.cpu_cores.unwrap_or(1.0);
    let command = wrap_with_resource_limits(&job.command, memory_mb, cpu_cores);

    if backend == "firecracker" {
        if let Some(fc_cfg) = crate::firecracker::FirecrackerConfig::from_env() {
            match crate::firecracker::run_command(&command, cwd, &fc_cfg).await {
                Ok(out) => {
                    return ExecuteResult {
                        msg_id: format!("{}-result", job.msg_id),
                        job_id: job.job_id.clone(),
                        exit_code: out.status.code().unwrap_or(-1),
                        stdout: String::from_utf8_lossy(&out.stdout).into(),
                        stderr: String::from_utf8_lossy(&out.stderr).into(),
                        success: out.status.success(),
                    };
                }
                Err(e) => {
                    warn!("firecracker backend failed ({e}); falling back to subprocess");
                }
            }
        } else {
            warn!("firecracker backend requested but AUTONOMIC_FC_KERNEL/ROOTFS not configured; using subprocess");
        }
    }

    let output = if options.network_blackhole {
        run_with_network_blackhole(&command, cwd, options.seccomp).await
    } else {
        run_plain(&command, cwd, options.seccomp).await
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
        memory_mb: None,
        cpu_cores: None,
        backend: None,
    };
    run_isolated(&job, options).await
}

fn wrap_with_resource_limits(command: &str, memory_mb: u32, cpu_cores: f32) -> String {
    #[cfg(target_os = "linux")]
    {
        let _ = cpu_cores;
        format!(
            "ulimit -v $(({memory_mb} * 1024)) 2>/dev/null; ulimit -u 64 2>/dev/null; {command}"
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (memory_mb, cpu_cores);
        command.to_string()
    }
}

async fn run_plain(
    command: &str,
    cwd: &Path,
    seccomp: bool,
) -> std::io::Result<std::process::Output> {
    spawn_shell(command, cwd, seccomp, false).await
}

async fn run_with_network_blackhole(
    command: &str,
    cwd: &Path,
    seccomp: bool,
) -> std::io::Result<std::process::Output> {
    #[cfg(target_os = "linux")]
    {
        tracing::info!("sandbox: network blackhole via unshare -n");
        return spawn_shell(command, cwd, seccomp, true).await;
    }

    #[cfg(not(target_os = "linux"))]
    {
        warn!("network blackhole requires Linux (unshare -n); running env-isolated subprocess");
        spawn_shell(command, cwd, seccomp, false).await
    }
}

async fn spawn_shell(
    command: &str,
    cwd: &Path,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] seccomp: bool,
    network_blackhole: bool,
) -> std::io::Result<std::process::Output> {
    #[cfg(target_os = "linux")]
    if seccomp {
        let cwd = cwd.to_path_buf();
        let command = command.to_string();
        return tokio::task::spawn_blocking(move || {
            let mut cmd = if network_blackhole {
                let mut c = std::process::Command::new("unshare");
                c.args(["-n", "sh", "-c", &command]);
                c
            } else {
                let mut c = std::process::Command::new("sh");
                c.args(["-c", &command]);
                c
            };
            cmd.current_dir(&cwd)
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            crate::seccomp::apply_to_command(&mut cmd);
            cmd.output()
        })
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    }

    let mut cmd = if network_blackhole {
        let mut c = Command::new("unshare");
        c.args(["-n", "sh", "-c", command]);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };

    cmd.current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
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
    use std::path::PathBuf;

    #[test]
    fn shell_escape_quotes_spaces() {
        let p = PathBuf::from("/tmp/my script.sh");
        assert!(shell_escape(&p).starts_with('\''));
    }
}
