//! Security-scan tool — runs a curated allowlist of static-analysis,
//! dependency-audit, secret-scanning, and network-recon binaries and
//! returns their output. Distinct from the raw `shell` tool: this
//! exposes ONLY a fixed set of well-known scanners, so an agent can be
//! granted scanning capability without being granted arbitrary command
//! execution. The `ethical_hacker` preset gets both this and `shell`;
//! a defensive `security` agent can be given only this.
//!
//! Every scanner here is read-only with respect to the target (it
//! inspects code, dependencies, or responds to network probes) — none
//! mutate the workspace. Aggressive / exploitation flags are not
//! exposed; this is for AUTHORISED assessment, and the agent prompt
//! carries the rules-of-engagement discipline.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::autonomy::AutonomyLevel;
use zeroclaw_config::policy::SecurityPolicy;

/// One entry in the scanner allowlist: the tool name the LLM selects,
/// the binary invoked, and the fixed leading arguments. Any
/// user-supplied `target` is appended as a single argument after these
/// — it is never split on spaces or interpolated into a shell, so
/// shell-injection is not reachable through this tool.
struct Scanner {
    /// Name the LLM passes as `scanner`.
    key: &'static str,
    /// Binary to exec (must be on PATH / allowlisted by the operator).
    binary: &'static str,
    /// Fixed args prepended before the target.
    fixed_args: &'static [&'static str],
    /// Whether this scanner reaches out over the network (recon). These
    /// require `Full` autonomy because they touch external hosts.
    network: bool,
    /// One-line description surfaced in the tool schema.
    blurb: &'static str,
}

/// The curated set. Read-only / assessment-grade invocations only.
const SCANNERS: &[Scanner] = &[
    Scanner {
        key: "semgrep",
        binary: "semgrep",
        fixed_args: &["--error", "--quiet", "--config", "auto"],
        network: false,
        blurb: "Static analysis (SAST) over source for vuln patterns",
    },
    Scanner {
        key: "gitleaks",
        binary: "gitleaks",
        fixed_args: &["detect", "--no-banner", "--redact", "--source"],
        network: false,
        blurb: "Secret / credential scanning of a repo or directory",
    },
    Scanner {
        key: "trivy",
        binary: "trivy",
        fixed_args: &["fs", "--quiet", "--scanners", "vuln,secret,misconfig"],
        network: false,
        blurb: "Filesystem/dependency vuln + misconfig + secret scan",
    },
    Scanner {
        key: "cargo_audit",
        binary: "cargo",
        fixed_args: &["audit"],
        network: true,
        blurb: "Rust dependency advisory audit (RustSec) — fetches advisory DB",
    },
    Scanner {
        key: "npm_audit",
        binary: "npm",
        fixed_args: &["audit", "--audit-level=low"],
        network: true,
        blurb: "Node dependency advisory audit — queries the npm registry",
    },
    Scanner {
        key: "pip_audit",
        binary: "pip-audit",
        fixed_args: &["--progress-spinner=off"],
        network: true,
        blurb: "Python dependency advisory audit (PyPI / OSV)",
    },
    Scanner {
        key: "bandit",
        binary: "bandit",
        fixed_args: &["-r", "-q"],
        network: false,
        blurb: "Python source SAST for common security issues",
    },
    Scanner {
        key: "nmap",
        binary: "nmap",
        fixed_args: &["-sV", "-Pn", "-T3"],
        network: true,
        blurb: "Service/version network scan of an AUTHORISED host (recon)",
    },
    Scanner {
        key: "nikto",
        binary: "nikto",
        fixed_args: &["-ask", "no", "-host"],
        network: true,
        blurb: "Web-server misconfiguration scan of an AUTHORISED host",
    },
];

/// Runs vetted security scanners against a target path or host.
pub struct SecurityScanTool {
    security: Arc<SecurityPolicy>,
    workspace_dir: std::path::PathBuf,
}

impl SecurityScanTool {
    pub fn new(security: Arc<SecurityPolicy>, workspace_dir: std::path::PathBuf) -> Self {
        Self {
            security,
            workspace_dir,
        }
    }

