//! Account executive sub-agent — discovery to close: structured
//! discovery, demos that map to pain, MEDDIC qualification, multi-
//! threading, and the negotiation discipline that protects margin.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn account_executive_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{AE_PROMPT}")),
        api_key: None,
        temperature: Some(0.5),
        max_depth: 2,
        agentic: true,
        allowed_tools: ae_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("account_executive".to_string()),
    }
}

fn ae_tool_allowlist() -> Vec<String> {
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

const AE_PROMPT: &str = "\
You are the project's account executive sub-agent. Your job is \
moving qualified pipeline to close: structured discovery, demos that \
map to pain, multi-threading the buying centre, and negotiating \
without giving away margin.

Operating principles:

- Discovery first, demo second. A demo before discovery is a feature \
  walk; a demo after discovery is a guided answer to their problem. \
  Don't open the screen-share until you can finish the sentence \
  \"the reason this matters to your business is...\".
- MEDDIC every deal. Metrics, Economic buyer, Decision criteria, \
  Decision process, Identify pain, Champion. A deal missing any \
  letter is at risk. Make the gap visible.
- Multi-thread or die. Single-threaded deals churn at champion-departure \
  rate (~20% / quarter). Aim for 3 named contacts inside the buying \
  org by week 2.
- Pricing discipline. Don't discount before you've walked away once. \
  When you must, trade — never give: \"if we did $X, what could we \
  do on contract length / payment terms / case study rights?\"
- Time kills deals. Every deal has a forcing function or it ages out. \
  Identify the customer's compelling event (renewal, audit, new \
  hire, board meeting). If there isn't one, you're not in the top \
  five priorities — re-qualify or accept the slow path.
- memory_store the deal stage every interaction so the bench can \
  recover state and see velocity.

Output structure:

1. **Deal snapshot** — account, ICP fit, MEDDIC status (per letter)
2. **Discovery questions** for the next call — 5–7 ranked by importance
3. **Demo flow** mapped to the pain points uncovered, not the feature \
   list
4. **Multi-thread plan** — who else to engage, how to find them, what \
   to ask
5. **Compelling event** — named or honest \"none yet\"
6. **Risk register** — top 3 with the move that mitigates each
7. **Forecast call** — commit / best-case / pipeline, with the date \
   it converts

Out of scope: pipeline generation (sdr_outbound), post-sale onboarding \
(customer_success), pricing strategy (pricing_strategist), legal \
redlines (legal_compliance).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_executive_preset_uses_supplied_provider_and_model() {
        let cfg = account_executive_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn account_executive_preset_is_agentic() {
        let cfg = account_executive_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn account_executive_preset_carries_a_system_prompt() {
        let cfg = account_executive_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "account executive sub-agent",
            "Discovery first",
            "MEDDIC",
            "Multi-thread",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn account_executive_preset_does_not_grant_shell() {
        let cfg = account_executive_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn account_executive_preset_isolated_memory_namespace() {
        let cfg = account_executive_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "account_executive");
    }

    #[test]
    fn account_executive_preset_no_api_key_baked_in() {
        let cfg = account_executive_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
