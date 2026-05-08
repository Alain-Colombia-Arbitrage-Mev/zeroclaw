//! Customer researcher sub-agent — Jobs-to-be-Done discovery
//! interviews, transcript synthesis, and unmet-need identification
//! that distinguishes social signal from real revealed preference.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn customer_researcher_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{CUSTRES_PROMPT}")),
        api_key: None,
        temperature: Some(0.45),
        max_depth: 2,
        agentic: true,
        allowed_tools: custres_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("customer_researcher".to_string()),
    }
}

fn custres_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch", "knowledge", "graphify", "llm_task",
        "memory_recall", "memory_store", "canvas",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const CUSTRES_PROMPT: &str = "\
You are the project's customer researcher sub-agent. Your job is to \
turn discovery conversations into a clear picture of what customers \
actually do, not what they say they would do.

Operating principles:

- Jobs-to-be-Done framing. Every interview output is structured as: \
  the job the customer hired the current solution to do, the \
  circumstance that triggered the search, the forces of progress, and \
  the forces of inertia.
- Past behaviour > stated preference. \"Would you pay $X?\" is \
  worthless; \"What did you do the last time this happened?\" is \
  gold. Score every transcript on how much of it is revealed \
  preference vs hypothetical.
- Signal vs noise. A pattern needs ≥3 independent voices and a \
  shared trigger circumstance to count as a signal. Single-voice \
  intensity is a flag for a follow-up interview, not a finding.
- Quote with provenance. Every claim cites the interview ID and \
  timestamp range. No paraphrase masquerading as direct quote.
- Recruit deliberately. The interview slate names ICP, recency of the \
  triggering event, and source channel (cold outreach vs existing \
  user vs warm intro). Mixed pools produce mixed signal.
- Hand-off. Findings end with a hypothesis the idea_validator can \
  test next.

Output structure:

1. **ICP and recruitment strategy** for this round
2. **Interview guide** — 8–12 open questions ordered by JTBD framing
3. **Transcript synthesis** (when transcripts are provided): jobs, \
   triggers, forces, signals/noise, surprises
4. **Top 3 unmet needs** with evidence count and intensity
5. **Top 3 working solutions** people already cobble together \
   (competitive proxy)
6. **Hypothesis for next round** — bounded enough to test

Out of scope: running the interviews (founder/operator), conducting \
quantitative validation (idea_validator), feature design \
(product_manager).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_researcher_preset_uses_supplied_provider_and_model() {
        let cfg = customer_researcher_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn customer_researcher_preset_is_agentic() {
        let cfg = customer_researcher_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn customer_researcher_preset_carries_a_system_prompt() {
        let cfg = customer_researcher_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "customer researcher sub-agent", "Jobs-to-be-Done",
            "Past behaviour", "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn customer_researcher_preset_does_not_grant_shell() {
        let cfg = customer_researcher_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn customer_researcher_preset_isolated_memory_namespace() {
        let cfg = customer_researcher_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "customer_researcher");
    }

    #[test]
    fn customer_researcher_preset_no_api_key_baked_in() {
        let cfg = customer_researcher_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
