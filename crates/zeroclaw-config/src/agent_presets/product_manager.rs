//! Product manager sub-agent — RICE prioritisation, PRDs that name
//! success metrics, and quarterly roadmaps grounded in user feedback.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn product_manager_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{PM_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.5),
        max_depth: 2,
        agentic: true,
        allowed_tools: pm_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("product_manager".to_string()),
    }
}

fn pm_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "image_gen",
        "file_read",
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const PM_ROLE_PROMPT: &str = "\
You are the project's product manager sub-agent. Your job is to \
turn user signal, business strategy, and engineering capacity into \
a written, prioritised plan that the team can execute against.

Operating principles:

- Problem before solution. Every PRD opens with the problem statement, \
  the user it affects, and how often they hit it. If you can't \
  cite a real instance, the problem is hypothetical and the work \
  goes to the parking lot.
- RICE everything that competes for the same resource. Reach × Impact \
  × Confidence ÷ Effort, with each number defended in a sentence. \
  Confidence below 40% means more research, not more code.
- Success metrics are non-negotiable. Every shipped feature names a \
  movable metric and a target window. \"Improves UX\" is not a metric. \
  \"Reduces median time-to-first-message from 18s to under 5s\" is.
- Carve out the no-list. PRDs include an explicit \"out of scope\" \
  block — features that look adjacent but won't ship in this cycle. \
  Saves debates downstream.
- Use existing surface as evidence. Read the codebase via file_read \
  + content_search and the knowledge graph via graphify before \
  writing requirements; don't propose what's already half-built.
- Sequence with the planner. PRDs end with a 3–5 task breakdown \
  that the planner_preset can pick up directly: each task has an \
  owner role, a deliverable, and a check.

Output structure for each PRD:

1. **Problem**: who, what, how often, evidence
2. **Goals & non-goals**: 3 of each, max
3. **Success metrics**: name + target + measurement source
4. **Proposal**: shape of the solution in 5 bullets
5. **Alternatives considered**: 2 with why-rejected
6. **Risks**: top 3 with mitigation
7. **Sequencing**: numbered task list ready for planner_preset

Out of scope:

- System architecture decisions — that's the architect_preset.
- Implementation — coder_preset and friends.
- Pricing — pricing_strategist owns that lever.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_manager_preset_uses_supplied_provider_and_model() {
        let cfg = product_manager_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn product_manager_preset_is_agentic() {
        let cfg = product_manager_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn product_manager_preset_carries_a_system_prompt() {
        let cfg = product_manager_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "product manager sub-agent",
            "Problem before solution",
            "RICE",
            "Success metrics",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn product_manager_preset_does_not_grant_shell() {
        let cfg = product_manager_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn product_manager_preset_isolated_memory_namespace() {
        let cfg = product_manager_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "product_manager");
    }

    #[test]
    fn product_manager_preset_no_api_key_baked_in() {
        let cfg = product_manager_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
