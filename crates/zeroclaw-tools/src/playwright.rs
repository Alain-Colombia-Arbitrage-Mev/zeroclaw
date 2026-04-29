//! Playwright browser-automation tool.
//!
//! The agent submits a JSON "playbook" — a URL plus a sequence of
//! steps (click, fill, wait, extract_text, screenshot, evaluate, …).
//! We spawn a Node subprocess running the bundled
//! `scripts/playwright_runner.js`, pipe the playbook to its stdin, and
//! parse its single JSON result line from stdout.
//!
//! Why subprocess instead of a Rust browser crate?
//!   - Playwright's Node API is the most complete; no Rust binding
//!     covers parity (chromiumoxide and others stop at CDP).
//!   - It keeps the Rust binary small — Playwright's browser binaries
//!     are managed by the host's Node install.
//!
//! Host requirements
//!   - Node ≥ 18 on `PATH`.
//!   - `playwright` resolvable to the runner. The simplest setup is
//!     `npm i -g playwright @playwright/test && npx playwright install
//!     chromium` (one-time).
//!
//! Security
//!   - Runs through the same SecurityPolicy gates as the Gemini / Codex
//!     subprocess tools (rate limit, ToolOperation::Act).
//!   - Only `PATH`, `HOME`, `TERM`, etc. are passed through — no
//!     secrets leak into the Node process.
//!   - The runner script lives under the workspace's tools crate and
//!     is dropped into a tempfile on each invocation; we never trust a
//!     filesystem path supplied by the model.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};

/// Default subprocess wall-clock timeout in seconds. Long enough for a
/// browser launch (~3 s on a cold cache) plus a few user-driven steps.
const DEFAULT_RUN_TIMEOUT_SECS: u64 = 90;

/// Maximum stdout we accept from the Node runner. Mostly defensive:
/// `extract_html` of a heavy SPA can be a few hundred KB even with one
/// page; we cap at 4 MB and surface a friendly error if exceeded.
const MAX_STDOUT_BYTES: usize = 4 * 1024 * 1024;

/// Environment variables we permit the Node subprocess to inherit. No
/// API keys / tokens — Playwright doesn't need any.
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
    // Windows-specific essentials.
    "PATHEXT",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "USERNAME",
    // Lets users pin a specific Playwright install when a non-default
    // npm global location is in use.
    "PLAYWRIGHT_BROWSERS_PATH",
    "NODE_PATH",
];

const RUNNER_SCRIPT: &str = include_str!("../scripts/playwright_runner.js");

/// Browser-automation tool exposed to the agent as `playwright`.
pub struct PlaywrightTool {
    security: Arc<SecurityPolicy>,
    timeout_secs: u64,
}

impl PlaywrightTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self {
            security,
            timeout_secs: DEFAULT_RUN_TIMEOUT_SECS,
        }
    }

    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

#[async_trait]
impl Tool for PlaywrightTool {
    fn name(&self) -> &str {
        "playwright"
    }

