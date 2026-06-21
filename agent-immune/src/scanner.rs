use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub ecosystem: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub packages: Vec<Package>,
    pub vulnerabilities: Vec<Vulnerability>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Vulnerability {
    pub package: String,
    pub version: String,
    pub ecosystem: String,
    pub osv_id: String,
    pub summary: String,
    pub severity: String,
}

#[derive(Debug, Deserialize)]
struct OsvResponse {
    vulns: Vec<OsvVuln>,
}

#[derive(Debug, Deserialize)]
struct OsvVuln {
    id: String,
    summary: Option<String>,
    severity: Option<Vec<OsvSeverity>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OsvSeverity {
    #[serde(rename = "type")]
    severity_type: String,
    score: String,
}

pub fn parse_manifest(path: &Path) -> Result<Vec<Package>> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match filename {
        "Cargo.toml" => parse_cargo_toml(path),
        "package.json" => parse_package_json(path),
        _ => anyhow::bail!(
            "Unsupported manifest: {}. Supported: Cargo.toml, package.json",
            filename
        ),
    }
}

fn parse_cargo_toml(path: &Path) -> Result<Vec<Package>> {
    let content = std::fs::read_to_string(path)?;
    let parsed: toml::Value = toml::from_str(&content)?;
    let mut packages = Vec::new();

    if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_table()) {
        for (name, val) in deps {
            let version = match val {
                toml::Value::String(v) => v.clone(),
                toml::Value::Table(t) => t
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                _ => "unknown".to_string(),
            };
            packages.push(Package {
                name: name.clone(),
                version,
                ecosystem: "crates.io".into(),
            });
        }
    }

    Ok(packages)
}

fn parse_package_json(path: &Path) -> Result<Vec<Package>> {
    let content = std::fs::read_to_string(path)?;
    let parsed: serde_json::Value = serde_json::from_str(&content)?;
    let mut packages = Vec::new();

    if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) {
        for (name, val) in deps {
            let version = val.as_str().unwrap_or("unknown").to_string();
            packages.push(Package {
                name: name.clone(),
                version,
                ecosystem: "npm".into(),
            });
        }
    }

    Ok(packages)
}

#[derive(Debug, Deserialize)]
struct OsvBatchResponse {
    results: Vec<OsvResponse>,
}

pub async fn query_osv(packages: &[Package]) -> Result<ScanResult> {
    if packages.is_empty() {
        return Ok(ScanResult {
            packages: Vec::new(),
            vulnerabilities: Vec::new(),
        });
    }

    let client = reqwest::Client::new();
    let queries: Vec<_> = packages
        .iter()
        .map(|pkg| {
            serde_json::json!({
                "package": {
                    "name": pkg.name,
                    "ecosystem": pkg.ecosystem,
                },
                "version": pkg.version,
            })
        })
        .collect();

    let batch_query = serde_json::json!({ "queries": queries });
    let mut vulnerabilities = Vec::new();

    match client
        .post("https://api.osv.dev/v1/querybatch")
        .json(&batch_query)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(osv_batch) = resp.json::<OsvBatchResponse>().await {
                for (i, result) in osv_batch.results.into_iter().enumerate() {
                    if let Some(pkg) = packages.get(i) {
                        for vuln in result.vulns {
                            vulnerabilities.push(Vulnerability {
                                package: pkg.name.clone(),
                                version: pkg.version.clone(),
                                ecosystem: pkg.ecosystem.clone(),
                                osv_id: vuln.id,
                                summary: vuln.summary.unwrap_or_default(),
                                severity: vuln
                                    .severity
                                    .and_then(|s| s.first().map(|se| se.score.clone()))
                                    .unwrap_or_default(),
                            });
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("OSV batch query failed: {}", e);
        }
    }

    Ok(ScanResult {
        packages: packages.to_vec(),
        vulnerabilities,
    })
}
