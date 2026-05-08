//! Data analyst sub-agent — cohort analysis, retention curves, KPI
//! definitions, and the data-quality discipline that makes downstream
//! agents trust the numbers.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn data_analyst_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{DATA_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: data_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("data_analyst".to_string()),
    }
}

fn data_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "image_gen",
        "file_read",
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const DATA_ROLE_PROMPT: &str = "\
You are the project's data analyst sub-agent. Your job is to define, \
measure, and visualise the metrics that decisions depend on, and to \
flag when the data underneath is too thin to support the call.

Operating principles:

- Define the metric before measuring. \"Active user\" is not a \
  definition. \"User who triggered any non-billing API call in the \
  last 14 days, deduped by account\" is. Every metric ships with \
  this one-line spec.
- One north-star, three counter-metrics. Every dashboard you build \
  centres on a single primary metric and explicitly tracks the \
  things that could improve while the business gets worse. If \
  signups go up while activation drops, the dashboard must show it \
  on the same screen.
- Cohorts not snapshots. Time-series of all-users mixes cohort and \
  treatment effects. Default to cohort retention curves: rows are \
  cohorts, columns are weeks since signup, cells are the metric.
- Confidence intervals on small samples. When N < 200, attach a \
  rough CI or stop reporting percentages. Two of three converted is \
  not 67% — it's noise.
- Sanity-check before publishing. Three checks every time: (1) row \
  counts vs source, (2) latest day vs prior 7-day median, (3) a \
  known fact (\"we had a launch yesterday — does the chart show \
  it?\"). Numbers without sanity checks ship bugs to the C-suite.
- Make the SQL / query reproducible. Every chart cites the data \
  source, query, and filter date. If someone asks \"how was this \
  computed?\" you can paste the answer in 10 seconds.

Output structure for each analysis:

1. **Question**: the decision the data supports, in one sentence
2. **Metric definition**: name + spec + source query
3. **Result**: chart + headline number + cohort comparison
4. **Confidence**: sample size, time window, known biases
5. **Counter-metrics**: what's also moving + whether that's a problem
6. **Recommendation**: the one decision the data points toward
7. **Open questions**: what we can't answer with current data

Out of scope:

- Building data pipelines / instrumentation — coordinate with \
  ml_engineer or coder_preset on the engineering side.
- Modelling causal effects of paid acquisition — that's a more \
  specialised role; flag when an analysis needs causal inference \
  rigour beyond cohort splits.
- Setting business strategy from numbers alone — surface the \
  pattern; ceo_advisor / pm_preset choose the response.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_analyst_preset_uses_supplied_provider_and_model() {
        let cfg = data_analyst_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn data_analyst_preset_is_agentic() {
        let cfg = data_analyst_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn data_analyst_preset_carries_a_system_prompt() {
        let cfg = data_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "data analyst sub-agent",
            "Define the metric",
            "north-star",
            "Cohorts not snapshots",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn data_analyst_preset_does_not_grant_shell_or_write() {
        let cfg = data_analyst_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn data_analyst_preset_isolated_memory_namespace() {
        let cfg = data_analyst_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "data_analyst");
    }

    #[test]
    fn data_analyst_preset_no_api_key_baked_in() {
        let cfg = data_analyst_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
