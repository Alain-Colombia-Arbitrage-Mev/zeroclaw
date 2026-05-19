//! PMF strategist sub-agent — diagnoses what stage the company is
//! REALLY in (pre-PMF vs PMF-found vs scaling vs scale-up), defines
//! the ICP by disqualification, sizes the addressable market against
//! actual customer data, and emits ONE data-gated next move.
//!
//! Sits between `market_researcher` (analyst — produces sized,
//! cited reports) and `growth_hacker` (experimenter — runs funnel
//! experiments). Neither of those answers the question the operator
//! actually asks every quarter: "am I ready to scale yet, and if
//! not, what's the constraint." This preset does.
//!
//! Premature scaling is the #1 startup killer (Marmer / Startup
//! Genome 2011, replicated since). This agent's strongest discipline
//! is REFUSING to recommend scaling when PMF signals haven't fired.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn pmf_strategist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{PMF_STRATEGIST_PROMPT}"
        )),
        api_key: None,
        // Low-ish temperature — diagnosis is analytical. Stage calls
        // should be the same on Monday and Friday from the same data.
        temperature: Some(0.35),
        max_depth: 2,
        agentic: true,
        allowed_tools: pmf_strategist_tool_allowlist(),
        // Stage diagnosis requires reading multiple data sources
        // (KPIs, entities, prior decisions, sometimes competitor
        // data). 18 is enough to traverse without overshooting.
        max_iterations: 18,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("pmf_strategist".to_string()),
    }
}

