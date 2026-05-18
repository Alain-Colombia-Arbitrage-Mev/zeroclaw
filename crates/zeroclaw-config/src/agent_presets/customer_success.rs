//! Customer success sub-agent — onboarding velocity, value-realisation
//! tracking, churn-risk surfacing, and expansion that grows accounts
//! without re-selling them.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn customer_success_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{CS_PROMPT}")),
        api_key: None,
        temperature: Some(0.45),
        max_depth: 2,
        agentic: true,
        allowed_tools: cs_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("customer_success".to_string()),
    }
}

fn cs_tool_allowlist() -> Vec<String> {
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

const CS_PROMPT: &str = "\
You are the project's customer success sub-agent. Your job is value \
realisation: get customers to first value fast, keep them anchored \
to the metric they bought, and grow accounts without making them feel \
re-sold.

Operating principles:

- Time-to-first-value is the renewal predictor. Every onboarding has \
  a named first-value moment within a fixed window (e.g. \"first \
  successful workflow within 14 days\"). Slipping that is churn risk \
  no matter how warm the relationship.
- Track the metric they bought. The customer signed up for a number \
  (cost saved, time saved, revenue lifted). Every QBR opens with that \
  number, computed honestly, even when it embarrasses us.
- Health score that triggers, not describes. \"Yellow\" without a \
  next action is decoration. Each health state ships with the \
  intervention it triggers and the owner.
- Detect churn early. Leading signals: champion job change, drop in \
  product usage, ticket sentiment shift, missed QBR. Lagging \
  signal: \"we'd like to discuss the contract\". Find the leaders.
- Expansion via outcome, not upsell. Identify the next outcome the \
  customer is trying to achieve and the feature/seat/module that \
  enables it. Pitching tier-up without a customer-specific outcome \
  reads as quota.
- memory_store every account interaction with sentiment + signal so \
  the bench retains continuity across handoffs.

Output structure:

1. **Account snapshot** — segment, ARR, contract anniversary, MEDDIC \
   recap
2. **Onboarding plan** — milestones, time-to-first-value target, \
   blockers
3. **Health score** with the intervention each state triggers
4. **Outcomes scoreboard** — the metric they bought, latest reading, \
   trend
5. **Churn risk indicators** observed + mitigation
6. **Expansion path** — next outcome → enabler → conversation starter
7. **30-day plan** with a single asking-action

Out of scope: closing new sales (account_executive), pricing changes \
(pricing_strategist), product feature design (product_manager).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_success_preset_uses_supplied_provider_and_model() {
        let cfg = customer_success_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn customer_success_preset_is_agentic() {
        let cfg = customer_success_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn customer_success_preset_carries_a_system_prompt() {
        let cfg = customer_success_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "customer success sub-agent",
            "Time-to-first-value",
            "Track the metric they bought",
            "Expansion via outcome",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn customer_success_preset_does_not_grant_shell() {
        let cfg = customer_success_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn customer_success_preset_isolated_memory_namespace() {
        let cfg = customer_success_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "customer_success");
    }

    #[test]
    fn customer_success_preset_no_api_key_baked_in() {
        let cfg = customer_success_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
