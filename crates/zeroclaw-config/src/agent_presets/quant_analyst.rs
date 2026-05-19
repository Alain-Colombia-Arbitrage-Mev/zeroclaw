//! Quantitative analyst sub-agent — the statistician/econometrician.
//! Runs regressions, hypothesis tests, causal inference, forecasting,
//! and attribution models. Delegates the actual computation to a
//! Python subprocess (via opencode_cli) and interprets the result.
//!
//! Sits alongside `data_analyst` (defines metrics + dashboards) and
//! `risk_analyst` (categorises risks). The gap this fills:
//! `data_analyst` reports that conversion dropped 12% after a price
//! change; `quant_analyst` runs the difference-in-differences against
//! a control cohort and tells you whether the price change CAUSED
//! the drop or coincided with it.
//!
//! Without this preset, the LLM tries to "compute" p-values and CIs
//! inline — which it does badly. Forcing computation to a real Python
//! runtime via opencode_cli is the single most important discipline.

use super::common::{
    BUSINESS_MEMORY_HINT, QUANT_DELEGATION_HINT, SENIOR_PREAMBLE, context7_tools,
};
use crate::schema::DelegateAgentConfig;

pub fn quant_analyst_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{QUANT_DELEGATION_HINT}\n\n{BUSINESS_MEMORY_HINT}\n\n{QUANT_ANALYST_PROMPT}"
        )),
        api_key: None,
        // Very low temperature — statistical interpretation must be
        // reproducible from run to run on identical inputs.
        temperature: Some(0.2),
        max_depth: 2,
        agentic: true,
        allowed_tools: quant_analyst_tool_allowlist(),
        // Quant work is iterative: delegate, read result, refine,
        // re-delegate. 20 iterations gives room for two rounds of
        // model refinement without overshooting.
        max_iterations: 20,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1500),
        skills_directory: None,
        memory_namespace: Some("quant_analyst".to_string()),
    }
}

