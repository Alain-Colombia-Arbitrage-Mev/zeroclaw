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
//! roles ZeroClaw expects to recur. The catalogue covers the full
//! development pipeline plus the surrounding GTM / research / quality
//! roles a project usually grows into:
//!
//! Engineering pipeline:
//! - `coder_preset`            — small, reviewable diffs
//! - `designer_preset`         — UI / visual artefacts in project tokens
//! - `reviewer_preset`         — read-only code review, categorized
//! - `tester_preset`           — failing-test-first, edge coverage
//! - `qa_preset`               — integration + E2E + release readiness
//! - `cicd_preset`             — pipelines, pinned actions, verified runs
//! - `devops_preset`           — IaC, K8s, observability, deploy safety
//! - `docs_preset`             — answer-led docs with verified samples
//! - `planner_preset`          — sized, sequenced tasks
//! - `architect_preset`        — system boundaries, NFRs, diagrams
//! - `server_architect_preset` — server-side topology + capacity
//! - `db_designer_preset`      — schema + paired forward/rollback migrations
//! - `adr_writer_preset`       — Michael Nygard ADRs
//! - `security_preset`         — threat model + OWASP-grade audit
//!
//! GTM / content:
//! - `marketing_preset`        — strategy, GTM, media plan
//! - `content_creator_preset`  — channel-ready copy
//! - `scriptwriter_preset`     — video / audio / podcast scripts
//! - `market_researcher_preset` — sized, cited market research
//!
//! Operators may use them three ways:
//!
//! 1. **Copy the equivalent TOML** from `docs/book/src/setup/<role>-agent.md`
//!    into their own config — fully transparent, no surprise behaviour.
//! 2. **Call the builder from extension code** to register the agent
//!    programmatically (`reviewer_preset(provider, model)`).
//! 3. **Override fields after building** — the returned struct is plain
//!    `DelegateAgentConfig`, every field is `pub`.
//!
//! Every preset prepends a shared `SENIOR_PREAMBLE` (see
//! `common.rs`) and includes the Context7 MCP tool pair
//! (`context7__resolve-library-id`, `context7__get-library-docs`)
//! in its allowlist so role-specific guidance can be cross-checked
//! against current documentation rather than the model's training
//! cutoff. Tools that don't exist at runtime are silently dropped
//! from the agent's effective surface — listing them is harmless
//! when the operator hasn't registered the Context7 server yet.
//!
//! The presets do NOT change ZeroClaw's defaults: an out-of-the-box
//! daemon with no `[agents.*]` section still has zero registered
//! sub-agents. This module is opt-in.

mod adr_writer;
mod architect;
mod cicd;
mod coder;
mod common;
mod content_creator;
mod db_designer;
mod designer;
mod devops;
mod docs;
mod market_researcher;
mod marketing;
mod planner;
mod qa;
mod reviewer;
mod scriptwriter;
mod security;
mod server_architect;
mod tester;

pub use self::adr_writer::adr_writer_preset;
pub use self::architect::architect_preset;
pub use self::cicd::cicd_preset;
pub use self::coder::coder_preset;
pub use self::content_creator::content_creator_preset;
pub use self::db_designer::db_designer_preset;
pub use self::designer::designer_preset;
pub use self::devops::devops_preset;
pub use self::docs::docs_preset;
pub use self::market_researcher::market_researcher_preset;
pub use self::marketing::marketing_preset;
pub use self::planner::planner_preset;
pub use self::qa::qa_preset;
pub use self::reviewer::reviewer_preset;
pub use self::scriptwriter::scriptwriter_preset;
pub use self::security::security_preset;
pub use self::server_architect::server_architect_preset;
pub use self::tester::tester_preset;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::DelegateAgentConfig;

    /// Single source of truth for cross-cutting tests: every preset
    /// gets a name and a freshly-built config. Adding a preset means
    /// adding one line here.
    fn all_presets() -> Vec<(&'static str, DelegateAgentConfig)> {
        vec![
            ("coder", coder_preset("openrouter", "x")),
            ("designer", designer_preset("openrouter", "x")),
            ("reviewer", reviewer_preset("openrouter", "x")),
            ("tester", tester_preset("openrouter", "x")),
            ("qa", qa_preset("openrouter", "x")),
            ("cicd", cicd_preset("openrouter", "x")),
            ("devops", devops_preset("openrouter", "x")),
            ("docs", docs_preset("openrouter", "x")),
            ("planner", planner_preset("openrouter", "x")),
            ("architect", architect_preset("openrouter", "x")),
            ("server_architect", server_architect_preset("openrouter", "x")),
            ("db_designer", db_designer_preset("openrouter", "x")),
            ("adr_writer", adr_writer_preset("openrouter", "x")),
            ("security", security_preset("openrouter", "x")),
            ("marketing", marketing_preset("openrouter", "x")),
            ("content_creator", content_creator_preset("openrouter", "x")),
            ("scriptwriter", scriptwriter_preset("openrouter", "x")),
            ("market_researcher", market_researcher_preset("openrouter", "x")),
        ]
    }

    /// Every preset must expose the Context7 MCP tools so role
    /// guidance can be cross-checked against current docs.
    #[test]
    fn every_preset_includes_context7_tools() {
        for (name, cfg) in all_presets() {
            for required in ["context7__resolve-library-id", "context7__get-library-docs"] {
                assert!(
                    cfg.allowed_tools.iter().any(|t| t == required),
                    "preset `{name}` is missing Context7 tool `{required}`",
                );
            }
        }
    }

    /// Every preset's system prompt must carry the senior preamble so
    /// the role is anchored on current docs rather than training
    /// memory.
    #[test]
    fn every_preset_carries_senior_preamble() {
        for (name, cfg) in all_presets() {
            let prompt = cfg
                .system_prompt
                .unwrap_or_else(|| panic!("`{name}` has no system prompt"));
            assert!(
                prompt.contains("senior level"),
                "preset `{name}` is missing the senior preamble",
            );
            assert!(
                prompt.contains("context7__resolve-library-id"),
                "preset `{name}` does not name Context7 in its preamble",
            );
        }
    }

    /// Memory namespaces must be unique so cross-role recall doesn't
    /// pollute one specialist's context with another's.
    #[test]
    fn every_preset_has_unique_memory_namespace() {
        let mut namespaces: Vec<String> = all_presets()
            .into_iter()
            .map(|(name, cfg)| {
                cfg.memory_namespace
                    .unwrap_or_else(|| panic!("`{name}` has no memory namespace"))
            })
            .collect();
        namespaces.sort();
        let total = namespaces.len();
        namespaces.dedup();
        assert_eq!(
            namespaces.len(),
            total,
            "memory namespaces collide — every preset must isolate its memory",
        );
    }

    /// Every preset must be agentic with at least a moderate iteration
    /// budget; non-agentic / tiny-budget presets defeat the point of
    /// shipping a role default.
    #[test]
    fn every_preset_is_agentic_with_workable_budget() {
        for (name, cfg) in all_presets() {
            assert!(cfg.agentic, "preset `{name}` must be agentic");
            assert!(
                cfg.max_iterations >= 8,
                "preset `{name}` has too small an iteration budget: {}",
                cfg.max_iterations,
            );
        }
    }

    /// No preset bakes in an api key — keys must come from the
    /// operator's environment / config, never from the preset itself.
    #[test]
    fn no_preset_bakes_in_an_api_key() {
        for (name, cfg) in all_presets() {
            assert!(
                cfg.api_key.is_none(),
                "preset `{name}` ships with an api_key — must be operator-supplied",
            );
        }
    }
}
