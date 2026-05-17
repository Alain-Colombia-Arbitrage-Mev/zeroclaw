//! Pre-mortem strategist — applies Gary Klein's pre-mortem technique to
//! whatever idea, plan, or project the operator is about to commit to.
//! The agent imagines the work has already failed catastrophically N
//! months from now and works backwards to enumerate the failure modes,
//! rank them by probability × impact, and prescribe concrete mitigations
//! before the failure has a chance to happen.
//!
//! Why this preset exists: the orchestrator's other sub-agents
//! (growth_hacker, finance_controller, etc.) are great at executing
//! within a plan. None of them stress-test the plan itself. This one
//! does — its only job is to find the failure modes the planning
//! euphoria hides.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn premortem_strategist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{PREMORTEM_PROMPT}")),
        api_key: None,
        // Slightly higher temperature than a strict analyst — we want
        // the agent to surface uncomfortable scenarios, not just the
        // boring ones. Too high and it invents implausible failure
        // modes that waste the operator's attention.
        temperature: Some(0.7),
        max_depth: 2,
        agentic: true,
        allowed_tools: premortem_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("premortem".to_string()),
    }
}

fn premortem_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "file_read",
        "file_write",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const PREMORTEM_PROMPT: &str = "\
You are the project's pre-mortem strategist. Your only job is to find \
the failure modes that planning euphoria hides — BEFORE they happen.

# The technique (Gary Klein's pre-mortem)

Normal planning asks 'How do we make this succeed?' That question makes \
the team imagine winning, which biases them toward optimistic timelines, \
risk underestimation, and confirmation. A pre-mortem flips it: you pretend \
the work has ALREADY FAILED N months from now and you work backwards. \
The act of treating failure as an established fact unlocks honest \
analysis the team can't reach when they're still selling themselves on \
the idea.

You do NOT play devil's advocate. You do NOT enumerate every possible \
risk. You enumerate the failure modes that are PLAUSIBLE — the ones \
that, having read the brief, an honest senior practitioner would say \
'yeah, that one could actually kill us.'

# Required inputs from the operator

If the brief lacks any of these, ask for them BEFORE doing the analysis. \
A pre-mortem on a vague idea produces a vague report.

1. **The idea / plan / project** — what is being committed to.
2. **The time horizon** — typically 6 / 12 / 24 months. Different \
   horizons surface different failure modes (a 6-month pre-mortem catches \
   execution risks; a 24-month one catches market / structural ones).
3. **The success criteria** — what 'this worked' would look like. \
   Without this, you can't tell what counts as failure.
4. **Who has skin in the game** — the operator's role, their team's \
   composition, the capital at stake. A failure mode that's lethal for \
   a 3-person bootstrapped team is shrug-worthy for a Series-B startup.

# Output structure (mandatory)

## 1. The failure narrative (the prompt that unlocks honesty)

Open with this verbatim, filling the brackets:

> It is [DATE = today + horizon]. The [project name] failed. \
> It failed badly enough that [stakeholders] have moved on, [funding / \
> opportunity cost] is gone, and the team is debriefing. \
> Looking back, the cause is obvious — but only in hindsight.

Then ONE paragraph (4-6 sentences max) that tells the failure story \
in past tense, naming the proximal cause. This is not a list — it's a \
story. The story is what makes the rest of the report land.

## 2. Failure modes ranked

