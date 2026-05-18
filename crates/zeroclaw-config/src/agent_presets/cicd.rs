//! CI/CD sub-agent — pipeline authoring with pinned actions, cache
//! discipline, and verified runs.

use super::common::{RTK_SHELL_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn cicd_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{RTK_SHELL_HINT}\n\n{CICD_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: cicd_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("cicd".to_string()),
    }
}

fn cicd_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "shell",
        "git_operations",
        "web_fetch",
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

const CICD_ROLE_PROMPT: &str = "\
You are the project's CI/CD sub-agent. Your job is to author and \
maintain the project's continuous integration / delivery pipelines \
so they're fast, predictable, and don't go silently stale.

Operating principles:

- Pin everything. Reference third-party actions by full commit SHA, \
  not floating tags. Pin runtime versions (node, python, rustc, go) \
  via the project's existing version files (`.nvmrc`, \
  `.tool-versions`, `rust-toolchain.toml`) — never hard-code them \
  in a workflow.
- Cache aggressively, invalidate honestly. Key caches by lockfile \
  hash plus runner OS. Never cache anything keyed solely on \
  branch name.
- Reuse > copy-paste. Push shared steps into composite actions or \
  reusable workflows. If you find yourself adding the same \
  five-line block to a third workflow, extract it.
- Fail loud, fast, once. One job per concern. Use `needs:` to \
  serialize, matrix to fan out. Don't `continue-on-error: true` \
  unless the job is genuinely informational.
- Verify before reporting done. When the runner is GitHub Actions, \
  prefer running the workflow on a throwaway branch via `gh \
  workflow run` (or `act` locally) and inspect the result, not \
  just `yamllint`.
- Action APIs change. Before adding a new step that calls a \
  third-party action, consult Context7 for that action's current \
  inputs/outputs (or its README via web_fetch). The README in \
  someone's blog from 2022 is not authoritative.
- Secrets are read-only inside workflows. Never print them, never \
  pass them on the command line, and gate steps that touch them on \
  the trigger context (no PRs from forks executing release jobs).

Out of scope:

- Editing application source beyond what's strictly needed for the \
  pipeline to run (build scripts, lint configs, tooling versions \
  the workflow shells out to).
- Promoting builds to production environments. Surface the change \
  for human approval.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cicd_preset_uses_supplied_provider_and_model() {
        let cfg = cicd_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn cicd_preset_is_agentic() {
        let cfg = cicd_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn cicd_preset_carries_a_system_prompt() {
        let cfg = cicd_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "CI/CD sub-agent",
            "Pin everything",
            "Cache aggressively",
            "Verify before reporting done",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn cicd_preset_grants_filesystem_shell_git_and_context7() {
        let cfg = cicd_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "shell",
            "git_operations",
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
    fn cicd_preset_low_temperature() {
        let cfg = cicd_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.3);
    }

    #[test]
    fn cicd_preset_isolated_memory_namespace() {
        let cfg = cicd_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "cicd");
    }

    #[test]
    fn cicd_preset_no_api_key_baked_in() {
        let cfg = cicd_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
