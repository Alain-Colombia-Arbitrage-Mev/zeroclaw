//! SocialClaw publishing tool — wraps the `socialclaw` npm CLI.
//!
//! Provides scheduled and ad-hoc publishing across X, LinkedIn, Instagram,
//! Facebook Pages, TikTok, Discord, Telegram, YouTube, Reddit, WordPress,
//! and Pinterest through the hosted SocialClaw workspace API.
//!
//! Credentials are passed via the `SOCIALCLAW_API_KEY` environment variable,
//! sourced from `[socialclaw].api_key` (handled by the keyring-backed
//! `#[secret]` config field).

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::SocialclawConfig;

/// Environment variables safe to pass through to the `socialclaw` subprocess.
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

/// Subcommands exposed to the agent when `allowed_commands` is empty.
///
/// Curated to cover the full publishing lifecycle (validate → preview → apply →
/// inspect → analytics) plus account/asset management and explicit destructive
/// operations. Operators who want a tighter surface set `allowed_commands`
/// explicitly in config.
const DEFAULT_ALLOWED_COMMANDS: &[&str] = &[
    "accounts list",
    "accounts capabilities",
    "accounts connect",
    "accounts disconnect",
    "assets upload",
    "assets delete",
    "campaigns preview",
    "validate",
    "apply",
    "posts get",
    "posts delete",
    "status",
    "analytics post",
    "usage",
    "workspace health",
];

/// Publishes content via the SocialClaw CLI.
pub struct SocialclawTool {
    security: Arc<SecurityPolicy>,
    config: SocialclawConfig,
}

impl SocialclawTool {
    pub fn new(security: Arc<SecurityPolicy>, config: SocialclawConfig) -> Self {
        Self { security, config }
    }

    fn allowed_commands(&self) -> Vec<String> {
        if self.config.allowed_commands.is_empty() {
            DEFAULT_ALLOWED_COMMANDS
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            self.config.allowed_commands.clone()
        }
    }

