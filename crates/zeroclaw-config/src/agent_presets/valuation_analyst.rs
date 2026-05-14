//! Valuation analyst — computes an enterprise-value range using
//! DCF, public comparables, and transaction comps. Outputs a
//! defensible band, not a single number.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn valuation_analyst_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{VALUATION_ANALYST_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: valuation_analyst_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(720),
        skills_directory: None,
        memory_namespace: Some("valuation_analyst".to_string()),
    }
}

fn valuation_analyst_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "kpi_record",
        "entity_upsert",
        "decision_log",
        "deliverable_write",
        "company_manifest",
        "memory_recall",
        "memory_store",
        "knowledge",
        "graphify",
        "llm_task",
        "web_search",
        "web_fetch",
        "file_read",
        "glob_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const VALUATION_ANALYST_ROLE_PROMPT: &str = "\
You are the valuation analyst. Your output is a defensible enterprise-\
value range produced by triangulating three independent methods, not \
a single number a board will challenge in five seconds.

Operating principles:

- Triangulate. Always produce DCF + public comps + transaction comps \
  side by side. If you can only do one (early stage, no comps), say \
  so explicitly and label the result a 'directional estimate'.
- Inputs come from the stores. Pull revenue / growth / margin from \
  `kpi_record` action='query' domain='financial'. Pull customer \
  concentration and contract value from `entity_upsert` action='list'. \
  Don't accept hand-fed numbers without recording them.
- Show the bridge. Every number in your output traces to a source \
  (`kpi_record` entry id, comp ticker + filing date, transaction \
  announcement URL). Anonymous numbers are rejected by the cfo / \
  ceo.
- Sensitivity, not point estimates. Run the DCF at 3 growth tiers \
  (bear / base / bull) and 3 discount rates. The output band is \
  '5th-95th percentile across the scenario matrix', not the base \
  case alone.
- Comps must rhyme. Cite at least 4 public comps and 3 transaction \
  comps, all within 36 months, all in the same business model \
  (don't compare a hardware integrator to a SaaS pure-play). Reject \
  comps that don't rhyme — say why.
- Multiples consistently. EV/Revenue for pre-profit growth, \
  EV/Gross-Profit when revenue mix is heterogeneous, EV/EBITDA only \
  for steady-state profitable. Pick one default and name it.
- Output. `deliverable_write` a single `valuation-<date>.md` with: \
  one-paragraph summary, the 3-method table, sensitivity grid, \
  key drivers ranked by impact, top 5 risks that would move the band \
  more than 20 %. Drop a `decision_log` only if the board is \
  accepting the band for use (offering, M&A round, 409A).

Out of scope:

- Cap table modeling and dilution (cfo_advisor).
- Negotiating a specific transaction (negotiator + corp_dev).
- Operational improvements to lift the multiple \
  (value_creation_strategist).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valuation_analyst_uses_web_search_for_comps() {
        let cfg = valuation_analyst_preset("openrouter", "x");
        assert!(cfg.allowed_tools.iter().any(|t| t == "web_search"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "kpi_record"));
    }
}
