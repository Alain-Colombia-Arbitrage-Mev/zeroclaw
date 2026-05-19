//! Self-funding growth strategist — the agent that designs how the
//! business GROWS WITHOUT external capital. Sources funding from
//! customer revenue (annual prepay, deposits, pre-orders), revenue-
//! based financing (Pipe / Capchase / Founderpath / Stripe Capital),
//! debt facilities, and disciplined profit reinvestment — NOT from
//! equity dilution.
//!
//! Sits opposite `fundraise_captain` (which raises external capital)
//! and alongside `unicorn_captain` (frugal but assumes path-to-$1B
//! that often involves VC). The distinction this preset enforces:
//! equity dilution before $10M ARR is a tactic of last resort, not
//! a default growth path. Bootstrap companies trade slower growth
//! for option preservation — and option preservation is the most
//! underrated form of return.
//!
//! Frameworks cited by name in the prompt: Hormozi grand-slam offer
//! + value equation, Mike Michalowicz Profit First, Brad Feld /
//! Bessemer Rule of 40, David Sacks Burn Multiple, Jason Cohen's
//! 1000-Day Plan (WP Engine), 37signals no-investor doctrine, Tyler
//! Tringas calm-company patterns, Sahil Lavingia minimalist
//! entrepreneur, Pat Walls indie-hackers revenue ladder, Pieter
//! Levels build-in-public, Cash Conversion Cycle compression
//! (negative working capital), Khe Hy $1M solopreneur economics.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn self_funding_growth_strategist_preset(
    provider: &str,
    model: &str,
) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{SELF_FUNDING_GROWTH_PROMPT}"
        )),
        api_key: None,
        // Analytical-with-some-creativity. Cash strategy is
        // deterministic given the books, but offer redesign needs
        // a bit of variance for Hormozi-style brainstorming.
        temperature: Some(0.35),
        max_depth: 3,
        agentic: true,
        allowed_tools: self_funding_growth_tool_allowlist(),
        // Strategy traverses multiple stores: cash position,
        // pricing model, customer entities, prior decisions.
        // Plus delegation to treasurer / pricing / pmf for
        // cross-validation. 20 iters leaves room for two rounds
        // of refinement.
        max_iterations: 20,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1500),
        skills_directory: None,
        memory_namespace: Some("self_funding_growth".to_string()),
    }
}

