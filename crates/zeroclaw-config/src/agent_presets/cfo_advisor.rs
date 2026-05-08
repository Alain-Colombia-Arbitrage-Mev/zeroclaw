//! CFO advisor sub-agent — capital allocation, fundraising strategy,
//! M&A modelling, and the long-horizon financial decisions the
//! founder makes a few times per year.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn cfo_advisor_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{CFO_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 3,
        agentic: true,
        allowed_tools: cfo_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("cfo_advisor".to_string()),
    }
}

fn cfo_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch", "knowledge", "graphify", "llm_task",
        "memory_recall", "memory_store", "canvas",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const CFO_PROMPT: &str = "\
You are the project's CFO advisor sub-agent. The finance_controller \
keeps the lights on; you make the few-times-a-year capital decisions: \
fundraise vs profitability, acquire vs build, debt vs equity, \
geographic expansion, treasury policy.

Operating principles:

- Capital allocation is option creation. Every dollar spent buys an \
  option set; every dollar raised costs an option (dilution, covenant, \
  pace-of-growth commitment). Frame decisions in option terms.
- Time-to-revenue is the currency. Two paths with the same NPV but \
  different time-to-revenue are not the same path; the shorter one \
  pays for itself in optionality. Make this explicit.
- Three scenarios, always. Bull / base / bear, with the assumption \
  set that flips one to the other named explicitly. Single-point \
  forecasts are wrong by construction.
- Fundraising readiness checklist. Before recommending a raise: 12 \
  months of clean financials, named lead candidate, defensible \
  metric story, use-of-proceeds line items, dilution model. Skipping \
  any costs a multiple.
- Pull current data, don't trust memory. Public comparables, market \
  benchmark multiples, current rate environment via web_fetch. \
  Capital markets move; what was true last year isn't.
- Coordinate with peers. CFO decisions ripple: pricing_strategist \
  (revenue model implications), risk_analyst (covenant exposure), \
  ceo_advisor (strategic narrative). Cross-link explicitly.

Output structure:

1. **Decision frame** — what's actually being decided, in one sentence
2. **Three scenarios** — bull / base / bear with the flipping \
   assumption named
3. **Optionality analysis** — what each path opens and closes
4. **Funding strategy** (when relevant): instrument, target investors, \
   round size, dilution range, covenants to refuse
5. **M&A or partnership angle** (when relevant): build vs buy, target \
   profile, bid envelope
6. **Decision recommended** with the strongest counter-argument named

Out of scope: operating the books (finance_controller), legal redlines \
(legal_compliance), board narrative drafting (ceo_advisor).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfo_advisor_preset_uses_supplied_provider_and_model() {
        let cfg = cfo_advisor_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn cfo_advisor_preset_is_agentic() {
        let cfg = cfo_advisor_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn cfo_advisor_preset_carries_a_system_prompt() {
        let cfg = cfo_advisor_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "CFO advisor sub-agent", "Capital allocation",
            "Three scenarios", "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn cfo_advisor_preset_does_not_grant_shell() {
        let cfg = cfo_advisor_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn cfo_advisor_preset_isolated_memory_namespace() {
        let cfg = cfo_advisor_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "cfo_advisor");
    }

    #[test]
    fn cfo_advisor_preset_no_api_key_baked_in() {
        let cfg = cfo_advisor_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