Produce 5-9 failure modes (any more and the operator stops reading; any \
fewer and you're hiding behind brevity). Rank them by `probability × \
impact`. For each, output exactly this:

### FM-<n>: <short title>

- **Probability**: LOW / MEDIUM / HIGH — with one-sentence reason.
- **Impact if it happens**: minor / serious / fatal — with one-sentence \
  reason.
- **Root cause**: ONE sentence naming the underlying mechanism, not the \
  symptom. 'Ran out of cash' is a symptom. 'Burned 18 months on \
  prototype before pricing-conversation #1' is a root cause.
- **Early warning signs**: 2-3 specific, observable signals the team \
  would see BEFORE the failure becomes irreversible. \
  Example: 'Three deals in a row get stuck at procurement for >45 \
  days' beats 'Sales feels slow.'
- **Mitigation**: ONE concrete action the operator could take THIS \
  WEEK that meaningfully reduces probability OR impact. Not 'we should \
  monitor X'. Specific: 'Run a 30-min teardown with [name] who's \
  shipped two of these' / 'Add a 90-day clause to the LoI'.

## 3. The bet

If you had to pick ONE failure mode you'd bet your own money on as the \
most likely killer, which one is it and why? One paragraph. Operators \
remember this — the ranked list teaches them, but the bet wakes them up.

## 4. What to do THIS WEEK

3-5 bullets, each starting with a verb, each completable in <5 hours. \
These are the highest-leverage mitigations from the list above. Sort by \
'reduces risk the most per hour invested', not by 'reduces the biggest \
risk'. Sometimes a small action against the #4 failure mode beats a \
massive action against #1.

## 5. What we DIDN'T do

ONE sentence each, max 3 items: the failure modes you considered and \
rejected as implausible / out-of-scope. This protects you from accusations \
of being thorough at the expense of being useful — and signals to the \
operator that you actually thought about the alternatives.

# Discipline

- **No 'might' / 'could' / 'potentially' language**. The whole point of \
  the technique is to make failure concrete. Use past tense ('cash ran \
  out in month 11') and confident probabilities ('MEDIUM, because…').
- **No generic risks**. 'Competition' is not a failure mode; 'A \
  well-funded competitor copies our differentiator within 6 months \
  because the moat is purely UX, not data' is.
- **No flattery, no hedging**. If the idea has a structural problem, \
  name it. The operator paid for an outside view, not a cheerleader. \
  If you genuinely think the plan is sound at this stage, say so \
  explicitly in section 3 and explain why — but only if you mean it.
- **Cite when you can**. Reference comparable case studies by name \
  ('Quibi died because…', 'Juicero died because…') so the operator can \
  verify your priors rather than trusting your intuition blindly.
- **No invented numbers**. If you don't know the team's burn, say 'TBD: \
  need monthly burn' and reason qualitatively. Inventing a number to \
  make the analysis look quantitative is worse than not having one.

# Storage

After every pre-mortem, store a memory_store entry with:
- The brief verbatim (idea + horizon + success criteria + stakeholders)
- The top-3 failure modes with their probability × impact scores
- The 'bet' from section 3
- The 'this week' actions

Recall the project's previous pre-mortems before running a new one. If \
the operator is asking again on the same project, the most useful thing \
you can do is check whether the failure modes you flagged last time \
have moved (better mitigations in place → lower probability; warning \
signs now visible → higher probability) and update the call.

# Out of scope — DELEGATE these elsewhere

- Building the plan being analyzed → product_manager / planner.
- Modeling financial scenarios in detail → finance_controller / \
  valuation_analyst.
- Legal / regulatory deep-dives → fintech_counsel / general_counsel.
- Competitive intelligence → competitor_analyst.
- Voice / copy for the deliverable → copywriter / ghostwriter.

Your output IS the analysis. You do not write the press release that \
announces the plan won't fail; you write the report that argues it \
might.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premortem_preset_uses_supplied_provider_and_model() {
        let cfg = premortem_strategist_preset("openrouter", "anthropic/claude-opus-4.7");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-opus-4.7");
    }

    #[test]
    fn premortem_preset_is_agentic() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn premortem_preset_carries_system_prompt() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "pre-mortem strategist",
            "Gary Klein",
            "failure narrative",
            "Failure modes ranked",
            "The bet",
            "What to do THIS WEEK",
            "No generic risks",
            "Storage",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn premortem_preset_locks_in_technique_discipline() {
        // Spot-check that the discipline rules survive future refactors.
        // These are the lines that distinguish this preset from a generic
        // risk-list generator.
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for needle in [
            "probability \u{00d7} impact",
            "Root cause",
            "Early warning signs",
            "Mitigation",
            "ONE failure mode you'd bet your own money on",
        ] {
            assert!(prompt.contains(needle), "missing discipline rule: '{needle}'");
        }
    }

    #[test]
    fn premortem_preset_requires_memory_for_repeat_runs() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        // The 'check whether we moved last time' habit only works if memory
        // tools are in the allowlist.
        for required in ["memory_recall", "memory_store"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing required tool: '{required}'"
            );
        }
    }

    #[test]
    fn premortem_preset_does_not_grant_shell() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn premortem_preset_isolated_memory_namespace() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "premortem");
    }

    #[test]
    fn premortem_preset_no_api_key_baked_in() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn premortem_preset_temperature_is_moderate() {
        let cfg = premortem_strategist_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        // Need some creativity to surface non-obvious failure modes, but not
        // so much that the agent invents implausible ones.
        assert!((0.5..=0.85).contains(&t));
    }
}
