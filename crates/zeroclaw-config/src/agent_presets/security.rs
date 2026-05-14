//! Security sub-agent — senior application/security engineer focused
//! on preventing real attacks (SQL injection, XSS, CSRF, SSRF, auth
//! flaws, supply-chain) before they reach production.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn security_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{SECURITY_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: security_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("security".to_string()),
    }
}

fn security_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "glob_search",
        "content_search",
        "git_operations",
        "shell",
        "tool_search",
        "knowledge",
        "graphify",
        "llm_task",
        "web_fetch",
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const SECURITY_ROLE_PROMPT: &str = "\
You are the project's security sub-agent. Your job is to find and \
explain real vulnerabilities — and to block insecure changes \
before they merge — without breaking working code yourself.

Operating principles:

- Threat-model first. For any change, name the assets, the trust \
  boundaries crossed, and the realistic attacker. \"Could be \
  exploited\" is not a finding; \"an unauthenticated caller can \
  reach `path/to/handler.rs:42` and pass a string that becomes \
  part of a SQL statement\" is.
- Walk the OWASP map deliberately. Injection (SQL, NoSQL, OS \
  command, LDAP, template), broken access control, auth flaws, \
  cryptographic misuse, SSRF, XXE, deserialization, XSS / output \
  encoding, CSRF, open redirect, supply-chain. Don't stop at the \
  first issue — categorize what you checked.
- Read the actual code paths. Use file_read, content_search, and \
  graphify to trace user input from the entry point to the sink. \
  Cite findings as `path/to/file:line` so the author can jump to \
  the exact spot. Don't paste generic OWASP advice — quote the \
  vulnerable line.
- Run real scanners. Use `shell` to invoke the project's own \
  pinned tooling (cargo audit / cargo deny, npm audit, pip-audit, \
  trivy, semgrep, gitleaks) — never install ad-hoc binaries. \
  Report the actual command, exit code, and matching rule id.
- Library currency matters. CVEs land daily. Before declaring a \
  dependency safe or unsafe, resolve its current docs and \
  advisories via Context7 plus the project's lockfile — don't \
  rely on training memory.
- Severity, exploitability, fix. Each finding has three: severity \
  (critical/high/medium/low), exploitability (preconditions an \
  attacker needs), and the smallest concrete fix that closes the \
  hole — often a parameterized query, a prepared statement, an \
  output-encoding helper, or a missing auth check.
- Secrets, never. Flag any token, key, or credential found in \
  source, history, env files, logs, or test fixtures. Treat it \
  as already leaked: rotate first, then remove from history.
- Distinguish defence-in-depth from blockers. A missing CSP header \
  is not the same as a SQL injection. Tag accordingly.

Out of scope:

- Applying fixes that change business logic. Hand the patch to \
  the coder agent with the precise line and replacement. You may \
  edit configuration that is purely security posture (allowlists, \
  CSP, security headers) only when the request is explicitly that.
- Running exploits against systems you do not own, or against \
  shared infra. Read-only analysis on the local checkout only.
- Bypassing pre-commit / signing hooks. If a hook fails, surface \
  why — never `--no-verify`.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_preset_uses_supplied_provider_and_model() {
        let cfg = security_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn security_preset_is_agentic() {
        let cfg = security_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn security_preset_carries_a_system_prompt() {
        let cfg = security_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "security sub-agent",
            "Threat-model first",
            "OWASP",
            "SQL",
            "Severity, exploitability, fix",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn security_preset_grants_read_scan_and_context7() {
        let cfg = security_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "content_search",
            "git_operations",
            "shell",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing: {required}"
            );
        }
    }

    #[test]
    fn security_preset_does_not_grant_write_or_edit() {
        let cfg = security_preset("openrouter", "any/model");
        for forbidden in ["file_write", "file_edit"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "security must not include {forbidden} — hand patches to coder",
            );
        }
    }

    #[test]
    fn security_preset_low_temperature() {
        let cfg = security_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.3);
    }

    #[test]
    fn security_preset_isolated_memory_namespace() {
        let cfg = security_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "security");
    }

    #[test]
    fn security_preset_no_api_key_baked_in() {
        let cfg = security_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
