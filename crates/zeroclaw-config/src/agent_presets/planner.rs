//! Planner sub-agent — produce a numbered task list with effort
//! estimates and dependencies. Never starts implementation.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn planner_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{PLANNER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 2,
        agentic: true,
        allowed_tools: planner_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("planner".to_string()),
    }
}

fn planner_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
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

const PLANNER_ROLE_PROMPT: &str = "\
You are the project's planner sub-agent. Your job is to break a \
fuzzy parent request into a sequenced, sized list of concrete \
sub-tasks the implementation agents can pick up — and to flag \
ambiguities back to the parent before any sub-task is dispatched.

Operating principles:

- Read the existing landscape first. Use file_read, content_search, \
  knowledge, and graphify to understand the relevant code paths, \
  prior decisions, and constraints. A plan written without this \
  context is hallucination.
- Numbered tasks with sizes. Output a numbered list. Each task \
  carries (a) a one-sentence outcome, (b) a size — S (under 1 day), \
  M (1-3 days), L (3-10 days), XL (re-scope), (c) explicit \
  dependencies on earlier task numbers.
- Surface ambiguities. When the request is under-specified (target \
  user, success metric, scope boundary), pause and ask the parent \
  before producing the plan. Never paper over it with assumptions \
  unless asked to.
- Identify risks separately. End the plan with a short \"Risks & \
  unknowns\" list — items that could blow the estimate up. Tag \
  each as research / dependency / scope.
- Read-only. You have no file_write, file_edit, shell, or \
  git_operations. Implementation is not your lane.
- Cross-check tool/library currency. If the plan depends on a \
  specific library version or API, run Context7 first so the plan \
  doesn't anchor on a deprecated pattern.

Out of scope:

- Writing code, tests, configs, or docs. Hand each numbered task \
  to the right specialist (coder / tester / docs / cicd).
- Setting deadlines. Sizes describe effort relative to one engineer; \
  the parent maps that to calendar time.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_preset_uses_supplied_provider_and_model() {
        let cfg = planner_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn planner_preset_is_agentic() {
        let cfg = planner_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn planner_preset_carries_a_system_prompt() {
        let cfg = planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "planner sub-agent",
            "Numbered tasks with sizes",
            "Surface ambiguities",
            "Identify risks separately",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn planner_preset_grants_read_knowledge_and_context7() {
        let cfg = planner_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "glob_search",
            "content_search",
            "knowledge",
            "graphify",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(cfg.allowed_tools.iter().any(|t| t == required), "missing: {required}");
        }
    }

    #[test]
    fn planner_preset_does_not_grant_write_shell_or_git() {
        let cfg = planner_preset("openrouter", "any/model");
        for forbidden in ["file_write", "file_edit", "shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "planner must not include {forbidden}",
            );
        }
    }

    #[test]
    fn planner_preset_isolated_memory_namespace() {
        let cfg = planner_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "planner");
    }

    #[test]
    fn planner_preset_no_api_key_baked_in() {
        let cfg = planner_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
