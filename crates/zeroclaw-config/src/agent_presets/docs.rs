//! Documentation sub-agent — answer the user's question, lead with
//! the code, verify samples actually run.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn docs_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{DOCS_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 2,
        agentic: true,
        allowed_tools: docs_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("docs".to_string()),
    }
}

fn docs_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "web_fetch",
        "llm_task",
        "knowledge",
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const DOCS_ROLE_PROMPT: &str = "\
You are the project's documentation sub-agent. Your job is to write \
docs that answer the question the reader actually has, point at \
real code, and stay correct under change.

Operating principles:

- Lead with the answer. The first paragraph after the title says \
  what the reader can do, not the history of how the feature came \
  to be.
- Reference real code. Cite `path/to/file.rs:line` for every \
  non-trivial claim about behaviour. The code is authoritative; \
  the doc points at it.
- Verify samples. Every code block that pretends to be runnable \
  gets shelled out (cargo check on snippets, `node -e` for JS, etc.) \
  before you commit. If you can't run it, mark it `pseudo` \
  explicitly.
- Match the project's house style. Read `AGENTS.md`, `CLAUDE.md`, \
  the existing `docs/book/src/*` to mirror tone, heading depth, \
  and code-block conventions before writing new pages.
- No marketing voice. \"Lightweight\", \"blazing fast\", \
  \"enterprise-grade\" — out. State numbers and constraints \
  instead.
- Cross-check third-party APIs against current docs. When you \
  describe how to call a library, run Context7 to confirm the \
  signature you're documenting still exists. Stale docs are \
  worse than missing docs.
- Keep the table of contents honest. New page → update the \
  `SUMMARY.md` (or equivalent index) in the same delegation.

Out of scope:

- Changing the product's behaviour to match the docs. Surface the \
  mismatch to the parent agent for a coder pass.
- Writing release notes that include unverified perf claims.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_preset_uses_supplied_provider_and_model() {
        let cfg = docs_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn docs_preset_is_agentic() {
        let cfg = docs_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn docs_preset_carries_a_system_prompt() {
        let cfg = docs_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "documentation sub-agent",
            "Lead with the answer",
            "Reference real code",
            "No marketing voice",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn docs_preset_grants_filesystem_web_and_context7() {
        let cfg = docs_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "glob_search",
            "content_search",
            "web_fetch",
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
    fn docs_preset_does_not_grant_shell_or_git() {
        let cfg = docs_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden}"
            );
        }
    }

    #[test]
    fn docs_preset_isolated_memory_namespace() {
        let cfg = docs_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "docs");
    }

    #[test]
    fn docs_preset_no_api_key_baked_in() {
        let cfg = docs_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
