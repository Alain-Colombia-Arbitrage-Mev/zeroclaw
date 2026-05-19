//! Calibration scorer sub-agent — measures whether the project's
//! forecasting agents (decision_scientist, pmf_strategist,
//! quant_analyst, risk_analyst) are CALIBRATED. When they say
//! "70% probability", does it actually happen 70% of the time
//! over many runs?
//!
//! Reads decision_log entries with explicit predictions, looks at
//! their falsification triggers and recorded actuals, computes
//! calibration metrics (Brier score, log-loss, reliability
//! diagram), and surfaces over/under-confidence patterns per
//! agent and per category.
//!
//! Compounds: each run improves the meta-prior the operator has
//! on which agent's forecasts to trust. After 30+ scored
//! predictions per agent, the operator gets a defensible answer
//! to "should I act on decision_scientist's 80% confidence" — and
//! the answer is data, not vibe.

use super::common::{
    BUSINESS_MEMORY_HINT, QUANT_DELEGATION_HINT, SENIOR_PREAMBLE, context7_tools,
};
use crate::schema::DelegateAgentConfig;

pub fn calibration_scorer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{QUANT_DELEGATION_HINT}\n\n{BUSINESS_MEMORY_HINT}\n\n{CALIBRATION_SCORER_PROMPT}"
        )),
        api_key: None,
        // Very low — calibration is arithmetic over a fixed table.
        // Same inputs must produce the same scores.
        temperature: Some(0.15),
        max_depth: 2,
        agentic: true,
        allowed_tools: calibration_scorer_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("calibration_scorer".to_string()),
    }
}

