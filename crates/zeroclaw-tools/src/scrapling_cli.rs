//! Scrapling CLI fallback tool — wraps the `scrapling extract` command.
//!
//! Complements the Scrapling MCP server with a direct subprocess interface
//! for environments where the MCP runtime is unavailable or undesirable.
//! Spawns `scrapling extract <op> <url> <output_file>` against a workspace-
//! local temp file, then returns the file contents (truncated). The temp
//! file is removed before returning.

use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::ScraplingCliConfig;

const SAFE_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "TERM",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "USER",
    "SHELL",
    "TMPDIR",
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    "PROGRAMFILES",
    "SYSTEMROOT",
];

/// `scrapling extract` subcommands the agent may invoke.
const SUPPORTED_OPERATIONS: &[&str] = &["get", "post", "put", "delete", "fetch", "stealthy_fetch"];

pub struct ScraplingCliTool {
    security: Arc<SecurityPolicy>,
    config: ScraplingCliConfig,
    allowed_domains: Vec<String>,
}

impl ScraplingCliTool {
    pub fn new(security: Arc<SecurityPolicy>, config: ScraplingCliConfig) -> Self {
        let allowed_domains = normalize_domains(config.allowed_domains.clone());
        Self {
            security,
            config,
            allowed_domains,
        }
    }

    fn validate_operation(op: &str) -> Result<&'static str, String> {
        SUPPORTED_OPERATIONS
            .iter()
            .find(|candidate| **candidate == op)
            .copied()
            .ok_or_else(|| {
                format!(
                    "unsupported operation '{op}'. Supported: {}",
                    SUPPORTED_OPERATIONS.join(", ")
                )
            })
    }

    fn validate_url(&self, raw: &str) -> Result<String, String> {
        let url = raw.trim();
        if url.is_empty() {
            return Err("url cannot be empty".into());
        }
        if url.chars().any(char::is_whitespace) {
            return Err("url cannot contain whitespace".into());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Only http:// and https:// URLs are allowed".into());
        }
        if self.allowed_domains.is_empty() {
            return Err(
                "scrapling_cli is enabled but no allowed_domains are configured. Add [scrapling_cli].allowed_domains in config.toml"
                    .into(),
            );
        }
        let host = extract_host(url)?;
        if !host_matches_allowlist(&host, &self.allowed_domains) {
            return Err(format!(
                "host '{host}' is not in scrapling_cli.allowed_domains"
            ));
        }
        Ok(url.to_string())
    }

    fn parse_extra_args(args_val: &serde_json::Value) -> Result<Vec<String>, String> {
        match args_val {
            serde_json::Value::Null => Ok(Vec::new()),
            serde_json::Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let s = item
                        .as_str()
                        .ok_or_else(|| "args items must be strings".to_string())?;
                    if s.chars().any(|c| c == '\0') {
                        return Err("args items cannot contain NUL bytes".into());
                    }
                    out.push(s.to_string());
                }
                Ok(out)
            }
            _ => Err("args must be an array of strings".into()),
        }
    }

    fn temp_output_path(&self) -> PathBuf {
        let mut p = self.security.workspace_dir.clone();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!(".zeroclaw-scrapling-{ts}.md"));
        p
    }

    fn truncate(&self, text: String) -> String {
        if text.len() <= self.config.max_output_bytes {
            return text;
        }
        let mut b = self.config.max_output_bytes.min(text.len());
        while b > 0 && !text.is_char_boundary(b) {
            b -= 1;
        }
        let mut out = text;
        out.truncate(b);
        out.push_str("\n... [output truncated]");
        out
    }
}

#[async_trait]
impl Tool for ScraplingCliTool {
    fn name(&self) -> &str {
        "scrapling_cli"
    }

