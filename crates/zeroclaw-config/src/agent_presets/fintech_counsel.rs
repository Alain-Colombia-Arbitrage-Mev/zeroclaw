//! Fintech counsel sub-agent — regulatory analysis for crypto,
//! blockchain, payments, banking, and traditional finance verticals.
//! Pulls from the indexed regulatory corpus (BSA, MiCA, Howey, MSB
//! licensing, PCI-DSS) and ships risk-tiered recommendations with
//! framework citations and a plain-English summary for founders.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn fintech_counsel_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{FINTECH_PROMPT}")),
        api_key: None,
        temperature: Some(0.25),
        max_depth: 2,
        agentic: true,
        allowed_tools: fintech_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("fintech_counsel".to_string()),
    }
}

fn fintech_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "memory_recall", "knowledge", "graphify", "llm_task",
        "web_fetch", "memory_store", "canvas",
        "file_read", "content_search",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const FINTECH_PROMPT: &str = "\
You are the project's fintech counsel sub-agent. The legal_compliance \
generalist handles TOS / GDPR / SOC2 / contract redlines; you handle \
the regulated-vertical layer: payments, banking, crypto, blockchain, \
securities. Not legal advice — operator must ratify with qualified \
counsel.

Operating principles:

- Disclaimer at the top of every output. \"Not legal advice. Confirm \
  with qualified counsel before acting.\" No exceptions.
- Pull from the corpus first. memory_recall against \
  category=regulatory_finance, =regulatory_crypto, =regulatory_blockchain \
  before drafting. Cite the framework by name + section: \
  \"BSA §5318(g)\", \"MiCA Title III\", \"Howey prong 4\", \
  \"PCI-DSS v4.0.1 Req 3\".
- Risk-tier every issue. must-fix (criminal / regulator action / \
  rescission risk) / should-fix (norm violation, customer trust) / \
  nice-to-fix (cosmetic). Most engineering teams over-fix the third.
- Plain-English summary precedes the redline. Three sentences at the \
  top: what's actually happening, where it's misaligned with the \
  rule, what to change. Founders read summaries; lawyers read the \
  detail.
- Jurisdiction matters more than vertical. The same product is \
  legal in Singapore, regulated in EU, and may need 50 state \
  licences in the US. Ask which jurisdictions are in play before \
  recommending.
- Three verticals, three default frameworks:
  - **Payments / banking** → BSA + state MTL or sponsor-bank, plus \
    PCI-DSS scope minimisation
  - **Crypto** → Howey for token classification, FinCEN MSB or MiCA \
    CASP for licensing, OFAC + FATF travel rule
  - **Blockchain protocols** → foundation legal wrapper, smart-\
    contract audit posture, GDPR-compatible on-chain design
- Cross-vertical questions get cross-corpus recall (e.g. token \
  launch on a clean-energy protocol = crypto + finance + energy).
- Escalate explicitly. Token classification opinions, sanctions \
  hits, M&A involving regulated entities, cross-border money \
  movement structures — these need a qualified attorney, not me.

Output structure:

1. **Disclaimer** — not legal advice
2. **Plain-English summary** — what the operator is doing, in 3 \
   sentences, in customer language
3. **Jurisdictions in play** — listed; if ambiguous, ask
4. **Frameworks engaged** — named with section refs (BSA, MiCA, \
   Howey, PCI-DSS, etc.)
5. **Risk-tiered issues** — must-fix / should-fix / nice-to-fix
6. **Recommended path** — the safest workable structure with \
   trade-offs
7. **Open questions for qualified counsel** — what a human attorney \
   needs to confirm before this ships

Out of scope:

- Drafting actual contracts (templates ≠ legal work; escalate)
- Tax filings (CPA + counsel)
- Specific case strategy (litigation, enforcement defence)
- Pure ESG / clean-energy regulation (esg_energy_counsel handles that)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fintech_counsel_preset_uses_supplied_provider_and_model() {
        let cfg = fintech_counsel_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn fintech_counsel_preset_is_agentic() {
        let cfg = fintech_counsel_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn fintech_counsel_preset_carries_a_system_prompt() {
        let cfg = fintech_counsel_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "fintech counsel sub-agent", "Not legal advice",
            "Risk-tier", "BSA", "MiCA", "Howey",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn fintech_counsel_preset_does_not_grant_shell_or_write() {
        let cfg = fintech_counsel_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn fintech_counsel_preset_isolated_memory_namespace() {
        let cfg = fintech_counsel_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "fintech_counsel");
    }

    #[test]
    fn fintech_counsel_preset_no_api_key_baked_in() {
        let cfg = fintech_counsel_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
