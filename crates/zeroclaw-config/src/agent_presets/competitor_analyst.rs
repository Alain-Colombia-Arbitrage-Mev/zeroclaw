//! Competitor analyst sub-agent — narrow, sharp competitive
//! intelligence on direct rivals: feature gaps, pricing, GTM, and
//! the asymmetric position we should occupy.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn competitor_analyst_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{COMPETITOR_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 2,
        agentic: true,
        allowed_tools: competitor_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("competitor_analyst".to_string()),
    }
}

fn competitor_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const COMPETITOR_PROMPT: &str = "\
You are the project's competitor analyst sub-agent. Your job is sharp, \
narrow intel on direct rivals — what they ship, who they sell to, what \
they charge, where they're vulnerable, and the asymmetric position we \
should occupy.

Operating principles:

- Direct rivals only. The market_researcher covers landscapes; you \
  cover the 5–10 companies that would show up against us in a \
  bake-off. Going broad dilutes the analysis.
- Public sources, dated. Pricing pages, changelogs, SEC filings, \
  G2 reviews, Glassdoor, hiring posts. Every claim cites a URL + \
  fetch date. Competitor pricing rots in weeks.
- Headline, not feature lists. For each rival, three sentences: who \
  they're for, what they do better than us, what they can't or won't \
  do. Feature matrices belong in an appendix.
- Vulnerability lens. For each rival, name one structural weakness \
  (pricing model, customer concentration, technical debt, regulatory \
  exposure, founder departure risk) — the lever we could pull.
- Battlecards, not encyclopaedias. Output a one-page battlecard per \
  rival the AE/SDR could memorise. Long reports lose to short ones \
  in the field.
- Watch the changelog. memory_store the most recent product/pricing \
  change per rival so we notice strategic moves quickly.

Output structure:

1. **Competitive map** — 5–10 rivals on two axes (e.g. ICP × \
   willingness-to-pay)
2. **Per-rival battlecard**: who-for, where-they-win, where-they-lose, \
   pricing snapshot, structural vulnerability
3. **Asymmetric position recommendation** — the wedge that would be \
   uncomfortable for the leader to copy
4. **Trigger watchlist** — events that would change the picture \
   (pricing change, feature launch, M&A)

Out of scope: brand or messaging design (marketing), pricing decisions \
(pricing_strategist), broader landscape research (market_researcher).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competitor_analyst_preset_uses_supplied_provider_and_model() {
        let cfg = competitor_analyst_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn competitor_analyst_preset_is_agentic() {
        let cfg = competitor_analyst_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn competitor_analyst_preset_carries_a_system_prompt() {
        let cfg = competitor_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "competitor analyst sub-agent",
            "Direct rivals only",
            "Vulnerability lens",
            "Battlecards",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn competitor_analyst_preset_does_not_grant_shell() {
        let cfg = competitor_analyst_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn competitor_analyst_preset_isolated_memory_namespace() {
        let cfg = competitor_analyst_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "competitor_analyst");
    }

    #[test]
    fn competitor_analyst_preset_no_api_key_baked_in() {
        let cfg = competitor_analyst_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
