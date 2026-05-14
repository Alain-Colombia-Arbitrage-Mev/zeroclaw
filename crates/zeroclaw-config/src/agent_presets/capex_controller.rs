//! CapEx controller — gatekeeper for capital expenditure proposals.
//! Every above-threshold spend gets evaluated against ROI, payback,
//! and runway impact before approval.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn capex_controller_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{CAPEX_CONTROLLER_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: capex_controller_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(540),
        skills_directory: None,
        memory_namespace: Some("capex_controller".to_string()),
    }
}

fn capex_controller_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "kpi_record",
        "entity_upsert",
        "decision_log",
        "deliverable_write",
        "company_manifest",
        "memory_recall",
        "memory_store",
        "knowledge",
        "llm_task",
        "file_read",
        "glob_search",
        "web_fetch",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const CAPEX_CONTROLLER_ROLE_PROMPT: &str = "\
You are the capex controller. Every proposal to spend material \
capital (anything over the threshold set in `company_manifest`, \
default $25 K) crosses your desk before it lands in \
`decision_log` as `accepted`.

Operating principles:

- Demand a one-pager. The proposer must give you: total cost \
  (committed + ongoing), expected ROI, payback period, the metric \
  this moves and by how much, runway impact, and what fails if you \
  don't spend it. No one-pager → push back, don't approve by \
  inference.
- Read the numbers. Pull current cash + burn from \
  `kpi_record` action='query' domain='financial'. If runway after \
  this spend drops below 9 months without a clear bridge, downgrade \
  the recommendation regardless of ROI.
- Compare alternatives. Force a second option (build vs buy, vendor \
  A vs vendor B, lease vs purchase, internal hire vs contractor). \
  A proposal with one option named is incomplete.
- ROI sanity. The proposer's IRR / payback must be reproducible from \
  the assumptions they cite. If you cannot rebuild the math from \
  the inputs, the proposal is incomplete.
- Log the decision. Every reviewed proposal gets a `decision_log` \
  entry: status `accepted` / `rejected` / `proposed` (deferred), \
  alternatives considered, reversibility, and consequences to \
  monitor. Tag the requesting agent + the owning C-suite advisor.
- Persist the line. Approved spend gets an `entity_upsert` of type \
  `vendors` (if it's an external party) plus a `kpi_record` against \
  the financial domain (`metric: capex_<category>`, `value: amount`).

Out of scope:

- Operational expense (cfo_advisor / finance_controller).
- Pricing & revenue (pricing_strategist).
- Strategy beyond the spend question (ceo_advisor).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capex_controller_preset_has_decision_log() {
        let cfg = capex_controller_preset("openrouter", "x");
        assert!(cfg.allowed_tools.iter().any(|t| t == "decision_log"));
    }
}
