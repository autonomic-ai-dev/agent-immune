use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub nats: NatsConfig,
    pub sandbox: SandboxConfig,
    pub scanner: ScannerConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    pub url: String,
    pub jetstream_consumer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub network_blackhole: bool,
    /// `subprocess` (default) or `firecracker`
    #[serde(default = "default_sandbox_backend")]
    pub backend: String,
    /// Apply seccomp-BPF before subprocess spawn (Linux only).
    #[serde(default = "default_true")]
    pub seccomp: bool,
}

fn default_sandbox_backend() -> String {
    "subprocess".into()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub osv_api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig { port: 3106 },
            nats: NatsConfig {
                url: "nats://localhost:4222".into(),
                jetstream_consumer: true,
            },
            sandbox: SandboxConfig {
                network_blackhole: true,
                backend: "subprocess".into(),
                seccomp: true,
            },
            scanner: ScannerConfig {
                osv_api_url: "https://api.osv.dev/v1".into(),
            },
            logging: LoggingConfig {
                level: "info".into(),
            },
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        agent_body_core::config_path()
    }

    pub fn load() -> Result<Self> {
        agent_body_core::organ_config::load("immune")
    }
}
