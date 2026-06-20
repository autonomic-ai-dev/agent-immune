use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub scanner: ScannerConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub osv_api_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
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
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("agent-immune")
            .join("config.yaml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let s = std::fs::read_to_string(&path)?;
            Ok(serde_yaml::from_str(&s)?)
        } else {
            let cfg = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let s = serde_yaml::to_string(&cfg)?;
            std::fs::write(&path, &s)?;
            Ok(cfg)
        }
    }
}