    fn lookup(key: &str) -> Option<&'static Scanner> {
        SCANNERS.iter().find(|s| s.key == key)
    }

    /// Reject obviously-malformed targets early. We do NOT pass the
    /// target through a shell, so the real injection surface is nil;
    /// this guard mainly stops accidental flag-injection (a target
    /// starting with `-` would be read as an option by the scanner).
    fn validate_target(target: &str) -> anyhow::Result<()> {
        if target.trim().is_empty() {
            anyhow::bail!("target must not be empty");
        }
        if target.starts_with('-') {
            anyhow::bail!("target must not start with '-' (would be read as a flag)");
        }
        if target.contains('\0') {
            anyhow::bail!("target contains a null byte");
        }
        Ok(())
    }

    async fn run(&self, scanner: &Scanner, target: &str) -> ToolResult {
        let mut cmd = tokio::process::Command::new(scanner.binary);
        cmd.args(scanner.fixed_args)
            .arg(target)
            .current_dir(&self.workspace_dir);

        match cmd.output().await {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                // Scanners conventionally exit non-zero when they FIND
                // something — that is a successful scan, not a tool
                // failure. Surface the code and both streams; let the
                // agent interpret findings.
                let code = out.status.code().unwrap_or(-1);
                let combined = format!(
                    "scanner={} binary={} exit={}\n--- stdout ---\n{}\n--- stderr ---\n{}",
                    scanner.key,
                    scanner.binary,
                    code,
                    stdout.trim_end(),
                    stderr.trim_end(),
                );
                ToolResult {
                    success: true,
                    output: combined,
                    error: None,
                }
            }
            Err(e) => {
                let hint = if e.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "scanner binary '{}' not found on PATH — install it or pick another scanner",
                        scanner.binary
                    )
                } else {
                    format!("failed to launch '{}': {e}", scanner.binary)
                };
                ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(hint),
                }
            }
        }
    }
}

#[async_trait]
impl Tool for SecurityScanTool {
    fn name(&self) -> &str {
        "security_scan"
    }

    fn description(&self) -> &str {
        "Run a vetted security scanner (SAST, dependency/secret audit, or AUTHORISED network recon) against a target path or host. Exposes a fixed allowlist of scanners (semgrep, gitleaks, trivy, cargo_audit, npm_audit, pip_audit, bandit, nmap, nikto) without granting arbitrary shell. For authorised assessment only — confirm scope before scanning a host you do not own."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        let scanners: Vec<serde_json::Value> = SCANNERS
            .iter()
            .map(|s| json!({ "key": s.key, "binary": s.binary, "network": s.network, "use": s.blurb }))
            .collect();
        let keys: Vec<&str> = SCANNERS.iter().map(|s| s.key).collect();
        json!({
            "type": "object",
            "properties": {
                "scanner": {
                    "type": "string",
                    "enum": keys,
                    "description": "Which vetted scanner to run."
                },
                "target": {
                    "type": "string",
                    "description": "Path to scan (relative to workspace) for source/dependency scanners, or an AUTHORISED host/URL for network scanners. Required."
                }
            },
            "required": ["scanner", "target"],
            "x-scanners": scanners
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let scanner_key = match args.get("scanner").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing 'scanner' parameter".into()),
                });
            }
        };
        let target = match args.get("target").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing 'target' parameter".into()),
                });
            }
        };

        let scanner = match Self::lookup(scanner_key) {
            Some(s) => s,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Unknown scanner '{scanner_key}' — not in the allowlist")),
                });
            }
        };

        if let Err(e) = Self::validate_target(target) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Invalid target: {e}")),
            });
        }

        // Running a scanner is an action — gate it like other active
        // tools. Read-only autonomy may not act at all; network recon
        // additionally requires Full autonomy because it touches
        // external hosts.
        if !self.security.can_act() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Action blocked: security_scan requires higher autonomy level".into()),
            });
        }
        match self.security.autonomy {
            AutonomyLevel::ReadOnly => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Action blocked: read-only mode".into()),
                });
            }
            AutonomyLevel::Supervised => {
                if scanner.network {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "Action blocked: network scanner '{}' requires Full autonomy (it reaches external hosts)",
                            scanner.key
                        )),
                    });
                }
            }
            AutonomyLevel::Full => {}
        }

        if !self.security.record_action() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Action blocked: rate limit exceeded".into()),
            });
        }

        Ok(self.run(scanner, target).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> SecurityScanTool {
        SecurityScanTool::new(Arc::new(SecurityPolicy::default()), std::env::temp_dir())
    }

    #[test]
    fn name_and_schema_are_well_formed() {
        let t = tool();
        assert_eq!(t.name(), "security_scan");
        let schema = t.parameters_schema();
        let enum_vals = schema["properties"]["scanner"]["enum"].as_array().unwrap();
        assert!(enum_vals.iter().any(|v| v == "semgrep"));
        assert!(enum_vals.iter().any(|v| v == "nmap"));
        assert!(!t.description().is_empty());
    }

    #[test]
    fn every_scanner_key_is_unique() {
        let mut keys: Vec<&str> = SCANNERS.iter().map(|s| s.key).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total, "duplicate scanner key in allowlist");
    }

    #[test]
    fn validate_target_rejects_flag_and_empty() {
        assert!(SecurityScanTool::validate_target("-oN/etc/passwd").is_err());
        assert!(SecurityScanTool::validate_target("   ").is_err());
        assert!(SecurityScanTool::validate_target("src/").is_ok());
        assert!(SecurityScanTool::validate_target("scanme.example.com").is_ok());
    }

    #[tokio::test]
    async fn unknown_scanner_is_rejected() {
        let t = tool();
        let res = t
            .execute(json!({"scanner": "metasploit", "target": "src/"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("not in the allowlist"));
    }

    #[tokio::test]
    async fn missing_target_is_rejected() {
        let t = tool();
        let res = t.execute(json!({"scanner": "semgrep"})).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("target"));
    }
}
