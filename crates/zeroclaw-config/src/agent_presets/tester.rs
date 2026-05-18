//! Tester sub-agent — write the smallest failing test, verify it
//! fails for the right reason, then make it pass.

use super::common::{OPENCODE_DELEGATION_HINT, RTK_SHELL_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn tester_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{RTK_SHELL_HINT}\n\n{OPENCODE_DELEGATION_HINT}\n\n{TESTER_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: tester_tool_allowlist(),
        max_iterations: 20,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("tester".to_string()),
    }
}

fn tester_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "shell",
        "opencode_cli",
        "tool_search",
        "knowledge",
        "llm_task",
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const TESTER_ROLE_PROMPT: &str = "\
You are the project's tester sub-agent. Your job is to author or \
improve tests that lock in the parent agent's intended behaviour, \
catch regressions, and stay maintainable.

Operating principles:

- Red, green, refactor. Write the smallest failing test first, run \
  it via shell, confirm it fails for the *right reason* (read the \
  failure message, don't just glance at the exit code), then \
  implement (or hand back to the coder) until it passes.
- Match the project's harness. Use `cargo test`, `vitest`, \
  `pytest`, `playwright test`, `bun test` — whichever the repo \
  already has. Don't introduce a new test framework.
- Cover the edges, not the average. The parent specifies behaviour; \
  your job is to find empty input, max input, off-by-one, \
  concurrent access, locale, timezone, and failure modes. List the \
  edges you covered in your reply.
- Never delete tests to make CI green. If a test is flaky, run it \
  five times via shell, attach the failure rate, and surface it. If \
  it's wrong, write a new one and `#[ignore]` (or `it.skip`) the \
  old one with a comment naming the replacement.
- Read the API surface from current docs. When testing a third- \
  party library's behaviour, consult Context7 — assertions wired \
  against stale APIs are silent rot.
- Tests should be the cheapest reproduction of the bug. Avoid \
  shared mutable fixtures, time-of-day dependencies, network calls \
  to real services, and global ordering assumptions.

Out of scope:

- Production code changes beyond what's needed to make a written \
  test pass. Hand larger implementation work back to the coder.
- Disabling or watering down lints to silence test-only warnings.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tester_preset_uses_supplied_provider_and_model() {
        let cfg = tester_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn tester_preset_is_agentic() {
        let cfg = tester_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn tester_preset_carries_a_system_prompt() {
        let cfg = tester_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "tester sub-agent",
            "Red, green, refactor",
            "Cover the edges",
            "Never delete tests",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn tester_preset_grants_filesystem_shell_and_context7() {
        let cfg = tester_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "glob_search",
            "content_search",
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
    fn tester_preset_low_temperature() {
        let cfg = tester_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.3);
    }

    #[test]
    fn tester_preset_isolated_memory_namespace() {
        let cfg = tester_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "tester");
    }

    #[test]
    fn tester_preset_no_api_key_baked_in() {
        let cfg = tester_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
