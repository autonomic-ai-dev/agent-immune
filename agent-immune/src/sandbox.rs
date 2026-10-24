use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::warn;

const SANDBOX_TIMEOUT_SECS: u64 = 60;
const OVERLAY_MAX_BYTES: u64 = 256 * 1024 * 1024;

use agent_body_core::{ExecuteResult, SandboxExecute};

const ENV_ALLOW_SUBPROCESS_FALLBACK: &str = "AUTONOMIC_IMMUNE_ALLOW_SUBPROCESS_FALLBACK";

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
    run_with_timeout(job, options, Duration::from_secs(SANDBOX_TIMEOUT_SECS)).await
}

async fn run_with_timeout(
    job: &SandboxExecute,
    options: &SandboxOptions,
    limit: Duration,
) -> ExecuteResult {
    let cwd = job
        .cwd
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));

    let backend = job.backend.as_deref().unwrap_or(options.backend.as_str());
    let memory_mb = job.memory_mb.unwrap_or(256);
    let cpu_cores = job.cpu_cores.unwrap_or(1.0);

    if backend == "wasm" {
        let wasm_path = job.wasm_path.as_deref().map(Path::new).or_else(|| {
            let cmd = job.command.trim();
            if cmd.ends_with(".wasm") {
                Some(Path::new(cmd))
            } else {
                None
            }
        });
        if let Some(path) = wasm_path {
            let opts = crate::wasm_sandbox::options_from_job(job);
            let result = crate::wasm_sandbox::run_path(path, &opts).await;
            return crate::wasm_sandbox::into_execute_result(job, result);
        }
        return crate::wasm_sandbox::into_execute_result(
            job,
            Err(crate::wasm_sandbox::WasmSandboxError::ValidationFailed(
                anyhow::anyhow!("wasm backend requires wasm_path or command ending in .wasm"),
            )),
        );
    }

    if let Some(reason) = policy_rejection(&job.command) {
        return failed_result(job, reason);
    }

    let command = wrap_with_resource_limits(&job.command, memory_mb, cpu_cores);

    let mut degraded_reason = None;
    if backend == "firecracker" {
        let fallback_allowed = std::env::var(ENV_ALLOW_SUBPROCESS_FALLBACK).as_deref() == Ok("1");
        let unavailable = match crate::firecracker::FirecrackerConfig::from_env() {
            Some(fc_cfg) => match crate::firecracker::run_command(&command, cwd, &fc_cfg).await {
                Ok(out) => {
                    return ExecuteResult {
                        msg_id: format!("{}-result", job.msg_id),
                        job_id: job.job_id.clone(),
                        exit_code: out.status.code().unwrap_or(-1),
                        stdout: String::from_utf8_lossy(&out.stdout).into(),
                        stderr: String::from_utf8_lossy(&out.stderr).into(),
                        success: out.status.success(),
                        fuel_consumed: None,
                        memory_peak_bytes: None,
degraded: false,
                    };
                }
                Err(e) => format!("firecracker backend failed: {e}"),
            },
            None => {
                "firecracker backend not configured: set AUTONOMIC_FC_KERNEL and AUTONOMIC_FC_ROOTFS"
                    .to_string()
            }
        };
        if !fallback_allowed {
            return failed_result(
                job,
                format!("{unavailable}; set {ENV_ALLOW_SUBPROCESS_FALLBACK}=1 to allow the subprocess fallback"),
            );
        }
        warn!("{unavailable}; running as a degraded subprocess");
        degraded_reason = Some(unavailable);
    }

    let overlay = match overlay_workspace(cwd) {
        Ok(dir) => dir,
        Err(e) => return failed_result(job, format!("overlay workspace failed: {e}")),
    };
    let run = async {
        if options.network_blackhole {
            run_with_network_blackhole(&command, overlay.path(), options.seccomp).await
        } else {
            run_plain(&command, overlay.path(), options.seccomp).await
        }
    };
    let output = match tokio::time::timeout(limit, run).await {
        Ok(output) => output,
        Err(_) => return failed_result(job, format!("killed after {limit:?} timeout")),
    };

    match output {
        Ok(out) => {
            let mut stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            if let Some(reason) = &degraded_reason {
                stderr = format!("degraded: {reason}; ran as subprocess\n{stderr}");
            }
            ExecuteResult {
                msg_id: format!("{}-result", job.msg_id),
                job_id: job.job_id.clone(),
                exit_code: out.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&out.stdout).into(),
                stderr,
                success: out.status.success(),
                fuel_consumed: None,
                memory_peak_bytes: None,
                degraded: degraded_reason.is_some(),
            }
        }
        Err(e) => failed_result(job, e.to_string()),
    }
}

fn policy_rejection(command: &str) -> Option<String> {
    agent_body_core::guard::inspect(command)
        .first()
        .map(|issue| {
            format!(
                "command blocked by policy (line {}): {}",
                issue.line, issue.message
            )
        })
}

fn failed_result(job: &SandboxExecute, stderr: String) -> ExecuteResult {
    ExecuteResult {
        msg_id: format!("{}-result", job.msg_id),
        job_id: job.job_id.clone(),
        exit_code: -1,
        stdout: String::new(),
        stderr,
        success: false,
        fuel_consumed: None,
        memory_peak_bytes: None,
        degraded: false,
    }
}

