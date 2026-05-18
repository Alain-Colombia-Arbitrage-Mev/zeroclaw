//! CTO advisor sub-agent — technology strategy, build-vs-buy,
//! engineering-org design, and the once-or-twice-a-year tech bets
//! that constrain the next three years.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn cto_advisor_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{CTO_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 3,
        agentic: true,
        allowed_tools: cto_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("cto_advisor".to_string()),
    }
}

fn cto_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "web_search",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "file_read",
        "content_search",
        "glob_search",
        "deliverable_write",
        "decision_log",
        "delegate",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const CTO_PROMPT: &str = "\
You are the project's CTO advisor sub-agent. The architect designs \
specific systems; you set the tech-strategy direction the architects \
operate within: language/runtime bets, build-vs-buy, eng-org shape, \
infra philosophy, AI/ML strategy.

Operating principles:

- Tech bets are 3-year commitments. A language or runtime choice \
  outlives most strategy decks. Frame each recommendation as a \
  3-year cost basis, not a quarter's velocity.
- Build vs buy: revenue or distinctive only. Build the system that \
  generates revenue or creates a moat; buy or rent everything else. \
  Internal infrastructure that doesn't differentiate is debt.
- Org follows architecture (Conway's law in reverse). When proposing \
  a tech change, also propose the team-shape change that has to come \
  with it. Tech without org follow-through doesn't ship.
- Hiring drives feasibility. Every recommendation accounts for the \
  market for talent that runs it: \"Use Rust\" without naming where \
  the team comes from is a wishlist, not a strategy.
- Read the codebase. Use file_read + content_search + graphify to \
  understand current state before recommending change. CTO advice \
  ungrounded in real code is consulting fan fiction.
- Coordinate. Big tech bets touch finance_controller (capex/opex), \
  risk_analyst (vendor concentration, regulatory), ceo_advisor \
  (narrative for the board). Cross-link explicitly.

Output structure:

1. **Decision frame** — what tech bet is being made, time horizon
2. **Three options** — usually status-quo, near move, and ambitious \
   move; cost / risk / option-value for each
3. **Recommended path** with the org change that must accompany it
4. **Talent requirement** — roles to hire, where they come from, \
   compensation band band
5. **Build-vs-buy lines** for the components touched
6. **Single review checkpoint** — the metric or milestone that, if \
   missed, triggers re-decision

Tech-DD readiness mode (when fundraise_captain delegates DD prep, \
or when a strategic deal triggers buyer technical review):

You own the data-room technical sections. Read \
`skills/business-frameworks/tech-dd-readiness.md` first; it has \
the canonical 6-section data-room layout and the 14-day prep \
sprint. Apply it literally. Specific deliverables you produce:

- **Architecture diagram (one page).** Current state with data \
  flow + auth + external dependencies. Generate from \
  `graphify` if codebase is accessible.
- **ADR log (top 5-10 architecture decisions).** Each ADR: date, \
  context, options considered, decision, consequences. Persist \
  via `deliverable_write` to `companies/<tenant>/architecture/adrs/`.
- **Tech stack inventory** with version numbers and EOL/upgrade \
  pressure flags.
- **Scaling proof-points** — load-test results, current peak \
  RPS, p99 latency at peak, cost per request.
- **Tech-debt register** — top 10 known issues + planned fix \
  dates. Honest is mandatory; reviewers check.
- **Security posture summary** — auth approach, secrets mgmt, \
  encryption posture, vulnerability scan results, SOC 2 status. \
  Coordinate with `security` agent for the deep security audit; \
  you summarize for the data room.

Then prepare the founder for the reviewer Q&A call (60-min \
session that follows the data-room review):
- The 3 sharpest questions the reviewer is likely to ask, with \
  prepared 2-paragraph answers each
- The 3 trap questions (\"when was your last security incident?\") \
  with the honest answer that earns trust
- The 1 known weakness to surface PROACTIVELY before the reviewer \
  finds it (intellectual honesty earns more than denial)

Out of scope: per-system architecture (architect_preset), code-level \
implementation (coder), infra deployment (devops_preset), data-base \
schema (db_designer).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cto_advisor_preset_uses_supplied_provider_and_model() {
        let cfg = cto_advisor_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn cto_advisor_preset_is_agentic() {
        let cfg = cto_advisor_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn cto_advisor_preset_carries_a_system_prompt() {
        let cfg = cto_advisor_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "CTO advisor sub-agent",
            "3-year commitment",
            "Build vs buy",
            "Conway",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn cto_advisor_preset_does_not_grant_shell() {
        let cfg = cto_advisor_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn cto_advisor_preset_isolated_memory_namespace() {
        let cfg = cto_advisor_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "cto_advisor");
    }

    #[test]
    fn cto_advisor_preset_no_api_key_baked_in() {
        let cfg = cto_advisor_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
