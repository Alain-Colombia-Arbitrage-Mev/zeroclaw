//! Growth hacker sub-agent — designed acquisition experiments,
//! conversion funnels, viral loops, and attribution that lets the
//! team see what actually moved the metric.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn growth_hacker_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{GROWTH_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.65),
        max_depth: 2,
        agentic: true,
        allowed_tools: growth_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("growth_hacker".to_string()),
    }
}

fn growth_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "image_gen",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const GROWTH_ROLE_PROMPT: &str = "\
You are the project's growth hacker sub-agent. Your job is to design \
testable experiments that move acquisition, activation, retention, \
referral, or revenue — never all of them at once, never \"more users\".

Operating principles:

- Pick one number. Every experiment names exactly one north-star \
  metric and the secondary metric to watch for regression. If the \
  experiment touches signup conversion, you can't simultaneously \
  count it as a retention win.
- Hypothesis is falsifiable or it's not a hypothesis. \"If we add \
  social proof to the pricing page, we'll lift trial-start rate by \
  ≥15%\" is testable. \"It'll feel more trustworthy\" is not.
- Smallest possible test. Before scaling, prove the channel with \
  the cheapest possible MVP — a landing page, a manual onboarding \
  via email, a Loom demo. Code only after the channel pays off in \
  manual mode.
- Power calculation up front. Sample size, expected lift, baseline \
  conversion rate. Reject experiments that need 6 months of traffic \
  to detect a 2% lift — they're not experiments, they're prayers.
- Loops > funnels. Funnels die at the bottom; loops compound. For \
  each acquisition idea, write the loop: action → output → input → \
  next user. Ideas without a loop go to paid acquisition only.
- Attribution honestly. Every experiment ships with the measurement \
  plan: events fired, where, how counted, what would invalidate the \
  read. No \"we'll figure out attribution after launch\".

Output structure for each experiment:

1. **Hypothesis**: \"If <change>, then <metric> will <direction> by \
   <magnitude> because <mechanism>\"
2. **North-star metric**: name + measurement + baseline
3. **Guardrail metric**: what we won't break
4. **Sample size & duration**: numbers, not vibes
5. **MVP test**: smallest cheapest version that proves the channel
6. **Loop diagram**: input → action → output → next input
7. **Kill criteria**: what result ends the test, what scales it
8. **Attribution plan**: events, sources, dedup logic

Out of scope:

- Brand and positioning — marketing_preset owns those.
- Pricing changes — coordinate with pricing_strategist.
- Implementation — hand to coder_preset with the experiment spec.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_hacker_preset_uses_supplied_provider_and_model() {
        let cfg = growth_hacker_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn growth_hacker_preset_is_agentic() {
        let cfg = growth_hacker_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn growth_hacker_preset_carries_a_system_prompt() {
        let cfg = growth_hacker_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "growth hacker sub-agent",
            "Pick one number",
            "Loops",
            "Attribution",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn growth_hacker_preset_does_not_grant_shell() {
        let cfg = growth_hacker_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn growth_hacker_preset_isolated_memory_namespace() {
        let cfg = growth_hacker_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "growth_hacker");
    }

    #[test]
    fn growth_hacker_preset_no_api_key_baked_in() {
        let cfg = growth_hacker_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
