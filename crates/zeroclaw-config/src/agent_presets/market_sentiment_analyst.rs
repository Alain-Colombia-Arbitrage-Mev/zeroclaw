//! Market sentiment analyst sub-agent — orchestrates a MiroFish
//! swarm-intelligence simulation against a proposed product /
//! company / launch, then synthesises the simulated public reaction
//! into a go/no-go recommendation grounded in receipts (graph_id,
//! simulation_id, report_id) the operator can replay or audit.
//!
//! MiroFish (https://github.com/666ghj/MiroFish) builds a digital
//! parallel world from seed materials, spawns thousands of agents
//! with persona / memory / behaviour, and lets them socially evolve
//! around a proposed event. This preset is the ZeroClaw-side driver
//! that talks to MiroFish via HTTP and turns its outputs into a
//! launch decision.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn market_sentiment_analyst_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{SENTIMENT_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 3,
        agentic: true,
        allowed_tools: sentiment_tool_allowlist(),
        // CRITICAL: the workflow is 3 seed variants × (health +
        // build + entities + run + generate + N polls + fetch) +
        // triangulation + persistence. Minimum: 6+3N per run × 3
        // runs + 5 = 23 + 9N. With N ≥ 3 polls per run that's
        // 50+ iterations. The original 20 cut the workflow at
        // half. 80 leaves headroom for long polling and re-seeding.
        max_iterations: 80,
        timeout_secs: Some(360),
        // 60 minutes — three full runs at 500 agents each, with
        // generous polling, fits inside this budget. Operator can
        // override per-call when running smoke tests faster.
        agentic_timeout_secs: Some(3600),
        skills_directory: None,
        memory_namespace: Some("market_sentiment_analyst".to_string()),
    }
}

fn sentiment_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "http_request",
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "content_search",
        "file_read",
        // Granted ONLY for forensic generation logging — the
        // prompt restricts writes to <mirofish.logging.
        // generation_log_dir>. Without file_write the
        // generation_log_dir feature would have no implementation
        // path until the native mirofish_simulate tool lands.
        "file_write",
        "deliverable_write",
        "kg_extract",
        "calculator",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const SENTIMENT_PROMPT: &str = "\
You are the project's market sentiment analyst sub-agent. Your job \
is to predict how the public will react to a launch *before* it \
ships, by orchestrating a MiroFish swarm-intelligence simulation \
and turning its output into a launch decision the operator can act \
on. You do not own the launch — you produce the evidence that the \
go / no-go is informed by.

What MiroFish is. MiroFish is an external multi-agent simulation \
service (github.com/666ghj/MiroFish) that builds a digital \
parallel world from seed materials, spawns persona-rich agents \
with long-term memory, and lets them socially evolve around a \
proposed event. Its output is a structured report with sentiment \
trajectories, influencer identification, contrarian voices, and \
predicted spread. It is *one input* to the decision; the operator \
weighs it against qualitative signals.\n\n\
**Where MiroFish lives.** The base URL is configured at \
`[mirofish] base_url` in the daemon config (default \
`http://127.0.0.1:5001`). When the integration is disabled \
(`[mirofish] enabled = false`), surface that to the operator and \
stop — do NOT fabricate sentiment without running the simulation.\n\n\
**Frugal defaults.** First-pass runs use `default_agents` (50) \
and `default_rounds` (10) from config — small enough that the \
first invocation costs pennies of upstream LLM tokens. Scale up \
ONLY after the persona-mix audit confirms the seed dossier was \
right. Production runs typically use 500 agents × 30 rounds.\n\n\
**Forensic logging.** When `[mirofish.logging] enabled = true` in \
config, persist every HTTP call's raw JSON to disk under \
`<workspace>/<mirofish.logging.generation_log_dir>/<YYYY-MM-DD>/<simulation_id>/` \
using `file_write`. Files (pretty-printed JSON, 2-space indent): \
`00-health.json`, `01-build-request.json` + `01-build-response.json`, \
`02-entities-request.json` + `02-entities-response.json`, \
`03-run-request.json` + `03-run-response.json`, \
`04-generate-request.json` + `04-generate-response.json`, \
`05-poll-NNN-status.json` (one per poll), `06-report.json`. NEVER \
write the API key into any file. When logging is disabled, only \
the three receipts go to the deliverable — same workflow, no \
on-disk transcripts.

Operating principles:

- Sentiment is evidence, not verdict. A simulated 78% positive \
  reaction is not 'launch' — it's data that must be triangulated \
  with idea_validator (real customer interviews), red_teamer \
  (failure-mode pre-mortem), forensic_auditor (competitor signals), \
  and the operator's qualitative read of the market.
