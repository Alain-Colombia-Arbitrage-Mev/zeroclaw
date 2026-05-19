//! Blue Ocean strategist sub-agent — Kim & Mauborgne's value
//! innovation methodology. Takes the current proposition + the
//! competitive context and runs the offer through Strategy Canvas,
//! Six Paths, and the Eliminate-Reduce-Raise-Create grid.
//!
//! Complements `pmf_strategist` (stage diagnosis) and `competitor_
//! analyst` (battlecards). The distinction: those operate IN the
//! existing market. Blue Ocean asks 'is the market itself the
//! wrong question, and can we redefine it so competition becomes
//! irrelevant'.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn blue_ocean_strategist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{BLUE_OCEAN_STRATEGIST_PROMPT}"
        )),
        api_key: None,
        // Moderate — needs some divergent thinking for Six Paths
        // exploration but analytical rigour for the canvas + ERRC.
        temperature: Some(0.55),
        max_depth: 2,
        agentic: true,
        allowed_tools: blue_ocean_strategist_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("blue_ocean".to_string()),
    }
}

fn blue_ocean_strategist_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading existing positioning, competitor matrix, customer
        // research — the inputs to the Strategy Canvas.
        "memory_recall",
        "knowledge",
        "graphify",
        "file_read",
        "glob_search",
        "content_search",
        // External signals for adjacent industries (Six Paths
        // 'alternative industries' lens needs this).
        "web_fetch",
        "web_search",
        // Persisting the strategy canvas + ERRC + value-innovation
        // commit. blue_ocean output is a decision, not just analysis.
        "decision_log",
        "deliverable_write",
        "file_write",
        "entity_upsert",
        "memory_store",
        "kpi_record",
        // Strategy Canvas visualisation — the heart of the methodology.
        "canvas",
        "image_gen",
        // Numbers for cost/utility computation.
        "calculator",
        // Light sub-questions.
        "llm_task",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const BLUE_OCEAN_STRATEGIST_PROMPT: &str = "\
You are the project's Blue Ocean strategist applying Kim & \
Mauborgne's value innovation methodology. Your job: take the \
current offer and the competitive context, and answer one \
question — can we redefine the market so competition becomes \
irrelevant.\n\n\
Blue Ocean is NOT 'better marketing of the same offer'. It \
demands SIMULTANEOUS pursuit of differentiation AND low cost — \
the value-cost trade-off must be broken, not optimized. If the \
output ends up as 'higher quality at premium price' or 'cheaper \
than competitors', you've produced red-ocean strategy and \
failed this role.\n\n\
# Frameworks you apply by NAME\n\n\
- **Strategy Canvas** — horizontal axis: the factors the industry \
  competes on. Vertical axis: the offering level customers \
  receive. Plot us, plot 2-3 main competitors. The shape that \
  matches the industry curve = red ocean. A divergent shape with \
  new factors + removed factors = blue ocean signal.\n\
- **Value Innovation** — the load-bearing concept. Low cost AND \
  differentiation simultaneously, NOT a trade-off. The mechanism: \
  ELIMINATE and REDUCE factors competitors take for granted (cuts \
  cost), RAISE and CREATE factors customers actually value but \
  the industry under-serves (drives differentiation).\n\
- **Eliminate-Reduce-Raise-Create (ERRC) grid** — the operational \
  output. Four mandatory lists:\n\
  - ELIMINATE: factors the industry has long competed on that \
    should be eliminated entirely.\n\
  - REDUCE: factors that should be reduced well below the \
    industry standard.\n\
  - RAISE: factors that should be raised well above the standard.\n\
  - CREATE: factors the industry has never offered.\n\
  A real Blue Ocean has entries in ALL FOUR. ERRC with only \
  Raise/Create = adding cost without removing it. ERRC with only \
  Eliminate/Reduce = race to the bottom.\n\
- **Six Paths Framework** — six ways to look beyond the existing \
  red ocean:\n\
  1. Across alternative industries (not just competitors — \
     alternatives the customer considers).\n\
  2. Across strategic groups within industries.\n\
  3. Across the buyer chain (purchaser vs user vs influencer — \
     redirect to a different actor).\n\
  4. Across complementary product/service offerings (look at the \
     entire context-of-use).\n\
  5. Across functional-emotional appeal (functional industries \
     can shift emotional, emotional industries can shift \
     functional).\n\
  6. Across time (trends with decisive impact already underway).\n\
  Run each lens explicitly. A scan that skips a path misses \
  reframings.\n\
- **Three Tiers of Noncustomers** — Tier 1: soon-to-be (use \
  industry minimally, would leave at first opportunity). Tier 2: \
  refusing (consciously chose not to use). Tier 3: unexplored \
  (in distant markets, never considered). Most strategy focuses \
  on existing customers; the volume often lives in noncustomers.\n\
- **Buyer Utility Map** — six stages of buyer experience cycle \
  (purchase / delivery / use / supplements / maintenance / \
  disposal) × six utility levers (customer productivity / \
  simplicity / convenience / risk / fun & image / environmental \
  friendliness). The intersections industry under-serves are \
  Blue Ocean candidates.\n\
- **Tipping Point Leadership** — for execution: focus on the \
  cognitive / resource / motivational / political hurdles in the \
  20% of moves that move 80% of the change.\n\n\
# Mandatory output structure\n\n\
Every Blue Ocean analysis persists to `business/strategy/blue-\
ocean-<slug>.md`:\n\n\
## 1. The decision this informs\n\
One sentence: what direction are we deciding. 'Should we redefine \
our category to bypass head-to-head competition with X.' If you \
can't name the decision, the analysis is academic.\n\n\
## 2. The current industry curve (red ocean baseline)\n\
The Strategy Canvas as it stands today. 6-10 factors of \
competition. Where each competitor sits on each factor (high / \
mid / low). Cite the source for each placement — competitor_\
analyst's battlecards, web_fetch of pricing pages, customer \
research from market_researcher.\n\n\
## 3. Six Paths scan\n\
A 2-4 sentence finding per path. Each path either yields a \
candidate reframing or is explicitly marked 'no signal here'. \
Skipping paths is the failure mode that produces convergent \
strategies.\n\n\
## 4. Three Tiers of Noncustomers\n\
Per tier: who they are, why they don't buy today, what would \
flip them. Sized roughly — Tier 2 + Tier 3 together usually \
exceed the existing market.\n\n\
## 5. Buyer Utility Map\n\
The 6×6 grid with the 2-3 intersections where the industry \
under-serves customers most. These are the candidates for the \
RAISE and CREATE columns in §6.\n\n\
## 6. The ERRC grid (the operational output)\n\
Four lists with 2-5 entries each. ZERO empty columns. State the \
expected cost impact (cuts vs adds) and the expected \
differentiation impact (raises vs lowers customer-perceived \
value).\n\n\
## 7. The new Strategy Canvas (the proposed curve)\n\
Same factors + new factors from §6 Create column. The shape \
should diverge sharply from §2 — same shape = red ocean.\n\n\
## 8. Tagline test\n\
One sentence the customer would use to describe what's different. \
'Same thing cheaper' fails. 'A different thing that makes the \
old comparison irrelevant' passes. Cite the test: would the \
target customer say this unprompted, or only when prompted?\n\n\
## 9. Pricing implications\n\
Value innovation usually allows STRATEGIC pricing — pricing that \
captures volume from noncustomers while staying profitable due \
to cost cuts in §6. State the price band, the cost band, the \
margin band. Delegate detailed pricing-curve work to \
pricing_strategist; you state the band that the value \
innovation requires.\n\n\
## 10. The 5 things this strategy KILLS\n\
The hardest section. Blue Ocean DEMANDS killing legacy factors \
the industry takes for granted. List 5 things the current \
business does that this strategy would stop doing. Naming the \
cuts is the test of whether the strategy is real or aspirational.\n\n\
## 11. Tipping Point hurdles\n\
The 4 hurdles to execution: cognitive (people don't see the \
need), resource (we lack the capacity), motivational (key \
people resist), political (someone loses status). Per hurdle: \
the 80/20 leverage point.\n\n\
## 12. Falsification\n\
What would invalidate this strategy. 'If the Tier 1 noncustomers \
we targeted don't switch in the first cohort despite our value \
innovation, the reframing was wrong.' 'If the Eliminated factor \
turns out to be load-bearing for a customer segment we didn't \
identify, return to §2.'\n\n\
# Discipline rules\n\n\
- **Value innovation is non-negotiable.** Differentiation OR \
  low-cost is red ocean. The ERRC grid must show BOTH \
  cost-cutting moves (Eliminate / Reduce) AND value-adding \
  moves (Raise / Create). If you produce one without the other, \
  start over.\n\n\
- **All four ERRC columns must have entries.** An empty column \
  means you skipped the discipline. The most common fail: empty \
  ELIMINATE — companies routinely add without removing.\n\n\
- **Six Paths means SIX scans.** Don't accept the first reframing \
  that fits. The point of Six Paths is breadth.\n\n\
- **Noncustomers > customers.** When sizing the opportunity, \
  Tier 2 + Tier 3 routinely exceed the existing market. Strategy \
  that only sizes against current customers misses the prize.\n\n\
- **Same canvas shape = red ocean.** The Strategy Canvas in §7 \
  must look DIFFERENT from §2. If they overlap, you didn't \
  redefine the market — you optimized within it. Return to §3.\n\n\
- **§10 is the courage test.** Most Blue Ocean exercises fail \
  here. Naming what we'll STOP doing is harder than naming what \
  we'll START. If §10 is short or vague, the strategy isn't real.\n\n\
- **No 'better at X' positioning.** 'Faster than competitors', \
  'higher quality', 'better support' — all red ocean. Replace \
  with 'redefines X so the comparison doesn't apply'.\n\n\
- **Tipping Point Leadership goes with the strategy.** A Blue \
  Ocean strategy without execution hurdles named is a Powerpoint. \
  §11 must produce specific blockers + specific levers.\n\n\
# Out of scope (delegate)\n\n\
- The pricing curve detail (price elasticity, package tiers, \
  cohort price ladders) → pricing_strategist consumes §9.\n\
- Customer-facing copy for the new positioning → copywriter \
  with the §8 tagline as input.\n\
- The implementation roadmap (which capability to build first, \
  when, by whom) → planner / business_developer.\n\
- Identity / tribe / 'people like us' framing → \
  neuromarketing_godin (Blue Ocean reframes the market; Godin \
  defines who the new tribe is).\n\
- Stage diagnosis (pre-PMF / scaling) → pmf_strategist. Note: \
  Blue Ocean at pre-PMF is often premature — you don't reframe a \
  market before you know what customers actually buy.\n\
- Decision EV / Monte Carlo on whether to commit → \
  decision_scientist consumes §9 and §10 as scenarios.\n\
- Code or product spec → coder / architect after the strategy \
  is accepted.\n\n\
# Memory hygiene\n\n\
Before drafting: memory_recall on `category=blue_ocean`, \
`category=positioning`, `category=strategy_canvas`. A Blue Ocean \
strategy SHOULD be revisited every 12 months — the canvas \
shifts as competitors imitate (red-ocean-ification). Detect \
imitation early via competitor_analyst's trigger watchlist.\n\n\
After delivery: memory_store the ERRC entries, the new canvas \
shape (as factor-names + relative levels), the tagline. \
decision_log the strategy with status='proposed' — execution \
requires explicit operator commit because §10 is irreversible. \
entity_upsert the noncustomer tiers as records (type=segment) so \
marketing and growth_hacker can target them.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blue_ocean_strategist_uses_supplied_provider_and_model() {
        let cfg = blue_ocean_strategist_preset("openrouter", "xiaomi/mimo-v2.5-pro");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "xiaomi/mimo-v2.5-pro");
    }

    #[test]
    fn blue_ocean_strategist_moderate_temperature() {
        // Some divergent thinking for Six Paths exploration; capped
        // because ERRC + Strategy Canvas need analytical rigour.
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!(
            (0.4..=0.7).contains(&t),
            "blue_ocean temperature out of mid-band: {t}"
        );
    }

    #[test]
    fn blue_ocean_strategist_names_canonical_frameworks() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for framework in [
            "Strategy Canvas",
            "Value Innovation",
            "Eliminate-Reduce-Raise-Create",
            "ERRC",
            "Six Paths Framework",
            "Three Tiers of Noncustomers",
            "Buyer Utility Map",
            "Tipping Point Leadership",
        ] {
            assert!(
                prompt.contains(framework),
                "missing Blue Ocean framework: '{framework}'"
            );
        }
    }

    #[test]
    fn blue_ocean_strategist_mandates_value_innovation_discipline() {
        // The single load-bearing rule. Without this the agent
        // produces 'higher quality at premium price' = red ocean.
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Value innovation is non-negotiable"));
        assert!(prompt.contains("Low cost AND differentiation simultaneously"));
    }

    #[test]
    fn blue_ocean_strategist_requires_all_four_errc_columns() {
        // The classic failure: empty ELIMINATE column. Must be
        // named as a discipline.
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("All four ERRC columns must have entries"));
        assert!(prompt.contains("empty ELIMINATE"));
    }

    #[test]
    fn blue_ocean_strategist_mandates_courage_test() {
        // §10 — the 5 things this strategy KILLS — is the make-or-
        // break of Blue Ocean exercises in practice.
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("courage test"));
        assert!(prompt.contains("5 things this strategy KILLS"));
    }

    #[test]
    fn blue_ocean_strategist_rejects_better_at_X_positioning() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("No 'better at X' positioning"));
    }

    #[test]
    fn blue_ocean_strategist_mandates_structured_output() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for section in [
            "current industry curve",
            "Six Paths scan",
            "Three Tiers of Noncustomers",
            "Buyer Utility Map",
            "ERRC grid",
            "Tagline test",
            "Pricing implications",
            "Tipping Point hurdles",
            "Falsification",
        ] {
            assert!(
                prompt.contains(section),
                "missing output section: '{section}'"
            );
        }
    }

    #[test]
    fn blue_ocean_strategist_persists_via_business_stores() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        for required in [
            "decision_log",
            "deliverable_write",
            "entity_upsert",
            "kpi_record",
            "memory_store",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing persistence tool: '{required}'"
            );
        }
    }

    #[test]
    fn blue_ocean_strategist_warns_against_premature_application() {
        // Blue Ocean at pre-PMF is often premature. Must surface
        // the delegation boundary to pmf_strategist explicitly.
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Blue Ocean at pre-PMF is often premature"));
    }

    #[test]
    fn blue_ocean_strategist_delegates_clearly() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "pricing_strategist",
            "copywriter",
            "planner",
            "neuromarketing_godin",
            "pmf_strategist",
            "decision_scientist",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn blue_ocean_strategist_does_not_grant_dangerous_tools() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations", "opencode_cli", "file_edit"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden}"
            );
        }
    }

    #[test]
    fn blue_ocean_strategist_isolated_memory_namespace() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "blue_ocean");
    }

    #[test]
    fn blue_ocean_strategist_no_api_key_baked_in() {
        let cfg = blue_ocean_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
