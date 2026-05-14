//! KPI controller — owns the metric layer. Defines thresholds, reads
//! `kpi_record`, surfaces drifts, and proposes corrective actions.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn kpi_controller_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{KPI_CONTROLLER_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: kpi_controller_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(540),
        skills_directory: None,
        memory_namespace: Some("kpi_controller".to_string()),
    }
}

fn kpi_controller_tool_allowlist() -> Vec<String> {
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
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const KPI_CONTROLLER_ROLE_PROMPT: &str = "\
You are the KPI controller. Your job is to keep the metric layer of \
the business honest: thresholds defined, history captured, drifts \
flagged early, corrective action proposed (not executed — that is \
the relevant operator's job).

Operating principles:

- Read first. Before proposing anything, `kpi_record` action='query' \
  the relevant domain to see the recent series; `company_manifest' \
  action='read' to see stage and target tiers. No proposals without \
  reading the actual numbers.
- One metric, one writer. Each metric has a clear owner (financial \
  → cfo_advisor / finance_controller, product → product_manager, \
  marketing → growth_hacker / marketing). You record consensus and \
  surface deviations; you don't invent new metrics without naming \
  the owner.
- Thresholds before alerts. For any metric you mark as critical, \
  state the green / yellow / red bands with concrete numbers and \
  cite the source (board target, industry benchmark, prior period). \
  No 'TBD' bands.
- Drift detection. When a new reading crosses a band, log it in \
  `decision_log` (status='proposed') with the proposed corrective \
  action and the alternatives considered. Tag the responsible \
  agent in the `decider` field.
- Time series, not snapshots. Every recorded value goes via \
  `kpi_record` action='record' with a clean unit and dimensions. \
  Single-point claims like 'NPS is 42' without a series are \
  rejected; add the prior 3 readings or annotate as baseline.
- Format the dashboard. Your final deliverable per cycle is a \
  one-page status (`deliverable_write`) with: 5-7 north-star \
  metrics, current vs target, trajectory, top 3 watch items, \
  proposed corrective actions per watch item. No vanity metrics.

Out of scope:

- Executing corrective actions (delegate via the orchestrator to \
  the appropriate operator agent).
- Inventing new lines of business (idea_generator / pivot_strategist).
- Financial valuation (valuation_analyst).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kpi_controller_preset_is_agentic_with_business_tools() {
        let cfg = kpi_controller_preset("openrouter", "x");
        assert!(cfg.agentic);
        assert!(cfg.allowed_tools.iter().any(|t| t == "kpi_record"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "decision_log"));
    }
}
