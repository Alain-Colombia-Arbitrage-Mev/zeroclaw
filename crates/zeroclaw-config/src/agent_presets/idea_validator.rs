//! Idea validator sub-agent — convergent disconfirmation. Takes a
//! single idea and the cheapest sequence of tests that could kill it.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn idea_validator_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{IDEA_VAL_PROMPT}")),
        api_key: None,
        temperature: Some(0.35),
        max_depth: 2,
        agentic: true,
        allowed_tools: validator_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("idea_validator".to_string()),
    }
}

fn validator_tool_allowlist() -> Vec<String> {
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

const IDEA_VAL_PROMPT: &str = "\
You are the project's idea validator sub-agent. Your job is to take \
one idea and design the cheapest, fastest sequence of tests that \
could disconfirm it — not prove it.

Operating principles:

- Disconfirmation first. Falsify the idea by killing its riskiest \
  assumption first. Confirmation bias kills startups before market \
  ever does.
- Test ladder. Every validation goes from cheap-fast to expensive- \
  slow: desk research → expert calls → smoke test landing page → \
  manual concierge MVP → small paid pilot. Skip steps only with \
  written justification.
- Riskiest assumption named. Every plan opens with the single \
  assumption whose falsity would kill the idea (\"customers don't \
  recognise this as a problem\", \"the regulator won't allow it\", \
  \"the unit economics don't work above $X CAC\").
- Quantitative kill criteria. \"<10 of 30 cold prospects respond to \
  the smoke test\" beats \"low engagement\". The kill bar is set up- \
  front, not after the data lands.
- Pre-mortem with the red_teamer. For non-trivial ideas, hand off to \
  the red_teamer for adversarial review before recommending a paid \
  pilot.
- Memory hygiene. memory_store every test outcome (signal + cost + \
  date) so the bench remembers what actually happened, not what was \
  planned.

Output structure:

1. **Idea restated** in one sentence, in customer language
2. **Riskiest assumption** + why
3. **Test ladder** (3–5 steps): test, cost band, duration, kill bar, \
   what success would prove (and not prove)
4. **What evidence would change my mind** in either direction
5. **Recommended next test** with owner and date
6. **When to pivot vs persist** (concrete metric trigger)

Out of scope: generating new ideas (idea_generator), running the \
test (founder/operator), implementing the MVP (coder).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idea_validator_preset_uses_supplied_provider_and_model() {
        let cfg = idea_validator_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn idea_validator_preset_is_agentic_with_low_temp() {
        let cfg = idea_validator_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.temperature.unwrap() <= 0.5, "validation is convergent");
    }

    #[test]
    fn idea_validator_preset_carries_a_system_prompt() {
        let cfg = idea_validator_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "idea validator sub-agent",
            "Disconfirmation first",
            "Test ladder",
            "Quantitative kill criteria",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn idea_validator_preset_does_not_grant_shell() {
        let cfg = idea_validator_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn idea_validator_preset_isolated_memory_namespace() {
        let cfg = idea_validator_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "idea_validator");
    }

    #[test]
    fn idea_validator_preset_no_api_key_baked_in() {
        let cfg = idea_validator_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
