use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const OSV_BATCH_URL: &str = "https://api.osv.dev/v1/querybatch";
const OSV_VULN_URL: &str = "https://api.osv.dev/v1/vulns/";

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
    pub blocking: bool,
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
    #[serde(default)]
    vulns: Vec<OsvVuln>,
}

#[derive(Debug, Deserialize)]
struct OsvVuln {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OsvDetail {
    summary: Option<String>,
    database_specific: Option<OsvDatabaseSpecific>,
}

#[derive(Debug, Deserialize)]
struct OsvDatabaseSpecific {
    severity: Option<String>,
}

fn is_blocking_severity(severity: &str) -> bool {
    matches!(severity, "CRITICAL" | "HIGH" | "UNKNOWN")
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
            blocking: false,
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
    let batch: OsvBatchResponse = client
        .post(OSV_BATCH_URL)
        .json(&batch_query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut details: HashMap<String, (String, String)> = HashMap::new();
    let mut vulnerabilities = Vec::new();
    for (i, result) in batch.results.into_iter().enumerate() {
        let Some(pkg) = packages.get(i) else {
            continue;
        };
        for vuln in result.vulns {
            if !details.contains_key(&vuln.id) {
                let detail = fetch_detail(&client, &vuln.id).await?;
                details.insert(vuln.id.clone(), detail);
            }
            let (summary, severity) = details[&vuln.id].clone();
            vulnerabilities.push(Vulnerability {
                package: pkg.name.clone(),
                version: pkg.version.clone(),
                ecosystem: pkg.ecosystem.clone(),
                osv_id: vuln.id,
                summary,
                severity,
            });
        }
    }

    let blocking = vulnerabilities
        .iter()
        .any(|v| is_blocking_severity(&v.severity));
    Ok(ScanResult {
        packages: packages.to_vec(),
        vulnerabilities,
        blocking,
    })
}

async fn fetch_detail(client: &reqwest::Client, osv_id: &str) -> Result<(String, String)> {
    let detail: OsvDetail = client
        .get(format!("{OSV_VULN_URL}{osv_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let severity = detail
        .database_specific
        .and_then(|d| d.severity)
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "UNKNOWN".into());
    Ok((detail.summary.unwrap_or_default(), severity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_high_and_unknown_severities_block() {
        assert!(is_blocking_severity("CRITICAL"));
        assert!(is_blocking_severity("HIGH"));
        assert!(is_blocking_severity("UNKNOWN"));
        assert!(!is_blocking_severity("MODERATE"));
        assert!(!is_blocking_severity("LOW"));
    }

    #[test]
    fn detail_without_database_severity_parses_to_no_severity() {
        let detail: OsvDetail = serde_json::from_str(r#"{"id":"X","summary":"s"}"#).unwrap();
        assert_eq!(detail.summary.as_deref(), Some("s"));
        assert!(detail.database_specific.and_then(|d| d.severity).is_none());
    }

    #[test]
    fn detail_reads_database_severity() {
        let detail: OsvDetail =
            serde_json::from_str(r#"{"id":"GHSA-x","database_specific":{"severity":"HIGH"}}"#)
                .unwrap();
        assert_eq!(
            detail.database_specific.and_then(|d| d.severity).as_deref(),
            Some("HIGH")
        );
    }
}
