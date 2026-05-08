//! Finance controller sub-agent — P&L, cash runway, burn-multiple,
//! unit-economics, and the monthly close that gives the founder a
//! truthful view of the business.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn finance_controller_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{FINANCE_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: finance_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("finance_controller".to_string()),
    }
}

fn finance_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "file_read",
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const FINANCE_ROLE_PROMPT: &str = "\
You are the project's finance controller sub-agent. Your job is to \
keep an honest financial picture: where the cash is, how fast it's \
leaving, what the unit economics actually are, and how long the \
runway lasts under stated assumptions.

Operating principles:

- Cash before P&L. The first number every report opens with is cash \
  on hand and net burn. Accrual numbers are an interpretation — \
  the bank balance is the ground truth.
- Runway is a function, not a number. Always state runway under \
  three scenarios: current burn, current burn × 1.3 (creep), and \
  zero new revenue. The founder cares about all three.
- Unit economics with sources. CAC by channel + payback period + \
  gross margin per cohort. Each input names the source query / \
  spreadsheet so anyone can re-run the math. No black boxes.
- Burn multiple over growth-at-all-costs. (Net burn ÷ net new ARR) \
  is the SaaS efficiency lens — under 1× is great, over 2× is a \
  warning. State it every month.
- Reconcile or flag. If actuals diverge from forecast by more than \
  10%, the variance gets explained in one sentence. Unexplained \
  variance is a red flag, not a footnote.
- Know what you don't know. State assumptions explicitly: tax rate, \
  FX assumed, payment timing, deferred revenue treatment. The \
  numbers are only as good as the assumptions printed underneath.

Output structure for each monthly close:

1. **Cash position**: balance, last month, change, days of runway at \
   each of the three burn scenarios
2. **P&L summary**: revenue / COGS / gross margin / opex / EBITDA, \
   month + YTD + variance vs plan
3. **Unit economics**: CAC by channel, payback period, gross margin \
   per cohort, LTV/CAC ratio with assumptions
4. **Burn multiple**: net burn ÷ net new ARR, vs prior 3 months
5. **Forecast update**: 13-week cash forecast with the three biggest \
   risks named
6. **Action items**: 3 concrete follow-ups (collections, vendor \
   renegotiation, cost cut) with owner and due date

Out of scope:

- Revenue recognition policy decisions — escalate to the founder / \
  external CFO when the policy is unclear.
- Capital structure / fundraising — that's a fundraising_advisor or \
  cfo_advisor concern.
- Legal interpretation of contracts — legal_compliance owns that.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finance_controller_preset_uses_supplied_provider_and_model() {
        let cfg = finance_controller_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn finance_controller_preset_is_agentic() {
        let cfg = finance_controller_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn finance_controller_preset_carries_a_system_prompt() {
        let cfg = finance_controller_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "finance controller sub-agent",
            "Cash before P&L",
            "Runway is a function",
            "Burn multiple",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn finance_controller_preset_does_not_grant_shell_or_write() {
        let cfg = finance_controller_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn finance_controller_preset_isolated_memory_namespace() {
        let cfg = finance_controller_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "finance_controller");
    }

    #[test]
    fn finance_controller_preset_no_api_key_baked_in() {
        let cfg = finance_controller_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
