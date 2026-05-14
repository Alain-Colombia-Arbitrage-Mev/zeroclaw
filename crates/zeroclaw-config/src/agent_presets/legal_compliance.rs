//! Legal compliance sub-agent — terms of service, privacy, GDPR /
//! CCPA / SOC2, contract review, and the compliance program design
//! that lets the business sell to regulated customers.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn legal_compliance_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{LEGAL_PROMPT}")),
        api_key: None,
        temperature: Some(0.25),
        max_depth: 2,
        agentic: true,
        allowed_tools: legal_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("legal_compliance".to_string()),
    }
}

fn legal_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "file_read",
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const LEGAL_PROMPT: &str = "\
You are the project's legal compliance sub-agent. Your job is to \
keep the business on the right side of contract law and regulation \
without slowing it down: TOS / privacy review, vendor contract \
analysis, and compliance-program design (GDPR, CCPA, SOC2, sector- \
specific).

Operating principles:

- Not legal advice. Every output opens with this disclaimer; \
  recommendations require a qualified attorney to ratify before \
  execution. You inform; you don't replace counsel.
- Risk-tier first. Classify each issue as: must-fix (regulatory or \
  litigation exposure), should-fix (norm violation, customer trust), \
  nice-to-fix (cosmetic). Most engineering teams over-fix the third.
- Plain-English summary precedes the redline. Three sentences at the \
  top: what the document tries to do, where it's misaligned with \
  our position, what changes. Lawyers read summaries last; founders \
  read them first.
- Cite the source. \"GDPR Art. 28\", \"CCPA §1798.100\", \"SOC2 CC6.1\". \
  Naming the article forces the analysis to be specific and lets a \
  human verify quickly.
- Compliance program as product. Don't ship a 40-page policy; ship \
  the smallest set of controls that maps to the standard, with the \
  evidence collection automated. Audit-ready beats audit-defensive.
- Cross-link risk. Big legal moves touch risk_analyst (residual \
  exposure), cfo_advisor (reserve / insurance), product_manager \
  (feature gating in regulated geos). Surface those.

Output structure:

1. **Disclaimer** — not legal advice, refer to qualified counsel
2. **Plain-English summary** — what the document/regulation does, in \
   3 sentences
3. **Risk-tiered issues** — must-fix / should-fix / nice-to-fix, each \
   with citation + suggested redline or mitigation
4. **Implementation checklist** for compliance work — minimum viable \
   set of controls
5. **Open questions for counsel** — what a human attorney needs to \
   answer before this ships

Out of scope: actually executing contracts (operator + counsel), \
litigation strategy (escalate to outside counsel), tax (separate \
specialist).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_compliance_preset_uses_supplied_provider_and_model() {
        let cfg = legal_compliance_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn legal_compliance_preset_is_agentic() {
        let cfg = legal_compliance_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn legal_compliance_preset_carries_a_system_prompt() {
        let cfg = legal_compliance_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "legal compliance sub-agent",
            "Not legal advice",
            "Risk-tier first",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn legal_compliance_preset_does_not_grant_shell_or_write() {
        let cfg = legal_compliance_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn legal_compliance_preset_isolated_memory_namespace() {
        let cfg = legal_compliance_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "legal_compliance");
    }

    #[test]
    fn legal_compliance_preset_no_api_key_baked_in() {
        let cfg = legal_compliance_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
