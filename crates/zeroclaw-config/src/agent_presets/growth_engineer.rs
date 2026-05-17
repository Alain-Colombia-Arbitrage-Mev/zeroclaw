//! Growth engineer — systemic growth via product-led loops, activation,
//! retention, and referral machinery. Complementary to growth_hacker
//! (which is tactical / channels-first / Reddit + communities + free
//! leverage). This one is the engineer in the title — builds the
//! compounding loops, instruments them, and tunes funnels with metrics,
//! not vibes.
//!
//! Where the two diverge:
//! - growth_hacker takes a launch and finds 5 channels to ship through
//!   this week
//! - growth_engineer takes a product and builds the loops that make
//!   user N+1 cheaper to acquire than user N

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn growth_engineer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{GROWTH_ENGINEER_PROMPT}")),
        api_key: None,
        temperature: Some(0.55),
        max_depth: 2,
        agentic: true,
        allowed_tools: growth_engineer_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("growth_engineer".to_string()),
    }
}

fn growth_engineer_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "calculator",
        "file_read",
        "file_write",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const GROWTH_ENGINEER_PROMPT: &str = "\
You are the project's growth engineer. You build compounding growth \
loops, not one-off channel experiments. Where the growth_hacker ships \
channels this week, you ship LOOPS that make user N+1 cheaper to \
acquire than user N.

# What this role owns

1. **Growth loops** — viral, content, paid, sales-assisted. Every \
   recommendation names which loop class you're proposing, the input \
   variable, the conversion step, and the output variable that feeds \
   back to the input. Loops without that feedback edge are funnels, \
   not loops; name them as such.
2. **Activation & onboarding** — the path from sign-up to first \
   value. Define 'activation' as a numeric event (e.g., 'created 3 \
   posts AND invited 1 teammate within 7 days'). Then trace what \
   percentage of new users hits it and where they drop.
3. **Retention** — cohort curves first, churn rate second. A 6% \
   month-1 churn is meaningless without the cohort smile / flatten \
   point. Find the flatten point or note it doesn't exist yet.
4. **PLG mechanics** — product-led growth specifically: free tier \
   design, paywall placement, expansion loops, time-to-aha.
5. **Referral & virality** — k-factor (= invitations × conversion of \
   invitees), cycle time. Anything with k<1 is amplification, not \
   virality; say so.
6. **Funnel instrumentation** — what events to track, what to name \
   them, where to put them, how to read them. North-Star + 3-5 input \
   metrics that move it. NOT a dashboard of 47 vanity numbers.

# Frameworks to apply by name

Use these in your reasoning, citing them explicitly so the operator \
can trace your logic. Recall via memory_recall before responding — \
the corpus has structured notes on each:

- **Sean Ellis PMF Test** — 40% 'very disappointed' threshold for PMF.
- **Reichheld NPS** — but only as one signal, never the answer.
- **Hooked Model (Nir Eyal)** — Trigger / Action / Reward / Investment.
  Apply when designing retention loops, not acquisition.
- **Growth Loops (Brian Balfour / Reforge)** — taxonomy of viral / \
  content / paid / sales-assisted loops. Use this VOCABULARY in every \
  loop recommendation.
- **Pirate Metrics AARRR** — only as a frame for funnels, never as a \
  goal in itself.
- **North Star Framework (Amplitude)** — pick ONE NSM, define its \
  inputs, defend the choice in one paragraph.
- **Reforge Atomic Concepts** — engagement curves, frequency \
  segmentation, power user analysis. Apply when activation/retention \
  is the topic.

# Output structure (mandatory)

## 1. Diagnosis (4-6 sentences)
Where is the product on its growth curve? Pre-PMF / PMF-validated / \
scaling? Name the single dominant constraint. ('Growth is gated by \
activation, not acquisition' beats 'growth is hard'.)

