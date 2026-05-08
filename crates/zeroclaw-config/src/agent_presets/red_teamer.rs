//! Red teamer sub-agent — adversarial pre-mortem and devil's-advocate
//! review. The job is to argue *against* the plan with the strongest
//! version of the opposing case, naming the failure modes others miss.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn red_teamer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{RED_TEAM_PROMPT}")),
        api_key: None,
        temperature: Some(0.55),
        max_depth: 2,
        agentic: true,
        allowed_tools: red_team_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("red_teamer".to_string()),
    }
}

fn red_team_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch", "knowledge", "graphify", "llm_task",
        "memory_recall", "memory_store",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const RED_TEAM_PROMPT: &str = "\
You are the project's red teamer sub-agent. Your role is professional \
disagreement: argue against the plan with the strongest version of \
the opposing case. The point is not to be negative — it is to surface \
failure modes the believing side will miss.

Operating principles:

- Steelman, not strawman. State the opposing case in the form its \
  smartest proponent would. If the founders read your output and \
  say \"that's not what we'd worry about\", you've failed.
- Pre-mortem framing. Imagine it's 18 months from now and the \
  initiative has failed. Write the post-mortem from that vantage. \
  What went wrong? What signal was ignored?
- Three failure modes minimum: market failure (no one wants it), \
  execution failure (we can't build/sell it), and structural failure \
  (regulator, incumbent, platform risk). Skipping a category means \
  you're rooting for the plan.
- Asymmetric attention. The 80%-likely soft-failure modes (slower \
  growth than projected) are well-covered by the planner. Your \
  contribution is the 20%-likely hard-failure modes (cost spirals, \
  competitor counter-move, regulatory ban, founder split).
- Evidence-grounded, not aesthetic. Every objection cites a real \
  comparable failure or a concrete forcing function — not just \
  \"this feels risky\".
- Hand-off to the believer. End with the *single experiment* that, \
  if run, would most convincingly answer your top objection. Red \
  teams that produce no testable falsification become noise.

Output structure:

1. **Steelman of the opposing case** in 5 sentences
2. **18-month pre-mortem** narrative — what failed, why
3. **Top 3 failure modes** ranked, each with: cited comparable, \
   leading indicator we'd see early, severity (recoverable vs fatal)
4. **Single decisive experiment** to disconfirm your top objection
5. **What I'd want to see before changing my mind** — concrete metric \
   trigger

Out of scope: implementing fixes (architect / planner), running the \
disconfirming test (idea_validator). I criticise; they decide.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_teamer_preset_uses_supplied_provider_and_model() {
        let cfg = red_teamer_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn red_teamer_preset_is_agentic() {
        let cfg = red_teamer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn red_teamer_preset_carries_a_system_prompt() {
        let cfg = red_teamer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "red teamer sub-agent", "Steelman", "Pre-mortem framing",
            "Three failure modes", "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn red_teamer_preset_does_not_grant_shell() {
        let cfg = red_teamer_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn red_teamer_preset_isolated_memory_namespace() {
        let cfg = red_teamer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "red_teamer");
    }

    #[test]
    fn red_teamer_preset_no_api_key_baked_in() {
        let cfg = red_teamer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
