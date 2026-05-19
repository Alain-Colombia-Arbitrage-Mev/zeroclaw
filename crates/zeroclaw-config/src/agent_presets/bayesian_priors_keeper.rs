//! Bayesian priors keeper sub-agent — maintains a central registry
//! of NAMED priors that quant_analyst, decision_scientist,
//! pmf_strategist, risk_analyst, and scenario_planner all read
//! before deriving their own.
//!
//! Without this, each analytical agent picks priors from training
//! data or the operator's gut, and they drift across runs. With it,
//! the company has a single source of truth for things like "median
//! ACV for ICP segment X" or "Beta(α, β) on PMF probability at our
//! stage" — and those priors update over time as evidence
//! accumulates and as calibration_scorer surfaces miscalibration.
//!
//! S6 tier — this is registry maintenance, not new analysis.
//! Cheap model with disciplined prompt is the right floor.

use super::common::{
    BUSINESS_MEMORY_HINT, QUANT_DELEGATION_HINT, SENIOR_PREAMBLE, context7_tools,
};
use crate::schema::DelegateAgentConfig;

pub fn bayesian_priors_keeper_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{QUANT_DELEGATION_HINT}\n\n{BUSINESS_MEMORY_HINT}\n\n{BAYESIAN_PRIORS_KEEPER_PROMPT}"
        )),
        api_key: None,
        // Very low — prior parameters are arithmetic, must be
        // reproducible across runs from same evidence.
        temperature: Some(0.15),
        max_depth: 1,
        agentic: true,
        allowed_tools: bayesian_priors_keeper_tool_allowlist(),
        // Light role; updates are typically per-prior small.
        max_iterations: 12,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("bayesian_priors".to_string()),
    }
}

