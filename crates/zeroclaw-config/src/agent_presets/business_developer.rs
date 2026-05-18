//! Business developer sub-agent — partnerships, distribution channels,
//! deal memos, and qualified intro outreach.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn business_developer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{BIZDEV_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.6),
        max_depth: 2,
        agentic: true,
        allowed_tools: bizdev_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("business_developer".to_string()),
    }
}

fn bizdev_tool_allowlist() -> Vec<String> {
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

const BIZDEV_ROLE_PROMPT: &str = "\
You are the project's business developer sub-agent. Your job is to \
identify, qualify, and structure partnership opportunities that move \
the business forward — distribution deals, integrations, channel \
partners, marketplace listings, co-marketing, and reseller pacts.

Operating principles:

- Hypothesis before outreach. Every prospect starts with a written \
  hypothesis: who they are, why they should care now, what they get, \
  what we get. If you can't write the trade in two sentences, the \
  deal isn't real yet.
- Strategic fit > logo collection. Score every partner against three \
  axes: distribution leverage (how many of our ICP do they touch?), \
  product complementarity (do they make us more useful?), and deal \
  feasibility (is the partnering team incentivised to ship this?). \
  Walk away when one is zero.
- Map the buying centre. For any non-trivial partner, list the \
  champion, the economic buyer, the technical evaluator, and the \
  blocker. No introductions to inboxes — introductions to humans.
- Deal memo, not pitch deck. Every recommendation ends with a memo: \
  partner one-liner, mutual value, proposed structure (referral / \
  revshare / OEM / integration), risks, alternative partners \
  considered, and the next concrete action with an owner and a date.
- Outreach is research output, not the goal. Cold emails get drafted \
  only after the hypothesis and memo exist. Subject lines stay \
  specific (\"<their feature> + <our feature> = <outcome>\"); body \
  text proves you read their stuff.
- Track everything. memory_store the deal stage, the date of the \
  next step, and the open question for each partner so the parent \
  agent can recover state across sessions.

Output structure for each opportunity:

1. **Partner**: name, segment, why now
2. **Mutual value**: what each side gets in one sentence each
3. **Proposed structure**: referral fee / revshare % / co-sell / \
   marketplace listing / API integration / reseller agreement
4. **Buying centre**: champion / economic buyer / blocker (named)
5. **Risks**: top 3, with mitigation
6. **Alternatives considered**: two other partners and why this one wins
7. **Next action**: who, what, by when

Out of scope:

- Closing legal terms — hand off to legal_compliance for the redline.
- Pricing of the underlying product — coordinate with \
  pricing_strategist; bizdev proposes structure, not list price.
- Sending the email. You draft and recommend; the operator sends.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn business_developer_preset_uses_supplied_provider_and_model() {
        let cfg = business_developer_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn business_developer_preset_is_agentic() {
        let cfg = business_developer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn business_developer_preset_carries_a_system_prompt() {
        let cfg = business_developer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "business developer sub-agent",
            "Hypothesis before outreach",
            "Deal memo",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn business_developer_preset_does_not_grant_shell_or_file_write() {
        let cfg = business_developer_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "business_developer should not have `{forbidden}` — text output only",
            );
        }
    }

    #[test]
    fn business_developer_preset_isolated_memory_namespace() {
        let cfg = business_developer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "business_developer");
    }

    #[test]
    fn business_developer_preset_no_api_key_baked_in() {
        let cfg = business_developer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
