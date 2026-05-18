use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_config::policy::ToolOperation;
use zeroclaw_config::schema::OpenCodeCliConfig;

/// Environment variables safe to pass through to the `opencode` subprocess.
///
/// `HOME` / `USERPROFILE` is critical so opencode finds its auth
/// state (`~/.local/share/opencode/auth.json` on Linux,
/// `~/Library/Application Support/opencode/auth.json` on macOS,
/// `%APPDATA%\opencode\auth.json` on Windows). `APPDATA`,
/// `LOCALAPPDATA` and `USERPROFILE` cover the Windows paths.
const SAFE_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "TERM",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "USER",
    "USERNAME",
    "SHELL",
    "TMPDIR",
    "TEMP",
    "TMP",
];

/// Delegates coding tasks to the [sst/opencode](https://opencode.ai)
/// CLI via `opencode run`.
///
/// Two-tier agent architecture: ZeroClaw orchestrates high-level
/// tasks and delegates the actual code-writing to opencode, which
/// has its own agent loop with file editing and shell tools.
///
/// Why this tool exists alongside `claude_code` and `codex_cli`:
/// opencode is provider-agnostic. The caller (or the project's
/// tier policy) picks the model — DeepSeek V4 Pro for cheap code
/// work, Opus 4.7 for irreversible refactors, Kimi K2.6 for
/// long-context creative coding. opencode handles its own
/// provider auth, so ZeroClaw doesn't shuttle API keys.
///
/// Auth setup is a one-time operator action: `opencode auth login`
/// in a terminal. ZeroClaw never sees the credentials.
pub struct OpenCodeCliTool {
    security: Arc<SecurityPolicy>,
    config: OpenCodeCliConfig,
}

impl OpenCodeCliTool {
    pub fn new(security: Arc<SecurityPolicy>, config: OpenCodeCliConfig) -> Self {
        Self { security, config }
    }
}

#[async_trait]
impl Tool for OpenCodeCliTool {
    fn name(&self) -> &str {
        "opencode_cli"
    }

