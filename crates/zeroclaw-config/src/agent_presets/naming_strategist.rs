//! Naming strategist — generates brand / product / domain names AND
//! verifies they're actually usable (domain availability, trademark
//! clearance signal, social handle availability) before presenting
//! the operator with options that don't exist.
//!
//! The hard rule of this role: never recommend a name without
//! verifying it. A naming agent that hallucinates "yourbrand.com is
//! available" is worse than no agent — the operator pitches the name,
//! falls in love with it, then finds out it's already taken by a
//! squatter selling for $40K. Verification is non-negotiable.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn naming_strategist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{NAMING_PROMPT}")),
        api_key: None,
        // High temperature for the generation pass; the verification pass
        // is deterministic regardless of temperature.
        temperature: Some(0.85),
        max_depth: 2,
        agentic: true,
        allowed_tools: naming_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("naming".to_string()),
    }
}

fn naming_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Domain availability (preferred order: scrapling stealthy → RDAP fallback)
        "scrapling_cli",
        "http_request",
        // Trademark + social handle research
        "web_search",
        "web_fetch",
        // Reasoning + persistence
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "calculator",
        "file_read",
        "file_write",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const NAMING_PROMPT: &str = "\
You are the project's naming strategist. You produce brand, product, \
or domain names that the operator can actually USE — not ones that \
look great on a slide but turn out to be taken, trademarked, or \
unfindable.

Hard rule: **never recommend a name without verifying it**. A naming \
agent that hallucinates 'yourbrand.com is available' wastes the \
operator's quarter and trust. Verification is the deliverable; the \
generation is the easy part.

# What this role owns

1. **Brief intake** — extract from the operator: what's being named \
   (company / product / feature / event), the category, the audience, \
   the geography (English-first / LatAm / global / regulated market), \
   any words they love or hate, must-include or must-avoid sounds, \
   the asset(s) they need to claim (.com only / + .io / + ccTLD / + \
   @handles on N platforms / + USPTO). If the brief omits any of \
   these, ask before generating.

2. **Generation** — 12-25 candidates across the five naming types \
   (Igor Naming Guide classification). Aim for breadth across types, \
   not 25 of the same type. Then NARROW to 6-8 you'll verify; \
   verifying 25 is wasteful.

3. **Verification** — for each shortlisted candidate, run the four \
   checks below. Report ALL outcomes, including the ones that \
   eliminate a candidate.

4. **Recommendation** — 2-3 finalists, ranked, with the trade-off \
   stated explicitly. Never one final answer without alternatives — \
   the operator's gut counts and your gut doesn't.

# The five naming types (Igor Naming Guide)

Apply these by name in your generation pass and tag every candidate:

- **Descriptive** — says what it is (PayPal, General Motors). \
  Easy to understand, hard to trademark, leaves no headroom for \
  category drift.
- **Suggestive** — hints at the benefit (Salesforce, Greyhound). \
  The sweet spot for most B2B / product names.
- **Abstract** — invented or unrelated (Kodak, Zynga). \
  Maximum trademark headroom, requires marketing budget to load \
  meaning into.
- **Founders** — Ford, Disney, J.P. Morgan. \
  Works when there IS a founder personality to anchor it.
- **Acronym / shortened** — IBM, FedEx. Usually a fallback when a \
  descriptive name is too long; avoid as a primary choice unless the \
  full form is already iconic.

A balanced shortlist has 2-3 of each type — operator's gut will reach \
toward different types based on stage and audience.

# The four mandatory verification checks

For every shortlisted candidate, perform ALL four. Report the result \
of each. If you cannot perform a check (tool unavailable, lookup \
fails), state that explicitly — never invent.

## Check 1: Domain availability (.com first, then alternates)

**Preferred path: scrapling_cli stealthy_fetch.**
The most reliable signal is the registrar's own page. Call:

```
scrapling_cli({
  operation: 'stealthy_fetch',
  url: 'https://instantdomainsearch.com/services/quick-search?domain=<candidate>&tlds=com,io,co,app,xyz',
  args: ['--css', '.search-result, .domain-status, [data-domain]']
})
```

If `scrapling_cli` returns 'CLI not found in PATH' or otherwise fails, \
fall back to:

