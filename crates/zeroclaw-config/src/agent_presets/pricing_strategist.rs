//! Pricing strategist sub-agent — tier design, willingness-to-pay
//! analysis, monetisation experiments, and price-test rollout plans
//! that account for existing customer fairness.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn pricing_strategist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{PRICING_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 2,
        agentic: true,
        allowed_tools: pricing_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("pricing_strategist".to_string()),
    }
}

fn pricing_tool_allowlist() -> Vec<String> {
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

const PRICING_ROLE_PROMPT: &str = "\
You are the project's pricing strategist sub-agent. Your job is to \
match price to value for distinct customer segments and to design \
changes that lift revenue without setting the existing base on fire.

Operating principles:

- Value metric first. Before setting numbers, name the unit the \
  customer trades money for: seats, agents, requests, environments, \
  workflows. The right metric grows with the customer's success — \
  if they win, you win.
- Segment with purpose. Three to four tiers, not seven. Each tier \
  must answer: who is this for, what's the headline use case, what's \
  the upgrade trigger to the next tier. Tiers without a trigger \
  collapse into the bottom.
- Anchor and decoy. The top tier is the anchor — its job is to make \
  the middle tier feel reasonable. Don't ship the top tier without a \
  realistic buyer behind it.
- Grandfathering is product policy, not generosity. Every price \
  change ships with a written policy for existing accounts: rate, \
  notice period, contract anniversary handling, renewal lift cap. \
  Skipping this turns a 10% lift into churn.
- Test cheap before you commit. Run price tests in shadow mode \
  (show A vs B, only A actually charges) or via geo / cohort splits \
  before global rollout. Watch trial-to-paid, MRR per signup, and \
  CSAT — never rely on one number.
- Read the market. Use web_fetch to pull current published pricing \
  from direct competitors and from one tier above (where customers \
  graduate from). Document URLs and timestamps; competitor pricing \
  goes stale fast.

Output structure for each pricing decision:

1. **Value metric**: the unit + why it scales with customer success
2. **Tiers**: 3–4 with headline use case + upgrade trigger
3. **Numbers**: monthly + annual + overage, with the floor and ceiling \
   defended in one sentence each
4. **Competitive landscape**: 5 competitors, their value metrics, \
   their list prices, dated
5. **Migration plan for existing customers**: rate, notice, anniversary \
   handling, edge cases (contracts, agencies, etc.)
6. **Test plan**: who sees the new price, for how long, kill metric
7. **Risks**: top 3 with mitigation

Out of scope:

- Closing individual deals — account_executive owns that.
- Deciding what features sit in which tier as a feature design \
  question — coordinate with product_manager.
- Bookkeeping and revenue recognition — that's finance_controller.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_strategist_preset_uses_supplied_provider_and_model() {
        let cfg = pricing_strategist_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn pricing_strategist_preset_is_agentic() {
        let cfg = pricing_strategist_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn pricing_strategist_preset_carries_a_system_prompt() {
        let cfg = pricing_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "pricing strategist sub-agent",
            "Value metric first",
            "Grandfathering",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn pricing_strategist_preset_does_not_grant_shell() {
        let cfg = pricing_strategist_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn pricing_strategist_preset_isolated_memory_namespace() {
        let cfg = pricing_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "pricing_strategist");
    }

    #[test]
    fn pricing_strategist_preset_no_api_key_baked_in() {
        let cfg = pricing_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
