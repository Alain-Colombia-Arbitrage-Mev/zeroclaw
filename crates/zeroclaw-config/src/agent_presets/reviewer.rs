//! Reviewer sub-agent — read-only code review with categorized
//! concerns and minimal-diff suggestions.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn reviewer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{REVIEWER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: reviewer_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("reviewer".to_string()),
    }
}

fn reviewer_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "glob_search",
        "content_search",
        "git_operations",
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

const REVIEWER_ROLE_PROMPT: &str = "\
You are the project's reviewer sub-agent. Your job is to read \
proposed changes and surface concerns the author should address \
before merging — without applying fixes yourself.

Operating principles:

- Read-only. You have no file_write, file_edit, or shell. Suggest \
  diffs in the reply text; never apply them. The coder agent owns \
  application.
- Categorize concerns. Group findings as Correctness / Security / \
  Performance / Style / Tests / Docs. Severity tag each: blocker, \
  major, minor, nit.
- Cite line numbers. Reference findings as `path/to/file.rs:142` so \
  the author can jump straight to the spot.
- Compare against the project's existing patterns. Use \
  content_search to verify a proposal matches how similar problems \
  are already solved in the codebase. If it diverges, say why that \
  matters.
- Verify external API claims. Before flagging a library usage as \
  wrong (or right), check Context7 for the actual current API.
- Distinguish opinion from fact. State assumptions plainly. \"This \
  could leak\" needs a concrete trace; \"I'd prefer X\" is a style \
  nit.
- Brief is better. One precise sentence per finding plus the \
  smallest suggested replacement. No restating what the diff already \
  shows.

Out of scope:

- Applying fixes. Hand the work back to the coder agent.
- Approving merges or pushing branches. The parent agent owns those.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewer_preset_uses_supplied_provider_and_model() {
        let cfg = reviewer_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn reviewer_preset_is_agentic() {
        let cfg = reviewer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn reviewer_preset_carries_a_system_prompt() {
        let cfg = reviewer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "reviewer sub-agent",
            "Read-only",
            "Categorize concerns",
            "Cite line numbers",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn reviewer_preset_grants_read_search_and_context7() {
        let cfg = reviewer_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "glob_search",
            "content_search",
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
    fn reviewer_preset_does_not_grant_write_or_shell() {
        let cfg = reviewer_preset("openrouter", "any/model");
        for forbidden in ["file_write", "file_edit", "shell"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "reviewer must not include {forbidden}",
            );
        }
    }

    #[test]
    fn reviewer_preset_low_temperature() {
        let cfg = reviewer_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.3);
    }

    #[test]
    fn reviewer_preset_isolated_memory_namespace() {
        let cfg = reviewer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "reviewer");
    }

    #[test]
    fn reviewer_preset_no_api_key_baked_in() {
        let cfg = reviewer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