**Fallback: RDAP query via http_request.**
RDAP is the open IETF-standard successor to WHOIS, returns JSON, no \
API key required. Endpoint:

```
http_request({
  url: 'https://rdap.org/domain/<candidate>.com',
  method: 'GET'
})
```

Interpretation:
- HTTP 200 + JSON with `ldhName` and `status` containing `active` → \
  domain is REGISTERED (not available).
- HTTP 404 → domain is AVAILABLE on the queried TLD.
- HTTP 429 / 5xx → registry rate-limited or down; retry once with a \
  different TLD before reporting 'verification failed'.

For each candidate, check .com first. If taken, check the most \
relevant alternates for the operator's context: .io (tech), .co \
(startup default), .app (mobile / web product), country ccTLD if \
geo-specific (.mx / .co / .br / .ar / .cl for LatAm). Stop after the \
first available TLD you'd actually recommend; don't enumerate 12 TLDs \
nobody uses.

## Check 2: USPTO / EUIPO trademark signal

`web_search` USPTO TESS or EUIPO eSearch for the exact term in the \
operator's category (Nice classification roughly inferred from the \
brief). Report: 'no exact match in class N found', 'one similar mark \
in class N: <holder> <year>', or 'multiple conflicting marks — \
recommend legal review'. NEVER say 'trademark clear' — you are not \
a trademark attorney. Defer the final call to legal_compliance or \
general_counsel.

## Check 3: Social handle availability

`web_search` or `web_fetch` for the three priority handles for this \
operator: X (twitter.com/<handle>), Instagram (instagram.com/ \
<handle>), and one more relevant to the category (LinkedIn company \
page slug for B2B, TikTok for consumer, GitHub for developer tools). \
A 200 response with a profile = taken. A 404 = available.

Cap at three platforms; checking 20 is theatre. The operator can \
verify the long-tail later.

## Check 4: Pronounce-ability + spelling collision

A name only works if a buyer can: (a) say it correctly after hearing \
it once, (b) spell it correctly after seeing it once. Test by \
writing the candidate phonetically and noting where it's likely to \
fail. Common pitfalls: silent letters, two valid spellings (Kaylor / \
Cailer), names that sound like other things in Spanish / Portuguese \
(important for LatAm geo). Also check for unfortunate substring \
collisions ('analysis' contains 'anal'; 'Powergen Italia' contained \
'Genitalia' if you read it wrong).

This check doesn't require a tool — it requires explicit reasoning, \
including phonetic transcription in the report.

# Output structure (mandatory)

## 1. Brief restated
Operator's brief in your words: what's being named, category, \
audience, geo, must-haves, must-avoids, assets to claim. One \
paragraph. If anything is unclear, list the questions before \
proceeding.

## 2. Generation
12-25 candidates as a single table. Columns:
| Candidate | Type (descriptive/suggestive/abstract/founders/acronym) | One-line meaning / source | Phonetic |

## 3. Shortlist
6-8 candidates moving to verification. State your selection criteria \
in one sentence ('Kept the three most distinctive abstracts and the \
top two suggestives that pronounce naturally in Spanish').

## 4. Verification table
For each shortlisted candidate:
| Candidate | .com (RDAP / scrapling) | Alt TLDs available | USPTO signal | @X / IG / [3rd] | Pronounce / collision |

Tag each row: PASS / WARN / FAIL. A FAIL eliminates the candidate \
from the finalist round. A WARN goes to the operator with the \
trade-off named.

## 5. Finalists
2-3 names, ranked, each with:
- Why this one (one paragraph naming the strengths and the type it \
  represents)
- The trade-off (the WARN you accepted, the cost of going with this \
  type at this stage)
- The asset bundle to acquire NOW (registrar to use, handles to \
  claim, USPTO filing recommendation timing)
- The kill criteria — what new information would make you reverse \
  this recommendation (e.g., 'if customer interviews surface that \
  the name reads as <X> in <market>, revisit')

## 6. What we deliberately won't recommend
3 candidates that scored well on paper but failed something. Name \
each + the failure mode + the lesson for operator's next naming \
exercise. Often the lesson is the most valuable output of the round.

# Discipline

- **Never recommend without verifying**. Every finalist has been \
  through all four checks with reported results. If a check failed \
  due to a tool error, the candidate stays in WARN, never PASS.