fn overlay_workspace(src: &Path) -> std::io::Result<tempfile::TempDir> {
    let dir = tempfile::Builder::new()
        .prefix("sandbox-overlay-")
        .tempdir()?;
    let mut budget = OVERLAY_MAX_BYTES;
    copy_tree(src, dir.path(), &mut budget)?;
    Ok(dir)
}

fn copy_tree(src: &Path, dst: &Path, budget: &mut u64) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir(&to)?;
            copy_tree(&from, &to, budget)?;
        } else if file_type.is_file() {
            let len = entry.metadata()?.len();
            *budget = budget
                .checked_sub(len)
                .ok_or_else(|| std::io::Error::other("workspace exceeds the overlay size limit"))?;
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
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
            fuel_consumed: None,
            memory_peak_bytes: None,
            degraded: false,
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
        wasm_path: None,
        fuel_limit: None,
        memory_limit_bytes: None,
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
        .kill_on_drop(true)
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

    fn job_in(cwd: &Path, command: &str) -> SandboxExecute {
        SandboxExecute {
            msg_id: "m".into(),
            job_id: "j".into(),
            command: command.into(),
            cwd: Some(cwd.display().to_string()),
            memory_mb: None,
            cpu_cores: None,
            backend: None,
            wasm_path: None,
            fuel_limit: None,
            memory_limit_bytes: None,
        }
    }

    fn plain_options() -> SandboxOptions {
        SandboxOptions {
            network_blackhole: false,
            backend: "subprocess".into(),
            seccomp: false,
        }
    }

    #[tokio::test]
    async fn destructive_command_is_rejected_before_any_spawn() {
        let src = tempfile::tempdir().unwrap();
        let result = run_isolated(
            &job_in(src.path(), "rm -rf /tmp/immune-policy-sentinel; echo ran"),
            &plain_options(),
        )
        .await;
        assert!(!result.success);
        assert_eq!(result.exit_code, -1);
        assert!(
            result.stderr.contains("blocked by policy"),
            "{}",
            result.stderr
        );
        assert!(!result.stdout.contains("ran"), "{}", result.stdout);
    }

    #[tokio::test]
    async fn writes_inside_the_sandbox_do_not_reach_the_source_directory() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("input.txt"), "seed").unwrap();
        let result = run_isolated(
            &job_in(src.path(), "cat input.txt && echo leak > leaked.txt"),
            &plain_options(),
        )
        .await;
        assert!(result.success, "{}", result.stderr);
        assert!(result.stdout.contains("seed"));
        assert!(!src.path().join("leaked.txt").exists());
    }

    #[test]
    fn policy_agrees_with_the_shared_command_policy_corpus() {
        for (command, rejected) in agent_body_core::guard_corpus::COMMAND_POLICY_CORPUS {
            assert_eq!(policy_rejection(command).is_some(), *rejected, "{command}");
        }
    }

    static FIRECRACKER_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn firecracker_job(dir: &Path, command: &str) -> SandboxExecute {
        let mut job = job_in(dir, command);
        job.backend = Some("firecracker".into());
        job
    }

    #[tokio::test]
    async fn firecracker_request_fails_closed_without_the_opt_in() {
        let _guard = FIRECRACKER_ENV.lock().unwrap();
        unsafe {
            std::env::remove_var(ENV_ALLOW_SUBPROCESS_FALLBACK);
        }
        let src = tempfile::tempdir().unwrap();
        let result = run_isolated(&firecracker_job(src.path(), "echo hi"), &plain_options()).await;
        assert!(!result.success);
        assert!(
            result.stderr.contains(ENV_ALLOW_SUBPROCESS_FALLBACK),
            "{}",
            result.stderr
        );
        assert!(!result.stdout.contains("hi"));
    }

    #[tokio::test]
    async fn firecracker_request_with_the_opt_in_runs_and_is_marked_degraded() {
        let _guard = FIRECRACKER_ENV.lock().unwrap();
        unsafe {
            std::env::remove_var("AUTONOMIC_FC_KERNEL");
            std::env::remove_var("AUTONOMIC_FC_ROOTFS");
            std::env::set_var(ENV_ALLOW_SUBPROCESS_FALLBACK, "1");
        }
        let src = tempfile::tempdir().unwrap();
        let result = run_isolated(&firecracker_job(src.path(), "echo hi"), &plain_options()).await;
        unsafe {
            std::env::remove_var(ENV_ALLOW_SUBPROCESS_FALLBACK);
        }
        assert!(result.success, "{}", result.stderr);
        assert!(result.stdout.contains("hi"));
        assert!(result.stderr.starts_with("degraded:"), "{}", result.stderr);
    }

    #[tokio::test]
    async fn job_running_past_the_limit_is_killed() {
        let src = tempfile::tempdir().unwrap();
        let result = run_with_timeout(
            &job_in(src.path(), "sleep 5"),
            &plain_options(),
            Duration::from_millis(200),
        )
        .await;
        assert!(!result.success);
        assert!(result.stderr.contains("timeout"), "{}", result.stderr);
    }

    #[test]
    fn overlay_copy_stops_at_the_size_budget() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("big.bin"), vec![0u8; 10]).unwrap();
        let dst = tempfile::tempdir().unwrap();
        let mut budget = 5;
        assert!(copy_tree(src.path(), dst.path(), &mut budget).is_err());
    }

    #[test]
    fn shell_escape_quotes_spaces() {
        let p = PathBuf::from("/tmp/my script.sh");
        assert!(shell_escape(&p).starts_with('\''));
    }
}