    fn description(&self) -> &str {
        "Drive a real browser via Playwright. Supply a URL and a sequence \
         of steps (click, fill, wait, extract_text, extract_html, \
         screenshot, evaluate, press_key, navigate). Returns per-step \
         results plus the final URL. Requires Node + Playwright on the \
         host."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Initial page URL. Optional if the first step is `navigate`."
                },
                "headless": {
                    "type": "boolean",
                    "description": "Run with headless Chromium (default true).",
                    "default": true
                },
                "timeoutMs": {
                    "type": "integer",
                    "description": "Per-page default timeout in milliseconds. Default 30000.",
                    "default": 30000
                },
                "viewport": {
                    "type": "object",
                    "properties": {
                        "width": { "type": "integer" },
                        "height": { "type": "integer" }
                    }
                },
                "userAgent": { "type": "string" },
                "steps": {
                    "type": "array",
                    "description": "Ordered list of automation steps.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": [
                                    "navigate", "click", "fill", "press_key",
                                    "wait", "extract_text", "extract_html",
                                    "screenshot", "evaluate"
                                ]
                            },
                            "selector": { "type": "string" },
                            "url": { "type": "string" },
                            "value": { "type": "string" },
                            "key": { "type": "string" },
                            "expression": { "type": "string" },
                            "timeout": { "type": "integer" },
                            "ms": { "type": "integer" },
                            "fullPage": { "type": "boolean" },
                            "state": {
                                "type": "string",
                                "enum": ["attached", "detached", "visible", "hidden"]
                            },
                            "waitUntil": {
                                "type": "string",
                                "enum": ["load", "domcontentloaded", "networkidle", "commit"]
                            }
                        },
                        "required": ["action"]
                    }
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(deny(
                "Rate limit exceeded: too many actions in the last hour",
            ));
        }
        if let Err(reason) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "playwright")
        {
            return Ok(deny(&reason));
        }

        // Drop the runner script into a temp file. Doing it per-call
        // keeps the implementation stateless; the file is removed on
        // tool exit by the OS even if we crash.
        let tmp = match tempfile_with_script() {
            Ok(t) => t,
            Err(e) => return Ok(deny(&format!("Failed to materialise runner script: {e}"))),
        };

        let payload = match serde_json::to_string(&args) {
            Ok(p) => p,
            Err(e) => return Ok(deny(&format!("Failed to serialise playbook: {e}"))),
        };

        let mut cmd = Command::new("node");
        cmd.arg(tmp.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        for var in SAFE_ENV_VARS {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Ok(deny(&format!(
                    "Failed to spawn `node` — is it on PATH? ({e})"
                )));
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(payload.as_bytes()).await {
                return Ok(deny(&format!("Failed to write playbook to stdin: {e}")));
            }
            // Drop closes the pipe so the runner sees EOF on stdin.
            drop(stdin);
        }

        let timeout = Duration::from_secs(self.timeout_secs);
        let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

        let output = match result {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Ok(deny(&format!("Subprocess error: {e}"))),
            Err(_) => {
                return Ok(deny(&format!(
                    "Playwright run timed out after {} seconds",
                    self.timeout_secs
                )));
            }
        };

        let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if stdout.len() > MAX_STDOUT_BYTES {
            stdout.truncate(MAX_STDOUT_BYTES);
            stdout.push_str("\n... [output truncated at 4 MB]");
        }

        if !output.status.success() && stdout.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Playwright runner exited with status {:?}: {}",
                    output.status.code(),
                    if stderr.is_empty() {
                        "(no stderr)"
                    } else {
                        stderr.as_str()
                    }
                )),
            });
        }

        Ok(ToolResult {
            success: output.status.success(),
            output: stdout,
            error: if output.status.success() || stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        })
    }
}

fn deny(reason: &str) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(reason.to_string()),
    }
}

fn tempfile_with_script() -> std::io::Result<tempfile::NamedTempFile> {
    let mut tmp = tempfile::Builder::new()
        .prefix("zeroclaw-playwright-")
        .suffix(".js")
        .tempfile()?;
    tmp.write_all(RUNNER_SCRIPT.as_bytes())?;
    tmp.flush()?;
    Ok(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::policy::AutonomyLevel;

    fn full_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        })
    }

    #[test]
    fn tool_metadata() {
        let tool = PlaywrightTool::new(full_security());
        assert_eq!(tool.name(), "playwright");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn schema_lists_supported_actions() {
        let tool = PlaywrightTool::new(full_security());
        let schema = tool.parameters_schema();
        let actions = &schema["properties"]["steps"]["items"]["properties"]["action"]["enum"];
        let actions = actions.as_array().expect("action enum should be array");
        let names: Vec<&str> = actions.iter().filter_map(|a| a.as_str()).collect();
        for expected in [
            "navigate",
            "click",
            "fill",
            "press_key",
            "wait",
            "extract_text",
            "extract_html",
            "screenshot",
            "evaluate",
        ] {
            assert!(
                names.contains(&expected),
                "action `{expected}` should be in the supported list",
            );
        }
    }

    #[test]
    fn runner_script_is_bundled() {
        // Sanity check that include_str! actually wired the JS file in.
        assert!(RUNNER_SCRIPT.contains("require('playwright')"));
        assert!(RUNNER_SCRIPT.contains("readStdin"));
    }

    #[tokio::test]
    async fn execute_blocks_when_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            max_actions_per_hour: 0,
            ..SecurityPolicy::default()
        });
        let tool = PlaywrightTool::new(security);
        let res = tool
            .execute(json!({ "url": "https://example.com" }))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap_or_default().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_blocks_in_readonly_mode() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let tool = PlaywrightTool::new(security);
        let res = tool
            .execute(json!({ "url": "https://example.com" }))
            .await
            .unwrap();
        assert!(!res.success);
        // The exact message comes from SecurityPolicy::enforce_tool_operation.
        assert!(res.error.is_some());
    }
}