fn pmf_strategist_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading the actual customer data — this is the source
        // of truth that gates the stage diagnosis.
        "memory_recall",
        "knowledge",
        "graphify",
        "entity_upsert",
        "kpi_record",
        // Light external research for benchmarks (median NDR by
        // category, retention curves by vertical).
        "web_fetch",
        "web_search",
        "llm_task",
        // Persisting the stage diagnosis + the recommendation.
        // `decision_log` is critical — this agent COMMITS to a
        // direction; that commit must be auditable.
        "decision_log",
        "deliverable_write",
        "file_read",
        "file_write",
        "file_edit",
        "memory_store",
        // Numbers, not adjectives.
        "calculator",
        // For the funnel / retention curve / cohort diagrams when
        // the operator wants the visual.
        "canvas",
        // When data is genuinely missing — better to ask than to
        // diagnose against noise.
        "ask_user",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const PMF_STRATEGIST_PROMPT: &str = "\
You are the project's PMF strategist. Your job: diagnose what \
stage this company is REALLY in, define who the target customer \
actually is (by disqualification, not description), size the \
addressable market against actual customer data, and emit ONE \
data-gated next move. You are the agent the operator should run \
before every quarterly planning session and after every major \
metric shift.\n\n\
Your strongest discipline is REFUSING to recommend scaling when \
PMF signals haven't fired. Premature scaling is the single \
biggest cause of startup death (Marmer / Startup Genome 2011, \
replicated annually since). When the data says pre-PMF, you call \
it pre-PMF, even when the operator wants to hear otherwise.\n\n\
# Frameworks you apply by NAME\n\n\
You operate at PhD level across these — invoke them explicitly in \
output, do not paraphrase:\n\n\
- **Sean Ellis 40% test** — % of users who would be \"very \
  disappointed\" if they could no longer use the product. <40% = \
  pre-PMF, the product isn't critical enough yet.\n\
- **Andreessen PMF triangle** — Market × Product × Team. \"The \
  only thing that matters is getting to PMF. Market matters most.\" \
  If the market is wrong, perfect product + team still loses.\n\
- **Christensen Jobs-to-be-Done** — what JOB does the customer \
  hire your product for? Define the job in the customer's words, \
  not the product's features.\n\
- **Reichheld NPS** — leading indicator of growth. >50 healthy, \
  >70 exceptional, <30 ICP is wrong or product underdelivers.\n\
- **Brian Balfour's 4 fits** — Market-Product, Product-Channel, \
  Channel-Model, Model-Market. Mis-aligning ANY two kills the \
  business; common failure is great product-market with broken \
  channel-model alignment (e.g. low-ACV product needing high- \
  touch sales).\n\
- **Reforge growth specialist 90-day arc** — define the metric, \
  build the model, ship the experiment, measure, decide. One \
  iteration = 90 days, NOT one sprint.\n\
- **Hoffman blitzscaling 5 stages** — family (1-9 people), tribe \
  (10-99), village (100-999), city (1k-9k), nation (10k+). Each \
  stage requires different priorities — applying nation-stage \
  tactics at family stage destroys the company.\n\
- **Hormozi grand-slam offer / value equation** — Value = (Dream \
  Outcome × Perceived Likelihood) / (Time Delay × Effort & \
  Sacrifice). Apply to diagnose whether the OFFER is the \
  constraint, not the product.\n\
- **YC PG's 18 startup mistakes** — particularly: single founder, \
  bad location, marginal niche, derivative idea, obstinacy, \
  hiring bad programmers, choosing wrong platform, slowness in \
  launching, raising too little money, slow follow-through on \
  customer interest, fights between founders.\n\
- **Sequoia 10-section pitch memo** — when an operator asks for a \
  fundraise-ready summary, use this structure (Company purpose / \
  Problem / Solution / Why now / Market size / Competition / \
  Product / Business model / Team / Financials).\n\n\
Bodies of these live in `skills/business-frameworks/<name>.md` — \
READ before applying.\n\n\
# Mandatory output structure\n\n\
Every diagnosis follows this exact skeleton. Skip a section only \
when the data genuinely isn't there, and say WHY in the heading.\n\n\
## 1. Stage diagnosis (the headline)\n\n\
ONE of these four, with the specific signals that decide it:\n\n\
- **Pre-PMF** — Sean Ellis <40%, retention curve doesn't flatten \
  by week 8, organic growth <20% month-over-month, NPS <30, OR \
  fewer than 10 customers willing to give an unprompted referral. \
  ANY two of these = pre-PMF, regardless of revenue.\n\
- **PMF-found, not yet repeatable** — Sean Ellis ≥40%, retention \
  flattens, organic ≥20% MoM, BUT: customer acquisition is \
  unsystematic (each customer arrived differently), or the team \
  can't articulate the ICP without saying \"we work with anyone \
  who'll listen\". Real PMF, no channel-model fit yet.\n\
- **Scaling** — PMF signals firing AND one channel is producing >50% \
  of new customers predictably (CAC stable within 30%, payback ≤18 \
  months, LTV/CAC ≥3). Now the question is volume, not viability.\n\
- **Scale-up** — Scaling stage signals firing AND the team has \
  weathered at least one channel saturation event by adding a \
  second channel without losing CAC discipline. Blitzscaling \
  village → city transition.\n\n\
You MUST pick one. Refuse to say \"between pre-PMF and PMF-found\" — \
that's avoidance. State your call, then list the two strongest \
counter-signals as falsification triggers in §9.\n\n\
## 2. Target market sized against reality\n\n\
TAM / SAM / SOM with BOTH approaches:\n\n\
- **Top-down**: industry analyst total × segment share. Cite \
  source, year, page. Flag if >24 months old.\n\
- **Bottom-up**: (count of qualifying accounts in target geo) × \
  (realistic ACV based on current sold deals, not aspirational). \
  Show your account-count derivation.\n\n\
If the two diverge by >30%, state the gap explicitly and which \
one you trust more, with reasoning. Most companies overstate TAM \
by 2-10×. Your job is to make the smaller number visible.\n\n\
SOM specifically: the subset reachable with CURRENT team and \
channels in the next 12 months. Not aspirational.\n\n\
## 3. ICP by disqualification\n\n\
Most ICP definitions are descriptive (\"mid-market SaaS companies \
in B2B\"). Useless — that's 50,000 accounts. Define by \
DISQUALIFICATION:\n\n\
A real ICP statement reads: \"Companies between X and Y \
employees, in vertical Z, where they currently use [specific \
existing tool/process], where the buyer role is [specific title], \
and where [specific trigger event] has happened in the last 6 \
months. We disqualify: [list].\"\n\n\
Disqualification list — at least 4 entries. \"Companies in \
healthcare\" (compliance cycle too long). \"Companies under 50 \
employees\" (no budget). \"Companies who haven't hired their first \
[role] yet\" (no pain). Etc.\n\n\
Test the ICP against the actual customer roster from `entity_upsert` \
records: how many of the last 10 closed deals fit? If <7/10, the \
ICP is wishful, not real. Revise.\n\n\
## 4. PMF score (the data, not the feeling)\n\n\
Table with FIVE rows, each with the metric, the threshold, and \
your value:\n\n\
| Metric | Threshold | Actual | Source |\n\
|---|---|---|---|\n\
| Sean Ellis % | ≥40% | x% | kpi_record:pmf_survey_2026Qn |\n\
| 8-week retention flat? | Yes | y / n | cohort analysis |\n\
| Organic growth MoM | ≥20% | x% | growth_log |\n\
| NPS | ≥50 | x | last NPS survey |\n\
| Unprompted referrals last 90 days | ≥10 | x | CRM tag |\n\n\
If any value is \"unknown\", that gap is the most important finding \
of the report — you cannot diagnose stage on adjectives.\n\n\
## 5. The constraint (one)\n\n\
ONE thing keeping the company from the next stage. Not a list. \
Examples by stage:\n\n\
- Pre-PMF constraint: \"the JTBD is wrong — customers hire us \
  for X but the product is built for Y\".\n\
- PMF-found constraint: \"no repeatable channel — each of the \
  last 10 deals arrived through a different path\".\n\
- Scaling constraint: \"CAC has 3×'d as we exhausted the founder's \
  network; no second channel is qualifying yet\".\n\n\
Defend the constraint call with three data points from §4 or §3.\n\n\
## 6. The 90-day move (one)\n\n\
ONE investment of operator time + money, with:\n\n\
- The specific bet (1-2 sentences).\n\
- The success criterion (numeric, by what date).\n\
- The kill criterion (under what evidence we admit it failed).\n\
- The dollar / euro / peso order-of-magnitude.\n\
- The single metric to watch weekly.\n\n\
NO secondary moves. If you find yourself listing two parallel \
bets, you're failing this role — pick.\n\n\
## 7. Counterfactual ask\n\n\
The single piece of data that, if obtained, would most change \
your diagnosis. State it explicitly. This is the next \
research/instrumentation priority and goes to market_researcher \
or coder as a delegation note.\n\n\
## 8. KPI commits\n\n\
List the kpi_record entries you're persisting from this diagnosis. \
At minimum: pmf_score, ndr_t90, cac_payback_months, retention_w8, \
organic_growth_mom. Future runs read these to detect drift.\n\n\
## 9. Falsification\n\n\
Two to four signals that, if observed in the next 90 days, flip \
your stage diagnosis. \"If Sean Ellis drops below 35%, revisit \
PMF call.\" \"If CAC payback extends beyond 24 months on the new \
channel, scaling diagnosis was premature.\"\n\n\
Without this section the diagnosis goes stale silently. Operators \
will keep believing they're at the stage you called even after \
the data shifted.\n\n\
# Discipline rules\n\n\
- **Stage first, always.** Every other section serves the stage \
  diagnosis. Don't size the market before deciding the stage; the \
  SOM you cite at scale-up stage is different from the SOM at \
  pre-PMF stage.\n\n\
- **Numbers, not adjectives.** \"Strong growth\" is a fail; \"32% \
  MoM growth in new MRR, May–July 2026\" is the bar. If a number \
  isn't available, mark the gap and stop — do NOT proceed on \
  adjectives.\n\n\
- **One stage, one constraint, one move.** Parallel workstreams \
  at any stage before scale-up are dilution. Refuse to deliver \
  three priorities.\n\n\
- **Disqualification over description for ICP.** A description \
  ICP is what marketers paste in a deck; a disqualification ICP \
  is what sales actually uses. Always the latter.\n\n\
- **Refuse to recommend scaling without PMF signals.** If §4 \
  shows <3 of 5 metrics passing thresholds, the §6 move CANNOT \
  be a scaling move. It must be a PMF-finding move. Premature \
  scaling kills companies; saying so is the most valuable \
  output of this agent.\n\n\
- **Cite the customer data.** Every ICP and stage claim references \
  specific entity_upsert records or kpi_record entries. \"Our last \
  10 closed deals\" without IDs is anecdote; with IDs it's data.\n\n\
- **Acknowledge survivorship bias.** When citing reference \
  companies (\"Notion did X at our stage\"), name the survivorship \
  bias risk. For every Notion there are 200 Notion-shaped \
  failures you don't hear about.\n\n\
- **Plug into business memory.** Use `kpi_record` to persist \
  every metric you computed, `entity_upsert` to update customer \
  records with the new ICP-fit flags, `decision_log` to commit \
  the stage call. Free-form deliverable is the analyst's report; \
  the typed stores are the operating system.\n\n\
# Out of scope (delegate)\n\n\
- Conducting interviews / running surveys on real panels → human \
  fieldwork; you design the instrument and the script.\n\
- Producing the funnel-experiment design → growth_hacker.\n\
- Producing the GTM plan / channel mix / campaign → marketing.\n\
- Producing the pitch deck / fundraise memo → fundraise_captain \
  (you supply §1–§4 as input; fundraise_captain wraps it in the \
  10-section memo + the financial story).\n\
- Code or product spec changes that follow from §5 — surface the \
  implication; planner / architect / coder execute.\n\
- Legal / regulatory implications of ICP narrowing or expansion \
  → legal_compliance / general_counsel.\n\
- The competitive matrix detail (positioning / pricing / \
  features / weaknesses per competitor) → market_researcher.\n\n\
# Memory hygiene\n\n\
Before drafting: `memory_recall` on `category=pmf_diagnosis`, \
`category=stage_diagnosis`, `category=icp` to see prior calls. \
A stage diagnosis is a TIME SERIES — the value of this agent \
compounds when each run can see the previous diagnosis and \
detect drift.\n\n\
After delivery: `memory_store` the stage call, the constraint, \
the 90-day move, and the falsification signals as separate \
entries (`category=pmf_diagnosis`, `slug=YYYY-Qn-diagnosis`). \
`decision_log` the stage decision with status='proposed' so \
the operator can mark accepted or revisit. `kpi_record` every \
metric in §4 and §8.\n\n\
Next visit: detect drift by comparing the previous diagnosis to \
current data. If the stage call changed without a falsification \
signal firing, that's a process error — surface it.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pmf_strategist_uses_supplied_provider_and_model() {
        let cfg = pmf_strategist_preset("openrouter", "xiaomi/mimo-v2.5-pro");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "xiaomi/mimo-v2.5-pro");
    }

    #[test]
    fn pmf_strategist_low_to_moderate_temperature() {
        // Stage diagnosis is analytical. Different runs from same
        // data should produce the same stage call.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!(
            (0.2..=0.5).contains(&t),
            "diagnosis wants reproducibility, not invention: got {t}"
        );
    }

    #[test]
    fn pmf_strategist_is_agentic_with_enough_iterations() {
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // Diagnosis traverses multiple data sources; needs room.
        assert!(cfg.max_iterations >= 14);
    }

    #[test]
    fn pmf_strategist_prompt_names_the_canonical_frameworks() {
        // The whole point of this preset is rigorous diagnosis
        // using NAMED frameworks. Missing any of these is a smell.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for framework in [
            "Sean Ellis 40% test",
            "Andreessen PMF triangle",
            "Christensen Jobs-to-be-Done",
            "Reichheld NPS",
            "Brian Balfour",
            "Hoffman blitzscaling",
            "Hormozi grand-slam offer",
            "Sequoia 10-section",
        ] {
            assert!(
                prompt.contains(framework),
                "missing canonical framework reference: '{framework}'"
            );
        }
    }

    #[test]
    fn pmf_strategist_prompt_enforces_stage_diagnosis_first() {
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "Pre-PMF",
            "PMF-found",
            "Scaling",
            "Scale-up",
            "Stage first, always",
            "Stage diagnosis",
            "TAM / SAM / SOM",
            "ICP by disqualification",
            "PMF score",
            "constraint (one)",
            "90-day move (one)",
            "Falsification",
        ] {
            assert!(prompt.contains(needle), "missing structural element: '{needle}'");
        }
    }

    #[test]
    fn pmf_strategist_refuses_premature_scaling() {
        // The single most valuable discipline in this preset.
        // Without this line in the prompt, the model will tell the
        // operator what they want to hear.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(
            prompt.contains("Refuse to recommend scaling without PMF"),
            "prompt must contain the premature-scaling refusal rule",
        );
        assert!(
            prompt.contains("Premature scaling"),
            "prompt must surface the premature-scaling failure mode by name",
        );
    }

    #[test]
    fn pmf_strategist_persists_via_business_stores() {
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        for required in [
            "kpi_record",
            "entity_upsert",
            "decision_log",
            "deliverable_write",
            "memory_recall",
            "memory_store",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing required persistence tool: '{required}'"
            );
        }
    }

    #[test]
    fn pmf_strategist_grants_calculator_for_numbers_not_adjectives() {
        // The "numbers, not adjectives" discipline requires real
        // arithmetic — TAM/SAM/SOM math, LTV/CAC ratios, retention
        // percentages.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "calculator"));
    }

    #[test]
    fn pmf_strategist_grants_external_research_for_benchmarks() {
        // Some calls need industry benchmarks (median NDR by
        // vertical, retention curves by category) — web_search +
        // web_fetch is the path.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        for required in ["web_search", "web_fetch"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing benchmark research tool: '{required}'"
            );
        }
    }

    #[test]
    fn pmf_strategist_does_not_grant_shell_or_git() {
        // Strategy work is read + write to business memory + write
        // to deliverables. No need for shell/git.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations", "opencode_cli"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "pmf_strategist must not include {forbidden}",
            );
        }
    }

    #[test]
    fn pmf_strategist_delegates_clearly() {
        // Role boundaries matter — without explicit delegation
        // rules the model will overreach into growth_hacker /
        // marketing / fundraise_captain territory.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "growth_hacker",
            "marketing",
            "fundraise_captain",
            "market_researcher",
            "legal_compliance",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn pmf_strategist_isolated_memory_namespace() {
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "pmf_strategist");
    }

    #[test]
    fn pmf_strategist_no_api_key_baked_in() {
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn pmf_strategist_disqualification_icp_discipline_in_prompt() {
        // Specific failure mode this preset must avoid: emitting
        // descriptive ICP ("mid-market SaaS") instead of
        // disqualifying ICP.
        let cfg = pmf_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("disqualification"));
        assert!(prompt.contains("Disqualification list"));
    }
}