    fn description(&self) -> &str {
        "Delegate a coding task to opencode (sst/opencode) CLI via \
        `opencode run`. Supports per-call model selection \
        (provider/model), session continuation, plan vs build \
        agent modes, and JSON output. Use for complex coding work \
        that benefits from opencode's full agent loop with \
        provider-agnostic model routing."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The coding task to delegate to opencode."
                },
                "model": {
                    "type": "string",
                    "description": "Provider/model to use for this call (e.g. \
                        'deepseek/deepseek-v4-pro' for code, \
                        'anthropic/claude-opus-4.7' for irreversible \
                        refactors, 'moonshotai/kimi-k2.6' for long-context \
                        creative coding). Overrides the configured \
                        default_model. Format must match a model the \
                        local opencode auth setup actually has access to."
                },
                "agent": {
                    "type": "string",
                    "enum": ["build", "plan"],
                    "description": "Agent mode. 'build' writes code (default); \
                        'plan' is read-only — produces a plan without \
                        editing files. Useful for cheap models doing \
                        scoping work before an expensive model implements."
                },
                "session_id": {
                    "type": "string",
                    "description": "Resume a previous opencode session by ID. \
                        Mutually exclusive with 'continue_last'."
                },
                "continue_last": {
                    "type": "boolean",
                    "description": "Continue the most recent opencode session \
                        instead of starting fresh. Mutually exclusive with \
                        'session_id'."
                },
                "working_directory": {
                    "type": "string",
                    "description": "Working directory within the workspace \
                        (must be inside workspace_dir). Defaults to the \
                        workspace root."
                },
                "json_output": {
                    "type": "boolean",
                    "description": "Request JSON-formatted output via \
                        opencode's structured output mode. Easier for \
                        downstream parsing; defaults to plain text."
                }
            },
            "required": ["prompt"]
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
            .enforce_tool_operation(ToolOperation::Act, "opencode_cli")
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(error),
            });
        }

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'prompt' parameter"))?;

        // Resolve model: per-call argument wins, then configured default,
        // then None (let opencode pick).
        let model: Option<String> = args
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.config.default_model.clone());

        // Resolve agent mode: per-call argument wins, then configured default.
        let agent: Option<String> = args
            .get("agent")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.config.default_agent.clone());

        if let Some(ref a) = agent
            && a != "build"
            && a != "plan"
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "agent='{}' is not valid — opencode accepts 'build' or 'plan'",
                    a
                )),
            });
        }

        let session_id = args.get("session_id").and_then(|v| v.as_str());
        let continue_last = args
            .get("continue_last")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if session_id.is_some() && continue_last {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "session_id and continue_last are mutually exclusive — pick one".into(),
                ),
            });
        }

        let json_output = args
            .get("json_output")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        // Working-directory containment. Identical pattern to claude_code:
        // require both ends to canonicalise so symlinks can't bypass the
        // workspace check.
        let work_dir = if let Some(wd) = args.get("working_directory").and_then(|v| v.as_str()) {
            let wd_path = std::path::PathBuf::from(wd);
            let workspace = &self.security.workspace_dir;
            let canonical_wd = match wd_path.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "working_directory '{}' does not exist or is not accessible",
                            wd
                        )),
                    });
                }
            };
            let canonical_ws = match workspace.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "workspace directory '{}' does not exist or is not accessible",
                            workspace.display()
                        )),
                    });
                }
            };
            if !canonical_wd.starts_with(&canonical_ws) {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "working_directory '{}' is outside the workspace '{}'",
                        wd,
                        workspace.display()
                    )),
                });
            }
            canonical_wd
        } else {
            self.security.workspace_dir.clone()
        };

        if !self.security.record_action() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: action budget exhausted".into()),
            });
        }

        // Binary detection. Windows installs of opencode (via npm or
        // the official installer) drop a `.cmd` shim on PATH; the bare
        // name doesn't resolve in `CreateProcess` on Windows the way
        // `bash` resolves it via PATHEXT.
        let opencode_bin = if cfg!(target_os = "windows") {
            "opencode.cmd"
        } else {
            "opencode"
        };

        let mut cmd = Command::new(opencode_bin);
        cmd.arg("run").arg(prompt);

        if let Some(ref m) = model {
            cmd.arg("--model").arg(m);
        }
        if let Some(ref a) = agent {
            cmd.arg("--agent").arg(a);
        }
        if let Some(sid) = session_id {
            cmd.arg("--session").arg(sid);
        } else if continue_last {
            cmd.arg("--continue");
        }
        if json_output {
            cmd.arg("--format").arg("json");
        }

        // Strict env scrubbing. opencode reads its own auth file from
        // the user's home; we only pass through what it actually needs
        // plus operator-configured extras.
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

        cmd.current_dir(&work_dir);
        let timeout = Duration::from_secs(self.config.timeout_secs);
        cmd.kill_on_drop(true);

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
                    || err_msg.contains("The system cannot find the file specified")
                {
                    "opencode CLI not found in PATH. Install via \
                    `curl -fsSL https://opencode.ai/install | bash` \
                    or `npm install -g opencode-ai`, then run \
                    `opencode auth login` to add a provider."
                        .into()
                } else {
                    format!("Failed to execute opencode: {e}")
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
                    "opencode timed out after {}s and was killed",
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
    use zeroclaw_config::schema::OpenCodeCliConfig;

    fn test_config() -> OpenCodeCliConfig {
        OpenCodeCliConfig::default()
    }

    fn test_security(autonomy: AutonomyLevel) -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy {
            autonomy,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        })
    }

    #[test]
    fn opencode_cli_tool_name() {
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Supervised), test_config());
        assert_eq!(tool.name(), "opencode_cli");
    }

    #[test]
    fn opencode_cli_schema_advertises_model_and_agent() {
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Supervised), test_config());
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["prompt"].is_object());
        assert!(schema["properties"]["model"].is_object());
        assert!(schema["properties"]["agent"].is_object());
        assert!(schema["properties"]["session_id"].is_object());
        assert!(schema["properties"]["continue_last"].is_object());
        assert!(schema["properties"]["working_directory"].is_object());
        assert!(schema["properties"]["json_output"].is_object());
        assert!(
            schema["required"]
                .as_array()
                .expect("required must be array")
                .contains(&json!("prompt"))
        );
        // agent must restrict to build/plan — opencode rejects anything else.
        let agent_enum = schema["properties"]["agent"]["enum"]
            .as_array()
            .expect("agent.enum must be present");
        assert!(agent_enum.contains(&json!("build")));
        assert!(agent_enum.contains(&json!("plan")));
        assert_eq!(agent_enum.len(), 2);
    }

    #[test]
    fn opencode_cli_description_mentions_tier_integration() {
        // The whole reason this tool exists alongside claude_code /
        // codex_cli is that it accepts a provider/model per call.
        // The description must surface that so the orchestrator
        // picks it for cheap-tier code work.
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Supervised), test_config());
        let desc = tool.description();
        assert!(desc.contains("model"));
        assert!(desc.contains("provider"));
    }

    #[tokio::test]
    async fn opencode_cli_blocks_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            max_actions_per_hour: 0,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let tool = OpenCodeCliTool::new(security, test_config());
        let result = tool
            .execute(json!({"prompt": "hello"}))
            .await
            .expect("rate-limited should return a result");
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap_or("").contains("Rate limit"));
    }

    #[tokio::test]
    async fn opencode_cli_blocks_readonly() {
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::ReadOnly), test_config());
        let result = tool
            .execute(json!({"prompt": "hello"}))
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
    async fn opencode_cli_missing_prompt_param() {
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Supervised), test_config());
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("prompt"));
    }

    #[tokio::test]
    async fn opencode_cli_rejects_invalid_agent_mode() {
        // A typo like 'planning' must fail at the ZeroClaw boundary
        // rather than waste a subprocess invocation just to discover
        // opencode rejects it.
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Full), test_config());
        let result = tool
            .execute(json!({
                "prompt": "hello",
                "agent": "planning"
            }))
            .await
            .expect("invalid agent should return a result, not error");
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("'build' or 'plan'")
        );
    }

    #[tokio::test]
    async fn opencode_cli_rejects_session_and_continue_together() {
        // The opencode CLI itself would error on this; we catch it
        // up front so the rate-limit budget isn't burned.
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Full), test_config());
        let result = tool
            .execute(json!({
                "prompt": "hello",
                "session_id": "abc-123",
                "continue_last": true
            }))
            .await
            .expect("conflicting session flags should return a result");
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("mutually exclusive")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn opencode_cli_rejects_path_outside_workspace() {
        let tool = OpenCodeCliTool::new(test_security(AutonomyLevel::Full), test_config());
        let result = tool
            .execute(json!({
                "prompt": "hello",
                "working_directory": "/etc"
            }))
            .await
            .expect("should return a result for path validation");
        assert!(!result.success);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("outside the workspace")
        );
    }

    #[test]
    fn opencode_cli_env_passthrough_defaults() {
        let config = OpenCodeCliConfig::default();
        assert!(
            config.env_passthrough.is_empty(),
            "env_passthrough should default to empty"
        );
    }

    #[test]
    fn opencode_cli_default_config_values() {
        let config = OpenCodeCliConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.timeout_secs, 600);
        assert_eq!(config.max_output_bytes, 2_097_152);
        assert!(config.default_model.is_none());
        assert!(config.default_agent.is_none());
    }

    #[test]
    fn opencode_cli_config_accepts_tier_aligned_default_model() {
        // Sanity: a tier-aligned default like deepseek-v4-pro for S4
        // code work survives a round trip through the struct.
        let config = OpenCodeCliConfig {
            default_model: Some("deepseek/deepseek-v4-pro".into()),
            default_agent: Some("build".into()),
            ..OpenCodeCliConfig::default()
        };
        assert_eq!(
            config.default_model.as_deref(),
            Some("deepseek/deepseek-v4-pro")
        );
        assert_eq!(config.default_agent.as_deref(), Some("build"));
    }
}
