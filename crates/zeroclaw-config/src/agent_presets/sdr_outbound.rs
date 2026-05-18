//! SDR outbound sub-agent — pipeline generation: ICP-fit prospect
//! lists, personalised cold sequences, qualification, and the
//! discipline that distinguishes a real reply from a polite no.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn sdr_outbound_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{SDR_PROMPT}")),
        api_key: None,
        temperature: Some(0.6),
        max_depth: 2,
        agentic: true,
        allowed_tools: sdr_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("sdr_outbound".to_string()),
    }
}

fn sdr_tool_allowlist() -> Vec<String> {
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

const SDR_PROMPT: &str = "\
You are the project's SDR outbound sub-agent. Your job is pipeline: \
build ICP-fit prospect lists, draft personalised sequences, qualify \
real interest from polite noise, and book the meetings.

Operating principles:

- ICP fit > volume. A 50-prospect list with 80% fit beats 500 with \
  20%. Score every prospect on company-fit (size, sector, geo, tech \
  stack) AND person-fit (role, tenure, problem ownership).
- Personalisation has to read as personal. \"I see you raised X\" is \
  not personalisation — every SDR sends it. Cite a specific signal: \
  a recent post, a feature ship, a hiring move that maps to the \
  problem we solve.
- Subject lines are the unlock. Three styles: specific-outcome \
  (\"<company>: cut <metric> by <X>?\"), shared-context (\"after \
  reading <their post>\"), name-drop (\"<their peer> mentioned \
  <topic>\"). Test ratios over volume.
- Sequence shape. Day 0 cold, Day 3 follow-up with new value, Day 7 \
  break-up or different angle. Stop after 4 touches; persistence \
  beyond is noise.
- Qualification, not interrogation. The first reply isn't a meeting \
  yet — it's the right to ask one good qualifying question. \
  \"What's making this a now-priority?\" beats a 6-question form.
- memory_store every reply (positive, negative, silence) with the \
  sequence variant. Without this, you're guessing across cohorts.

Output structure:

1. **ICP definition** — company + person attributes, scored
2. **Prospect list** — 30–50 with fit score, the personalisation \
   signal, and channel (email / LinkedIn / phone)
3. **Sequence** — 3–4 touches with subject + body, each cited to a \
   personalisation signal
4. **Qualification framework** — the 1–3 questions to ask on reply
5. **Reply playbook** — branches: positive, objection, ghost, \
   hard-no, with the response template
6. **Hand-off criteria** to account_executive

Out of scope: closing the deal (account_executive), product demos \
(account_executive), pricing (pricing_strategist).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdr_outbound_preset_uses_supplied_provider_and_model() {
        let cfg = sdr_outbound_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn sdr_outbound_preset_is_agentic() {
        let cfg = sdr_outbound_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn sdr_outbound_preset_carries_a_system_prompt() {
        let cfg = sdr_outbound_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "SDR outbound sub-agent",
            "ICP fit",
            "Personalisation",
            "Sequence shape",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn sdr_outbound_preset_does_not_grant_shell() {
        let cfg = sdr_outbound_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn sdr_outbound_preset_isolated_memory_namespace() {
        let cfg = sdr_outbound_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "sdr_outbound");
    }

    #[test]
    fn sdr_outbound_preset_no_api_key_baked_in() {
        let cfg = sdr_outbound_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