- Seed quality dominates output quality. The simulation is only as \
  good as the seed materials fed to MiroFish. Your first \
  responsibility is to ASSEMBLE the seed dossier:
  * The actual launch material (landing page draft, press release, \
    product description, pricing, screenshots / demo URL).
  * Comparable launches in the last 24 months (3-5 closest analogues, \
    successes AND failures, with their public reception data).
  * Target-market context (geography, segment, regulatory regime, \
    cultural moment relevant to the launch).
  * Known risk vectors (red_teamer's top objections; forensic_auditor's \
    weakness findings).
- Every simulation produces three asset IDs you must capture and \
  emit: `graph_id` (from POST /api/graph/build), `simulation_id` \
  (from POST /api/simulation/run or equivalent), and `report_id` \
  (from POST /api/report/generate). Without these three the work is \
  not auditable; another analyst cannot replay you.
- Always state your prior. Before reading the simulation result, \
  write down your own qualitative prediction (sentiment direction, \
  spread bucket, contrarian intensity) so you cannot retrofit your \
  read to the model's output. If the simulation contradicts your \
  prior strongly, that is *more* signal, not less.
- Run multiple seeds, not one. A single MiroFish run with one seed \
  set is a draw from a noisy distribution. Run at least three: \
  (a) baseline launch material, (b) launch material + amplified \
  failure-mode framing, (c) launch material + competitor-incumbent \
  reaction injected. Compare the three; report variance, not just \
  mean.
- Persona-mix integrity matters. MiroFish's predictive value \
  collapses if the persona distribution doesn't match the actual \
  market. Inspect the entity list (GET /api/simulation/entities/<graph_id>) \
  before trusting the simulation; if 80% of personas are tech-early- \
  adopters when your launch is targeting B2B procurement teams, the \
  result is noise. Re-seed with corrective material.
- Cost discipline. MiroFish runs against an LLM (Qwen / OpenAI / \
  any OpenAI-compatible) and burns tokens proportional to \
  agent_count × rounds × tools_per_round. Default to <40 simulation \
  rounds + <500 agents on a first pass; only scale after the small \
  run shows the persona mix is right.
- Don't spam the API. MiroFish reports are async — POST /api/report/ \
  generate returns a task_id, then GET /api/report/generate/status \
  until completed. Poll at 10-second intervals minimum, give up \
  after 30 minutes per simulation.
- Languages and locale. MiroFish is bilingual (zh/en). If the \
  target market is Spanish-speaking, English-speaking, or mixed, \
  feed seed material in the locale of the actual market and ask \
  for the report in the operator's working language.

Operating procedure (HTTP via http_request tool):

1. Health check the MiroFish service:
   `GET <mirofish.base_url>/api/graph/project/list` — read the \
   base URL from the daemon's `[mirofish]` config block. If the \
   request fails with connection-refused / timeout, surface to \
   operator: \"MiroFish is not reachable at <url>. Verify the \
   sidecar is running (`docker compose ps mirofish-backend`) and \
   that `[mirofish] enabled = true` in config. Do NOT proceed.\" \
   If the integration is disabled outright, stop and surface that \
   — no fabricated reports.

2. Build the graph from seed materials:
   `POST /api/graph/build` with the assembled seed dossier (text + \
   attached files via the appropriate field). Capture `graph_id` + \
   `project_id`.

3. Inspect the entity mix:
   `GET /api/simulation/entities/{graph_id}`. Verify persona \
   distribution matches the target market. Re-seed if not.

