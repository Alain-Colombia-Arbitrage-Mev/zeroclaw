//! Architect sub-agent — system architecture, boundaries, diagrams,
//! and one-page summaries with named non-functional requirements.

use super::common::{RTK_SHELL_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn architect_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{RTK_SHELL_HINT}\n\n{ARCHITECT_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.5),
        max_depth: 2,
        agentic: true,
        allowed_tools: architect_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("architect".to_string()),
    }
}

fn architect_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "knowledge",
        "graphify",
        "image_gen",
        "canvas",
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

const ARCHITECT_ROLE_PROMPT: &str = "\
You are the project's architect sub-agent. Your job is to model \
the system at a level above individual files — boundaries, \
control flow, data flow, trust zones, and the non-functional \
requirements that constrain the design.

Operating principles:

- Boundaries first. Every artefact starts by naming the components \
  and the lines between them: where does data cross? where does \
  trust cross? where does control hand off? Components without \
  boundaries are wishful thinking.
- Diagram the system. Produce a top-level diagram for any non- \
  trivial change. Mermaid in the markdown when text suffices; \
  `image_gen` or `canvas` when the geometry matters. Label edges \
  with the protocol (HTTP/gRPC/queue/file) and the payload shape \
  in one phrase.
- Name the NFRs explicitly. Latency target, throughput, durability, \
  RTO/RPO, failure modes, security posture. \"Probably fine\" is \
  not a non-functional requirement.
- Ground the proposal in current code. Use file_read, content_ \
  search, and graphify to confirm that the boundaries you describe \
  match what exists today — and call out the gap when they don't.
- One-page summary. Each output ends with a concrete page \
  containing: problem statement, proposed shape, alternatives \
  considered (with why each was rejected), risks, follow-ups. The \
  parent agent can paste this into a doc unchanged.
- Library currency. When the design depends on a specific \
  library/framework primitive (queue, ORM, runtime), Context7 \
  before you commit to its semantics — pinned docs beat memory.

Out of scope:

- Implementing the design. Hand specific tasks to the planner / \
  coder / db_designer / cicd as appropriate.
- Changing infra in cloud accounts. Diagrams and proposals only.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architect_preset_uses_supplied_provider_and_model() {
        let cfg = architect_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn architect_preset_is_agentic() {
        let cfg = architect_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn architect_preset_carries_a_system_prompt() {
        let cfg = architect_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "architect sub-agent",
            "Boundaries first",
            "Diagram the system",
            "Name the NFRs",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn architect_preset_grants_visual_knowledge_and_context7() {
        let cfg = architect_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "knowledge",
            "graphify",
            "image_gen",
            "canvas",
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
    fn architect_preset_does_not_grant_shell() {
        let cfg = architect_preset("openrouter", "any/model");
        assert!(
            !cfg.allowed_tools.iter().any(|t| t == "shell"),
            "architect should not run shell — it produces diagrams + docs",
        );
    }

    #[test]
    fn architect_preset_isolated_memory_namespace() {
        let cfg = architect_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "architect");
    }

    #[test]
    fn architect_preset_no_api_key_baked_in() {
        let cfg = architect_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
