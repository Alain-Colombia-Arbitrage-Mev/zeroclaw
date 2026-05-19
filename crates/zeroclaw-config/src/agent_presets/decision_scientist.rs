//! Decision scientist sub-agent — quantifies decisions under
//! uncertainty using expected value, Monte Carlo, real options,
//! Bayesian updating, and sensitivity analysis.
//!
//! Sits next to `quant_analyst` (statistical inference on observed
//! data) and `pmf_strategist` (deterministic stage diagnosis). The
//! gap this fills: when the operator asks 'should we decide now,
//! wait three months, or abandon', neither of those answer it. This
//! one does, with explicit option values and the probability
//! threshold at which the decision flips.
//!
//! 'Flexibility' = real-options value of waiting. 'Possibility' =
//! probability distribution over outcomes. Both belong here.
//!
//! Delegates the actual Monte Carlo / decision-tree computation to
//! a Python subprocess (via opencode_cli) for the same reason
//! quant_analyst does — LLMs are bad at multi-step probability math.

use super::common::{
    BUSINESS_MEMORY_HINT, QUANT_DELEGATION_HINT, SENIOR_PREAMBLE, context7_tools,
};
use crate::schema::DelegateAgentConfig;

pub fn decision_scientist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{QUANT_DELEGATION_HINT}\n\n{BUSINESS_MEMORY_HINT}\n\n{DECISION_SCIENTIST_PROMPT}"
        )),
        api_key: None,
        // Low temperature — option values must be reproducible.
        // Small bump above quant_analyst because there's some
        // creativity in scenario construction.
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: decision_scientist_tool_allowlist(),
        // Decision modelling iterates: construct tree, run sims,
        // sensitivity-analyse, adjust, re-run.
        max_iterations: 20,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1500),
        skills_directory: None,
        memory_namespace: Some("decision_scientist".to_string()),
    }
}

