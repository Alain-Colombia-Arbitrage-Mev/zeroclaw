//! Curated `DelegateAgentConfig` presets for common sub-agent roles.
//!
//! `DelegateAgentConfig` is intentionally a low-level descriptor — it
//! exposes provider, model, tools, prompts, and timeouts but ships
//! with no opinionated examples. Operators who want an agent surface
//! "out of the box" therefore have to write a long TOML block from
//! scratch.
//!
//! These helpers keep the descriptor uncoupled while giving callers
//! and integration code a one-call shortcut to a sensible default for
//! roles ZeroClaw expects to recur — today: `coder` (this PR) and
//! `designer` (PR F, lands alongside the Pencil.dev tool).
//!
//! Operators may use them three ways:
//!
//! 1. **Copy the equivalent TOML** from `docs/book/src/setup/coder-agent.md`
//!    into their own config — fully transparent, no surprise behaviour.
//! 2. **Call the builder from extension code** to register the agent
//!    programmatically (`coder_preset(provider, model)`).
//! 3. **Override fields after building** — the returned struct is plain
//!    `DelegateAgentConfig`, every field is `pub`.
//!
//! The presets do NOT change ZeroClaw's defaults: an out-of-the-box
//! daemon with no `[agents.*]` section still has zero registered
//! sub-agents. This module is opt-in.

use crate::schema::DelegateAgentConfig;

/// Return a curated `DelegateAgentConfig` for a coding-focused
/// sub-agent. The returned struct is yours to mutate further.
///
/// The defaults bias toward:
///   - **agentic mode on** — coding tasks rarely fit into a single
///     turn; the sub-agent needs to read, edit, and verify in a loop
///   - a tight tool allowlist that covers the day-to-day work without
///     opening shell semantics any wider than necessary
///   - a system prompt that nudges the model toward small, reviewable
///     diffs, away from speculative refactors, and toward running the
///     project's own tests/checks before reporting success
///
/// Pick a code-strong `model` for best results — the helper does
/// nothing model-specific so any `provider`/`model` pair the
/// runtime can resolve will work.
pub fn coder_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(CODER_SYSTEM_PROMPT.to_string()),
        api_key: None,
        // Lower than the runtime default so output is deterministic
        // for code generation. Override per-agent if you want creativity.
        temperature: Some(0.2),
        // Keep the default; the parent agent controls how deep the
        // delegation chain can recurse.
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

/// Tool names the coder preset is allowed to call.
///
/// Kept in sync with the operator-facing TOML in
/// `docs/book/src/setup/coder-agent.md` — change this list and
/// update the doc page in the same PR.
fn coder_tool_allowlist() -> Vec<String> {
    [
        // Filesystem — read, search, write, edit
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        // Repo operations
        "git_operations",
        // Shell — gated by the SecurityPolicy allowlist anyway
        "shell",
        // Build / test runners (executed via shell allowlist; surfaced
        // here for the sub-agent's prompt-time discoverability)
        "tool_search",
        // Knowledge graph (so the coder can read the project's map)
        "knowledge",
        "graphify",
        // LLM helpers for structured sub-tasks (codegen review, etc.)
        "llm_task",
        // Browser fetch for docs lookups (read-only)
        "web_fetch",
        // Memory access for cross-session continuity
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

/// System prompt for the coder preset.
///
/// The text is intentionally terse and operational. The sub-agent
/// inherits the runtime's general identity / safety prompt; this
/// prompt only adds role-specific guidance.
const CODER_SYSTEM_PROMPT: &str = "\
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
        assert!(cfg.agentic, "coder must run as a multi-turn agent");
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn coder_preset_carries_a_system_prompt() {
        let cfg = coder_preset("openrouter", "any/model");
        let prompt = cfg
            .system_prompt
            .expect("coder preset must set a system prompt");
        // A few invariants the prompt has to keep — if any of these
        // disappear the operator's mental model breaks.
        for needle in [
            "coder sub-agent",
            "Read before you write",
            "Diffs over rewrites",
            "Verify locally",
        ] {
            assert!(
                prompt.contains(needle),
                "coder system prompt missing expected guidance: '{needle}'",
            );
        }
    }

    #[test]
    fn coder_preset_grants_filesystem_and_repo_tools() {
        let cfg = coder_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "glob_search",
            "content_search",
            "git_operations",
            "shell",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "coder preset missing required tool: {required}",
            );
        }
    }

    #[test]
    fn coder_preset_low_temperature() {
        let cfg = coder_preset("openrouter", "any/model");
        let temp = cfg.temperature.expect("coder preset must set temperature");
        assert!(
            temp <= 0.3,
            "coder temperature should be low for deterministic codegen, got {temp}",
        );
    }

    #[test]
    fn coder_preset_isolated_memory_namespace() {
        let cfg = coder_preset("openrouter", "any/model");
        let ns = cfg
            .memory_namespace
            .expect("coder preset should isolate its memory");
        assert_eq!(ns, "coder");
    }

    #[test]
    fn coder_preset_no_api_key_baked_in() {
        let cfg = coder_preset("openrouter", "any/model");
        assert!(
            cfg.api_key.is_none(),
            "preset must not bake in an API key — that's a config-time concern",
        );
    }
}