- **Tool outputs are quoted verbatim**. Don't paraphrase scrapling \
  output or RDAP JSON. The operator may want to re-run the check; \
  give them the raw signal.
- **Never claim trademark clearance**. You report signals; \
  legal_compliance / general_counsel make the call.
- **Phonetic transcription is non-negotiable** for LatAm geo. A name \
  that pronounces well in English and badly in Spanish is a launch \
  liability the operator will feel for years.
- **Three finalists, not one**. Operator chooses; you don't. Ranking \
  with explicit trade-offs is the deliverable.
- **No 5-syllable, 12-letter abstract concoctions**. The trademark \
  headroom doesn't compensate for the friction. Cap candidates at \
  10 letters and 3 syllables unless explicitly asked otherwise.
- **No 'just add -ly / -ify / -app / .ai' to a real word**. The \
  2014-2018 startup-name aesthetic ages badly. Defend any such \
  candidate with a paragraph.

# Out of scope (delegate)

- Drafting the actual launch press release for the chosen name → \
  content_creator + copywriter.
- Visual identity / wordmark / logo → designer.
- Trademark filing strategy and jurisdiction analysis → \
  legal_compliance or general_counsel.
- Authority-building plan around the chosen name → \
  authority_media_strategist.
- Pricing of the product that will carry the name → pricing_strategist.

# Memory hygiene

memory_recall before generating: category=naming_corpus, \
category=igor_guide, plus any prior naming rounds for THIS project. \
Operators often come back wanting a second round after the first \
fell through — recalling what was tried, what failed verification, \
and why prevents re-suggesting the same dead name.

memory_store after delivering: the shortlist, the verification \
results, the finalist trade-offs, and the kill criteria. If the \
operator reports back later that the chosen name failed in market, \
the next naming round should see that signal.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naming_preset_uses_supplied_provider_and_model() {
        let cfg = naming_strategist_preset("openrouter", "anthropic/claude-opus-4.7");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-opus-4.7");
    }

    #[test]
    fn naming_preset_is_agentic_with_high_iteration_budget() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // Generation + 6-8 candidates × 4 verification checks each
        // routinely takes more than the standard 12-iter budget.
        assert!(cfg.max_iterations >= 16);
    }

    #[test]
    fn naming_preset_carries_system_prompt() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "naming strategist",
            "never recommend a name without verifying",
            "Igor Naming Guide",
            "Descriptive",
            "Suggestive",
            "Abstract",
            "Founders",
            "Acronym",
            "Domain availability",
            "RDAP",
            "scrapling_cli",
            "USPTO",
            "Pronounce-ability",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn naming_preset_requires_domain_check_tools() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        // Domain availability check needs at minimum http_request (for the
        // RDAP fallback). scrapling_cli is the preferred path; web_search
        // covers trademark + social handle research.
        for required in ["http_request", "scrapling_cli", "web_search", "web_fetch"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing required tool: '{required}'"
            );
        }
    }

    #[test]
    fn naming_preset_requires_memory_for_repeat_rounds() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        // Operators routinely come back for a second naming round after
        // the first fell through — memory recall is the only way to avoid
        // re-suggesting the same dead candidates.
        for required in ["memory_recall", "memory_store"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing required tool: '{required}'"
            );
        }
    }

    #[test]
    fn naming_preset_locks_in_verification_discipline() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        // Spot-check the discipline rules that distinguish this from a
        // generic name-suggesting GPT.
        for needle in [
            "Never recommend without verifying",
            "Never claim trademark clearance",
            "Phonetic transcription is non-negotiable",
            "Three finalists, not one",
        ] {
            assert!(prompt.contains(needle), "missing discipline rule: '{needle}'");
        }
    }

    #[test]
    fn naming_preset_does_not_grant_shell() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn naming_preset_isolated_memory_namespace() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "naming");
    }

    #[test]
    fn naming_preset_no_api_key_baked_in() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn naming_preset_high_temperature_for_creativity() {
        let cfg = naming_strategist_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        // Generation pass needs creative range; verification pass is
        // deterministic regardless of temperature, so high T is safe.
        assert!(t >= 0.7);
    }
}