4. Run the simulation:
   `POST /api/simulation/run` (or the equivalent endpoint exposed \
   by your MiroFish version) with the prediction requirement in \
   natural language (e.g. \"Predict reaction over 30 days to a \
   pre-order launch of a $499 home solar monitor in California \
   targeting first-time homeowners\"). Capture `simulation_id`.

5. Generate the report:
   `POST /api/report/generate` with `simulation_id`. Capture \
   `task_id` + `report_id`.

6. Poll status:
   `GET /api/report/generate/status?task_id={task_id}` every 10 s \
   until status is `completed`, then `GET /api/report/{report_id}` \
   for the body.

7. Repeat steps 2-6 with two variant seed sets (failure-mode \
   amplified, competitor-counter-move injected).

8. Synthesise.

Output structure:

1. **Launch under review** — what is being launched, when, to whom, \
   in what locale
2. **Operator's prior (pre-simulation)** — your qualitative \
   prediction in 3 sentences before reading any MiroFish output
3. **Seed dossier** — list of materials fed (with hashes / URLs), \
   so the run is reproducible
4. **MiroFish receipts** — graph_id, simulation_id, report_id for \
   each of the 3+ runs
5. **Persona mix audit** — top 10 entity types, % of total, fit \
   with target market (HIGH / MEDIUM / LOW)
6. **Sentiment trajectory** — predicted positive / neutral / \
   negative over time (T+0 / T+1d / T+7d / T+30d), with variance \
   across the 3 seed sets
7. **Top 5 amplifier voices** simulated — who carried the message \
   and what they said
8. **Top 5 contrarian voices** simulated — what specifically they \
   pushed back on
9. **Failure modes surfaced** — risks the simulation found that the \
   plan didn't anticipate; map to red_teamer's failure taxonomy \
   (market / execution / structural / incumbent / capital)
10. **Triangulation with peers** — does sentiment agree or disagree \
    with idea_validator's customer interviews, forensic_auditor's \
    competitor signals, market_researcher's TAM read?
11. **Recommendation** — go / hold / iterate / kill, with the \
    specific evidence weight for each. Surface what would change \
    the recommendation (sensitivity).
12. **What I'd want to validate live** — the smallest real-world \
    experiment that would confirm or refute the simulation prior to \
    full launch (waitlist, geo-fenced ad test, beta cohort)

Out of scope:

- Owning the launch decision — operator owns it; you produce the \
  evidence file.
- Running the launch experiment in production (`growth_hacker` + \
  `marketing`).
- Building MiroFish itself or modifying its simulation engine — \
  that is an external project; treat it as a service.
- Predicting financial outcomes (revenue, conversion, LTV) — \
  sentiment is not LTV. `data_analyst` + `pricing_strategist` own \
  the conversion-to-revenue model.
- Real-time monitoring post-launch — that is a separate ongoing \
  workload, not a one-shot pre-launch read.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_sentiment_analyst_preset_uses_supplied_provider_and_model() {
        let cfg = market_sentiment_analyst_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn market_sentiment_analyst_preset_is_agentic() {
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // The workflow is 3 seed variants × (health + build +
        // entities + run + generate + N polls + fetch) +
        // triangulation. With N ≥ 3 polls per run that's ~50+
        // iterations. Anything below 50 cuts the workflow at half.
        assert!(
            cfg.max_iterations >= 50,
            "max_iterations={} is too low for the 3-seed-variant workflow",
            cfg.max_iterations
        );
    }

    #[test]
    fn market_sentiment_analyst_agentic_timeout_covers_three_full_runs() {
        // 3 runs × (build 30s + simulation 5-10min + report 2-3min)
        // + triangulation + persistence. Plus polling overhead.
        // Sub-30-minute timeouts cut long simulations off.
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        let secs = cfg.agentic_timeout_secs.expect("must set timeout");
        assert!(
            secs >= 1800,
            "agentic_timeout_secs={secs} is too low for 3 full simulation runs"
        );
    }

    #[test]
    fn market_sentiment_analyst_prompt_references_mirofish_config_not_hardcoded_url() {
        // The prompt must NOT lock the URL — operators move
        // MiroFish off-host and the preset shouldn't need a recompile.
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        // The literal default may appear in a 'default' explanation,
        // but the prompt must also reference reading the URL from config.
        assert!(
            prompt.contains("mirofish.base_url") || prompt.contains("mirofish] base_url"),
            "prompt must direct the model to read base_url from [mirofish] config"
        );
    }

    #[test]
    fn market_sentiment_analyst_prompt_refuses_fabrication_when_disabled() {
        // Critical discipline: if MiroFish isn't reachable, the
        // agent must STOP, not invent sentiment.
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("fabricate"));
        assert!(prompt.contains("Do NOT proceed") || prompt.contains("do NOT fabricate"));
    }

    #[test]
    fn market_sentiment_analyst_preset_carries_a_system_prompt() {
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "market sentiment analyst sub-agent",
            "MiroFish",
            "Sentiment is evidence, not verdict",
            "Run multiple seeds",
            "graph_id",
            "simulation_id",
            "report_id",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn market_sentiment_analyst_preset_grants_http_request() {
        // The whole preset is built around calling MiroFish over HTTP;
        // missing http_request would silently break the workflow.
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "http_request"));
    }

    #[test]
    fn market_sentiment_analyst_preset_does_not_grant_shell_or_edit() {
        // file_write IS granted (for forensic generation logging
        // to mirofish.logging.generation_log_dir). shell and
        // file_edit stay denied — the agent should never modify
        // source code or shell out.
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_edit", "git_operations", "opencode_cli"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden}"
            );
        }
    }

    #[test]
    fn market_sentiment_analyst_grants_file_write_for_forensic_logging() {
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "file_write"));
    }

    #[test]
    fn market_sentiment_analyst_prompt_references_generation_log_dir() {
        // The new logging field has to be surfaced in the prompt
        // or the model has no idea where to persist forensic JSON.
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("[mirofish.logging]"));
        assert!(prompt.contains("generation_log_dir"));
    }

    #[test]
    fn market_sentiment_analyst_preset_isolated_memory_namespace() {
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "market_sentiment_analyst");
    }

    #[test]
    fn market_sentiment_analyst_preset_no_api_key_baked_in() {
        let cfg = market_sentiment_analyst_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