    fn description(&self) -> &str {
        "Scrape a URL with the Scrapling CLI (`scrapling extract`). Operations: get, post, put, \
         delete, fetch, stealthy_fetch. Returns the captured page content as text. \
         Allowlist-only domains, http(s) only."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["get", "post", "put", "delete", "fetch", "stealthy_fetch"],
                    "description": "Scrapling extract subcommand to invoke"
                },
                "url": {
                    "type": "string",
                    "description": "HTTP(S) URL to scrape (must be in allowed_domains)"
                },
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional extra CLI flags, e.g. ['--css', '.product .price'] or ['-H', 'User-Agent: foo']"
                }
            },
            "required": ["operation", "url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(failure(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }
        if let Err(error) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "scrapling_cli")
        {
            return Ok(failure(error));
        }

        let op_raw = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'operation' parameter"))?;
        let operation = match Self::validate_operation(op_raw) {
            Ok(o) => o,
            Err(e) => return Ok(failure(e)),
        };

        let url_raw = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;
        let url = match self.validate_url(url_raw) {
            Ok(u) => u,
            Err(e) => return Ok(failure(e)),
        };

        let extra =
            match Self::parse_extra_args(args.get("args").unwrap_or(&serde_json::Value::Null)) {
                Ok(v) => v,
                Err(e) => return Ok(failure(e)),
            };

        if !self.security.record_action() {
            return Ok(failure(
                "Rate limit exceeded: action budget exhausted".into(),
            ));
        }

        let output_path = self.temp_output_path();
        let bin = if cfg!(target_os = "windows") {
            "scrapling.cmd"
        } else {
            "scrapling"
        };
        let mut cmd = Command::new(bin);
        cmd.arg("extract")
            .arg(operation)
            .arg(&url)
            .arg(&output_path);
        for a in &extra {
            cmd.arg(a);
        }

        cmd.env_clear();
        for var in SAFE_ENV_VARS {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        for var in &self.config.env_passthrough {
            let trimmed = var.trim();
            if !trimmed.is_empty()
                && let Ok(val) = std::env::var(trimmed)
            {
                cmd.env(trimmed, val);
            }
        }
        cmd.current_dir(&self.security.workspace_dir);
        cmd.kill_on_drop(true);

        let timeout = Duration::from_secs(self.config.timeout_secs);
        let exec_result = tokio::time::timeout(timeout, cmd.output()).await;

        // Always try to clean up the temp file, even on error.
        let cleanup = |path: &PathBuf| {
            let _ = std::fs::remove_file(path);
        };

        match exec_result {
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if !output.status.success() {
                    cleanup(&output_path);
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(if stderr.is_empty() {
                            format!(
                                "scrapling exited with status {}",
                                output.status.code().unwrap_or(-1)
                            )
                        } else {
                            stderr
                        }),
                    });
                }

                let content = match tokio::fs::read_to_string(&output_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        cleanup(&output_path);
                        return Ok(failure(format!(
                            "scrapling succeeded but reading output file failed: {e}"
                        )));
                    }
                };
                cleanup(&output_path);
                let truncated = self.truncate(content);
                Ok(ToolResult {
                    success: true,
                    output: truncated,
                    error: if stderr.is_empty() {
                        None
                    } else {
                        Some(stderr)
                    },
                })
            }
            Ok(Err(e)) => {
                cleanup(&output_path);
                let err_msg = e.to_string();
                let msg = if err_msg.contains("No such file or directory")
                    || err_msg.contains("not found")
                    || err_msg.contains("cannot find")
                {
                    "Scrapling CLI ('scrapling') not found in PATH. Install with: pip install \"scrapling[shell]\" && scrapling install".into()
                } else {
                    format!("Failed to execute scrapling: {e}")
                };
                Ok(failure(msg))
            }
            Err(_) => {
                cleanup(&output_path);
                Ok(failure(format!(
                    "Scrapling CLI timed out after {}s and was killed",
                    self.config.timeout_secs
                )))
            }
        }
    }
}

fn failure(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}

// ── helpers (locally owned; parallel to http_request) ────────────