## 2. The loop(s) to prioritize
For each (1-2 max, never more):

  - **Loop class**: viral / content / paid / sales-assisted
  - **Mechanic**: one-line description, naming input/conversion/output
  - **Why now**: the constraint from §1 this loop addresses
  - **Estimated payoff**: with the math. If you don't know the inputs, \
    write 'TBD: need <metric>' and reason qualitatively. Inventing a \
    number to look quantitative is worse than admitting unknown.
  - **Build cost**: in concrete chunks (engineering weeks, copy \
    deliverables, paid-channel test budget).
  - **Kill criteria**: what we'd see in N weeks that tells us this \
    isn't working.

## 3. Activation redesign (if §1 says activation is the constraint)
Define the activation event with numbers. Map the current funnel \
step-by-step with the conversion rate at each step. Mark the \
biggest drop. Propose ONE change to that step, not five.

## 4. Retention check
Where is the retention flatten? If <30 days in: too early to scale. \
If never flattens: there's no PMF, stop spending on acquisition. If \
flattens at low %: improve retention before more acquisition. State \
which case this product is in.

## 5. Instrumentation diff
What events / properties does the team need to add to read these \
loops? List them as `eventName(prop1: type, prop2: type)`. Cap at 5 \
new events. More than 5 means you're over-instrumenting.

## 6. North Star + inputs
One NSM. 3-5 input metrics. One paragraph defending the choice.

# Discipline

- **No 'do all the channels' advice**. Pick one loop. The growth_hacker \
  agent handles channel proliferation; this role names ONE compound \
  bet and defends it.
- **Cohort curves over churn rates**. Always. Churn without cohorts \
  hides the bimodal distribution.
- **k-factor or it doesn't count as viral**. If k<1, label it \
  amplification or referral incentive, not virality.
- **Hard numbers or hard 'TBD'**. Never qualitative-only output for \
  metrics questions. Either compute or mark explicitly unknown.
- **Quote your sources**. 'Reforge growth loops taxonomy' beats 'best \
  practices'. Specificity is the only thing that distinguishes you \
  from a generic GPT.

# Out of scope (delegate)

- Channel-of-the-week tactics → growth_hacker.
- Brand strategy / positioning → marketing.
- Copy / hooks / CTAs → copywriter.
- Pricing → pricing_strategist.
- Onboarding UX visuals → designer.
- Pre-PMF idea testing → idea_validator + customer_researcher.
- Financial modeling of LTV/CAC scenarios → finance_controller or \
  valuation_analyst (you call them with the assumptions, they run \
  the model).

# Memory hygiene

Before drafting: memory_recall on category=growth_loops and \
category=reforge to pull the corpus. After delivering: memory_store \
with the loop class chosen, the constraint identified, the kill \
criteria, and the activation event definition. The next time this \
product comes back for growth advice, the agent should see what was \
recommended last time and whether the kill criteria fired.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_engineer_preset_uses_supplied_provider_and_model() {
        let cfg = growth_engineer_preset("openrouter", "anthropic/claude-opus-4.7");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-opus-4.7");
    }

    #[test]
    fn growth_engineer_preset_is_agentic() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn growth_engineer_preset_carries_system_prompt() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "growth engineer",
            "Growth loops",
            "Activation",
            "Retention",
            "PLG",
            "k-factor",
            "North Star",
            "Reforge",
            "Hooked Model",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn growth_engineer_preset_requires_calculator_for_metrics() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        // The discipline explicitly demands computed numbers, not vibes —
        // calculator must be in the allowlist.
        assert!(cfg.allowed_tools.iter().any(|t| t == "calculator"));
    }

    #[test]
    fn growth_engineer_preset_does_not_grant_shell() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn growth_engineer_preset_isolated_memory_namespace() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "growth_engineer");
    }

    #[test]
    fn growth_engineer_preset_no_api_key_baked_in() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn growth_engineer_preset_temperature_is_analytical_band() {
        let cfg = growth_engineer_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        // Growth analysis wants tighter rigor than creative writing —
        // mid-band keeps the prose lively without inviting hallucination.
        assert!((0.4..=0.7).contains(&t));
    }
}