fn calibration_scorer_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading past predictions + actuals.
        "memory_recall",
        "knowledge",
        "graphify",
        "file_read",
        "glob_search",
        "content_search",
        // Computing Brier / log-loss / reliability diagram.
        "opencode_cli",
        "shell",
        "calculator",
        // Persisting the calibration scores and the meta-prior
        // ("decision_scientist is 8pp overconfident on 60-80%
        // forecasts of revenue events").
        "kpi_record",
        "entity_upsert",
        "decision_log",
        "deliverable_write",
        "file_write",
        "memory_store",
        // Reliability diagram visualisation.
        "canvas",
        "image_gen",
        // Light interpretation only.
        "llm_task",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const CALIBRATION_SCORER_PROMPT: &str = "\
You are the project's calibration scorer. Your job: take every \
PREDICTION the project's forecasting agents have committed to \
(via decision_log entries with explicit probabilities, or \
kpi_record entries with P10/P50/P90), pair each one with its \
ACTUAL outcome, and compute whether the agents are calibrated.\n\n\
A calibrated agent that says '70% probability' is right 70% of \
the time. An overconfident agent is right less often than they \
claim; an underconfident one more often. Both are bugs. Neither \
shows up without this scoring run.\n\n\
# What this role owns\n\n\
1. **Pair predictions to outcomes** — for each decision_log entry \
   with status='accepted' and a falsification trigger that has \
   reached its date, find the actual outcome. Sources:\n\
   - kpi_record entries dated after the falsification deadline.\n\
   - entity_upsert state changes (deal closed-won / closed-lost, \
     customer churned).\n\
   - decision_log entries that explicitly close out earlier \
     decisions.\n\
   - ask_user when the outcome is observable but not yet captured \
     in business memory.\n\
   Skip predictions whose deadline hasn't elapsed — they're not \
   yet scorable.\n\n\
2. **Compute calibration metrics** (delegate to opencode_cli):\n\
   - **Brier score** = mean((predicted_prob - actual_outcome)^2). \
     Lower is better; 0.25 is no-information baseline for binary.\n\
   - **Log-loss** = mean(-actual * log(predicted) - (1-actual) * \
     log(1-predicted)). Penalises confident wrong answers heavily.\n\
   - **Reliability diagram** — bin predictions by probability \
     decile, plot mean predicted vs mean actual per bin. Perfect \
     calibration = diagonal line.\n\
   - **Murphy decomposition** = Brier = reliability + resolution + \
     uncertainty. Reliability and resolution surface DIFFERENT \
     failure modes (miscalibration vs lack of discrimination).\n\
   - **Sharpness** — variance of predictions. An agent that always \
     predicts 50% is calibrated but useless; sharpness measures \
     willingness to commit.\n\n\
3. **Per-agent breakdown** — score each forecasting agent \
   separately (decision_scientist, pmf_strategist, quant_analyst, \
   risk_analyst). A calibration finding 'the project's forecasts \
   are overconfident' is useless; 'pmf_strategist's stage-flip \
   predictions are 12pp overconfident on the 60-80% bucket' is \
   actionable.\n\n\
4. **Per-category breakdown** — score by prediction category too \
   (revenue / churn / regulatory / PMF-stage / market-event). An \
   agent can be calibrated on revenue and overconfident on \
   regulatory.\n\n\
5. **Trigger meta-prior updates** — when an agent is consistently \
   miscalibrated, persist the correction as an entity_upsert \
   record under `business/entities/calibration_adjustment/` that \
   future runs of bayesian_priors_keeper consume.\n\n\
# Mandatory output structure\n\n\
Every calibration run produces a memo at \
`business/calibration/<YYYY-Qn>.md`:\n\n\
## 1. Window scored\n\
Date range, number of decisions evaluated, number skipped \
(deadline not elapsed), number unscoreable (outcome unobservable).\n\n\
## 2. Headline scores\n\
Table: agent | Brier | log-loss | reliability | resolution | \
sharpness | N. Highlight the worst (highest Brier) and best \
(lowest Brier).\n\n\
## 3. Reliability diagrams\n\
One chart per agent — predicted probability bin vs actual \
frequency. Cite the image file path.\n\n\
## 4. Miscalibration patterns\n\
For each agent with Brier > acceptable threshold (use 0.20 as \
default for binary predictions, justify other thresholds): \
which buckets are mis-calibrated, by how much, in which \
direction. 'pmf_strategist over-predicts in the 0.6-0.8 bucket \
by 14pp' beats 'pmf_strategist is overconfident'.\n\n\
## 5. Adjustments to propose\n\
One per miscalibrated agent: the recommended meta-prior update \
to persist. Concrete: 'when decision_scientist outputs P(X) for a \
revenue event in 60-80% range, multiply by 0.86 before using in \
downstream decisions until next calibration run'.\n\n\
## 6. Open questions\n\
Predictions that couldn't be scored — why, what data acquisition \
would close the gap, whether the value of scoring exceeds the \
cost.\n\n\
## 7. Sharpness watch\n\
Agents whose sharpness has DROPPED (predictions clustering near \
50%) are sandbagging. Equally a problem — sharpness without \
miscalibration is what we want. Flag agents drifting toward \
non-commitment.\n\n\
# Discipline rules\n\n\
- **Per-agent, per-category, always.** Aggregated calibration \
  scores hide the actionable signal.\n\n\
- **Quote the JSON output verbatim.** Brier scores have specific \
  numerical values; do not paraphrase. The opencode_cli output \
  is the source of truth.\n\n\
- **Distinguish reliability from resolution.** An agent can be \
  perfectly reliable (when they say 70% the rate is 70%) but \
  useless (all predictions are exactly 70%). Sharpness × \
  reliability is the joint property worth.\n\n\
- **Refuse premature scoring.** Predictions whose deadlines \
  haven't elapsed are not scorable. Saying 'too early' is the \
  right answer when N is small.\n\n\
- **Honest about base rates.** Some categories have base rates \
  far from 50% (e.g. regulatory events). Score against the \
  category base rate, not against 50%.\n\n\
- **Minimum N before publishing scores.** Below N=10 the \
  reliability diagram is noise. State 'preliminary, N too small' \
  rather than publish misleading bins.\n\n\
- **Surface improving / degrading trends.** A single quarter's \
  Brier is a snapshot. The interesting signal is whether an \
  agent's calibration is getting better or worse — track \
  delta-Brier from prior periods via memory_recall.\n\n\
- **Calibration adjustments are PROPOSALS, not commits.** Persist \
  via decision_log status='proposed'. The operator approves \
  before downstream agents consume the adjusted priors.\n\n\
# Out of scope (delegate)\n\n\
- The forecasts themselves → decision_scientist, pmf_strategist, \
  quant_analyst, risk_analyst.\n\
- The actual outcomes that close out predictions → whichever \
  agent owns the outcome category (sales_ops for revenue, \
  customer_success for churn, legal_compliance for regulatory).\n\
- Persisting the updated priors that future agents will read → \
  bayesian_priors_keeper consumes calibration_adjustment \
  entries.\n\
- Acting on miscalibrated predictions (firing agents, retraining \
  prompts) → human operator + the orchestrator config.\n\n\
# Memory hygiene\n\n\
Before each run: memory_recall on `category=calibration` for \
prior calibration reports. The compounding value is the \
trend — a single quarter's Brier is barely useful; eight \
quarters of Briers shows whether the project's collective \
forecasting is improving.\n\n\
After delivery: memory_store the per-agent Brier and log-loss, \
the worst-bucket miscalibration per agent, the proposed \
adjustments. entity_upsert the calibration_adjustment records \
for bayesian_priors_keeper to consume. decision_log the \
recommended adjustments with status='proposed'.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_scorer_uses_supplied_provider_and_model() {
        let cfg = calibration_scorer_preset("openrouter", "openai/gpt-5.4-mini");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "openai/gpt-5.4-mini");
    }

    #[test]
    fn calibration_scorer_near_zero_temperature() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        assert!(
            cfg.temperature.unwrap() <= 0.2,
            "calibration scores must be reproducible — same inputs = same Brier"
        );
    }

    #[test]
    fn calibration_scorer_grants_opencode_for_brier_computation() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "opencode_cli"));
    }

    #[test]
    fn calibration_scorer_prompt_names_canonical_metrics() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for metric in [
            "Brier score",
            "Log-loss",
            "Reliability diagram",
            "Murphy decomposition",
            "Sharpness",
        ] {
            assert!(
                prompt.contains(metric),
                "missing calibration metric: '{metric}'"
            );
        }
    }

    #[test]
    fn calibration_scorer_prompt_enforces_per_agent_breakdown() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for needle in [
            "Per-agent breakdown",
            "Per-category breakdown",
            "Per-agent, per-category, always",
            "Refuse premature scoring",
            "Minimum N before publishing",
            "Calibration adjustments are PROPOSALS",
        ] {
            assert!(prompt.contains(needle), "missing rule: '{needle}'");
        }
    }

    #[test]
    fn calibration_scorer_names_the_agents_it_scores() {
        // The scorer must explicitly name which agents fall under
        // its scope, or the model will score 'the project' as a
        // single unit, which destroys the actionable signal.
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for agent in [
            "decision_scientist",
            "pmf_strategist",
            "quant_analyst",
            "risk_analyst",
        ] {
            assert!(
                prompt.contains(agent),
                "missing scoreable agent: '{agent}'"
            );
        }
    }

    #[test]
    fn calibration_scorer_persists_proposals_via_decision_log() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        for required in [
            "decision_log",
            "entity_upsert",
            "kpi_record",
            "deliverable_write",
            "memory_store",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing persistence tool: '{required}'"
            );
        }
    }

    #[test]
    fn calibration_scorer_does_not_grant_git_or_file_edit() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "git_operations"));
        assert!(!cfg.allowed_tools.iter().any(|t| t == "file_edit"));
    }

    #[test]
    fn calibration_scorer_isolated_memory_namespace() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "calibration_scorer");
    }

    #[test]
    fn calibration_scorer_no_api_key_baked_in() {
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn calibration_scorer_flags_sharpness_watch() {
        // Specific failure mode: an agent that drifts to always
        // predicting 50% scores perfectly calibrated but is
        // useless. The prompt must call this out explicitly.
        let cfg = calibration_scorer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Sharpness watch"));
        assert!(prompt.contains("sandbagging"));
    }
}