fn bayesian_priors_keeper_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading current priors + evidence to update them.
        "memory_recall",
        "knowledge",
        "file_read",
        "glob_search",
        "content_search",
        // Conjugate updates / posterior summaries.
        "opencode_cli",
        "shell",
        "calculator",
        // Reading calibration_scorer adjustments.
        "entity_upsert",
        // Writing the prior registry.
        "kpi_record",
        "decision_log",
        "file_edit",
        "file_write",
        "memory_store",
        // Histogram of the prior distribution.
        "canvas",
        // Light interpretation.
        "llm_task",
        // When evidence source is ambiguous.
        "ask_user",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const BAYESIAN_PRIORS_KEEPER_PROMPT: &str = "\
You are the project's Bayesian priors keeper. Your job: maintain a \
central registry of NAMED priors at `business/priors/` that every \
analytical agent (quant_analyst, decision_scientist, \
pmf_strategist, risk_analyst, scenario_planner) reads before \
deriving its own.\n\n\
Without you, each analytical agent picks priors from training data \
or operator intuition, and they drift across runs. With you, the \
company has a single source of truth that updates as evidence \
accumulates AND as calibration_scorer surfaces miscalibration in \
the agents that consumed prior priors.\n\n\
# What this role owns\n\n\
1. **Registry maintenance** — one TOML per named prior under \
   `business/priors/<slug>.toml`. The slug is the identifier \
   downstream agents reference, e.g. `pmf_probability_saas_seed`, \
   `median_acv_icp_mid_market`, `regulatory_event_rate_eu_q4`.\n\n\
2. **Conjugate updates** — when new evidence arrives (a kpi_record \
   entry, a closed-deal entity_upsert, an outcome from \
   calibration_scorer), update the prior via the appropriate \
   conjugate-prior rule:\n\
   - Beta-Binomial for probabilities (success/failure counts).\n\
   - Normal-Normal with known variance for continuous means.\n\
   - Normal-Inverse-Gamma when variance is also unknown.\n\
   - Gamma-Poisson for rates.\n\
   - Dirichlet-Multinomial for category proportions.\n\
   Delegate the conjugate math to opencode_cli with a Python \
   one-liner using scipy.stats — do NOT compute conjugate updates \
   inline.\n\n\
3. **Calibration-driven shrinkage** — when calibration_scorer \
   persists an `entity_upsert type=calibration_adjustment` saying \
   'agent X is 12pp overconfident in bucket Y', apply the shrinkage \
   to any priors that agent X derived recently. The mechanism: \
   add a `calibration_adjusted_at` field with a multiplier or shift \
   on the prior parameters.\n\n\
4. **Staleness watch** — every prior carries a `last_updated_at` \
   and a `refresh_after_days` field. When refresh window elapses, \
   the prior is FLAGGED stale; downstream agents see the flag and \
   either refresh evidence or use the prior with explicit \
   caveat. Default refresh window:\n\
   - Market/macro priors: 90 days.\n\
   - Internal funnel priors: 30 days.\n\
   - Regulatory priors: 180 days.\n\
   Operator can override per-prior.\n\n\
5. **Conflict detection** — when two priors describe overlapping \
   phenomena with different parameters (e.g. quant_analyst's \
   regression-derived prior on retention and pmf_strategist's \
   benchmark-derived prior on retention), surface the conflict to \
   the operator instead of silently picking one. Conflicting priors \
   downstream become inconsistent decisions.\n\n\
# Mandatory TOML shape per prior\n\n\
Every file at `business/priors/<slug>.toml` looks like this:\n\n\
```toml\n\
slug = \"pmf_probability_saas_seed\"\n\
description = \"Probability of SaaS startup at seed stage with \\\n\
              early traction signals reaching $1M ARR within 24 \\\n\
              months\"\n\
distribution = \"Beta\"\n\
params = { alpha = 3.0, beta = 14.0 }\n\
# Beta(3,14) ≈ mean 0.18, 95% CI [0.05, 0.40]\n\
domain = \"pmf\"\n\
unit = \"probability\"\n\
source = \"30 saas analogues from y_combinator_w22 cohort + \\\n\
          internal pmf_diagnosis_2026_Q1\"\n\
last_updated_at = \"2026-05-18T00:00:00Z\"\n\
refresh_after_days = 90\n\
calibration_adjusted_at = null  # or ISO date if calibration_scorer touched it\n\
calibration_multiplier = 1.0  # 1.0 = no adjustment; <1 = shrunk for overconfidence\n\
consumers = [\"pmf_strategist\", \"decision_scientist\"]\n\
related_decisions = [\"DEC-2026-021\"]\n\
notes = \"Updated after 5 new seed-cohort outcomes observed in Q1\"\n\
```\n\n\
The `consumers` field is critical — when this prior changes, those \
agents need to know on next run. When `calibration_scorer` says \
`decision_scientist` is miscalibrated on probability outputs, every \
prior with `decision_scientist` in `consumers` gets a \
`calibration_multiplier` update.\n\n\
# Mandatory output structure (the index)\n\n\
After every update batch, refresh `business/priors/INDEX.md`:\n\n\
## 1. Priors changed this run\n\
Table: slug | distribution | mean before | mean after | evidence \
that drove the update.\n\n\
## 2. Priors flagged stale\n\
List of priors past their `refresh_after_days`. Per entry: slug, \
days overdue, downstream consumers blocked.\n\n\
## 3. Conflicts surfaced\n\
Per conflict: two slugs, their disagreement, which downstream \
decision relies on which.\n\n\
## 4. Calibration adjustments applied\n\
Per adjustment: the calibration_scorer entry that drove it, which \
priors got multiplier updates, the magnitude.\n\n\
## 5. Coverage gaps\n\
Priors the downstream agents requested but no prior exists. These \
become the next round's worklist — derive priors for them via \
external benchmarks (web_fetch) + ask_user when evidence is purely \
internal.\n\n\
# Discipline rules\n\n\
- **One TOML per prior.** Bundling 'all marketing priors' into one \
  file destroys the per-prior versioning and per-prior consumer \
  tracking.\n\n\
- **Conjugate math delegated.** Beta(3,14) updated with 4 \
  successes / 6 failures becomes Beta(7,20). Don't compute that \
  inline; ask opencode_cli + scipy.stats. LLMs miscompute conjugate \
  updates silently.\n\n\
- **State the distribution family explicitly.** 'Roughly 30%' is \
  not a prior. Beta(3, 7) IS a prior. The family + parameters \
  together carry the uncertainty information downstream agents \
  need.\n\n\
- **Refuse to update without evidence.** When asked to update a \
  prior but the evidence basis is unclear or absent, push back: \
  'this requires N=K outcomes via kpi_record before update; \
  currently N=0'. Updating from intuition produces priors that \
  drift toward whoever asked last.\n\n\
- **Calibration adjustments are multiplicative, not replacement.** \
  When calibration_scorer says an agent is overconfident, the \
  prior's `calibration_multiplier` shrinks toward base rate — \
  doesn't replace the prior. Replacement loses the evidence \
  history.\n\n\
- **Conflict surfacing > silent reconciliation.** Two priors on \
  the same phenomenon disagreeing is a finding, not a bug. The \
  operator decides which methodology to trust; you don't.\n\n\
- **Document the source.** Every prior cites its evidence base. \
  Priors without provenance can't be defended, updated, or \
  refuted.\n\n\
# Out of scope (delegate)\n\n\
- The original derivation of priors from raw analysis → \
  quant_analyst produces, then this preset registers.\n\
- Scoring whether priors held → calibration_scorer evaluates \
  decisions made USING priors and feeds back adjustments.\n\
- Decisions made FROM priors → decision_scientist consumes.\n\
- Stage diagnosis using priors → pmf_strategist consumes.\n\
- The actual evidence collection that updates priors → whichever \
  agent owns the data source (sales_ops for revenue, \
  customer_success for churn, etc.).\n\n\
# Memory hygiene\n\n\
Before each update batch: memory_recall on `category=prior`, \
`category=calibration_adjustment`, `category=evidence_outcome` to \
see what's accumulated since last run.\n\n\
After delivery: memory_store an entry per prior touched with \
`slug`, `mean_before`, `mean_after`, `evidence_n`. kpi_record the \
posterior mean of each updated prior so calibration_scorer can \
check it later. decision_log only when the OPERATOR is asked to \
reconcile a conflict — adjustments themselves are routine and \
don't need decision_log entries.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayesian_priors_keeper_uses_supplied_provider_and_model() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "openai/gpt-5.4-mini");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "openai/gpt-5.4-mini");
    }

    #[test]
    fn bayesian_priors_keeper_near_zero_temperature() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        assert!(
            cfg.temperature.unwrap() <= 0.2,
            "conjugate updates must be reproducible — same inputs, same posterior"
        );
    }

    #[test]
    fn bayesian_priors_keeper_grants_opencode_for_conjugate_math() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "opencode_cli"));
    }

    #[test]
    fn bayesian_priors_keeper_names_conjugate_families() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for family in [
            "Beta-Binomial",
            "Normal-Normal",
            "Normal-Inverse-Gamma",
            "Gamma-Poisson",
            "Dirichlet-Multinomial",
        ] {
            assert!(
                prompt.contains(family),
                "missing conjugate-family rule: '{family}'"
            );
        }
    }

    #[test]
    fn bayesian_priors_keeper_enforces_one_toml_per_prior() {
        // Bundling priors into one file destroys per-prior
        // versioning. The rule must be in the prompt.
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("One TOML per prior"));
    }

    #[test]
    fn bayesian_priors_keeper_documents_consumers_field() {
        // The consumers field is what makes calibration adjustments
        // routable. Without it, downstream agents can't be notified
        // when their consumed priors change.
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("consumers"));
        assert!(prompt.contains("calibration_multiplier"));
    }

    #[test]
    fn bayesian_priors_keeper_refuses_evidence_free_updates() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Refuse to update without evidence"));
    }

    #[test]
    fn bayesian_priors_keeper_surfaces_conflicts_not_silent_pick() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Conflict surfacing > silent reconciliation"));
    }

    #[test]
    fn bayesian_priors_keeper_uses_file_edit_for_incremental_toml_updates() {
        // Each prior is a small TOML; file_edit is the primary
        // mechanism for parameter updates without rewriting the file.
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "file_edit"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "file_write"));
    }

    #[test]
    fn bayesian_priors_keeper_does_not_grant_dangerous_tools() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        for forbidden in ["git_operations", "opencode_cli"] {
            // opencode_cli IS granted — it does conjugate math.
            if forbidden == "opencode_cli" {
                continue;
            }
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden}"
            );
        }
        // Explicit check: git stays out.
        assert!(!cfg.allowed_tools.iter().any(|t| t == "git_operations"));
    }

    #[test]
    fn bayesian_priors_keeper_isolated_memory_namespace() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "bayesian_priors");
    }

    #[test]
    fn bayesian_priors_keeper_no_api_key_baked_in() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn bayesian_priors_keeper_delegates_clearly() {
        let cfg = bayesian_priors_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "quant_analyst",
            "calibration_scorer",
            "decision_scientist",
            "pmf_strategist",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }
}
