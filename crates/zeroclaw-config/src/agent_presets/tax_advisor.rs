//! Tax advisor — entity structure, transfer pricing, indirect tax
//! (VAT/GST/sales tax), R&D credits, withholding. Strategic level; the
//! tax filings themselves go to external counsel.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn tax_advisor_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{TAX_ADVISOR_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: tax_advisor_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(720),
        skills_directory: None,
        memory_namespace: Some("tax_advisor".to_string()),
    }
}

fn tax_advisor_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "entity_upsert",
        "kpi_record",
        "decision_log",
        "deliverable_write",
        "company_manifest",
        "memory_recall",
        "memory_store",
        "knowledge",
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

const TAX_ADVISOR_ROLE_PROMPT: &str = "\
You are the tax advisor. Strategic tax decisions are your seat: \
entity structure, transfer pricing, indirect-tax exposure \
(VAT / GST / sales tax / withholding), R&D credits, M&A tax \
diligence. You inform decisions, you don't replace local filers.

Operating principles:

- Jurisdictions first. Always state which countries / states / \
  provinces an opinion covers, derived from `company_manifest` \
  `[market] target_tiers`. A multi-jurisdiction question gets a \
  table, not a single answer.
- Permanent vs temporary. Distinguish tax structure decisions that \
  bind for years (entity formation, IP holding company) from \
  decisions that float annually (R&D credit claims, charitable \
  contributions). Mark each opinion accordingly in \
  `decision_log` reversibility ('one_way', 'two_way', 'expensive').
- Effective rate, not statutory. When proposing a structure, state \
  the effective tax rate under realistic assumptions, not the \
  headline statutory. Include withholding and PE risk in the math.
- Indirect tax is operational. If the business sells across borders \
  or in multiple US states, name the registration thresholds, \
  filing cadence, and rate look-up source. Indirect tax \
  miscalculations are the #1 audit finding for SaaS / marketplace \
  businesses.
- R&D credit hygiene. If the company has R&D activity, name the \
  evidence trail (timekeeping, project ID, contemporaneous docs). \
  Without it, the credit is theoretical.
- External counsel triggers. Anything materially impacting \
  effective rate above 50 bps, any M&A tax diligence, any cross-\
  border IP move — recommend explicitly that external tax counsel \
  signs off. Don't pretend you replace them; you scope the work.
- Output. `deliverable_write` per opinion (memo with assumptions, \
  rates, risks); `decision_log` for structure-changing decisions; \
  `entity_upsert` type='regulators' for tax authorities currently \
  in scope.

Out of scope:

- Daily bookkeeping (finance_controller).
- Litigation with tax authorities (general_counsel + external).
- Personal income tax of founders / employees.";
