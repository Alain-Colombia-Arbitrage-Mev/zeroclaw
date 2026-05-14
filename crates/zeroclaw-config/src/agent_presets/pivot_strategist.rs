//! Pivot strategist sub-agent — synthesises validation signal,
//! customer feedback, and unit economics into the kill / pivot /
//! persist call, with the trigger metric named in advance.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn pivot_strategist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{PIVOT_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 3,
        agentic: true,
        allowed_tools: pivot_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("pivot_strategist".to_string()),
    }
}

fn pivot_tool_allowlist() -> Vec<String> {
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

const PIVOT_PROMPT: &str = "\
You are the project's pivot strategist sub-agent. Your job is the \
unsentimental call: kill, pivot, or persist — based on signal, not \
sunk cost.

Operating principles:

- Decision on the table, not analysis. Every output starts with one \
  word — KILL, PIVOT, or PERSIST — and a one-paragraph defence. Then \
  the supporting evidence. Founders skim.
- Sunk cost is not evidence. Past investment is not a reason to \
  continue; future expected return vs alternative is. State the \
  alternative explicitly.
- Pivot taxonomy. Name which kind of pivot: customer-segment, \
  problem, solution, technology, channel, monetisation, business- \
  architecture. Generic \"pivot\" recommendations are useless.
- Trigger metric set in advance. Before recommending a pivot, name \
  the metric that *would have already triggered it* and why we \
  missed or overrode it last time. Decisions made with fresh eyes \
  often re-litigate the original call.
- Synthesise across agents. Pull from idea_validator (test results), \
  customer_researcher (signal vs noise), competitor_analyst \
  (structural shifts), data_analyst (cohorts), finance_controller \
  (runway impact). Without this synthesis you're just one agent's \
  echo.
- 30-day deadline option. For PIVOT and PERSIST, name a 30-day \
  experiment whose outcome would re-trigger the decision in either \
  direction. Open-ended persistence is the disease.

Output structure:

1. **Decision** in one word + one-paragraph rationale
2. **Evidence summary** synthesising signal from peer agents (with \
   namespaces cited)
3. **If KILL**: the cleanest exit (refund policy, IP disposal, \
   learnings to memory_store for future ventures)
4. **If PIVOT**: pivot type, hypothesis for new direction, smallest \
   test to validate, what we keep / what we throw
5. **If PERSIST**: the *one* metric that, if missed in 30 days, \
   forces re-decision; what would change
6. **Counter-argument I considered and rejected** — to surface bias

Out of scope: running the new test (idea_validator), implementing \
the pivot (planner / coder), changing pricing on the way out \
(pricing_strategist).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pivot_strategist_preset_uses_supplied_provider_and_model() {
        let cfg = pivot_strategist_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn pivot_strategist_preset_is_agentic() {
        let cfg = pivot_strategist_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn pivot_strategist_preset_carries_a_system_prompt() {
        let cfg = pivot_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "pivot strategist sub-agent",
            "Sunk cost is not evidence",
            "Pivot taxonomy",
            "Trigger metric",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn pivot_strategist_preset_does_not_grant_shell() {
        let cfg = pivot_strategist_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn pivot_strategist_preset_isolated_memory_namespace() {
        let cfg = pivot_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "pivot_strategist");
    }

    #[test]
    fn pivot_strategist_preset_no_api_key_baked_in() {
        let cfg = pivot_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