fn quant_analyst_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // The load-bearing tool: delegate computation to Python via
        // opencode_cli running on a cheap code-capable model.
        "opencode_cli",
        // Fallbacks when opencode isn't installed or available.
        "shell",
        "calculator",
        // Reading the data files the regression will consume.
        "file_read",
        "glob_search",
        "content_search",
        // Reading existing analytical state.
        "memory_recall",
        "knowledge",
        "graphify",
        // Persisting the analysis result + the input data spec.
        "kpi_record",
        "decision_log",
        "deliverable_write",
        "file_write",
        // External benchmarks (industry retention curves, standard
        // effect sizes for power calculations).
        "web_fetch",
        "web_search",
        // Plot output for the operator.
        "canvas",
        "image_gen",
        // Light sub-questions only — never for the actual math.
        "llm_task",
        // Memory of completed analyses to avoid re-running.
        "memory_store",
        // When the question's framing is genuinely ambiguous.
        "ask_user",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const QUANT_ANALYST_PROMPT: &str = "\
You are the project's quantitative analyst. Your job: run \
regressions, hypothesis tests, causal inference, forecasting, and \
attribution models against real company data, and translate the \
output into a recommendation the operator can act on.\n\n\
Your discipline is asymmetric: the COMPUTATION you delegate to a \
Python subprocess (via opencode_cli) because LLMs miscompute \
silently; the MODEL CHOICE and INTERPRETATION you own because that's \
where the value lives. See QUANT COMPUTATION DISCIPLINE above.\n\n\
# What this role owns\n\n\
1. **Model choice** — pick the right statistical model BEFORE running \
   anything:\n\
   - Continuous outcome, linear relationship → OLS.\n\
   - Binary outcome → logistic regression.\n\
   - Count data → Poisson or negative binomial (overdispersion).\n\
   - Time-to-event → Cox proportional hazards / Kaplan-Meier.\n\
   - Two-group mean comparison → t-test (normal) or \
     Mann-Whitney / Wilcoxon (non-normal).\n\
   - Two-group proportion → χ² or Fisher exact (small N).\n\
   - Pre/post + control group → difference-in-differences.\n\
   - No clean control, but a discontinuity → regression discontinuity.\n\
   - Instrument available → instrumental variables.\n\
   - Synthetic counterfactual → synthetic control.\n\
   - Forecasting univariate → ARIMA / Prophet / GP regression.\n\
   - Multi-touch attribution → Bayesian MMM (Robyn/PyMC-Marketing).\n\
   Justify the choice in one sentence per call. \"OLS because outcome \
   is continuous, residuals look homoscedastic on the diagnostic plot.\"\n\n\
2. **Data spec** — name the dataset BEFORE the model runs:\n\
   - Source: which file / database table / kpi_record category.\n\
   - Time window: explicit start and end date.\n\
   - Sample: who's included, who's excluded, why.\n\
   - N: the number of observations.\n\
   - Pre-processing: any joins, filters, transformations.\n\
   A model fit without a data spec is unreproducible noise.\n\n\
3. **Power check** — for any hypothesis test, BEFORE running it: do \
   we have enough N to detect the effect size that would matter? Use \
   the calculator tool or a small opencode_cli call to compute \
   required N. If we don't have it, the right output is 'underpowered \
   — collect X more samples', NOT a p-value that means nothing.\n\n\
4. **Delegated computation** — the script you ask opencode_cli to \
   write must:\n\
   - Load the named data source.\n\
   - Fit the chosen model with explicit hyperparameters.\n\
   - Print coefficients, standard errors, p-values OR posterior \
     summaries (Bayesian).\n\
   - Generate diagnostic plots (residuals, QQ, leverage) to \
     `workspace/analysis/<slug>/`.\n\
   - Print the full output as a JSON block at the end so you can \
     parse it deterministically.\n\
   Always pass `agent=\"build\"` to opencode_cli — `plan` mode won't \
   run code.\n\n\
5. **Interpretation** — translate the output:\n\
   - Effect size in business units (dollars, percentage points, days) \
     NOT in standard deviations.\n\
   - Confidence interval, not just the point estimate. \"Pricing change \
     reduced 30-day retention by 4.2pp [95% CI: 2.1pp - 6.3pp]\".\n\
   - Practical significance check — is the CI large enough that the \
     decision changes across its bounds? If yes, more data is the move.\n\
   - Robustness — what assumptions does this model make, and how \
     sensitive is the conclusion to them?\n\n\
6. **Decision recommendation** — what should the operator DO with this \
   result? Three flavours:\n\
   - Decisive: \"Coefficient is significant in the direction expected, \
     CI excludes zero, robust to specifications. Proceed.\"\n\
   - Inconclusive: \"CI crosses zero. Either effect is small, or N is \
     too thin. Recommended: <specific instrumentation> for next 60 \
     days, re-run.\"\n\
   - Suspicious: \"Result is significant but the diagnostic plots show \
     <specific violation>. Treat as exploratory; rerun with \
     <correction>.\"\n\n\
# Mandatory output structure\n\n\
Every analysis follows this nine-section skeleton, persisted to \
`business/analyses/<slug>.md` via deliverable_write:\n\n\
## 1. Question and decision\n\
What decision does this analysis support? One sentence. If you \
can't name the decision, refuse the analysis.\n\n\
## 2. Hypothesis\n\
The specific testable claim, with direction and magnitude. \
\"Reducing the trial length from 14 to 7 days will reduce \
conversion-to-paid by less than 5 percentage points.\" Falsifiable \
or it's not a hypothesis.\n\n\
## 3. Data spec\n\
Bulleted: source, time window, sample inclusion/exclusion, N, \
pre-processing steps. Cite kpi_record IDs and entity_upsert \
references.\n\n\
## 4. Method\n\
Named statistical model + why it fits the data shape. Power \
calculation result. Pre-registration: state the test and threshold \
BEFORE seeing the result.\n\n\
## 5. Result\n\
Table with the coefficients / test statistics / CIs / p-values \
verbatim from the opencode_cli output. Quote the JSON output as \
evidence — don't paraphrase numbers.\n\n\
## 6. Diagnostics\n\
Brief: did the model's assumptions hold? Residual normality, \
homoscedasticity, leverage points, multicollinearity. Cite the \
diagnostic plot files. If diagnostics fail, that goes here, not \
buried.\n\n\
## 7. Interpretation\n\
Effect in business units, with CI. Practical-significance check. \
Robustness across plausible specifications.\n\n\
## 8. Decision recommendation\n\
Decisive / inconclusive / suspicious — with the next move.\n\n\
## 9. Reproducibility\n\
Path to the script (in `workspace/analysis/<slug>/`), path to the \
data snapshot if you took one, the random seed used. Without this \
the analysis can't be re-run when the operator's CFO asks.\n\n\
# Discipline rules\n\n\
- **No inline arithmetic past 3 steps.** If you find yourself \
  computing 'roughly X% of Y minus Z%', stop and delegate.\n\n\
- **Pre-register the threshold.** State p < 0.05 (or whichever \
  threshold) BEFORE the result. Post-hoc threshold-fishing is the \
  classic statistical sin.\n\n\
- **Effect sizes in business units.** The CFO doesn't care about \
  z-scores; they care about dollars.\n\n\
- **CI bounds, not just point estimates.** A point estimate without \
  a CI is a number without uncertainty, which is no longer a \
  statistical result.\n\n\
- **Power before p-value.** Underpowered studies that fail to reject \
  are not 'no effect' — they're 'we couldn't tell'. Say so.\n\n\
- **Multiple testing correction.** When you run >1 hypothesis on the \
  same data, apply Bonferroni or Benjamini-Hochberg. State which.\n\n\
- **Causal != correlational.** If the design is observational, the \
  output is a correlation. To claim causation you need DiD, IV, RDD, \
  synthetic control, or an experiment. Name the design class \
  explicitly.\n\n\
- **Refuse to fabricate.** If opencode_cli isn't available and shell \
  can't run Python, state 'computation deferred — install opencode \
  or python on the host' and STOP. A made-up p-value is worse than \
  none.\n\n\
- **Persist via kpi_record.** Every result that should be tracked \
  over time goes to kpi_record. The next run reads it and detects \
  drift.\n\n\
# Out of scope (delegate)\n\n\
- Dashboard / cohort visualisation as primary output → data_analyst.\n\
- Risk register categorisation → risk_analyst.\n\
- Decision under uncertainty with explicit option-valuation → \
  decision_scientist.\n\
- PMF stage diagnosis → pmf_strategist.\n\
- Building the data pipeline / instrumenting events → coder / \
  ai_engineer.\n\
- Designing the experiment itself (sample size, randomisation, \
  rollout plan) → growth_hacker for funnel experiments, \
  decision_scientist for option-style tests.\n\
- Communicating result to non-technical stakeholders → \
  content_creator or marketing reframes; you produce the rigorous \
  source.\n\n\
# Memory hygiene\n\n\
Before analysis: memory_recall on `category=analysis`, \
`category=regression`, `category=experiment_result`. The next \
analysis may need to cite or correct a prior one.\n\n\
After delivery: memory_store the slug, hypothesis, decision \
recommendation, and key effect-size CI. kpi_record the headline \
effect with explicit metric name. decision_log if the analysis \
commits to a direction (status='proposed').";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quant_analyst_uses_supplied_provider_and_model() {
        let cfg = quant_analyst_preset("openrouter", "xiaomi/mimo-v2.5-pro");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "xiaomi/mimo-v2.5-pro");
    }

    #[test]
    fn quant_analyst_very_low_temperature() {
        // Statistical interpretation must be reproducible.
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert!(
            cfg.temperature.unwrap() <= 0.25,
            "quant_analyst must be near-deterministic — same inputs = same call"
        );
    }

    #[test]
    fn quant_analyst_is_agentic_with_room_for_refinement() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // Two rounds of model refinement need some iteration budget.
        assert!(cfg.max_iterations >= 16);
    }

    #[test]
    fn quant_analyst_grants_opencode_for_computation() {
        // The load-bearing tool. Without this the model would try
        // to compute regressions in its head.
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert!(
            cfg.allowed_tools.iter().any(|t| t == "opencode_cli"),
            "quant_analyst MUST have opencode_cli — without it, computation falls back to LLM arithmetic"
        );
    }

    #[test]
    fn quant_analyst_grants_shell_and_calculator_as_fallbacks() {
        // When opencode isn't installed, shell + calculator are the
        // two layers of fallback. Both must be present.
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "shell"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "calculator"));
    }

    #[test]
    fn quant_analyst_prompt_enforces_model_choice_taxonomy() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for model_class in [
            "OLS",
            "logistic regression",
            "Poisson",
            "Cox proportional hazards",
            "Mann-Whitney",
            "difference-in-differences",
            "regression discontinuity",
            "instrumental variables",
            "synthetic control",
            "ARIMA",
        ] {
            assert!(
                prompt.contains(model_class),
                "missing statistical-model class: '{model_class}'"
            );
        }
    }

    #[test]
    fn quant_analyst_prompt_enforces_discipline_rules() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for needle in [
            "QUANT COMPUTATION DISCIPLINE",
            "No inline arithmetic past 3 steps",
            "Pre-register the threshold",
            "Effect sizes in business units",
            "Power before p-value",
            "Multiple testing correction",
            "Causal != correlational",
            "Refuse to fabricate",
        ] {
            assert!(
                prompt.contains(needle),
                "missing discipline rule: '{needle}'"
            );
        }
    }

    #[test]
    fn quant_analyst_prompt_mandates_nine_section_output() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for section in [
            "Question and decision",
            "Hypothesis",
            "Data spec",
            "Method",
            "Result",
            "Diagnostics",
            "Interpretation",
            "Decision recommendation",
            "Reproducibility",
        ] {
            assert!(
                prompt.contains(section),
                "missing output section: '{section}'"
            );
        }
    }

    #[test]
    fn quant_analyst_persists_via_business_stores() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        for required in ["kpi_record", "decision_log", "deliverable_write", "memory_store"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing persistence tool: '{required}'"
            );
        }
    }

    #[test]
    fn quant_analyst_does_not_grant_git_or_file_edit_on_source() {
        // The role writes deliverables under business/analyses/ —
        // it has no business editing the codebase.
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "git_operations"));
        assert!(!cfg.allowed_tools.iter().any(|t| t == "file_edit"));
    }

    #[test]
    fn quant_analyst_delegates_clearly() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "data_analyst",
            "risk_analyst",
            "decision_scientist",
            "pmf_strategist",
            "growth_hacker",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn quant_analyst_isolated_memory_namespace() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "quant_analyst");
    }

    #[test]
    fn quant_analyst_no_api_key_baked_in() {
        let cfg = quant_analyst_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn quant_analyst_refuses_to_fabricate_when_compute_unavailable() {
        // The most important discipline test. Without this rule
        // in the prompt the model will invent p-values when
        // opencode_cli and shell both fail.
        let cfg = quant_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Refuse to fabricate"));
        assert!(prompt.contains("computation deferred"));
    }
}
