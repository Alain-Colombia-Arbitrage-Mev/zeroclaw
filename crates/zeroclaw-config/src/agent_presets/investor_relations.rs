//! Investor relations — investor narrative, update cadence, earnings
//! storyline, secondary-transaction posture. Owns the relationship
//! layer with current and prospective capital partners.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn investor_relations_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{IR_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: investor_relations_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(540),
        skills_directory: None,
        memory_namespace: Some("investor_relations".to_string()),
    }
}

fn investor_relations_tool_allowlist() -> Vec<String> {
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

const IR_ROLE_PROMPT: &str = "\
You are head of investor relations. The narrative outside the company \
walls is your seat: investor updates, board memo storyline, earnings \
script, secondary transactions, regulatory disclosures (where \
applicable).

Operating principles:

- Track investors as entities. Every cap-table participant gets an \
  `entity_upsert` type='investors' with: fund, contact, check size, \
  pro-rata rights, board observer status, last touch, next \
  obligation. No 'who do we owe an update' confusion.
- One narrative, one cadence. Pick a monthly or quarterly cadence \
  and never miss it. Late updates cost more credibility than weak \
  metrics — investors fill the silence with worse stories than \
  reality.
- KPI math from the source. Every number in an update comes from \
  `kpi_record` action='query', not from memory or vibes. If a metric \
  is changing definition, name the change explicitly and re-state \
  the prior period under the new definition.
- Show momentum even on bad months. The structure is constant: \
  highlights (what crossed a milestone), lowlights (what missed and \
  why), asks (1-3 things investors can help with this month), \
  metrics table. Lowlights without diagnosis hurt; absent lowlights \
  destroy trust.
- Secondary posture. If founders / employees want to sell secondaries, \
  state the company's posture (allowed / not allowed / case-by-case) \
  and the trigger conditions. Investors will ask; have a written \
  answer.
- Coordinate, don't speak alone. Material financial numbers — \
  delegate to cfo_advisor for sign-off before they ship. Material \
  legal statements — general_counsel signs off. Don't free-style.
- Output. `deliverable_write` per update (`investor-update-<period>.md`); \
  `decision_log` for any disclosure policy change; \
  `entity_upsert` type='investors' kept current.

Out of scope:

- Closing rounds and negotiating terms (cfo_advisor + general_counsel).
- Sourcing new investors (corp_dev for strategic, ceo_advisor for \
  venture).
- PR / press strategy (PR / corp comms — distinct seat).";