fn normalize_domains(domains: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = domains
        .into_iter()
        .filter_map(|d| {
            let mut s = d.trim().to_lowercase();
            if s.is_empty() {
                return None;
            }
            if let Some(rest) = s.strip_prefix("https://") {
                s = rest.to_string();
            } else if let Some(rest) = s.strip_prefix("http://") {
                s = rest.to_string();
            }
            if let Some((h, _)) = s.split_once('/') {
                s = h.to_string();
            }
            if let Some((h, _)) = s.split_once(':') {
                s = h.to_string();
            }
            s = s.trim_matches('.').to_string();
            if s.is_empty() || s.chars().any(char::is_whitespace) {
                None
            } else {
                Some(s)
            }
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn extract_host(url: &str) -> Result<String, String> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or_else(|| "Only http:// and https:// URLs are allowed".to_string())?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .ok_or_else(|| "Invalid URL".to_string())?;
    if authority.is_empty() {
        return Err("URL must include a host".into());
    }
    if authority.contains('@') {
        return Err("URL userinfo is not allowed".into());
    }
    let host = authority
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .to_lowercase();
    if host.is_empty() {
        return Err("URL must include a valid host".into());
    }
    Ok(host)
}

fn host_matches_allowlist(host: &str, allowed: &[String]) -> bool {
    if allowed.iter().any(|d| d == "*") {
        return true;
    }
    allowed.iter().any(|d| {
        host == d
            || host
                .strip_suffix(d.as_str())
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;

    fn tool_with(domains: Vec<&str>) -> ScraplingCliTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let cfg = ScraplingCliConfig {
            enabled: true,
            allowed_domains: domains.into_iter().map(String::from).collect(),
            timeout_secs: 30,
            max_output_bytes: 1_000_000,
            env_passthrough: vec![],
        };
        ScraplingCliTool::new(security, cfg)
    }

    #[test]
    fn name_is_scrapling_cli() {
        let t = tool_with(vec!["example.com"]);
        assert_eq!(t.name(), "scrapling_cli");
    }

    #[test]
    fn schema_requires_operation_and_url() {
        let t = tool_with(vec!["example.com"]);
        let s = t.parameters_schema();
        let req = s["required"].as_array().unwrap();
        assert!(req.contains(&json!("operation")));
        assert!(req.contains(&json!("url")));
    }

    #[test]
    fn validate_operation_accepts_supported() {
        for op in ["get", "post", "put", "delete", "fetch", "stealthy_fetch"] {
            assert!(ScraplingCliTool::validate_operation(op).is_ok(), "{op}");
        }
    }

    #[test]
    fn validate_operation_rejects_unknown() {
        assert!(ScraplingCliTool::validate_operation("head").is_err());
        assert!(ScraplingCliTool::validate_operation("crawl").is_err());
        assert!(ScraplingCliTool::validate_operation("").is_err());
    }

    #[test]
    fn validate_url_requires_allowlist() {
        let t = tool_with(vec![]);
        assert!(t.validate_url("https://example.com").is_err());
    }

    #[test]
    fn validate_url_accepts_https_in_allowlist() {
        let t = tool_with(vec!["example.com"]);
        assert!(t.validate_url("https://example.com/page").is_ok());
    }

    #[test]
    fn validate_url_accepts_subdomain() {
        let t = tool_with(vec!["example.com"]);
        assert!(t.validate_url("https://api.example.com/v1").is_ok());
    }

    #[test]
    fn validate_url_rejects_unrelated_host() {
        let t = tool_with(vec!["example.com"]);
        assert!(t.validate_url("https://evil.com").is_err());
    }

    #[test]
    fn validate_url_rejects_ftp_and_empty() {
        let t = tool_with(vec!["example.com"]);
        assert!(t.validate_url("ftp://example.com").is_err());
        assert!(t.validate_url("").is_err());
    }

    #[test]
    fn validate_url_rejects_whitespace() {
        let t = tool_with(vec!["example.com"]);
        assert!(t.validate_url("https://example.com/with space").is_err());
    }

    #[test]
    fn validate_url_rejects_userinfo() {
        let t = tool_with(vec!["example.com"]);
        assert!(t.validate_url("https://user@example.com").is_err());
    }

    #[test]
    fn normalize_domains_strips_scheme_path_port() {
        let out = normalize_domains(vec![
            "https://Example.com/path".into(),
            "EXAMPLE.com:443".into(),
            "  ".into(),
        ]);
        assert_eq!(out, vec!["example.com".to_string()]);
    }

    #[test]
    fn parse_extra_args_accepts_array() {
        let v = json!(["--css", ".price"]);
        let parsed = ScraplingCliTool::parse_extra_args(&v).unwrap();
        assert_eq!(parsed, vec!["--css", ".price"]);
    }

    #[test]
    fn parse_extra_args_rejects_non_array() {
        assert!(ScraplingCliTool::parse_extra_args(&json!("foo")).is_err());
        assert!(ScraplingCliTool::parse_extra_args(&json!({"a": 1})).is_err());
    }

    #[test]
    fn parse_extra_args_rejects_nul_byte() {
        assert!(ScraplingCliTool::parse_extra_args(&json!(["a\0b"])).is_err());
    }

    #[test]
    fn truncate_below_limit_unchanged() {
        let t = tool_with(vec!["example.com"]);
        assert_eq!(t.truncate("hello".into()), "hello");
    }

    #[test]
    fn truncate_above_limit_marked() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let cfg = ScraplingCliConfig {
            enabled: true,
            allowed_domains: vec!["example.com".into()],
            timeout_secs: 30,
            max_output_bytes: 10,
            env_passthrough: vec![],
        };
        let t = ScraplingCliTool::new(security, cfg);
        let out = t.truncate("this string is definitely longer than ten bytes".into());
        assert!(out.contains("[output truncated]"));
        assert!(out.starts_with("this strin"));
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let cfg = ScraplingCliConfig {
            enabled: true,
            allowed_domains: vec!["example.com".into()],
            timeout_secs: 30,
            max_output_bytes: 1000,
            env_passthrough: vec![],
        };
        let t = ScraplingCliTool::new(security, cfg);
        let res = t
            .execute(json!({"operation": "get", "url": "https://example.com"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("read-only"));
    }

    #[tokio::test]
    async fn execute_blocks_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            max_actions_per_hour: 0,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let cfg = ScraplingCliConfig {
            enabled: true,
            allowed_domains: vec!["example.com".into()],
            timeout_secs: 30,
            max_output_bytes: 1000,
            env_passthrough: vec![],
        };
        let t = ScraplingCliTool::new(security, cfg);
        let res = t
            .execute(json!({"operation": "get", "url": "https://example.com"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_missing_operation_errors() {
        let t = tool_with(vec!["example.com"]);
        let r = t.execute(json!({"url": "https://example.com"})).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_missing_url_errors() {
        let t = tool_with(vec!["example.com"]);
        let r = t.execute(json!({"operation": "get"})).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_rejects_disallowed_url_non_fatal() {
        let t = tool_with(vec!["example.com"]);
        let r = t
            .execute(json!({"operation": "get", "url": "https://evil.com"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("allowed_domains"));
    }

    #[tokio::test]
    async fn execute_rejects_unknown_operation_non_fatal() {
        let t = tool_with(vec!["example.com"]);
        let r = t
            .execute(json!({"operation": "crawl", "url": "https://example.com"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Supported"));
    }
}