    fn validate_command(&self, command: &str) -> Result<Vec<String>, String> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Err("command cannot be empty".into());
        }

        let parts: Vec<String> = trimmed.split_whitespace().map(String::from).collect();

        let allowed = self.allowed_commands();
        if !allowed.iter().any(|c| c == trimmed) {
            return Err(format!(
                "command '{trimmed}' is not in socialclaw.allowed_commands"
            ));
        }

        Ok(parts)
    }

    /// Parses `args` for any `--provider <name>` or `-p <name>` flag and
    /// rejects it if the provider is not in `allowed_providers` (unless the
    /// allowlist is `["*"]`).
    fn validate_providers(&self, args: &[String]) -> Result<(), String> {
        if self.config.allowed_providers.iter().any(|p| p == "*") {
            return Ok(());
        }

        let mut i = 0;
        while i < args.len() {
            let needs_lookup = args[i] == "--provider" || args[i] == "-p";
            if needs_lookup {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("flag '{}' missing value", args[i]))?;
                let value_lc = value.to_lowercase();
                if !self
                    .config
                    .allowed_providers
                    .iter()
                    .any(|p| p.to_lowercase() == value_lc)
                {
                    return Err(format!(
                        "provider '{value}' is not in socialclaw.allowed_providers"
                    ));
                }
                i += 2;
            } else if let Some(rest) = args[i].strip_prefix("--provider=") {
                let value_lc = rest.to_lowercase();
                if !self
                    .config
                    .allowed_providers
                    .iter()
                    .any(|p| p.to_lowercase() == value_lc)
                {
                    return Err(format!(
                        "provider '{rest}' is not in socialclaw.allowed_providers"
                    ));
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        Ok(())
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
}

#[async_trait]
impl Tool for SocialclawTool {
    fn name(&self) -> &str {
        "socialclaw"
    }

    fn description(&self) -> &str {
        "Publish and schedule social content via SocialClaw (X, LinkedIn, Instagram, Facebook \
         Pages, TikTok, Discord, Telegram, YouTube, Reddit, WordPress, Pinterest). Subcommand-driven; \
         all calls append `--json`. Subject to allowed_commands and allowed_providers in config."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Socialclaw subcommand, e.g. 'accounts list', 'validate', 'apply', 'posts get', 'workspace health'. Must be in socialclaw.allowed_commands."
                },
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Additional CLI arguments (e.g. ['-f', 'schedule.json']). '--json' is always appended automatically."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions in the last hour".into()),
            });
        }

        if let Err(error) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "socialclaw")
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(error),
            });
        }

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

        let command_parts = match self.validate_command(command) {
            Ok(parts) => parts,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                });
            }
        };

        let extra_args =
            match Self::parse_extra_args(args.get("args").unwrap_or(&serde_json::Value::Null)) {
                Ok(v) => v,
                Err(e) => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    });
                }
            };

        if let Err(e) = self.validate_providers(&extra_args) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
            });
        }

        if !self.security.record_action() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: action budget exhausted".into()),
            });
        }

        let socialclaw_bin = if cfg!(target_os = "windows") {
            "socialclaw.cmd"
        } else {
            "socialclaw"
        };
        let mut cmd = Command::new(socialclaw_bin);
        for part in &command_parts {
            cmd.arg(part);
        }
        for a in &extra_args {
            cmd.arg(a);
        }
        cmd.arg("--json");

        cmd.env_clear();
        for var in SAFE_ENV_VARS {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        if let Some(key) = self.config.api_key.as_deref()
            && !key.is_empty()
        {
            cmd.env("SOCIALCLAW_API_KEY", key);
        }
        if !self.config.base_url.is_empty() {
            cmd.env("SOCIALCLAW_BASE_URL", &self.config.base_url);
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
        let result = tokio::time::timeout(timeout, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if stdout.len() > self.config.max_output_bytes {
                    let mut b = self.config.max_output_bytes.min(stdout.len());
                    while b > 0 && !stdout.is_char_boundary(b) {
                        b -= 1;
                    }
                    stdout.truncate(b);
                    stdout.push_str("\n... [output truncated]");
                }

                Ok(ToolResult {
                    success: output.status.success(),
                    output: stdout,
                    error: if stderr.is_empty() {
                        None
                    } else {
                        Some(stderr)
                    },
                })
            }
            Ok(Err(e)) => {
                let err_msg = e.to_string();
                let msg = if err_msg.contains("No such file or directory")
                    || err_msg.contains("not found")
                    || err_msg.contains("cannot find")
                {
                    "SocialClaw CLI ('socialclaw') not found in PATH. Install with: npm install -g socialclaw".into()
                } else {
                    format!("Failed to execute socialclaw: {e}")
                };
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(msg),
                })
            }
            Err(_) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "SocialClaw CLI timed out after {}s and was killed",
                    self.config.timeout_secs
                )),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;
    use zeroclaw_config::policy::SecurityPolicy;
    use zeroclaw_config::schema::SocialclawConfig;

    fn test_config() -> SocialclawConfig {
        SocialclawConfig {
            enabled: true,
            ..SocialclawConfig::default()
        }
    }

    fn test_config_with_providers(allowed: Vec<&str>) -> SocialclawConfig {
        SocialclawConfig {
            enabled: true,
            allowed_providers: allowed.into_iter().map(String::from).collect(),
            ..SocialclawConfig::default()
        }
    }

    fn test_security(autonomy: AutonomyLevel) -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy {
            autonomy,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        })
    }

    #[test]
    fn tool_name() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Supervised), test_config());
        assert_eq!(tool.name(), "socialclaw");
    }

    #[test]
    fn schema_requires_command() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Supervised), test_config());
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["command"].is_object());
        assert!(
            schema["required"]
                .as_array()
                .expect("required is an array")
                .contains(&json!("command"))
        );
    }

    #[test]
    fn validate_command_accepts_default_allowlist() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Full), test_config());
        assert!(tool.validate_command("accounts list").is_ok());
        assert!(tool.validate_command("validate").is_ok());
        assert!(tool.validate_command("apply").is_ok());
        assert!(tool.validate_command("posts delete").is_ok());
        assert!(tool.validate_command("workspace health").is_ok());
    }

    #[test]
    fn validate_command_rejects_unlisted() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Full), test_config());
        let err = tool.validate_command("login").unwrap_err();
        assert!(err.contains("allowed_commands"));
    }

    #[test]
    fn validate_command_rejects_empty() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Full), test_config());
        assert!(tool.validate_command("   ").is_err());
    }

    #[test]
    fn validate_command_honors_custom_allowlist() {
        let cfg = SocialclawConfig {
            enabled: true,
            allowed_commands: vec!["accounts list".into(), "usage".into()],
            ..SocialclawConfig::default()
        };
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Full), cfg);
        assert!(tool.validate_command("accounts list").is_ok());
        assert!(tool.validate_command("apply").is_err());
    }

    #[test]
    fn validate_providers_wildcard_allows_anything() {
        let tool = SocialclawTool::new(
            test_security(AutonomyLevel::Full),
            test_config_with_providers(vec!["*"]),
        );
        assert!(
            tool.validate_providers(&["--provider".into(), "x".into()])
                .is_ok()
        );
        assert!(
            tool.validate_providers(&["--provider".into(), "linkedin".into()])
                .is_ok()
        );
    }

    #[test]
    fn validate_providers_rejects_unlisted_long_flag() {
        let tool = SocialclawTool::new(
            test_security(AutonomyLevel::Full),
            test_config_with_providers(vec!["x", "linkedin"]),
        );
        let err = tool
            .validate_providers(&["--provider".into(), "tiktok".into()])
            .unwrap_err();
        assert!(err.contains("allowed_providers"), "got: {err}");
    }

    #[test]
    fn validate_providers_rejects_unlisted_short_flag() {
        let tool = SocialclawTool::new(
            test_security(AutonomyLevel::Full),
            test_config_with_providers(vec!["x"]),
        );
        let err = tool
            .validate_providers(&["-p".into(), "reddit".into()])
            .unwrap_err();
        assert!(err.contains("allowed_providers"));
    }

    #[test]
    fn validate_providers_rejects_unlisted_equals_form() {
        let tool = SocialclawTool::new(
            test_security(AutonomyLevel::Full),
            test_config_with_providers(vec!["x"]),
        );
        let err = tool
            .validate_providers(&["--provider=youtube".into()])
            .unwrap_err();
        assert!(err.contains("allowed_providers"));
    }

    #[test]
    fn validate_providers_is_case_insensitive() {
        let tool = SocialclawTool::new(
            test_security(AutonomyLevel::Full),
            test_config_with_providers(vec!["LinkedIn"]),
        );
        assert!(
            tool.validate_providers(&["--provider".into(), "linkedin".into()])
                .is_ok()
        );
    }

    #[test]
    fn validate_providers_missing_value_errors() {
        let tool = SocialclawTool::new(
            test_security(AutonomyLevel::Full),
            test_config_with_providers(vec!["x"]),
        );
        let err = tool.validate_providers(&["--provider".into()]).unwrap_err();
        assert!(err.contains("missing value"));
    }

    #[test]
    fn parse_extra_args_accepts_null_and_empty() {
        assert!(
            SocialclawTool::parse_extra_args(&serde_json::Value::Null)
                .unwrap()
                .is_empty()
        );
        assert!(
            SocialclawTool::parse_extra_args(&json!([]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn parse_extra_args_rejects_non_array() {
        assert!(SocialclawTool::parse_extra_args(&json!("foo")).is_err());
        assert!(SocialclawTool::parse_extra_args(&json!({"a": 1})).is_err());
    }

    #[test]
    fn parse_extra_args_rejects_non_string_items() {
        assert!(SocialclawTool::parse_extra_args(&json!([1, 2])).is_err());
        assert!(SocialclawTool::parse_extra_args(&json!(["ok", null])).is_err());
    }

    #[test]
    fn parse_extra_args_rejects_nul_byte() {
        assert!(SocialclawTool::parse_extra_args(&json!(["bad\0arg"])).is_err());
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::ReadOnly), test_config());
        let result = tool
            .execute(json!({"command": "accounts list"}))
            .await
            .expect("readonly should return a result");
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("read-only mode")
        );
    }

    #[tokio::test]
    async fn execute_blocks_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            max_actions_per_hour: 0,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let tool = SocialclawTool::new(security, test_config());
        let result = tool
            .execute(json!({"command": "accounts list"}))
            .await
            .expect("rate-limited should return a result");
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap_or("").contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_missing_command_errors() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Supervised), test_config());
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("command"));
    }

    #[tokio::test]
    async fn execute_disallowed_command_returns_non_fatal() {
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Supervised), test_config());
        let result = tool
            .execute(json!({"command": "login"}))
            .await
            .expect("should return a result");
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("allowed_commands")
        );
    }

    #[tokio::test]
    async fn execute_disallowed_provider_returns_non_fatal() {
        let cfg = SocialclawConfig {
            enabled: true,
            allowed_providers: vec!["x".into()],
            ..SocialclawConfig::default()
        };
        let tool = SocialclawTool::new(test_security(AutonomyLevel::Supervised), cfg);
        let result = tool
            .execute(json!({
                "command": "accounts connect",
                "args": ["--provider", "tiktok"]
            }))
            .await
            .expect("should return a result");
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("allowed_providers")
        );
    }
}
