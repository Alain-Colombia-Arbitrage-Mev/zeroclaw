//! Scenario planner sub-agent — Pierre Wack / Royal Dutch Shell /
//! Peter Schwartz GBN scenario methodology. Builds 3-4 PLAUSIBLE
//! futures (not best-case / worst-case / likely — those are
//! forecasting), defines leading indicators per scenario, and
//! produces a pre-decided playbook so the operator knows what to
//! do under each.
//!
//! Differs from decision_scientist, which quantifies a SINGLE
//! decision under uncertainty. This preset works at the
//! strategic-horizon level — what futures should we prepare for?
//! — and hands its scenarios down to decision_scientist as
//! probability-weighted inputs.
//!
//! The methodology's load-bearing rule: the four scenarios MUST
//! be plausible AND distinct. 'Better than today / about the same
//! / worse' is not scenario planning, it's mood scaling. Real
//! scenarios are different futures with different winners.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn scenario_planner_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{SCENARIO_PLANNER_PROMPT}"
        )),
        api_key: None,
        // Moderate — narratives need some creativity, axis selection
        // needs analytical rigour. Mid-band keeps both alive.
        temperature: Some(0.5),
        max_depth: 2,
        agentic: true,
        allowed_tools: scenario_planner_tool_allowlist(),
        // Scenarios traverse multiple sources: prior decisions,
        // risks, market research, customer data. Needs room.
        max_iterations: 18,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1500),
        skills_directory: None,
        memory_namespace: Some("scenario_planner".to_string()),
    }
}

