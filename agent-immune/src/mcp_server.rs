use rmcp::model::{CallToolResult, Content, ErrorData as McpError, ServerInfo};
use rmcp::serve_server;
use rmcp::tool;
use rmcp::ServerHandler;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

use crate::config::Config;

#[derive(Clone)]
pub struct ImmuneMcp {
    config: Config,
}

impl ImmuneMcp {
    pub async fn run(config: Config) -> anyhow::Result<()> {
        let server = Self { config };
        let service = serve_server(server, rmcp::transport::io::stdio()).await?;
        service.waiting().await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScanManifestParams {
    /// Path to a manifest file (Cargo.toml, package.json)
    path: Option<String>,
    /// Raw file content as an alternative to path
    content: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SandboxRunParams {
    /// Path to an executable script
    script_path: Option<String>,
    /// Inline script content as an alternative to script_path
    script_content: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LintAstParams {
    /// Code snippet to analyse
    code: String,
    /// Language of the code (python, javascript)
    language: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct LintWarning {
    line: usize,
    column: usize,
    message: String,
    severity: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct LintReport {
    warnings: Vec<LintWarning>,
    language: String,
}

#[tool(tool_box)]
impl ImmuneMcp {
    #[tool(
        description = "Scan a manifest file (Cargo.toml, package.json) against OSV.dev for known CVEs"
    )]
    async fn immune_scan_manifest(
        &self,
        #[tool(aggr)] params: ScanManifestParams,
    ) -> Result<CallToolResult, McpError> {
        let (_keep, path) = resolve_path_or_content(params.path, params.content, None)?;

        let pkgs = crate::scanner::parse_manifest(&path)
            .map_err(|e| McpError::internal_error(format!("parse failed: {e}"), None))?;
        let results = crate::scanner::query_osv(&pkgs)
            .await
            .map_err(|e| McpError::internal_error(format!("osv query failed: {e}"), None))?;

        let text = serde_json::to_string_pretty(&results).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "Execute an untrusted script in a sandboxed environment; returns output and exit code"
    )]
    async fn immune_sandbox_run(
        &self,
        #[tool(aggr)] params: SandboxRunParams,
    ) -> Result<CallToolResult, McpError> {
        let (_keep, path) =
            resolve_path_or_content(params.script_path, params.script_content, None)?;

        let options = crate::sandbox::SandboxOptions::from(&self.config.sandbox);
        let result = crate::sandbox::run_script(&path, &options).await;

        let text = serde_json::to_string_pretty(&result).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "Run AST-based security linting against a code snippet; supports python and javascript"
    )]
    async fn immune_lint_ast(
        &self,
        #[tool(aggr)] params: LintAstParams,
    ) -> Result<CallToolResult, McpError> {
        let warnings = lint_code(&params.code, &params.language)?;
        let report = LintReport {
            warnings,
            language: params.language,
        };
        let text = serde_json::to_string_pretty(&report).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[tool(tool_box)]
impl ServerHandler for ImmuneMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                concat!(
                    "Agent-Immune MCP server provides security scanning tools:\n",
                    "- immune_scan_manifest: Scan Cargo.toml / package.json for CVEs via OSV.dev\n",
                    "- immune_sandbox_run:  Execute an untrusted script in an isolated subprocess\n",
                    "- immune_lint_ast:     Pattern-based security linting for python / javascript"
                )
                .into(),
            ),
            ..Default::default()
        }
    }
}

fn resolve_path_or_content(
    path: Option<String>,
    content: Option<String>,
    _ext_hint: Option<&str>,
) -> Result<(Option<tempfile::TempDir>, PathBuf), McpError> {
    match (path, content) {
        (Some(p), None) => Ok((None, PathBuf::from(p))),
        (None, Some(c)) => {
            let guess = if c.trim_start().starts_with('{') {
                "package.json"
            } else {
                "Cargo.toml"
            };
            let tmp = tempfile::tempdir()
                .map_err(|e| McpError::internal_error(format!("tempdir: {e}"), None))?;
            let fp = tmp.path().join(guess);
            let mut f = std::fs::File::create(&fp)
                .map_err(|e| McpError::internal_error(format!("create: {e}"), None))?;
            f.write_all(c.as_bytes())
                .map_err(|e| McpError::internal_error(format!("write: {e}"), None))?;
            Ok((Some(tmp), fp))
        }
        _ => Err(McpError::invalid_params(
            "provide exactly one of 'path' or 'content'",
            None,
        )),
    }
}

fn lint_code(code: &str, language: &str) -> Result<Vec<LintWarning>, McpError> {
    let rules: &[(&str, &str, &str)] = match language {
        "python" | "py" => &[
            (
                "eval",
                "call",
                "Dangerous eval() — arbitrary code execution risk",
            ),
            (
                "exec",
                "call",
                "Dangerous exec() — arbitrary code execution risk",
            ),
            ("os.system", "call", "Shell injection risk via os.system()"),
            ("os.popen", "call", "Shell injection risk via os.popen()"),
            (
                "pickle.loads",
                "call",
                "Insecure deserialization via pickle.loads()",
            ),
            (
                "pickle.load",
                "call",
                "Insecure deserialization via pickle.load()",
            ),
        ],
        "javascript" | "js" | "typescript" | "ts" => &[
            (
                "eval(",
                "call",
                "Dangerous eval() — arbitrary code execution risk",
            ),
            (
                ".innerHTML",
                "assignment",
                "XSS risk via innerHTML assignment",
            ),
            (
                "new Function(",
                "call",
                "Dangerous Function constructor usage",
            ),
        ],
        _ => {
            return Err(McpError::invalid_params(
                format!("unsupported language: {language} (supported: python, javascript)"),
                None,
            ))
        }
    };

    let mut warnings = Vec::new();
    let lines: Vec<&str> = code.lines().collect();

    for (pattern, _kind, message) in rules {
        for (i, line) in lines.iter().enumerate() {
            if let Some(col) = line.find(pattern) {
                warnings.push(LintWarning {
                    line: i + 1,
                    column: col + 1,
                    message: message.to_string(),
                    severity: "warning".into(),
                });
            }
        }
    }

    warnings.sort_by_key(|w| (w.line, w.column));
    warnings.dedup_by_key(|w| (w.line, w.column, w.message.clone()));

    Ok(warnings)
}