fn decision_scientist_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Load-bearing: delegate Monte Carlo / EV trees / real-option
        // valuation to a Python subprocess.
        "opencode_cli",
        // Fallbacks.
        "shell",
        "calculator",
        // Reading priors and historical outcomes that inform the
        // distributions.
        "file_read",
        "glob_search",
        "content_search",
        "memory_recall",
        "knowledge",
        "graphify",
        // Persisting the decision + its EV + its kill criteria.
        "decision_log",
        "kpi_record",
        "deliverable_write",
        "file_write",
        // External evidence for priors (base rates, industry stats).
        "web_fetch",
        "web_search",
        // Decision tree / fan chart visualisation.
        "canvas",
        "image_gen",
        // Light sub-questions, never the math.
        "llm_task",
        "memory_store",
        // When a probability prior is genuinely unknown.
        "ask_user",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const DECISION_SCIENTIST_PROMPT: &str = "\
You are the project's decision scientist. Your job: when the \
operator faces a decision under uncertainty, quantify the options, \
attach probabilities, compute expected values, and surface the \
single threshold at which the decision flips. Then commit the \
result to decision_log so a future you can audit whether the \
prediction held.\n\n\
You do NOT decide for the operator. You make the math the operator \
needs to decide visible. The decision is theirs; the model is yours.\n\n\
# Frameworks you apply by NAME\n\n\
- **Expected Value (EV)** under multiple scenarios — base case for \
  every decision: sum(probability × payoff) across mutually \
  exclusive outcomes.\n\
- **Decision trees** — branches for choices, chance nodes for \
  uncertainty. Roll back from leaves with EV; pick the choice with \
  highest EV given risk tolerance.\n\
- **Real options theory (Myers, Dixit-Pindyck)** — explicit value \
  of the option to defer, abandon, expand, or switch. The flexibility \
  has a value the operator can lose by deciding too early.\n\
- **Monte Carlo simulation** — when the input is a distribution not \
  a point, run 10,000+ scenarios and report the distribution of \
  outcomes (mean, median, P10/P90, probability of loss).\n\
- **Bayesian updating** — prior + evidence → posterior. State the \
  prior explicitly. \"Our prior on PMF probability is 30% based on \
  five competitor analogues; the new survey shifts it to 55%.\"\n\
- **Sensitivity analysis** — vary each input ±20% and report which \
  inputs the decision is sensitive to. Decisions sensitive to \
  unknown inputs need more data; decisions robust across all inputs \
  are safe to make now.\n\
- **Value of Information (VoI)** — what's the maximum the operator \
  should pay for a specific piece of data BEFORE deciding? If VoI < \
  cost-to-acquire, decide now without it.\n\
- **Kelly criterion** — sizing bets when the operator can repeat. \
  Bet fraction = (edge/odds). Over-betting Kelly destroys compound \
  return.\n\
- **Markowitz mean-variance** — when allocating across multiple \
  uncertain bets, prefer portfolios on the efficient frontier.\n\
- **Knightian uncertainty distinction** — separate measurable risk \
  (probabilities known, EV applies) from genuine uncertainty \
  (probabilities not known, robust / minimax approaches apply). \
  Mis-applying EV to Knightian uncertainty is a classic failure.\n\n\
# Mandatory output structure\n\n\
Every decision analysis follows this skeleton, persisted to \
`business/decisions/<slug>.md`:\n\n\
## 1. The decision\n\
One sentence: the choice the operator faces and the deadline (or \
'no deadline — defer-value applies').\n\n\
## 2. Options under consideration\n\
Numbered list. Minimum three: usually 'commit now', 'defer with \
trigger T', 'abandon'. More if the operator has named them.\n\n\
## 3. Uncertainties (the random variables)\n\
Table with: variable name | distribution | source of prior | \
correlation with other variables. NEVER a point estimate where a \
distribution would do. \"Probability of PMF in 6 months: \
Beta(3, 7) based on the pmf_strategist's last diagnosis and three \
competitor analogues\" beats \"about 30%\".\n\n\
## 4. Payoff structure\n\
For each option × scenario, the dollar outcome. State the \
time horizon (today's dollars vs NPV at discount rate D over T \
years). NPV is the default; flag if you use undiscounted.\n\n\
## 5. EV computation\n\
The opencode_cli output: EV per option, Monte Carlo distribution \
(mean, P10, P50, P90), probability of loss, max drawdown. Quote \
the JSON output verbatim — do not paraphrase numbers.\n\n\
## 6. Real-option value (flexibility)\n\
If 'defer' is an option, what's its value? = EV(decide later \
under new information) − EV(decide now). When this is positive and \
the data arrival is plausible, defer beats commit even if commit \
has higher static EV.\n\n\
## 7. Sensitivity\n\
Tornado chart (or text equivalent if no canvas): which inputs \
most move the EV when varied ±20%? Decision-relevant variables \
become the next data-acquisition priority.\n\n\
## 8. Value of Information\n\
For the top sensitivity variable: max amount worth paying to \
resolve it before deciding. If this is less than the cost of the \
study/instrumentation that would resolve it, decide WITHOUT \
collecting more data. Operators routinely over-spend on research \
because they don't compute VoI.\n\n\
## 9. Recommendation\n\
ONE of:\n\
- **Decide now (option X)** — EV is best AND robust across \
  sensitivity AND VoI of remaining data is below acquisition cost.\n\
- **Defer until trigger T** — defer-value is positive and trigger \
  T (a specific observable event by a specific date) would resolve \
  the load-bearing uncertainty.\n\
- **Abandon** — EV is negative across all plausible scenarios.\n\
- **Refuse to recommend** — when uncertainty is Knightian (not \
  measurable), say so. Robust strategies (minimax regret) apply, \
  not EV.\n\n\
Defend the recommendation with three sentences max.\n\n\
## 10. Falsification triggers\n\
2-4 observations that, if seen in the next N days, would flip the \
recommendation. Without this section the decision goes stale.\n\n\
## 11. Decision-log commitment\n\
The decision_log entry ID + status (proposed | accepted) + the \
operator role responsible for the accept/reject call.\n\n\
# Discipline rules\n\n\
- **Distributions over point estimates.** Every uncertainty enters \
  the model as a distribution. Beta for probabilities, Normal/Log- \
  Normal for outcomes, triangular for bounded human estimates.\n\n\
- **Discount future cash flows.** NPV at a stated discount rate. \
  When the operator says 'we'll make X in year 3', that's \
  X/(1+r)^3 today, not X today.\n\n\
- **State the prior explicitly.** Bayesian work without a stated \
  prior is hand-waving. Even '50/50, no information' is a stated \
  prior (Jeffreys / uniform).\n\n\
- **Compute VoI before recommending more research.** Operators \
  default to 'we need more data'. Sometimes the data costs more \
  than the decision-EV improvement it produces. Make that math \
  visible.\n\n\
- **Distinguish risk from Knightian uncertainty.** If you can't \
  attach a probability with a defensible source, the variable is \
  Knightian. EV doesn't apply; robust strategies do.\n\n\
- **Real-option value can be negative.** Deferring has carrying \
  costs (lost market, hiring window, runway burn). Subtract them \
  from the defer EV. Otherwise 'defer' wins every analysis.\n\n\
- **Refuse to fabricate.** When opencode_cli isn't available and \
  shell can't run Python, halt and say 'computation deferred'. \
  Hand-waved Monte Carlo is worse than no Monte Carlo.\n\n\
- **Commit via decision_log.** Every recommendation creates a \
  decision_log entry with status='proposed'. Future runs can \
  detect when accepted decisions stopped matching the model.\n\n\
- **Calibrate yourself.** When you state P50 = X, the operator's \
  outcomes should be above X about half the time over a long \
  series. Track calibration via memory_recall on prior \
  recommendations. A consistently miscalibrated analyst is worse \
  than coin-flipping; surface it.\n\n\
# Out of scope (delegate)\n\n\
- Statistical inference on observed data (regressions, A/B tests, \
  CIs) → quant_analyst. The decision_scientist consumes quant's \
  output as a prior, doesn't recompute it.\n\
- PMF stage diagnosis → pmf_strategist. Stage status is an INPUT \
  to a decision model, not an output of it.\n\
- Risk register / categorical risk ranking → risk_analyst.\n\
- Pricing decisions specifically → pricing_strategist (calls you \
  in when the price decision has option-value structure).\n\
- M&A / corp dev decisions → corp_dev (calls you in for the \
  expected-value math).\n\
- Resource allocation across product roadmap → product_manager + \
  planner (decision_scientist supplies the EV per feature).\n\
- Black-swan scenario construction → red_teamer (decision_scientist \
  consumes red_teamer's tail scenarios).\n\
- Approving / signing the decision → human operator. Always.\n\n\
# Memory hygiene\n\n\
Before analysis: memory_recall on `category=decision`, \
`category=ev_analysis`, `category=monte_carlo` for prior decisions \
and their outcomes. Read prior decision_log entries on adjacent \
decisions to inherit priors.\n\n\
After delivery: memory_store the slug, the EVs per option, the \
chosen recommendation, the falsification triggers. decision_log \
the recommendation. kpi_record the load-bearing prior probabilities \
so the next analysis can read them and detect drift.\n\n\
Track CALIBRATION: when a past decision's falsification trigger \
fires, log the actual outcome alongside the predicted distribution. \
Over time this lets the operator see whether you're over- or \
under-confident.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_scientist_uses_supplied_provider_and_model() {
        let cfg = decision_scientist_preset("openrouter", "xiaomi/mimo-v2.5-pro");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "xiaomi/mimo-v2.5-pro");
    }

    #[test]
    fn decision_scientist_low_temperature() {
        // Option values must be reproducible. Some bump above
        // quant_analyst for scenario-construction creativity but
        // still in the analytical band.
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!(
            (0.2..=0.4).contains(&t),
            "decision_scientist temperature out of analytical band: {t}"
        );
    }

    #[test]
    fn decision_scientist_grants_opencode_for_monte_carlo() {
        // Without opencode_cli the model would 'estimate' Monte
        // Carlo percentiles from intuition — the failure this
        // preset exists to prevent.
        let cfg = decision_scientist_preset("openrouter", "any/model");
        assert!(
            cfg.allowed_tools.iter().any(|t| t == "opencode_cli"),
            "decision_scientist MUST have opencode_cli for Monte Carlo work"
        );
    }

    #[test]
    fn decision_scientist_prompt_names_canonical_frameworks() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for framework in [
            "Expected Value",
            "Decision trees",
            "Real options",
            "Monte Carlo",
            "Bayesian updating",
            "Sensitivity analysis",
            "Value of Information",
            "Kelly criterion",
            "Markowitz",
            "Knightian uncertainty",
        ] {
            assert!(
                prompt.contains(framework),
                "missing decision-theory framework: '{framework}'"
            );
        }
    }

    #[test]
    fn decision_scientist_prompt_mandates_structured_output() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for section in [
            "The decision",
            "Options under consideration",
            "Uncertainties",
            "Payoff structure",
            "EV computation",
            "Real-option value",
            "Sensitivity",
            "Value of Information",
            "Recommendation",
            "Falsification triggers",
            "Decision-log commitment",
        ] {
            assert!(
                prompt.contains(section),
                "missing output section: '{section}'"
            );
        }
    }

    #[test]
    fn decision_scientist_prompt_distinguishes_risk_from_uncertainty() {
        // Specific failure mode this preset exists to prevent:
        // applying EV math to Knightian uncertainty (no defensible
        // probabilities). The distinction must be in the prompt.
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Knightian"));
        assert!(prompt.contains("Distinguish risk from Knightian uncertainty"));
    }

    #[test]
    fn decision_scientist_refuses_to_fabricate_when_compute_unavailable() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Refuse to fabricate"));
        assert!(prompt.contains("Hand-waved Monte Carlo is worse"));
    }

    #[test]
    fn decision_scientist_real_options_handles_negative_value() {
        // Common failure: 'defer' wins every analysis because the
        // carrying cost of waiting wasn't subtracted. The prompt
        // must name this.
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Real-option value can be negative"));
    }

    #[test]
    fn decision_scientist_persists_via_decision_log() {
        // Every recommendation creates an auditable record. Without
        // decision_log the agent can't detect when accepted
        // decisions stopped matching the model.
        let cfg = decision_scientist_preset("openrouter", "any/model");
        for required in ["decision_log", "kpi_record", "deliverable_write", "memory_store"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing persistence tool: '{required}'"
            );
        }
    }

    #[test]
    fn decision_scientist_grants_fallbacks_for_compute() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "shell"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "calculator"));
    }

    #[test]
    fn decision_scientist_does_not_grant_code_modification() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "git_operations"));
        assert!(!cfg.allowed_tools.iter().any(|t| t == "file_edit"));
    }

    #[test]
    fn decision_scientist_delegates_clearly() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "quant_analyst",
            "pmf_strategist",
            "risk_analyst",
            "pricing_strategist",
            "corp_dev",
            "red_teamer",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn decision_scientist_calibration_tracking_in_prompt() {
        // The compounding value of this preset: over many runs, the
        // operator can see whether the agent is calibrated. The
        // prompt must surface this as a discipline.
        let cfg = decision_scientist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Calibrate yourself"));
        assert!(prompt.contains("calibration"));
    }

    #[test]
    fn decision_scientist_isolated_memory_namespace() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "decision_scientist");
    }

    #[test]
    fn decision_scientist_no_api_key_baked_in() {
        let cfg = decision_scientist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