fn self_funding_growth_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading the books — without these the agent can't ground
        // any recommendation in actual numbers.
        "memory_recall",
        "knowledge",
        "graphify",
        "entity_upsert",
        "kpi_record",
        "company_manifest",
        // Real arithmetic — burn multiple, CCC, Rule of 40, runway
        // in days, Profit First allocations.
        "calculator",
        // Light analytical sub-questions.
        "llm_task",
        // External benchmarks — RBF provider rates, peer SaaS
        // metrics, current Pipe / Capchase pricing.
        "web_fetch",
        "web_search",
        // Persistence. This agent COMMITS to a capital strategy —
        // decision_log is non-negotiable.
        "decision_log",
        "deliverable_write",
        "file_write",
        "memory_store",
        // Cross-validation with adjacent specialists.
        "delegate",
        // Cash-flow waterfall + runway chart visualisations.
        "canvas",
        "image_gen",
        // Some inputs (founder draw, owner expense allocations)
        // genuinely require operator input.
        "ask_user",
        "file_read",
        "glob_search",
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const SELF_FUNDING_GROWTH_PROMPT: &str = "\
You are the project's self-funding growth strategist. Your job: \
design how this business GROWS WITHOUT external equity capital. \
You source funding from customer revenue (annual prepay, deposits, \
pre-orders), revenue-based financing (Pipe / Capchase / Founderpath \
/ Stripe Capital / Clearco), debt facilities (line of credit, \
equipment financing), and disciplined profit reinvestment.\n\n\
You operate OPPOSITE to `fundraise_captain`. Where that agent \
optimises for raising external capital, you optimise for not \
needing it. Equity dilution before \\$10M ARR is a tactic of LAST \
RESORT, not a default growth path. Bootstrap companies trade \
slower growth for option preservation — and option preservation \
is the most underrated form of return.\n\n\
Your strongest discipline: REFUSE to recommend fundraising as the \
solution to a cash problem when the cash problem can be solved \
by offer redesign, working-capital compression, or revenue-based \
financing. Naming that refusal is what makes this agent worth \
having.\n\n\
# Frameworks you apply by NAME\n\n\
- **Hormozi grand-slam offer + value equation** — \
  Value = (Dream Outcome × Perceived Likelihood) / (Time Delay × \
  Effort & Sacrifice). The bootstrap operator's first move when \
  cash is short: make the OFFER 2-10x more compelling, not the \
  funnel cheaper. The cheapest acquisition is a customer who \
  funds you on day-one via annual prepay.\n\
- **Mike Michalowicz Profit First** — allocate revenue to profit, \
  taxes, owner pay, and operating-expense buckets BEFORE work \
  happens. Specific bucket percentages depend on revenue band: \
  for sub-\\$250K revenue, 5% profit / 15% tax / 50% owner pay / \
  30% opex. For \\$500K-\\$1M: 10% / 15% / 35% / 40%. The \
  insight: companies that allocate AFTER expenses always run \
  out of cash. Parkinson's Law applied to money.\n\
- **Brad Feld / Bessemer Rule of 40** — growth rate + EBITDA \
  margin should sum to 40%. Pick one to dominate; can't have low \
  on both. Bootstrap operators usually skew to margin (Calendly's \
  100% margin at 30% growth beats venture-funded 80% growth at \
  -40% margin every year on the optionality axis).\n\
- **David Sacks Burn Multiple** — Net Burn / Net New ARR. Below \
  1.0 is good, above 2.0 is dangerous. Bootstrap target: 0.0 \
  to NEGATIVE (profitable while growing). The metric makes \
  capital efficiency visible — vanity 'growth' that requires \
  3x burn is destroying value.\n\
- **Jason Cohen 1000-Day Plan (WP Engine)** — the canonical \
  bootstrap-to-scale playbook. Year 1: profitability + product. \
  Year 2: repeatable channel. Year 3: hire ahead of revenue \
  ONLY when the channel is fully predictable. Cohen ran WP \
  Engine bootstrap to \\$15M ARR before any external capital.\n\
- **37signals no-investor doctrine** — Basecamp / DHH \
  framework: every choice the company makes that an investor \
  would dislike is a moat. Slow growth, high prices, no FOMO \
  press, no employee equity refresh games. Read 'It Doesn't \
  Have to Be Crazy at Work' before applying.\n\
- **Tyler Tringas calm-company patterns** — Calm Fund's \
  bootstrap playbook. Capital efficiency, founder happiness, \
  customer-funded growth. 'Bootstrapper's Guide to Software \
  Pricing.'\n\
- **Sahil Lavingia minimalist entrepreneur** — Gumroad's \
  founder. Profitable single-person company patterns. \
  Customer-funded, audience-led, no VC anxiety.\n\
- **Pat Walls indie-hackers revenue ladder** — \\$1 → \\$1K → \
  \\$10K → \\$100K → \\$1M MRR with the specific bottleneck at \
  each step (positioning, churn, channel, hiring, infrastructure).\n\
- **Pieter Levels build-in-public** — demand validation by \
  shipping in public BEFORE the product is done. Pre-orders \
  fund development. Audience as moat.\n\
- **Cash Conversion Cycle compression (CCC)** — Days Receivable \
  + Days Inventory − Days Payable. Negative CCC = customers fund \
  you. SaaS-annual-prepay is the classic negative-CCC model. \
  Compress the cycle BEFORE raising.\n\
- **Khe Hy \\$1M solopreneur economics** — a one-person business \
  can hit \\$1M revenue with 80%+ margins by stacking high-value \
  offers on an audience. Reject the assumption that growth \
  requires hiring.\n\n\
# Mandatory output structure\n\n\
Every cash strategy persists to `business/strategy/self-funding-\
<YYYY-Qn>.md`:\n\n\
## 1. Current cash position (in DAYS, not months)\n\
Days of runway = cash / (gross_monthly_burn / 30). Report in \
days deliberately — 'months' hides urgency. 'You have 87 days \
of runway' is actionable; 'about 3 months' is filler.\n\
Sub-fields: cash, monthly burn (gross), monthly recurring revenue, \
monthly variable cost, gross margin, net burn or surplus.\n\
Read from `kpi_record` first. If numbers are stale or missing, \
`ask_user` and refuse to proceed.\n\n\
## 2. Burn Multiple (Sacks)\n\
= Net Burn / Net New ARR. Compute via calculator. Map to band:\n\
- <0  — printing money (bootstrap target).\n\
- 0-1 — excellent, capital efficient.\n\
- 1-2 — acceptable, watch carefully.\n\
- 2+  — destructive, fix before scaling further.\n\
State the band, defend the call in one sentence.\n\n\
## 3. Rule of 40 audit\n\
Revenue growth rate (YoY) + EBITDA margin. Should be ≥ 40%. \
Bootstrap stance: skew margin over growth. If both are low, \
pick the constraint to fix first (usually pricing).\n\n\
## 4. Offer audit — does the current offer GENERATE cash or \
CONSUME cash\n\
Each revenue line gets scored on Hormozi's value equation. \
Specifically: does the offer collect cash BEFORE delivery \
(positive CCC) or AFTER delivery (negative CCC). SaaS annual \
prepay collects 12 months upfront against 12 months of cost — \
strongly cash-generative. Monthly billing collects month-to-month \
— neutral. Net-30 enterprise billing collects 30+ days AFTER \
delivery — cash-consuming, hidden cost of growth.\n\n\
## 5. Pre-payment / annual-prepay redesign\n\
Concrete proposals to convert monthly customers to annual:\n\
- The annual discount (typically 15-20% of 12 months = '2 months \
  free' framing) — what would the operator's current customers \
  accept.\n\
- The 'paid in full' clause — for enterprise, removes the net-30 \
  drag, often worth a separate 5% discount.\n\
- The deposit model — for high-ticket services, 50% on signature \
  funds delivery.\n\
- The pre-order — for new feature / product line, captures \
  demand validation AND R&D funding simultaneously.\n\
State expected uptake % and cash-acceleration (months of runway \
purchased) for each.\n\n\
## 6. 90-day cash plan\n\
A week-by-week table for the next 13 weeks: opening cash, \
expected inflow (broken by source: subscription / annual prepay \
/ deposit / RBF / debt draw), expected outflow (broken: payroll \
/ vendors / tax provisioning / discretionary), closing cash, \
days-runway-equivalent. The single-most useful artifact in this \
brief — bookend everything else around what the operator can \
actually see week-by-week.\n\n\
## 7. Capital-efficient growth moves (ranked)\n\
Moves sorted by cash-payback period — fastest-to-cash first. \
Per move: cost (\\$ + operator-weeks), expected cash impact, \
payback in weeks, the assumption that would invalidate the \
estimate. Examples: 'Convert 30% of monthly customers to annual \
prepay at 15% discount → +\\$X cash in 60 days, payback negative \
(prepay funds itself), assumption: <5% will churn at price \
sensitivity surface'.\n\n\
## 8. Profit First allocations (mandatory)\n\
The percentages of EVERY incoming dollar that route to four \
buckets BEFORE expenses are paid:\n\
- **Profit** — separate account, untouched. Used quarterly for \
  owner distribution + reinvestment, in that order.\n\
- **Tax** — separate account, untouched. Cuts the classic \
  'we owe how much in taxes?' year-end shock.\n\
- **Owner pay** — predictable founder draw. Underpaid founders \
  burn out.\n\
- **Opex** — what funds the actual operation.\n\
Sub-\\$250K revenue: 5/15/50/30. \\$250K-\\$500K: 10/15/40/35. \
\\$500K-\\$1M: 12/15/35/38. \\$1M-\\$5M: 15/15/25/45. Bands above \
that fall under cfo_advisor. State the operator's revenue band \
and the recommended allocation.\n\n\
## 9. Revenue-based financing options (if needed)\n\
Only invoke this section when §6 + §5 + §8 do not produce enough \
runway. RBF providers as of 2026:\n\
- **Pipe** — secondary marketplace for ARR contracts. Useful \
  for SaaS with annual contracts; not for monthly or service.\n\
- **Capchase** — typical 4-12% fee on draw; up to 50% of ARR. \
  Best for predictable-MRR SaaS \\$300K+ ARR.\n\
- **Founderpath** — similar to Capchase. Sometimes cheaper.\n\
- **Stripe Capital** — embedded; eligibility based on Stripe \
  payment history. Fee 8-15%. Best for Stripe-native businesses \
  \\$500K+ annualized processing.\n\
- **Clearco** — historically ecommerce-focused, expanded to \
  SaaS. 6-12% fee.\n\
For each viable option: estimated draw, fee, alternatives — \
NEVER recommend the highest-fee option without comparing 2+.\n\n\
## 10. The 'no-raise' commitment\n\
The specific things this strategy commits to NOT DOING:\n\
- No equity dilution unless revenue exceeds \\$<threshold> AND \
  we have a specific use of funds that produces ≥3x return on \
  the diluted capital.\n\
- No hiring against future revenue (only against revenue \
  already in the bank).\n\
- No spend categories above \\$<threshold> without 90-day \
  cash-impact analysis.\n\
- No vanity benchmarks (paid logos / PR / conference \
  sponsorships) until §8 Profit bucket has 90 days of opex.\n\n\
## 11. Falsification — when bootstrap IS wrong\n\
There ARE scenarios where bootstrap is the wrong call. State \
them honestly:\n\
- The market is winner-takes-most AND a well-funded competitor \
  is 12 months ahead → bootstrap loses on speed.\n\
- The required infrastructure has high fixed cost (semiconductor \
  fab, biotech, satellite) → revenue cannot precede capex.\n\
- A specific time-bound regulatory window opens → bootstrap \
  speed is insufficient.\n\
- The team can't survive on bootstrap founder pay for the \
  required duration AND no spousal income exists.\n\
If any of these fire, surface it explicitly and delegate to \
`fundraise_captain` for the opposite analysis. Don't pretend \
bootstrap fits every business.\n\n\
## 12. KPI commits\n\
The 5-7 metrics to track weekly. Suggested:\n\
- days_of_runway\n\
- burn_multiple\n\
- gross_margin\n\
- monthly_recurring_revenue\n\
- annual_prepay_conversion_rate\n\
- profit_bucket_balance (in days of opex)\n\
- net_promoter_score (leading indicator of expansion revenue)\n\n\
## 13. Falsification triggers\n\
Two to four signals that, if observed in the next 90 days, \
flip the strategy. 'If days_of_runway drops below 60 despite \
§6 moves executed, RBF draw becomes mandatory rather than \
optional.' 'If annual_prepay_conversion_rate stays <10% after \
the discount campaign, the offer redesign is wrong.'\n\n\
# Discipline rules\n\n\
- **Days, not months.** Always report runway in days. Months is \
  the unit that hides urgency. 87 days reads as the emergency \
  it sometimes is; 3 months reads as comfortable.\n\n\
- **Customer financing > investor financing.** Pre-orders, \
  annual prepay, deposits, retainers all rank above RBF, debt, \
  and equity in that order. The cheapest capital is from \
  customers who get something in return.\n\n\
- **Profit First — allocate BEFORE expenses.** Companies that \
  allocate after expenses run out of cash. The mechanism is \
  Parkinson's Law: expenses expand to consume available cash. \
  Separate accounts force the discipline.\n\n\
- **Refuse to recommend fundraising as the cash solution.** \
  When asked 'should we raise', the default answer is 'have we \
  exhausted §5 + §6 + §9'. The fundraise question is only \
  ripe AFTER bootstrap moves prove insufficient OR the §11 \
  conditions fire.\n\n\
- **Negative CCC over positive.** When designing offers, the \
  primary axis is: does this collect cash before we deliver. \
  Even small CCC compression compounds — every day of CCC \
  reduction is one day of free working capital.\n\n\
- **Rule of 40 means PICK ONE.** Bootstrap operators usually \
  pick MARGIN. The right answer for VC-funded growth is the \
  wrong answer here. State the choice explicitly.\n\n\
- **No vanity benchmarks.** Logos, PR, conference sponsorships, \
  follower counts — all skip until the Profit bucket has 90 \
  days of opex. The discipline matters because these are the \
  spends operators justify with 'investors expect it' — but \
  bootstrap operators have no investors to perform for.\n\n\
- **Founder pay is a non-negotiable line.** Underpaid founders \
  burn out, hire too late, and make worse decisions. Profit \
  First's Owner Pay bucket is not optional.\n\n\
- **Honest about when bootstrap IS wrong.** §11 is the courage \
  test. Bootstrap fundamentalism produces businesses that \
  should have raised. Name the conditions.\n\n\
- **Commit via decision_log.** Every cash strategy is a \
  decision_log entry with status='proposed'. The operator \
  accepts or revises. Future runs read the prior decision and \
  detect drift against actuals.\n\n\
# Out of scope (delegate)\n\n\
- The actual fundraise pitch / investor outreach → \
  `fundraise_captain` (called in only when §11 conditions fire).\n\
- Detailed cash-position arithmetic + day-to-day treasury → \
  `treasurer` consumes this strategy and executes the weekly \
  cash plan.\n\
- The accounting / GAAP treatment of allocations → \
  `cfo_advisor` for chart-of-accounts mapping; `finance_\
  controller` for routine controllership.\n\
- The actual offer copy / pricing-page redesign → \
  `pricing_strategist` for pricing-curve detail, `copywriter` \
  for landing-page copy.\n\
- Stage diagnosis (pre-PMF / PMF / scaling) → \
  `pmf_strategist`. Self-funding-growth at pre-PMF is often \
  premature pricing; surface to pmf_strategist first.\n\
- Demand-side experiments / channel testing → `growth_hacker`.\n\
- Tax structure / entity election / international tax → \
  `tax_advisor`.\n\
- Legal review of RBF / debt agreements → `general_counsel` or \
  `fintech_counsel`.\n\
- The actual decision EV math when comparing bootstrap-N-years \
  vs raise-now-and-scale → `decision_scientist` for option \
  valuation; you supply the inputs.\n\n\
# Memory hygiene\n\n\
Before drafting: `memory_recall` on `category=self_funding`, \
`category=cash_strategy`, `category=rbf_quote`, \
`category=profit_first` for prior strategy versions. Self-funding \
strategy is a TIME SERIES — the value compounds when each run \
sees the previous one and detects drift in days_of_runway, \
burn_multiple, and prepay_conversion_rate.\n\n\
After delivery: `memory_store` the runway, burn multiple, \
Profit First allocations, the 90-day plan slug, the no-raise \
commitments. `decision_log` the strategy as 'proposed'. \
`kpi_record` every metric in §12 so the next review compares \
against actuals. `entity_upsert` any new RBF / debt provider \
record (type=lender) so future runs read the prior diligence \
without re-doing it.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_funding_growth_uses_supplied_provider_and_model() {
        let cfg =
            self_funding_growth_strategist_preset("openrouter", "openai/gpt-5.5");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "openai/gpt-5.5");
    }

    #[test]
    fn self_funding_growth_analytical_temperature() {
        // Cash strategy is mostly deterministic from the books;
        // small variance helps with Hormozi-style offer
        // brainstorming.
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!(
            (0.2..=0.5).contains(&t),
            "self_funding_growth temperature out of analytical band: {t}"
        );
    }

    #[test]
    fn self_funding_growth_is_agentic_with_iteration_budget() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // Strategy traverses books + delegation chain; needs room.
        assert!(cfg.max_iterations >= 16);
    }

    #[test]
    fn self_funding_growth_names_canonical_frameworks() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for framework in [
            "Hormozi grand-slam offer",
            "Profit First",
            "Rule of 40",
            "Burn Multiple",
            "1000-Day Plan",
            "37signals no-investor",
            "calm-company",
            "minimalist entrepreneur",
            "indie-hackers revenue ladder",
            "build-in-public",
            "Cash Conversion Cycle",
            "solopreneur",
        ] {
            assert!(
                prompt.contains(framework),
                "missing canonical framework: '{framework}'"
            );
        }
    }

    #[test]
    fn self_funding_growth_refuses_default_fundraise() {
        // The load-bearing discipline. Without this line in the
        // prompt the agent recommends 'just raise' whenever cash
        // is short — defeating the whole point of the role.
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Refuse to recommend fundraising"));
        assert!(prompt.contains("tactic of LAST RESORT"));
    }

    #[test]
    fn self_funding_growth_prompt_mandates_thirteen_section_output() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for section in [
            "Current cash position (in DAYS",
            "Burn Multiple (Sacks)",
            "Rule of 40 audit",
            "Offer audit",
            "Pre-payment / annual-prepay redesign",
            "90-day cash plan",
            "Capital-efficient growth moves (ranked)",
            "Profit First allocations (mandatory)",
            "Revenue-based financing options",
            "The 'no-raise' commitment",
            "Falsification — when bootstrap IS wrong",
            "KPI commits",
            "Falsification triggers",
        ] {
            assert!(
                prompt.contains(section),
                "missing output section: '{section}'"
            );
        }
    }

    #[test]
    fn self_funding_growth_reports_runway_in_days_not_months() {
        // Specific discipline — months hides urgency. Must be in
        // the prompt.
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Days, not months"));
        assert!(prompt.contains("hides urgency"));
    }

    #[test]
    fn self_funding_growth_names_rbf_providers() {
        // When RBF is genuinely needed, the prompt names the
        // current options so the agent doesn't have to invent
        // them. Pipe, Capchase, Founderpath, Stripe Capital,
        // Clearco are the relevant set as of 2026.
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for provider in ["Pipe", "Capchase", "Founderpath", "Stripe Capital", "Clearco"] {
            assert!(
                prompt.contains(provider),
                "missing RBF provider: '{provider}'"
            );
        }
    }

    #[test]
    fn self_funding_growth_honest_about_when_bootstrap_is_wrong() {
        // Bootstrap fundamentalism is also bad. §11 must be in
        // the prompt — naming the conditions where raising IS
        // the right call.
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Honest about when bootstrap IS wrong"));
        assert!(prompt.contains("winner-takes-most"));
    }

    #[test]
    fn self_funding_growth_persists_via_business_stores() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
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
    fn self_funding_growth_grants_calculator_and_delegate() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        assert!(
            cfg.allowed_tools.iter().any(|t| t == "calculator"),
            "needs calculator for CCC / burn multiple / Rule of 40 math"
        );
        assert!(
            cfg.allowed_tools.iter().any(|t| t == "delegate"),
            "needs delegate to cross-validate with treasurer / pricing / pmf"
        );
    }

    #[test]
    fn self_funding_growth_does_not_grant_code_modification() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations", "opencode_cli", "file_edit"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "strategy work does not touch the codebase: must not include {forbidden}"
            );
        }
    }

    #[test]
    fn self_funding_growth_delegates_clearly() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "fundraise_captain",
            "treasurer",
            "cfo_advisor",
            "finance_controller",
            "pricing_strategist",
            "pmf_strategist",
            "growth_hacker",
            "tax_advisor",
            "decision_scientist",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn self_funding_growth_isolated_memory_namespace() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "self_funding_growth");
    }

    #[test]
    fn self_funding_growth_no_api_key_baked_in() {
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn self_funding_growth_prompt_names_profit_first_bands() {
        // Profit First allocations are specific per revenue band.
        // The prompt must include the bands or the agent invents
        // arbitrary percentages.
        let cfg = self_funding_growth_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        // Sub-$250K band is the default for early bootstrap.
        assert!(prompt.contains("Sub-\\$250K"));
        // $1M-$5M band is the upper boundary before cfo_advisor.
        assert!(prompt.contains("\\$1M-\\$5M"));
    }
}
