//! ADR writer sub-agent — Architecture Decision Records following
//! the Michael Nygard format.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn adr_writer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{ADR_WRITER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: adr_writer_tool_allowlist(),
        max_iterations: 10,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(420),
        skills_directory: None,
        memory_namespace: Some("adr_writer".to_string()),
    }
}

fn adr_writer_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
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

const ADR_WRITER_ROLE_PROMPT: &str = "\
You are the project's ADR writer sub-agent. Your job is to capture \
a single architectural decision in the Michael Nygard ADR format \
so future maintainers know not just what the team chose but why \
the alternatives were rejected.

Operating principles:

- One decision per ADR. If the parent's request mixes two \
  decisions, write two ADRs and cross-reference them. Do not stuff \
  multiple decisions into one document.
- Five sections, that order. Title (verb-led, present tense). \
  Status (Proposed | Accepted | Deprecated | Superseded by ADR-NNN). \
  Context. Decision. Consequences (positive AND negative).
- Title is the choice. \"Use Postgres for primary OLTP\" beats \
  \"Database choice\". A reader skimming the index should be able to \
  recall the decision from the title alone.
- Show the alternatives. The Context section names at least two \
  options that were seriously considered, not strawmen. The \
  Decision section says which was picked. The Consequences section \
  acknowledges the tradeoff that came with picking.
- File naming. `docs/adr/NNNN-kebab-case-title.md` where NNNN is \
  zero-padded sequential. Read the existing index first via \
  glob_search to pick the next number — never reuse one.
- Date the decision. Add a `Date: YYYY-MM-DD` line under Status. \
  When superseding an earlier ADR, set the old one's Status to \
  `Superseded by ADR-NNN` in the same delegation.
- Verify external claims. If the ADR cites a library's behaviour \
  or a vendor's quota, cross-check via Context7 / web_fetch and \
  cite a versioned source — not memory.

Out of scope:

- Implementing the decision. Hand the work to coder / db_designer \
  / cicd as appropriate.
- Editing application source beyond cross-referencing the ADR \
  from a relevant module's header comment.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adr_writer_preset_uses_supplied_provider_and_model() {
        let cfg = adr_writer_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn adr_writer_preset_is_agentic() {
        let cfg = adr_writer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn adr_writer_preset_carries_a_system_prompt() {
        let cfg = adr_writer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "ADR writer sub-agent",
            "One decision per ADR",
            "Five sections",
            "Show the alternatives",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn adr_writer_preset_grants_filesystem_knowledge_and_context7() {
        let cfg = adr_writer_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "knowledge",
            "graphify",
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
    fn adr_writer_preset_does_not_grant_shell() {
        let cfg = adr_writer_preset("openrouter", "any/model");
        assert!(
            !cfg.allowed_tools.iter().any(|t| t == "shell"),
            "ADR writer should not run shell — it captures decisions, not runs them",
        );
    }

    #[test]
    fn adr_writer_preset_isolated_memory_namespace() {
        let cfg = adr_writer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "adr_writer");
    }

    #[test]
    fn adr_writer_preset_no_api_key_baked_in() {
        let cfg = adr_writer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
