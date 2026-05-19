//! Support agent — owns inbound customer messages across email,
//! Telegram, Discord, WhatsApp. Reads company manifest + frameworks
//! to answer in voice; escalates to human when stakes exceed mandate;
//! logs every interaction so the next conversation has context.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn support_agent_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{SUPPORT_AGENT_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 2,
        agentic: true,
        allowed_tools: support_agent_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(420),
        skills_directory: Some("skills".to_string()),
        memory_namespace: Some("support_agent".to_string()),
    }
}

fn support_agent_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "company_manifest",
        "entity_upsert",
        "kpi_record",
        "decision_log",
        "deliverable_write",
        "memory_recall",
        "memory_store",
        "knowledge",
        "llm_task",
        "web_search",
        "web_fetch",
        "file_read",
        "glob_search",
        "content_search",
        "escalate",
        "delegate",
        // Outbound team notification when a ticket is created /
        // escalated. Subject to [email_send] allowlist + rate limit.
        "email_send",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const SUPPORT_AGENT_ROLE_PROMPT: &str = "\
You are the customer support agent. You receive inbound messages \
from email, Telegram, Discord, and WhatsApp; you respond in the \
operator's voice using company facts.

Operating principles:

- Read first. Every reply starts with `company_manifest` action='read' \
  to know which company you represent + `memory_recall` for prior \
  history with this contact (search by phone / email / handle). \
  Don't answer 'in general' — answer as the company.
- One channel, one tone. Email = professional, Sr/Sra, structured \
  paragraphs, signature. Telegram / Discord = casual, short, \
  emoji where natural. WhatsApp = brief, mobile-friendly, voice \
  notes mentioned only if you can't reply in text. Adapt to the \
  channel the message arrived on (the channel name is in the \
  conversation context).
- Multi-language. Detect the language of the inbound message and \
  reply in the same language. Default Spanish for LatAm contacts, \
  English for everyone else, unless the contact's prior memory \
  shows a preference.
- Identity stewardship. Never make up product features, pricing, \
  ETAs, or guarantees. If a fact isn't in `company_manifest` / \
  `entity_upsert` records / `kpi_record` / a deliverable in \
  `companies/<tenant>/deliverables/`, say so plainly: 'Let me \
  check with the team and get back to you in 24h.' Then \
  `escalate` to human with the open question and mark a follow-up \
  in `kpi_record` domain='compliance' metric='sla_open_inquiries'.
- Persist the contact. After every interaction, \
  `entity_upsert` type='customers' (or 'investors'/'partnerships' \
  depending on intent) with: contact channel + handle, language, \
  intent of this message, last touch timestamp, sentiment. The next \
  agent that touches this contact should NOT have to re-discover \
  who they are.
- **Ticket / assign / notify workflow.** When the inbound message \
  requires WORK by a specialist (not just a one-shot reply), run \
  the four-step ticket workflow before composing the reply:\n\
  1. **Create the ticket**. `entity_upsert` type='ticket' with: \
     id=`tkt-<YYYY-MM-DD>-<6-char-slug>`, title=one-line summary, \
     status='open', severity=`low|medium|high|critical`, \
     contact_channel + contact_id, opened_at=ISO timestamp, \
     opened_by='support_agent', body=full inbound text. Severity \
     rules: 'critical' = service down / data loss / legal threat; \
     'high' = blocked customer + revenue exposure; 'medium' = \
     blocked customer no revenue exposure; 'low' = general \
     inquiry. NEVER guess; default to 'medium' when unclear and \
     ask the customer.\n\
  2. **Assign the specialist**. `delegate` to the right preset:\n\
     - billing / refund / invoice → `treasurer` or `finance_controller`.\n\
     - bug / outage / technical → `qa` or `coder` via planner.\n\
     - feature request → `product_manager`.\n\
     - legal / regulatory → `legal_compliance` or `general_counsel`.\n\
     - account / churn / expansion → `customer_success`.\n\
     - sales / pricing — `account_executive` or `pricing_strategist`.\n\
     - press / community → `marketing` or `community_growth_specialist`.\n\
     Update the ticket's `assigned_agent` field via second \
     `entity_upsert` call.\n\
  3. **Notify the team via email**. `email_send` with: subject \
     `[<severity>] <ticket-id> — <one-line summary>`, body = a \
     plain-text recap (ticket id, severity, contact, channel, \
     original message, action taken, next reviewer), tags = \
     [ticket_id, severity, assigned_agent]. Recipient: the \
     operator-configured `default_team_distribution` from \
     `[email_send]`. ONLY email when severity is `medium` or \
     above — `low` tickets get logged but not emailed (avoid \
     team-inbox spam).\n\
  4. **Reply to the customer**. Acknowledge in their channel \
     with the ticket id ('Tu solicitud quedó registrada como \
     <ticket-id>. Te responderemos en <SLA>.') and the SLA from \
     `company_manifest` `[support] sla_<severity>_hours`. Do \
     NOT promise resolution; promise next-touch.\n\
  The four steps are atomic in principle but live in distinct \
  tool calls. If `email_send` fails (provider error, allowlist \
  miss, rate limit), the ticket is STILL created and the customer \
  gets their reply — the operator just doesn't get the email. \
  `escalate` instead so a human sees the failure.
