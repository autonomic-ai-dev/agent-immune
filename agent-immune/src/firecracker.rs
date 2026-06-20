//! Firecracker microVM execution when kernel + rootfs are configured.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct FirecrackerConfig {
    pub kernel_path: PathBuf,
    pub rootfs_path: PathBuf,
    pub vcpu_count: u32,
    pub mem_mib: u32,
}

impl FirecrackerConfig {
    pub fn from_env() -> Option<Self> {
        let kernel = std::env::var("AUTONOMIC_FC_KERNEL").ok()?;
        let rootfs = std::env::var("AUTONOMIC_FC_ROOTFS").ok()?;
        if !Path::new(&kernel).is_file() || !Path::new(&rootfs).is_file() {
            return None;
        }
        Some(Self {
            kernel_path: PathBuf::from(kernel),
            rootfs_path: PathBuf::from(rootfs),
            vcpu_count: std::env::var("AUTONOMIC_FC_VCPUS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            mem_mib: std::env::var("AUTONOMIC_FC_MEM_MIB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(512),
        })
    }
}

pub fn binary_available() -> bool {
    which_firecracker().is_some()
}

pub fn is_available() -> bool {
    binary_available() && FirecrackerConfig::from_env().is_some()
}

fn which_firecracker() -> Option<PathBuf> {
    std::env::var("AUTONOMIC_FC_BIN")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            std::process::Command::new("which")
                .arg("firecracker")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| PathBuf::from(s.trim()))
                })
                .filter(|p| p.is_file())
        })
}

/// Run a shell command inside a Firecracker microVM.
pub async fn run_command(
    command: &str,
    cwd: &Path,
    config: &FirecrackerConfig,
) -> std::io::Result<std::process::Output> {
    let fc_bin = which_firecracker().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "firecracker binary not found")
    })?;

    let work = tempfile::tempdir()?;
    let socket_path = work.path().join("fc.sock");
    let config_path = work.path().join("config.json");

    let escaped_cmd = command.replace('\'', "'\\''");
    let boot_args = format!(
        "console=ttyS0 reboot=k panic=1 pci=off init=/bin/sh -- -c '{}'",
        escaped_cmd.replace('\n', " ")
    );

    let fc_config = serde_json::json!({
        "boot-source": {
            "kernel_image_path": config.kernel_path,
            "boot_args": boot_args,
        },
        "drives": [{
            "drive_id": "rootfs",
            "path_on_host": config.rootfs_path,
            "is_root_device": true,
            "is_read_only": false
        }],
        "machine-config": {
            "vcpu_count": config.vcpu_count,
            "mem_size_mib": config.mem_mib,
        }
    });
    std::fs::write(&config_path, serde_json::to_vec_pretty(&fc_config)?)?;

    info!(
        "firecracker: booting microVM in {} ({} MiB, {} vCPU)",
        cwd.display(),
        config.mem_mib,
        config.vcpu_count
    );

    let output = Command::new(fc_bin)
        .args([
            "--api-sock",
            socket_path.to_str().unwrap_or("/tmp/fc.sock"),
            "--config-file",
            config_path.to_str().unwrap_or("config.json"),
            "--no-api",
        ])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        warn!(
            "firecracker exited {} — stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output)
}
