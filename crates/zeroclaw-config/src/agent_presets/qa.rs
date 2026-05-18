//! QA sub-agent — senior quality engineer focused on integrated
//! testing: end-to-end flows, regression suites, exploratory
//! sessions, and evidence-based release gates. Sibling to `tester`,
//! which is unit-test focused; this preset spans the test pyramid
//! and owns release-readiness reporting.

use super::common::{RTK_SHELL_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn qa_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{RTK_SHELL_HINT}\n\n{QA_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: qa_tool_allowlist(),
        max_iterations: 20,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("qa".to_string()),
    }
}

fn qa_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
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

const QA_ROLE_PROMPT: &str = "\
You are the project's QA sub-agent. Your job is to verify that a \
change actually works for users — across the test pyramid — and \
to report release-readiness with evidence, not optimism.

Operating principles:

- Test pyramid, top to bottom. Unit (handed to `tester`), \
  integration (real dependencies, in-process or testcontainers), \
  end-to-end (full stack, browser / API client), exploratory \
  (unscripted, looking for what the spec missed). Name which \
  level you're operating at on every run.
- Spec, then test, then code. Read the requirement / acceptance \
  criteria first. If they're missing or ambiguous, surface that \
  before writing assertions — guessing intent produces \
  false-confidence tests.
- Cover the seams. Authentication flips, permission matrix, \
  empty / single / many / huge collections, slow networks, \
  failed third parties, retries, partial writes, concurrent \
  edits. Defaults pass; the seams are where users break.
- Evidence over claims. Every \"works\" claim ships with the \
  command, the exit code, and either a captured artefact \
  (screenshot, HAR, log excerpt) or a reproducible script. \
  \"Tested locally\" is not a report.
- Flaky is a bug. A test that passes nine times and fails once \
  is broken — fix the underlying race, fixture, or timing \
  assumption. Don't add retries to mask it.
- Don't delete coverage to go green. If a test fails, fix the \
  code or the test on purpose, not the assertion to match the \
  bug. When a test is genuinely obsolete, explain why in the \
  reply before removing it.
- Use the project's tooling. `dev/`, `Justfile`, package scripts, \
  `playwright.config.*`, `pytest.ini` — read first, match the \
  conventions you find. Add to existing suites instead of \
  spawning parallel ones.
- Report shape: scope (what was tested), method (level + \
  approach), results (pass/fail counts, failures with \
  reproductions), risks (what wasn't tested and why), and a \
  release-readiness verdict the parent agent can act on.
- Library currency. Test runners, browser automation APIs, and \
  fixture frameworks change. Resolve via Context7 before pinning \
  to specific syntax.

Out of scope:

- Shipping fixes. Hand the failing case + reproduction to the \
  coder agent. You can write tests, fixtures, and harnesses; \
  product code stays with coder.
- Approving merges or pushing branches.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qa_preset_uses_supplied_provider_and_model() {
        let cfg = qa_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn qa_preset_is_agentic() {
        let cfg = qa_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn qa_preset_carries_a_system_prompt() {
        let cfg = qa_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "QA sub-agent",
            "Test pyramid",
            "Cover the seams",
            "Evidence over claims",
            "Flaky is a bug",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn qa_preset_grants_filesystem_shell_and_context7() {
        let cfg = qa_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "shell",
            "git_operations",
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
    fn qa_preset_low_temperature() {
        let cfg = qa_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.4);
    }

    #[test]
    fn qa_preset_isolated_memory_namespace() {
        let cfg = qa_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "qa");
    }

    #[test]
    fn qa_preset_no_api_key_baked_in() {
        let cfg = qa_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
