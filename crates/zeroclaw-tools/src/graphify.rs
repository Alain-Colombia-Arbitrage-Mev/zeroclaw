//! Wrapper for the [Graphify](https://graphify.net) CLI.
//!
//! Graphify is a Python tool (`pip install graphifyy`) that turns a
//! folder of code, docs, papers, images, or videos into a queryable
//! knowledge graph (NetworkX + Tree-sitter; subagents extract semantic
//! relationships). It writes `graphify-out/{graph.html, graph.json,
//! GRAPH_REPORT.md, cache/}` next to where it was run.
//!
//! This tool surfaces Graphify to the agent so a model can build,
//! query, and explain a project graph without the operator having to
//! drop to a terminal. Like the other CLI delegators
//! (`gemini_cli`, `codex_cli`, `opencode_cli`), we run the binary as
//! a sandboxed subprocess with a cleared environment.
//!
//! Host requirements
//!   - Python 3.10+
//!   - `graphify` on PATH (one of: `uv tool install graphifyy`,
//!     `pipx install graphifyy`, or `pip install graphifyy`)
//!   - Run `graphify install --platform claw` once to register the
//!     skill — ZeroClaw is wire-compatible with the OpenClaw integration.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};

/// Default subprocess wall-clock timeout. Graph builds on a fresh repo
/// can take several minutes when the LLM extraction step runs hot;
/// 600 s is the floor we use for non-init actions, which are fast.
const DEFAULT_TIMEOUT_SECS: u64 = 600;
/// Maximum stdout we accept from Graphify. Build reports are ~10–50 KB
/// and graph JSON dumps can spike to several MB on large repos.
const MAX_STDOUT_BYTES: usize = 4 * 1024 * 1024;

/// Environment variables forwarded to the subprocess. No secrets — the
/// LLM API key is owned by the upstream CLI Graphify is bound to
/// (Claude Code, Codex, Aider, etc.) and reaches Graphify through that
/// CLI's own environment, not ours.
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
    "PYTHONPATH",
    "PYTHONHOME",
    "VIRTUAL_ENV",
];

/// Graphify wrapper exposed to the agent as `graphify`.
pub struct GraphifyTool {
    security: Arc<SecurityPolicy>,
    timeout_secs: u64,
}

impl GraphifyTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self {
            security,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

#[async_trait]
impl Tool for GraphifyTool {
    fn name(&self) -> &str {
        "graphify"
    }

    fn description(&self) -> &str {
        "Run the Graphify CLI to build, query, or explain a knowledge \
         graph of a code/docs folder. Actions: init (scan a folder and \
         emit graphify-out/), query (semantic question over the graph), \
         path (find shortest path between two nodes), explain (rationale \
         behind a node). Requires `graphify` on PATH — install via \
         `pip install graphifyy`."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["init", "query", "path", "explain"],
                    "description": "init = scan folder; query = ask the graph; path = node-to-node; explain = rationale for a node"
                },
                "directory": {
                    "type": "string",
                    "description": "Directory inside the workspace to scan (init) or whose graphify-out to read (query/path/explain). Defaults to '.'.",
                    "default": "."
                },
                "question": {
                    "type": "string",
                    "description": "Natural-language question (action=query) or rationale focus (action=explain)."
                },
                "from_node": {
                    "type": "string",
                    "description": "Source node identifier or label (action=path)."
                },
                "to_node": {
                    "type": "string",
                    "description": "Target node identifier or label (action=path)."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(deny(
                "Rate limit exceeded: too many actions in the last hour",
            ));
        }

        let action = match args.get("action").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(deny("Missing 'action' parameter")),
        };

        // `init` writes to disk (creates `graphify-out/`); the others
        // are read-only. Gate accordingly.
        let op = match action {
            "init" => ToolOperation::Act,
            "query" | "path" | "explain" => ToolOperation::Read,
            other => return Ok(deny(&format!("Unsupported action '{other}'"))),
        };
        if let Err(reason) = self.security.enforce_tool_operation(op, "graphify") {
            return Ok(deny(&reason));
        }

