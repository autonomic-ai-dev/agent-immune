use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize)]
pub struct LeakCheckReport {
    pub passed: bool,
    pub peak_rss_kb: u64,
    pub samples: u64,
    pub growth_kb: i64,
    pub threshold_kb: u64,
    pub message: String,
}

pub const DEFAULT_THRESHOLD_KB: u64 = 512 * 1024; // 512 MiB growth
const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);

/// Run a command and fail if resident memory grows beyond `threshold_kb`.
pub async fn verify_no_memory_leak(
    command: &str,
    cwd: Option<&Path>,
    threshold_kb: u64,
) -> Result<LeakCheckReport> {
    let threshold_kb = if threshold_kb == 0 {
        DEFAULT_THRESHOLD_KB
    } else {
        threshold_kb
    };
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let mut child = cmd.spawn().context("spawn leak-check process")?;
    let pid = child.id().unwrap_or(0);

    let mut baseline_kb = 0u64;
    let mut peak_kb = 0u64;
    let mut samples = 0u64;

    while child.try_wait()?.is_none() {
        if let Some(rss) = read_rss_kb(pid) {
            if samples == 0 {
                baseline_kb = rss;
            }
            peak_kb = peak_kb.max(rss);
            samples += 1;
            if rss.saturating_sub(baseline_kb) > threshold_kb {
                let _ = child.kill().await;
                return Ok(LeakCheckReport {
                    passed: false,
                    peak_rss_kb: peak_kb,
                    samples,
                    growth_kb: rss as i64 - baseline_kb as i64,
                    threshold_kb,
                    message: format!(
                        "memory growth exceeded threshold (+{} KiB > {} KiB)",
                        rss.saturating_sub(baseline_kb),
                        threshold_kb
                    ),
                });
            }
        }
        sleep(SAMPLE_INTERVAL).await;
    }

    let status = child.wait().await?;
    let growth = peak_kb.saturating_sub(baseline_kb) as i64;
    let passed = status.success() && growth <= threshold_kb as i64;

    Ok(LeakCheckReport {
        passed,
        peak_rss_kb: peak_kb,
        samples,
        growth_kb: growth,
        threshold_kb,
        message: if passed {
            "memory stable".into()
        } else if !status.success() {
            format!("process exited with {}", status)
        } else {
            format!("memory growth {growth} KiB exceeds threshold {threshold_kb} KiB")
        },
    })
}

fn read_rss_kb(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        for line in status.lines() {
            if let Some(kb) = line.strip_prefix("VmRSS:") {
                let kb = kb.trim().trim_end_matches(" kB").parse().ok()?;
                return Some(kb);
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command as SyncCommand;
        let output = SyncCommand::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let rss = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .ok()?;
        Some(rss)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

/// Gate dataset commits: run script and require stable memory profile.
pub async fn gate_trajectory_script(path: &Path, threshold_kb: u64) -> Result<LeakCheckReport> {
    let command = if path.is_file() {
        format!("sh {}", shell_escape(path))
    } else {
        anyhow::bail!("script not found: {}", path.display());
    };
    let started = Instant::now();
    let report = verify_no_memory_leak(&command, path.parent(), threshold_kb).await?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        passed = report.passed,
        growth_kb = report.growth_kb,
        "trajectory memory gate"
    );
    Ok(report)
}

fn shell_escape(path: &Path) -> String {
    let s = path.display().to_string();
    if s.contains(' ') || s.contains('\'') {
        format!("'{s}'")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stable_command_passes() {
        let report = verify_no_memory_leak("echo ok", None, DEFAULT_THRESHOLD_KB)
            .await
            .unwrap();
        assert!(report.passed, "{report:?}");
    }
}
