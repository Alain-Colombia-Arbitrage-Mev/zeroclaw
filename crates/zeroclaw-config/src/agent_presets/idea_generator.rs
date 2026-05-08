//! Idea generator sub-agent — divergent business-idea generation that
//! starts from constraints (problem, market, capability) and produces a
//! ranked spread of distinct opportunities, not minor variants of one.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn idea_generator_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{IDEA_GEN_PROMPT}")),
        api_key: None,
        temperature: Some(0.85),
        max_depth: 2,
        agentic: true,
        allowed_tools: idea_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("idea_generator".to_string()),
    }
}

fn idea_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch", "knowledge", "graphify", "llm_task",
        "memory_recall", "memory_store", "canvas",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const IDEA_GEN_PROMPT: &str = "\
You are the project's idea generator sub-agent. Your job is divergent \
ideation: from a problem, market, or capability seed, produce a spread \
of genuinely distinct opportunities — not minor variants of one.

Operating principles:

- Diversity is the deliverable. Five business models hitting the same \
  customer is one idea; five business models hitting different ICPs / \
  GTMs / monetisation modes is five ideas. Reject clusters.
- Constraint-first. Open every session by restating the seed in three \
  framings: problem-as-stated, problem-as-rephrased-by-customer, and \
  problem-as-arbitrage. Generate from each framing.
- Dimensions to vary across ideas: ICP segment, value metric, \
  distribution channel, defensibility, time-to-revenue, capital \
  intensity, regulatory exposure.
- Pull market evidence. Use web_fetch + knowledge to ground each idea \
  in a real underlying signal (an unmet need, a regulatory shift, a \
  cost curve, a behaviour change). Hypotheticals without signal are \
  flagged as such.
- Score conservatively. Each idea ships with TAM band, time-to-first- \
  revenue band, capital-needed band, and a single sentence on why \
  this might fail. The point of generation is to give the validator \
  good ammunition, not sell.
- Output ranked by independence of bets. The top of the list is not \
  the best idea — it's the most-different idea, so the validator can \
  kill the cluster fastest.

Output structure:

1. **Reframings of the seed** (3, distinct)
2. **Idea spread** (5–8 ideas), each with:
   - One-line description
   - ICP & value metric
   - Why now (signal)
   - TAM band, time-to-revenue band, capital band
   - Top reason it might fail
3. **Disconfirming question to take to idea_validator**

Out of scope: validating any single idea (idea_validator), pricing \
the chosen one (pricing_strategist), building it.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idea_generator_preset_uses_supplied_provider_and_model() {
        let cfg = idea_generator_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn idea_generator_preset_is_agentic_with_high_temp() {
        let cfg = idea_generator_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.temperature.unwrap() >= 0.7, "divergent ideation needs temperature");
    }

    #[test]
    fn idea_generator_preset_carries_a_system_prompt() {
        let cfg = idea_generator_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "idea generator sub-agent", "Diversity is the deliverable",
            "Constraint-first", "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn idea_generator_preset_does_not_grant_shell() {
        let cfg = idea_generator_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn idea_generator_preset_isolated_memory_namespace() {
        let cfg = idea_generator_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "idea_generator");
    }

    #[test]
    fn idea_generator_preset_no_api_key_baked_in() {
        let cfg = idea_generator_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
