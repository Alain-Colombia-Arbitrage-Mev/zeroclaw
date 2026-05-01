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
//! roles ZeroClaw expects to recur. Today the catalogue covers a full
//! development pipeline:
//!
//! - `coder_preset`       — small, reviewable diffs that pass checks
//! - `designer_preset`    — UI / visual artefacts in the project's tokens
//! - `reviewer_preset`    — read-only code review with categorized concerns
//! - `tester_preset`      — failing-test-first, edge-case coverage
//! - `cicd_preset`        — pipelines with pinned actions + verified runs
//! - `docs_preset`        — answer-led docs with verified samples
//! - `planner_preset`     — sized, sequenced task lists with risks
//! - `architect_preset`   — system boundaries, NFRs, diagrams
//! - `db_designer_preset` — schema + migrations forward and back
//! - `adr_writer_preset`  — Michael Nygard ADRs, one decision each
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
mod db_designer;
mod designer;
mod docs;
mod planner;
mod reviewer;
mod tester;

pub use self::adr_writer::adr_writer_preset;
pub use self::architect::architect_preset;
pub use self::cicd::cicd_preset;
pub use self::coder::coder_preset;
pub use self::db_designer::db_designer_preset;
pub use self::designer::designer_preset;
pub use self::docs::docs_preset;
pub use self::planner::planner_preset;
pub use self::reviewer::reviewer_preset;
pub use self::tester::tester_preset;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every preset must expose the Context7 MCP tools so role
    /// guidance can be cross-checked against current docs.
    #[test]
    fn every_preset_includes_context7_tools() {
        let presets = [
            ("coder", coder_preset("openrouter", "x").allowed_tools),
            ("designer", designer_preset("openrouter", "x").allowed_tools),
            ("reviewer", reviewer_preset("openrouter", "x").allowed_tools),
            ("tester", tester_preset("openrouter", "x").allowed_tools),
            ("cicd", cicd_preset("openrouter", "x").allowed_tools),
            ("docs", docs_preset("openrouter", "x").allowed_tools),
            ("planner", planner_preset("openrouter", "x").allowed_tools),
            ("architect", architect_preset("openrouter", "x").allowed_tools),
            ("db_designer", db_designer_preset("openrouter", "x").allowed_tools),
            ("adr_writer", adr_writer_preset("openrouter", "x").allowed_tools),
        ];
        for (name, tools) in presets {
            for required in ["context7__resolve-library-id", "context7__get-library-docs"] {
                assert!(
                    tools.iter().any(|t| t == required),
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
        let presets = [
            ("coder", coder_preset("openrouter", "x").system_prompt),
            ("designer", designer_preset("openrouter", "x").system_prompt),
            ("reviewer", reviewer_preset("openrouter", "x").system_prompt),
            ("tester", tester_preset("openrouter", "x").system_prompt),
            ("cicd", cicd_preset("openrouter", "x").system_prompt),
            ("docs", docs_preset("openrouter", "x").system_prompt),
            ("planner", planner_preset("openrouter", "x").system_prompt),
            ("architect", architect_preset("openrouter", "x").system_prompt),
            ("db_designer", db_designer_preset("openrouter", "x").system_prompt),
            ("adr_writer", adr_writer_preset("openrouter", "x").system_prompt),
        ];
        for (name, prompt) in presets {
            let prompt = prompt.unwrap_or_else(|| panic!("`{name}` has no system prompt"));
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
        let mut namespaces: Vec<String> = vec![
            coder_preset("openrouter", "x").memory_namespace.unwrap(),
            designer_preset("openrouter", "x").memory_namespace.unwrap(),
            reviewer_preset("openrouter", "x").memory_namespace.unwrap(),
            tester_preset("openrouter", "x").memory_namespace.unwrap(),
            cicd_preset("openrouter", "x").memory_namespace.unwrap(),
            docs_preset("openrouter", "x").memory_namespace.unwrap(),
            planner_preset("openrouter", "x").memory_namespace.unwrap(),
            architect_preset("openrouter", "x").memory_namespace.unwrap(),
            db_designer_preset("openrouter", "x").memory_namespace.unwrap(),
            adr_writer_preset("openrouter", "x").memory_namespace.unwrap(),
        ];
        namespaces.sort();
        let total = namespaces.len();
        namespaces.dedup();
        assert_eq!(
            namespaces.len(),
            total,
            "memory namespaces collide — every preset must isolate its memory",
        );
    }
}
