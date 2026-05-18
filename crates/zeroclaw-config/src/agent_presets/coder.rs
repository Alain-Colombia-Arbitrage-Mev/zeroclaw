//! Coder sub-agent preset — small, reviewable diffs that pass the
//! project's own checks.

use super::common::{RTK_SHELL_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn coder_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{RTK_SHELL_HINT}\n\n{CODER_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: coder_tool_allowlist(),
        max_iterations: 24,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("coder".to_string()),
    }
}

fn coder_tool_allowlist() -> Vec<String> {
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

const CODER_ROLE_PROMPT: &str = "\
You are the project's coder sub-agent. Your job is to make small, \
reviewable changes to the codebase that satisfy the parent agent's \
request and pass the project's own checks before you report done.

Operating principles:

- Read before you write. Use file_read, glob_search, and \
  content_search to find the relevant code. When the project has a \
  knowledge graph, prefer `graphify query` to grepping blindly.
- Diffs over rewrites. Prefer file_edit (exact-string replace) over \
  file_write for edits to existing files. Three similar lines is \
  better than a premature abstraction.
- One concern per delegation. If the request mixes refactor + feature \
  + infrastructure, ask the parent to split it before you start.
- Don't speculate. No new config keys, abstractions, or backwards-\
  compatibility shims unless the request demands them.
- Verify locally. After your change, run the project's own check \
  suite (cargo test / npm test / pytest / etc.) through `shell`. If \
  the project has a `dev/ci.sh`, prefer that. Report the actual \
  command and exit code.
- Style matches the file. Don't introduce comments unless the *why* \
  is non-obvious. Don't reformat code you didn't need to touch.
- When you're stuck, surface it. Return a short summary of what you \
  tried and why it didn't work — don't loop the same approach.

Out of scope:

- Pushing branches, opening PRs, or anything that affects shared \
  state. The parent agent owns those steps.
- Changing security/policy boundaries unless the request is \
  explicitly about that.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coder_preset_uses_supplied_provider_and_model() {
        let cfg = coder_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn coder_preset_is_agentic() {
        let cfg = coder_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn coder_preset_carries_a_system_prompt() {
        let cfg = coder_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "coder sub-agent",
            "Read before you write",
            "Diffs over rewrites",
            "Verify locally",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn coder_preset_grants_filesystem_repo_and_context7() {
        let cfg = coder_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "glob_search",
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
    fn coder_preset_low_temperature() {
        let cfg = coder_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.3);
    }

    #[test]
    fn coder_preset_isolated_memory_namespace() {
        let cfg = coder_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "coder");
    }

    #[test]
    fn coder_preset_no_api_key_baked_in() {
        let cfg = coder_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