        // Resolve the working directory inside the workspace. We follow
        // the same canonicalisation pattern the other CLI tools use:
        // require the path to exist and to live under workspace_dir,
        // never trust the raw arg as-is.
        let directory = args
            .get("directory")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let workspace = &self.security.workspace_dir;
        let dir_path = if directory == "." {
            workspace.clone()
        } else {
            workspace.join(directory)
        };
        let canonical_dir = match dir_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return Ok(deny(&format!(
                    "directory '{directory}' does not exist or is not accessible"
                )));
            }
        };
        let canonical_ws = match workspace.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return Ok(deny(&format!(
                    "workspace directory '{}' does not exist or is not accessible",
                    workspace.display()
                )));
            }
        };
        if !canonical_dir.starts_with(&canonical_ws) {
            return Ok(deny(&format!(
                "directory '{directory}' is outside the workspace '{}'",
                workspace.display()
            )));
        }

        if !self.security.record_action() {
            return Ok(deny("Rate limit exceeded: action budget exhausted"));
        }

        let mut cmd = Command::new(graphify_bin());

        match action {
            "init" => {
                // `graphify .` is the canonical "scan a folder" command.
                cmd.arg(".");
            }
            "query" => {
                let q = match args.get("question").and_then(|v| v.as_str()) {
                    Some(s) if !s.trim().is_empty() => s,
                    _ => return Ok(deny("Missing 'question' parameter for action=query")),
                };
                cmd.arg("query").arg(q);
            }
            "path" => {
                let from = args.get("from_node").and_then(|v| v.as_str()).unwrap_or("");
                let to = args.get("to_node").and_then(|v| v.as_str()).unwrap_or("");
                if from.is_empty() || to.is_empty() {
                    return Ok(deny(
                        "Missing 'from_node' / 'to_node' parameters for action=path",
                    ));
                }
                cmd.arg("path").arg(from).arg(to);
            }
            "explain" => {
                let q = match args.get("question").and_then(|v| v.as_str()) {
                    Some(s) if !s.trim().is_empty() => s,
                    _ => return Ok(deny("Missing 'question' parameter for action=explain")),
                };
                cmd.arg("explain").arg(q);
            }
            _ => unreachable!("checked above"),
        }

        cmd.current_dir(&canonical_dir)
            .stdin(std::process::Stdio::null())
            .env_clear();
        for var in SAFE_ENV_VARS {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        cmd.kill_on_drop(true);

        let timeout = Duration::from_secs(self.timeout_secs);
        let result = tokio::time::timeout(timeout, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if stdout.len() > MAX_STDOUT_BYTES {
                    let mut b = MAX_STDOUT_BYTES.min(stdout.len());
                    while b > 0 && !stdout.is_char_boundary(b) {
                        b -= 1;
                    }
                    stdout.truncate(b);
                    stdout.push_str("\n... [output truncated at 4 MB]");
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
                let msg = format!(
                    "Failed to spawn `graphify` — is it on PATH? Try \
                     `pip install graphifyy && graphify install --platform claw`. \
                     Underlying error: {e}"
                );
                Ok(deny(&msg))
            }
            Err(_) => Ok(deny(&format!(
                "Graphify run timed out after {} seconds",
                self.timeout_secs
            ))),
        }
    }
}

fn graphify_bin() -> &'static str {
    if cfg!(target_os = "windows") {
        "graphify.exe"
    } else {
        "graphify"
    }
}

fn deny(reason: &str) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(reason.to_string()),
    }
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
    fn name_and_description() {
        let t = GraphifyTool::new(full_security());
        assert_eq!(t.name(), "graphify");
        assert!(!t.description().is_empty());
    }

    #[test]
    fn schema_lists_supported_actions() {
        let t = GraphifyTool::new(full_security());
        let schema = t.parameters_schema();
        let actions = schema["properties"]["action"]["enum"]
            .as_array()
            .expect("action enum is array");
        let names: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
        for expected in ["init", "query", "path", "explain"] {
            assert!(names.contains(&expected), "missing action: {expected}");
        }
    }

    #[test]
    fn binary_name_is_platform_specific() {
        let bin = graphify_bin();
        if cfg!(target_os = "windows") {
            assert_eq!(bin, "graphify.exe");
        } else {
            assert_eq!(bin, "graphify");
        }
    }

    #[tokio::test]
    async fn execute_blocks_when_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            max_actions_per_hour: 0,
            ..SecurityPolicy::default()
        });
        let tool = GraphifyTool::new(security);
        let res = tool.execute(json!({ "action": "init" })).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap_or_default().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_rejects_unknown_action() {
        let tool = GraphifyTool::new(full_security());
        let res = tool.execute(json!({ "action": "evolve" })).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap_or_default().contains("Unsupported action"));
    }

    #[tokio::test]
    async fn query_requires_question() {
        let tool = GraphifyTool::new(full_security());
        let res = tool.execute(json!({ "action": "query" })).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap_or_default().contains("question"));
    }

    #[tokio::test]
    async fn path_requires_endpoints() {
        let tool = GraphifyTool::new(full_security());
        let res = tool
            .execute(json!({ "action": "path", "from_node": "A" }))
            .await
            .unwrap();
        assert!(!res.success);
        let msg = res.error.unwrap_or_default();
        assert!(msg.contains("from_node") || msg.contains("to_node"));
    }
}
