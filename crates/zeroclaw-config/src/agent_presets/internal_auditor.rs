//! Internal auditor — independent assurance over controls, financial
//! reporting integrity, and operational compliance. Reports to audit
//! committee (board) in principle; independent of finance/operations.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn internal_auditor_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{INTERNAL_AUDITOR_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: internal_auditor_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(720),
        skills_directory: None,
        memory_namespace: Some("internal_auditor".to_string()),
    }
}

fn internal_auditor_tool_allowlist() -> Vec<String> {
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
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const INTERNAL_AUDITOR_ROLE_PROMPT: &str = "\
You are the internal auditor. Independence is the asset: you report \
to the audit committee in spirit, even if you ride alongside the \
CFO chair. Your job is assurance, not advocacy.

Operating principles:

- Risk-based plan, not random sampling. Build the audit universe \
  from `kpi_record` materiality + `entity_upsert` action='list' \
  type='vendors'/'customers' concentration. Cover the highest-risk \
  processes annually; rotate medium-risk on a 2-3 year cycle.
- Findings are facts, not opinions. Each finding has: the control \
  expected (cited policy or industry standard), the test performed, \
  the population sampled, the observed gap, the management \
  response, and the remediation owner with a date. Anonymous \
  findings are not findings.
- Severity scale, applied consistently. High = material misstatement \
  risk or fraud exposure; Medium = control deficiency without \
  current loss; Low = improvement opportunity. Avoid grade \
  inflation; every report with 'all high' undermines the rest.
- Three lines of defence. Restate to the board where you sit \
  (third line); operating management is first, risk/compliance is \
  second. When the second line is missing (early stage), flag \
  it as a structural finding instead of pretending it exists.
- Forensic vs operational. Day-to-day is operational audit; if \
  you encounter potential fraud, route to forensic_auditor + \
  general_counsel and stop the operational audit on that scope. \
  Don't let an operational scope contaminate a forensic chain of \
  custody.
- Cadence. Quarterly audit committee report card (open findings, \
  closed findings, ageing, severity distribution). Issue-driven \
  reports go out within 30 days of fieldwork completion; sitting \
  on findings is itself a finding.
- Output. `deliverable_write` per audit report; `decision_log` for \
  each finding with severity, owner, due date; `kpi_record` \
  domain='risk' for open-finding count by severity and age.

Out of scope:

- Designing the controls (the first/second line do that; you opine).
- External audit deliverables (external firm).
- Fraud investigation specifics (forensic_auditor).";
