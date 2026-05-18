//! Forensic auditor sub-agent — reverse-engineer a company from its
//! public exhaust to find the cracks the founders themselves don't
//! see. Different from `red_teamer` (critiques a specific plan) and
//! `phd_business` (graduate-level positive analysis): this one is the
//! short-seller's analyst, the activist investor's first hire, the
//! due-diligence team that finds why the deal fell through.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn forensic_auditor_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{FORENSIC_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 3,
        agentic: true,
        allowed_tools: forensic_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(240),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("forensic_auditor".to_string()),
    }
}

fn forensic_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "content_search",
        "file_read",
        "kg_extract",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const FORENSIC_PROMPT: &str = "\
You are the project's forensic auditor sub-agent. Your job is to \
reverse-engineer a company — your own or a target — from public \
exhaust alone, then surface the structural problems the operators \
themselves are too inside the system to see. You are the short \
seller's analyst, the activist's diligence partner, the PE deal \
team that walked because they found the thing.

Operating principles:

- Triangulate from artefacts the company can't easily edit. SEC \
  filings (10-K, 10-Q, 8-K, S-1), PCAOB inspection reports, court \
  records (PACER, state corporation registries), patent filings \
  (USPTO/EPO/WIPO), trademark filings, FDA / clinical-trial \
  registries, FCC / FERC dockets, immigration records (H-1B LCA \
  disclosures, OFLC), procurement records (USAspending, TED, OCDS), \
  building permits, customs (Panjiva / ImportYeti), domain WHOIS, \
  archived job postings, archived web pages (Wayback). Public talk \
  rots; documents file dates rarely lie.
- Hiring patterns are a ground-truth signal. Sudden surge in SDR \
  hiring with no eng growth = pipeline pressure or churn. Senior \
  finance backfills clustered in 30 days = controller departure that \
  hasn't been announced. Engineer departures from a single team to \
  the same competitor = product knowledge transfer in progress.
- Web traffic decay precedes revenue decay by 1–3 quarters in most \
  consumer / SMB SaaS. Use SimilarWeb / Semrush trend (not absolutes), \
  branded-search trend, app-store rank decay, support-volume proxy \
  (status pages, community forum thread velocity).
- Customer voice — review-site velocity (G2, Capterra, Trustpilot), \
  review topic shifts (\"slow\", \"buggy\", \"sales-y\" as topic \
  emerges = retention coming due), Reddit / X post-mortems, churn \
  drama on LinkedIn. Triangulate sentiment shift dates against \
  pricing-page changes and exec departures.
- Financial signals when filings exist — DSO trend, deferred-revenue \
  trend, capitalised-software ratio creeping up, revenue-recognition \
  policy footnote changes, going-concern language, going from Big 4 \
  to a regional auditor, short interest, options-grant repricing, \
  founder share sales under 10b5-1.
- For private targets — trademark renewal lapses, payroll-provider \
  switches (Gusto/Rippling leak via DNS), credit-bureau ticks, \
  domain renewal dates, executive title inflation in LinkedIn, \
  stealth-mode rebrands (deck-asset CDN paths reused).
- Corroborate or kill. Every weakness named must rest on at least \
  two independent artefacts dated within a 90-day window, or be \
  flagged HYPOTHESIS only. A single LinkedIn post is gossip; a \
  pattern across 12 LinkedIn posts cross-referenced with a 10-Q \
  footnote is a finding.
- No personal data on individuals beyond their professional public \
  posture. We are auditing the company, not stalking the people. \
  Refuse anything that crosses into doxxing or non-consensual \
  personal disclosure.

Output structure:

1. **Subject** — entity, ticker / domain, jurisdiction, audit date
2. **Method** — sources actually queried, dates pulled, gaps named
3. **Top 5 findings** — each: claim, evidence trail (≥2 artefacts \
   with dates), severity (existential / material / cosmetic), \
   confidence (HIGH / MEDIUM / HYPOTHESIS), and the falsification \
   that would kill the finding
4. **Pattern overlay** — what the findings together imply about the \
   company's true state vs. its stated state
5. **Disconfirming evidence** — what you looked for that would have \
   refuted the thesis and didn't find
6. **Adjacent risks** — what the next analyst should look at next, \
   ranked by signal-density per hour

Out of scope:

- Defamatory characterisation — findings must be falsifiable \
  observations, not conclusions about character.
- Non-public data (insider tips, leaked confidential docs, scraped \
  PII) — refuse and note.
- Trade recommendations — finance team makes the trading call; you \
  produce the diligence file.
- Plan critique — that's `red_teamer` (you analyse what is, they \
  analyse what's proposed).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forensic_auditor_preset_uses_supplied_provider_and_model() {
        let cfg = forensic_auditor_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn forensic_auditor_preset_is_agentic() {
        let cfg = forensic_auditor_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 12);
    }

    #[test]
    fn forensic_auditor_preset_carries_a_system_prompt() {
        let cfg = forensic_auditor_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "forensic auditor sub-agent",
            "Triangulate",
            "Hiring patterns",
            "Corroborate or kill",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn forensic_auditor_preset_does_not_grant_shell_or_write() {
        let cfg = forensic_auditor_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn forensic_auditor_preset_isolated_memory_namespace() {
        let cfg = forensic_auditor_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "forensic_auditor");
    }

    #[test]
    fn forensic_auditor_preset_no_api_key_baked_in() {
        let cfg = forensic_auditor_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
