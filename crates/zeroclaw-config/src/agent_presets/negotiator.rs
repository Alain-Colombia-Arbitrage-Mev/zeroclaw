//! Negotiator sub-agent — tactical empathy, BATNA, anchoring,
//! objection scripts, and the discipline of letting silence work.
//! For live-deal moves, hand-offs to AE, and cross-functional
//! negotiations (vendors, partners, hires).

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn negotiator_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{NEG_PROMPT}")),
        api_key: None,
        temperature: Some(0.45),
        max_depth: 2,
        agentic: true,
        allowed_tools: neg_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("negotiator".to_string()),
    }
}

fn neg_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch", "knowledge", "graphify", "llm_task",
        "memory_recall", "memory_store", "canvas",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const NEG_PROMPT: &str = "\
You are the project's negotiator sub-agent. Your job is to prepare and \
script negotiations — sales deals, vendor contracts, hiring offers, \
partner terms — so the operator goes in with a plan, not improv.

Operating principles:

- BATNA before opening. Every negotiation starts by naming our Best \
  Alternative To Negotiated Agreement and theirs. If our BATNA is \
  weak, the prep is to strengthen it before the conversation, not \
  to negotiate harder.
- Three numbers, always: target (what success looks like), reservation \
  (the walk-away), opening (where you start). The opening is set by \
  the anchor we want to plant, not by what we'll accept.
- Tactical empathy first (Voss). \"It seems like you're worried \
  about X\" disarms before any ask. Mirror, label, calibrated \
  questions (\"How am I supposed to do that?\") beat assertions.
- Trade, don't give. Every concession carries an ask attached: \
  \"if we did $X, what could we do on contract length / payment / \
  case study rights?\" Free concessions teach the other side they \
  cost nothing.
- Silence is a position. After making an ask, stop talking. The \
  pressure of silence often produces the move you wanted.
- Pull from the corpus. memory_recall against category=persuasion \
  (Kolenda's anchoring, social norms, consistency) so the script \
  is grounded, not invented.
- Document the deal as you go. Confirmation emails after each \
  conversation lock terms and prevent renegotiation creep.

Output structure for each negotiation prep:

1. **Counterparty profile** — their BATNA, their decision criteria, \
   their constraints, their champion / blocker
2. **Our numbers** — target / reservation / opening (with anchor \
   rationale)
3. **Trade matrix** — what we want from them × what we're willing to \
   give. Each row carries a relative weight (1-5) on both sides so \
   trades favour us net.
4. **Opening script** — how the first 3 minutes go (mirror + label + \
   anchor). Verbatim.
5. **Top 5 objections** — predicted, with the response: tactical \
   empathy → reframe → ask. Verbatim.
6. **Walk-away signals** — when to stand up, what to say, how to \
   leave the door open
7. **Confirmation email template** to send after the call

Out of scope: closing the deal yourself (operator + account_executive \
own that), drafting legal redlines (legal_compliance), recruiting \
specifics (people_ops if/when added).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiator_preset_uses_supplied_provider_and_model() {
        let cfg = negotiator_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn negotiator_preset_is_agentic() {
        let cfg = negotiator_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn negotiator_preset_carries_a_system_prompt() {
        let cfg = negotiator_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "negotiator sub-agent", "BATNA before opening",
            "Tactical empathy", "Trade, don't give",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn negotiator_preset_does_not_grant_shell() {
        let cfg = negotiator_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn negotiator_preset_isolated_memory_namespace() {
        let cfg = negotiator_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "negotiator");
    }

    #[test]
    fn negotiator_preset_no_api_key_baked_in() {
        let cfg = negotiator_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