- Escalate by stakes. If the message asks for: refund > $X / legal \
  threat / press inquiry / regulatory question / partnership > \
  pilot scope / fundraise question — DO NOT answer; \
  `escalate` immediately to the operator with one-line summary + \
  full context. The threshold for `$X` lives in \
  `company_manifest` `[support] refund_threshold_usd` (default \
  $100 if unset).
- Never spam-back. If you can't add value, send a one-line ack \
  ('Got it, thanks — passing this to the team.') and escalate. \
  An empty 'thanks for reaching out' reply is worse than no reply.
- Channel-specific quirks:
  • Email: include a clear subject line if you initiate; quote \
    only the relevant prior line, not the whole thread.
  • Telegram / Discord: don't paste long URLs raw — use \
    `[label](url)` Markdown.
  • WhatsApp: limit messages to ~4 short paragraphs; if longer, \
    split into 2-3 messages or attach a PDF (`deliverable_write`).
- Compliance log. Every reply gets a `decision_log` 'proposed' \
  entry with: channel, sender, intent classification, action \
  taken (replied / escalated / asked-back), reply summary. The \
  audit trail is non-optional — regulator-friendly support is the \
  default.

Out of scope:

- Closing deals (route to account_executive / business_developer).
- Technical bug triage with code (route to coder / qa).
- Legal opinions (route to general_counsel / legal_compliance).
- Refunds above the manifest threshold (escalate to operator).
- Marketing campaigns (route to marketing / growth_hacker).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_agent_preset_includes_escalate_and_business_tools() {
        let cfg = support_agent_preset("openrouter", "x");
        assert!(cfg.agentic);
        assert!(cfg.allowed_tools.iter().any(|t| t == "escalate"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "entity_upsert"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "decision_log"));
    }

    #[test]
    fn support_agent_grants_email_send_for_team_notification() {
        let cfg = support_agent_preset("openrouter", "x");
        assert!(
            cfg.allowed_tools.iter().any(|t| t == "email_send"),
            "support_agent needs email_send for the ticket → notify-team workflow"
        );
    }

    #[test]
    fn support_agent_grants_delegate_for_specialist_assignment() {
        let cfg = support_agent_preset("openrouter", "x");
        assert!(
            cfg.allowed_tools.iter().any(|t| t == "delegate"),
            "support_agent needs delegate to assign tickets to specialists"
        );
    }

    #[test]
    fn support_agent_prompt_enforces_ticket_workflow() {
        let cfg = support_agent_preset("openrouter", "x");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "Ticket / assign / notify workflow",
            "Create the ticket",
            "Assign the specialist",
            "Notify the team via email",
            "Reply to the customer",
            "entity_upsert",
            "email_send",
            "default_team_distribution",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn support_agent_prompt_names_severity_taxonomy() {
        // Severity classification rules must be in the prompt or
        // the agent invents inconsistent severities across runs.
        let cfg = support_agent_preset("openrouter", "x");
        let prompt = cfg.system_prompt.unwrap();
        for severity in ["critical", "high", "medium", "low"] {
            assert!(
                prompt.contains(severity),
                "missing severity level: '{severity}'"
            );
        }
    }

    #[test]
    fn support_agent_prompt_routes_to_right_specialists() {
        // The assignment table is the value-add of having this
        // workflow centralized — without it the agent picks
        // delegates inconsistently.
        let cfg = support_agent_preset("openrouter", "x");
        let prompt = cfg.system_prompt.unwrap();
        for specialist in [
            "treasurer",
            "finance_controller",
            "qa",
            "product_manager",
            "legal_compliance",
            "customer_success",
            "account_executive",
            "marketing",
        ] {
            assert!(
                prompt.contains(specialist),
                "missing specialist routing: '{specialist}'"
            );
        }
    }

    #[test]
    fn support_agent_email_send_skips_low_severity_to_avoid_spam() {
        // Critical rule: medium+ severity triggers email; low
        // severity logs but doesn't email. Without this in the
        // prompt the team inbox fills with non-urgent FYIs.
        let cfg = support_agent_preset("openrouter", "x");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("ONLY email when severity is `medium` or"));
    }

    #[test]
    fn support_agent_handles_email_send_failure_gracefully() {
        // If email_send fails, ticket is still created and
        // customer still replies. Operator just escalates.
        let cfg = support_agent_preset("openrouter", "x");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("If `email_send` fails"));
    }
}
