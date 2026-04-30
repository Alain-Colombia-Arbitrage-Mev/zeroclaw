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
//! roles ZeroClaw expects to recur — today: `coder` and `designer`.
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

/// Return a curated `DelegateAgentConfig` for a UI/visual design
/// sub-agent. Pairs naturally with the Pencil.dev MCP server when
/// configured, but works on its own with the built-in `image_gen`
/// and `canvas` tools as the visual back-ends.
///
/// Defaults bias toward:
///   - **agentic mode on** — design iterations rarely fit one turn
///   - **higher temperature** (0.6) than the coder — divergent
///     ideation is part of the job
///   - a tool allowlist that covers Pencil (when registered as an
///     MCP server, the MCP wrapper exposes `pencil_*` tools), the
///     native image generator and canvas, and the filesystem so the
///     agent can land HTML/CSS or component files when asked
///   - a system prompt that biases toward design tokens, visual
///     hierarchy, brand consistency, accessibility, and small
///     iterations over big rewrites
pub fn designer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(DESIGNER_SYSTEM_PROMPT.to_string()),
        api_key: None,
        // Higher than the coder — visuals benefit from a touch of
        // divergent thinking. Lower than full creative writing,
        // because design has hard constraints (tokens, a11y).
        temperature: Some(0.6),
        max_depth: 2,
        agentic: true,
        allowed_tools: designer_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("designer".to_string()),
    }
}

/// Tool names the designer preset is allowed to call.
///
/// Pencil-specific tools are registered automatically when the
/// operator adds a `pencil` MCP server (see
/// `docs/book/src/setup/designer-agent.md`). The MCP registry
/// surfaces them with a `pencil_*` prefix; we list the most common
/// ones so the agent's tool-list prompt is informative even when
/// the server is offline. Tools that don't exist at runtime are
/// silently dropped from the agent's effective surface — listing
/// them is harmless.
fn designer_tool_allowlist() -> Vec<String> {
    [
        // Pencil.dev MCP — visual design surface (when registered)
        "pencil_open_document",
        "pencil_get_editor_state",
        "pencil_batch_get",
        "pencil_batch_design",
        "pencil_get_screenshot",
        "pencil_search_all_unique_properties",
        "pencil_replace_all_matching_properties",
        // Native generative + composition tools
        "image_gen",
        "canvas",
        // Filesystem — landing HTML / CSS / component files
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        // Reference fetching — design libraries, docs
        "web_fetch",
        // LLM helpers for structured copy / a11y review
        "llm_task",
        // Memory access for cross-session design language continuity
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

const DESIGNER_SYSTEM_PROMPT: &str = "\
You are the project's designer sub-agent. Your job is to produce or \
refine UI / visual artefacts — design tokens, components, mockups, \
layouts, illustrations — that fit the project's existing language \
and the parent agent's brief.

Operating principles:

- Tokens before pixels. When the project has a design-system \
  surface (Pencil document, CSS variables, Figma variables), read \
  it first and reference its colours, type scale, and spacing — do \
  not invent new values that fight the system.
- Hierarchy over decoration. Each artefact should answer \"what is \
  the user's first action here?\" before it answers \"what looks \
  cool?\". When in doubt, ship a calmer version.
- Accessibility is non-negotiable. Verify contrast (WCAG AA at \
  minimum), tab order, focus states, and alt text on every \
  artefact you emit. Surface failures as part of your reply, not \
  buried.
- Iterate small. Prefer `pencil_batch_design` updates and \
  `file_edit` over wholesale `pencil_open_document` / \
  `file_write` rewrites. Three coordinated tweaks beat one \
  ambitious rewrite.
- Show your work. After each iteration, take a screenshot \
  (`pencil_get_screenshot` or `image_gen` for native artefacts) \
  and reference what changed. The parent agent uses these to \
  decide whether to ship or keep iterating.
- Respect existing files. Don't rename components, restructure \
  folders, or break import paths just because you find a better \
  arrangement. Surface the suggestion in your reply instead.
- Brand voice carries through. If the project has tone-of-voice \
  rules in MEMORY.md or a brand guide, copy and microcopy match \
  them — never default to generic SaaS prose.

Tooling map:

- Pencil tools (`pencil_*`) for the canonical design surface — \
  open documents, edit nodes in batches, search and replace \
  properties, capture screenshots. Use these when the project has \
  a registered Pencil MCP server.
- `image_gen` for raster artefacts and `canvas` for ad-hoc \
  composition when no Pencil server is available.
- `file_edit` / `file_write` to land HTML / CSS / SVG / Tailwind \
  classes once the design is approved.
- `web_fetch` to consult component libraries (shadcn, Radix, \
  Material) when the parent agent gestures at one.

Out of scope:

- Shipping changes to production code paths beyond the visual \
  layer the parent asked about. Hand routing, state, business \
  logic back up.
- Reorganising the design-system tokens themselves without an \
  explicit request. Suggest, don't act.";

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

    // ── Designer preset ─────────────────────────────────────────────

    #[test]
    fn designer_preset_uses_supplied_provider_and_model() {
        let cfg = designer_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn designer_preset_is_agentic() {
        let cfg = designer_preset("openrouter", "any/model");
        assert!(cfg.agentic, "designer must run as a multi-turn agent");
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn designer_preset_carries_a_system_prompt() {
        let cfg = designer_preset("openrouter", "any/model");
        let prompt = cfg
            .system_prompt
            .expect("designer preset must set a system prompt");
        for needle in [
            "designer sub-agent",
            "Tokens before pixels",
            "Accessibility is non-negotiable",
            "Iterate small",
        ] {
            assert!(
                prompt.contains(needle),
                "designer system prompt missing expected guidance: '{needle}'",
            );
        }
    }

    #[test]
    fn designer_preset_grants_visual_and_filesystem_tools() {
        let cfg = designer_preset("openrouter", "any/model");
        for required in [
            "pencil_open_document",
            "pencil_batch_design",
            "pencil_get_screenshot",
            "image_gen",
            "canvas",
            "file_edit",
            "file_write",
            "web_fetch",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "designer preset missing required tool: {required}",
            );
        }
    }

    #[test]
    fn designer_preset_does_not_grant_shell_or_git() {
        // The designer is a visual agent — opening shell semantics or
        // pushing git would be scope creep into the coder's lane.
        let cfg = designer_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "designer preset must not include {forbidden} — that's the coder's surface",
            );
        }
    }

    #[test]
    fn designer_preset_higher_temperature_than_coder() {
        let coder = coder_preset("openrouter", "x").temperature.unwrap();
        let designer = designer_preset("openrouter", "x").temperature.unwrap();
        assert!(
            designer > coder,
            "designer should have a higher temperature than coder \
             (creative iteration vs deterministic codegen), got designer={designer} coder={coder}",
        );
    }

    #[test]
    fn designer_preset_isolated_memory_namespace() {
        let cfg = designer_preset("openrouter", "any/model");
        let ns = cfg
            .memory_namespace
            .expect("designer preset should isolate its memory");
        assert_eq!(ns, "designer");
    }

    #[test]
    fn designer_preset_no_api_key_baked_in() {
        let cfg = designer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