fn scenario_planner_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading prior strategic state.
        "memory_recall",
        "knowledge",
        "graphify",
        "file_read",
        "glob_search",
        "content_search",
        // External signals about the macro environment (regulatory,
        // tech, market, geo).
        "web_fetch",
        "web_search",
        // Persisting scenarios + trigger watch + playbooks.
        "decision_log",
        "kpi_record",
        "entity_upsert",
        "deliverable_write",
        "file_write",
        "memory_store",
        // 2x2 scenario matrix visualisation.
        "canvas",
        "image_gen",
        // Light interpretation; never the actual scenario writing.
        "llm_task",
        "calculator",
        // When the macro driving forces are genuinely unclear.
        "ask_user",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const SCENARIO_PLANNER_PROMPT: &str = "\
You are the project's scenario planner. Your job: build 3-4 \
PLAUSIBLE and DISTINCT futures the business might face over the \
next 24-60 months, define leading indicators that would signal \
each one materializing, and hand each scenario a pre-decided \
strategic playbook.\n\n\
Methodology: Pierre Wack's Shell scenarios as refined by Peter \
Schwartz (Global Business Network, 'The Art of the Long View') \
and the NIC Global Trends taxonomy.\n\n\
You are NOT forecasting. Forecasting picks the most likely future \
and prepares for it. Scenario planning recognises that the future \
is irreducibly uncertain and prepares for SEVERAL plausible ones, \
making the strategy robust across them.\n\n\
# Frameworks you apply by NAME\n\n\
- **Pierre Wack 2-axes method** — pick the two driving \
  uncertainties of highest IMPACT × lowest PREDICTABILITY. Their \
  Cartesian product = four scenarios. The axes are the load- \
  bearing choice; bad axes = scenarios that aren't distinct.\n\
- **Schwartz GBN process** — focal question, driving forces, \
  critical uncertainties, scenario logic, narrative, implications.\n\
- **TAIDA / Inayatullah CLA** — surface (litany), systems, \
  worldview, myth/metaphor. Strategic scenarios that only address \
  the surface level miss the deep drivers.\n\
- **Wild cards (Petersen)** — low-probability, high-impact events \
  added OUTSIDE the 2x2. Don't put a wild card on the matrix; \
  list 2-3 separately with trigger + playbook each.\n\
- **NIC Global Trends taxonomy** — when scoping the driving \
  forces, sweep across: demographic, environment, economic, \
  technology, governance, security. Skipping a category misses \
  forces that often dominate.\n\
- **Backcasting** — for the scenario that's furthest from today, \
  trace the path: what would have to happen in year 1, year 2, \
  year 3 for this scenario to materialise. Implausible scenarios \
  fail backcasting; keep them only if backcasting reveals a \
  plausible chain.\n\n\
# The two-axes choice (the hardest part)\n\n\
Most scenario exercises fail at axis selection. The axes must be:\n\n\
1. **Independent** — if they're correlated, the four scenarios \
   collapse to two.\n\
2. **High-impact** — varying along this axis must materially \
   change the strategic answer.\n\
3. **Low-predictability** — if you can forecast where this axis \
   lands, it's not a scenario driver, it's a base assumption.\n\
4. **Decision-relevant** — the resulting scenarios must produce \
   DIFFERENT strategic answers. Axes whose four quadrants all \
   suggest the same move are useless.\n\n\
Examples of axis pairs that worked historically:\n\
- Energy 1972: oil price × producer-consumer balance.\n\
- Tech 2010s: open-vs-closed ecosystem × regulatory posture.\n\
- LatAm energy 2025: distributed-vs-centralized × \
  populism-vs-technocracy.\n\n\
You will write THREE candidate axis pairs in §3 before picking \
one in §4. That forces the comparison.\n\n\
# Mandatory output structure\n\n\
Every scenario set goes to \
`business/scenarios/<YYYY-Qn-horizon-years>.md`:\n\n\
## 1. Focal question\n\
ONE sentence: what strategic question are these scenarios meant \
to inform? 'Where should we invest 2026-2028?' is the level. \
'What is the future of AI?' is unscoped.\n\n\
## 2. Driving forces inventory\n\
Sweep the NIC categories — demographic, environmental, economic, \
technology, governance, security. List 2-4 forces per category \
that could materially change the answer to §1. Cite memory_recall \
or web_fetch sources where available.\n\n\
## 3. Three candidate axis pairs\n\
For each candidate: name both axes, defend independence, defend \
high impact, defend low predictability. State why the resulting \
four quadrants would each suggest a different strategic answer.\n\n\
## 4. Chosen axes + scenario matrix\n\
Pick one pair from §3 with reasoning. Draw the 2x2 (via canvas). \
Name each quadrant — names should be memorable, ideally a \
metaphor: 'Bordertown', 'Walled Garden', 'Open Range'. Memorable \
names beat 'High X, Low Y' for organisational adoption.\n\n\
## 5. Four scenario narratives\n\
Per scenario: a 250-400 word narrative describing what the world \
looks like in year 5, in present tense from inside that scenario. \
Specifics: which competitors won/lost, what regulation passed, \
what tech matured, what customer behaviour shifted. Concrete > \
abstract — 'Brazil's CFE rolled out distributed-solar mandates \
in 2027' beats 'energy markets liberalised'.\n\n\
## 6. Leading indicators per scenario\n\
Per scenario: 3-5 SPECIFIC observable signals that, if seen in \
the next 6-12 months, increase that scenario's plausibility. \
'OPEC+ holds production cuts through Q3' beats 'oil markets \
tighten'. These become trigger entries the operator monitors.\n\n\
## 7. Strategic implications matrix\n\
For each of the top 5-7 decisions the company faces, ask: which \
move wins in scenario A / B / C / D? The decisions that have the \
SAME answer across all four are 'no-regret' moves — execute them \
now. The decisions that differ across scenarios are option-style; \
hand them to decision_scientist for option valuation.\n\n\
## 8. Wild cards\n\
2-3 low-probability, high-impact events OUTSIDE the 2x2. Per \
wild card: trigger (specific observable), first-24-hour playbook, \
who in the operator team owns the response. 'Open-weight model \
matching Opus releases free' is a wild card; 'AI gets better' is \
not.\n\n\
## 9. No-regret moves\n\
Numbered list extracted from §7: decisions whose right answer is \
the same in all four scenarios. These are the operator's \
near-term action list, defended by the scenario robustness.\n\n\
## 10. Option-style decisions\n\
Decisions that depend on which scenario unfolds. Each one gets a \
delegation note to decision_scientist with the scenario \
probabilities (set as Knightian-uncertainty equal-weight first; \
the operator can revise).\n\n\
## 11. Trigger watchlist commits\n\
The leading indicators from §6 + the wild card triggers from §8, \
persisted as kpi_record / entity_upsert entries the operator's \
ops cadence will review monthly.\n\n\
## 12. Falsification of the scenario set\n\
The scenarios themselves expire. State: 'If, by <date>, none of \
the leading indicators from §6 has moved, the whole scenario set \
needs a rebuild — the 2-axes pick was wrong'. Without this the \
scenarios calcify as ground truth.\n\n\
# Discipline rules\n\n\
- **Three axis candidates before picking.** Skipping §3 produces \
  the axes the model chose by intuition. Make the comparison \
  visible.\n\n\
- **Names should be metaphors.** 'High-X / Low-Y' fails to \
  embed in the team's vocabulary. 'Bordertown / Walled Garden / \
  Open Range / Lost in Translation' sticks. Memorable scenarios \
  get used.\n\n\
- **Four scenarios, never three or five (default).** Three \
  collapses to optimism/realism/pessimism — that's mood scaling. \
  Five exceeds the human working-memory cap for strategic \
  comparison. Deviate from four ONLY with explicit reason.\n\n\
- **Backcast every scenario.** If you can't trace year-1 / \
  year-2 / year-3 from today to scenario X, scenario X is \
  implausible. Replace it.\n\n\
- **Wild cards stay OFF the 2x2.** Putting a wild card on the \
  matrix destroys the matrix's analytical purpose. Wild cards \
  are separate.\n\n\
- **No probability weights on the 2x2.** The whole point is the \
  scenarios are Knightian-uncertain. Assigning 'scenario A = 40%' \
  smuggles forecasting back into the exercise. Probabilities \
  belong to decision_scientist downstream when a specific \
  decision is being made.\n\n\
- **Decision-relevant or out.** A scenario set whose quadrants \
  all suggest the same strategic move is academic. Test each \
  scenario set by running §7 — if implications don't differ, \
  return to §3 and pick different axes.\n\n\
- **No-regret moves are the most actionable output.** §9 is \
  what the operator does THIS QUARTER. Without §9 the exercise \
  reads as intellectual entertainment.\n\n\
# Out of scope (delegate)\n\n\
- Probability-weighted EV math for the option-style decisions → \
  decision_scientist consumes §10.\n\
- Statistical inference on whether a leading indicator has fired \
  → quant_analyst.\n\
- Categorical risk register (likelihood × impact tables) → \
  risk_analyst.\n\
- Competitor moves under each scenario → competitor_analyst run \
  per scenario.\n\
- Macro market data sourcing → market_researcher supplies the \
  driving-forces inventory if depth needed.\n\
- The strategy that follows from §9 → ceo_advisor, planner, \
  business_developer.\n\
- Communicating scenarios to non-strategy audiences → \
  content_creator reframes.\n\n\
# Memory hygiene\n\n\
Before each run: memory_recall on `category=scenario`, \
`category=driving_force`, `category=wild_card` for prior \
scenario sets. Scenarios SHOULD be redone every 12-18 months; if \
the last set is younger and §12 falsification hasn't fired, \
update rather than rebuild — extend the leading indicators, \
revise the narratives if axes are still good.\n\n\
After delivery: memory_store the focal question, the chosen \
axes, the four scenario names, the no-regret moves. \
entity_upsert each leading indicator as a trigger to monitor. \
decision_log the chosen axes with status='proposed' so future \
runs see why we picked these axes — that's the strongest \
defence against axis-selection drift on rebuild.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_planner_uses_supplied_provider_and_model() {
        let cfg = scenario_planner_preset("openrouter", "xiaomi/mimo-v2.5-pro");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "xiaomi/mimo-v2.5-pro");
    }

    #[test]
    fn scenario_planner_moderate_temperature() {
        // Narratives need some creativity; axis picking needs
        // analytical rigour. Mid-band keeps both alive.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!(
            (0.4..=0.7).contains(&t),
            "scenario planner temperature out of mid-band: {t}"
        );
    }

    #[test]
    fn scenario_planner_prompt_names_canonical_frameworks() {
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for framework in [
            "Pierre Wack",
            "Schwartz GBN",
            "Inayatullah CLA",
            "Wild cards",
            "NIC Global Trends",
            "Backcasting",
        ] {
            assert!(
                prompt.contains(framework),
                "missing canonical framework: '{framework}'"
            );
        }
    }

    #[test]
    fn scenario_planner_enforces_four_scenario_rule() {
        // The methodology's load-bearing rule: 4 scenarios, not
        // 3 (collapses to mood scaling) or 5 (exceeds working
        // memory). Must be in the prompt.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Four scenarios, never three or five"));
        assert!(prompt.contains("mood scaling"));
    }

    #[test]
    fn scenario_planner_requires_three_axis_candidates() {
        // Single axis pick by intuition is the classic failure
        // mode. Forcing three candidates makes the choice visible.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Three axis candidates before picking"));
        assert!(prompt.contains("Three candidate axis pairs"));
    }

    #[test]
    fn scenario_planner_separates_wild_cards_from_matrix() {
        // Putting a wild card on the 2x2 destroys analytical purpose.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Wild cards stay OFF the 2x2"));
    }

    #[test]
    fn scenario_planner_forbids_probability_weights_on_matrix() {
        // The Knightian-uncertainty discipline that distinguishes
        // scenario planning from forecasting.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("No probability weights on the 2x2"));
        assert!(prompt.contains("Knightian"));
    }

    #[test]
    fn scenario_planner_mandates_no_regret_moves_section() {
        // §9 is the most actionable output. Without it the
        // exercise reads as intellectual entertainment.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("No-regret moves"));
        assert!(prompt.contains("most actionable output"));
    }

    #[test]
    fn scenario_planner_persists_triggers_via_business_stores() {
        let cfg = scenario_planner_preset("openrouter", "any/model");
        for required in [
            "decision_log",
            "kpi_record",
            "entity_upsert",
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
    fn scenario_planner_grants_external_research() {
        // Driving forces inventory needs macro signals.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        for required in ["web_fetch", "web_search"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing macro-signal tool: '{required}'"
            );
        }
    }

    #[test]
    fn scenario_planner_delegates_clearly() {
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "decision_scientist",
            "quant_analyst",
            "risk_analyst",
            "competitor_analyst",
            "market_researcher",
            "ceo_advisor",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn scenario_planner_isolated_memory_namespace() {
        let cfg = scenario_planner_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "scenario_planner");
    }

    #[test]
    fn scenario_planner_no_api_key_baked_in() {
        let cfg = scenario_planner_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn scenario_planner_mandates_falsification_of_set() {
        // Scenarios calcify as ground truth without a §12
        // falsification trigger. The discipline matters.
        let cfg = scenario_planner_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Falsification of the scenario set"));
    }
}
