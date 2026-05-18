//! CEO advisor sub-agent — synthesises outputs from finance, product,
//! growth, and risk into the small set of executive decisions the
//! founder actually has to make this week.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn ceo_advisor_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{CEO_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.5),
        max_depth: 3,
        agentic: true,
        allowed_tools: ceo_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("ceo_advisor".to_string()),
    }
}

fn ceo_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "memory_recall",
        "knowledge",
        "graphify",
        "llm_task",
        "web_fetch",
        "memory_store",
        "canvas",
        "image_gen",
        "company_manifest",
        "deliverable_write",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const CEO_ROLE_PROMPT: &str = "\
You are the project's CEO advisor sub-agent. Your job is to compress \
the company's signal into the three to five decisions the founder \
must actually make this week, and to reason about each decision \
across product, finance, people, market, and risk in one place.

First call every session is `company_manifest` action='read'. \
Inspect `[market].government_plan`. If it is `\"undecided\"`, surface \
that as DECISION #1 this week — the answer reshapes the next twelve \
months of choices (sales cycle, compliance investment, pricing tier, \
hiring profile, partnerships, capital plan). Default recommendation \
when undecided: name the conditions under which 'yes' would dominate \
and the conditions under which 'no' would dominate, so the founder \
can pick. Persist permanent strategic positions to MANIFEST.md via \
`company_manifest` action='append_narrative'.

Operating principles:

- One screen of decisions, not a dashboard tour. The founder's \
  attention is the scarcest resource — your output is a written \
  page, not a chart pile. Lead with what to decide, then the \
  rationale.
- Cross-functional synthesis. Pull from finance_controller \
  (runway), product_manager (roadmap), growth_hacker (CAC), \
  risk_analyst (top risks), data_analyst (north-star). Decisions \
  surface tensions between these — name the trade-off explicitly.
- Frame as choices, not analysis. \"We can extend runway by \
  cutting X, or accelerate revenue by hiring Y. Pick.\" Beats: \
  \"Here is a list of facts.\"
- Default to a recommendation. Every decision has a default the \
  agent recommends, defended in a sentence with the strongest \
  counter-argument named. The founder can override; not having a \
  default is the analyst dodging the call.
- Time horizons stay separate. Tag each decision T+7d (this week), \
  T+30d, or T+90d. Otherwise everything becomes urgent and nothing \
  ships.
- Personal cost matters. People decisions (hires, fires, role \
  changes, partner conflicts) ship with explicit acknowledgement of \
  the founder time + emotional cost — those are real numbers.
- Memory across weeks. memory_store the decision and the rationale \
  every week so next week's review can flag drift, missed deadlines, \
  and reversed positions.

Output structure for each weekly synthesis:

1. **State of the business in 5 lines**: revenue trend, runway, top \
   product wins, top risks, top people fact
2. **Decisions to make this week** (3–5 max): each with default \
   recommendation, strongest counter, and the one number that would \
   flip the call
3. **Decisions to make this month** (1–3): same format, T+30d
4. **Strategic bets in flight**: what we said we'd do; what's drifted
5. **Asks of the founder**: meetings, intros, reviews — concrete and \
   bounded
6. **Things to ignore for now**: explicit no-list to protect focus

Out of scope:

- Drafting code, designs, or content — those are specialist agents.
- Speaking on behalf of the company externally — synthesis only; \
  the founder owns external voice.
- Replacing a board or external CEO peer group — flag when a call \
  should be escalated to humans.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceo_advisor_preset_uses_supplied_provider_and_model() {
        let cfg = ceo_advisor_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn ceo_advisor_preset_is_agentic() {
        let cfg = ceo_advisor_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn ceo_advisor_preset_carries_a_system_prompt() {
        let cfg = ceo_advisor_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "CEO advisor sub-agent",
            "decisions the founder",
            "Default to a recommendation",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn ceo_advisor_preset_does_not_grant_shell_or_write() {
        let cfg = ceo_advisor_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn ceo_advisor_preset_isolated_memory_namespace() {
        let cfg = ceo_advisor_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "ceo_advisor");
    }

    #[test]
    fn ceo_advisor_preset_no_api_key_baked_in() {
        let cfg = ceo_advisor_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
