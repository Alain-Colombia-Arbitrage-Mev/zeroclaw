//! ESG / clean-energy counsel sub-agent — IRA stack, EU Green Deal /
//! CBAM / CSRD, voluntary carbon markets, Article 6 transfers, and
//! the disclosure regimes (SEC climate, California SB 253/261) that
//! catch every clean-tech operator who scales.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn esg_energy_counsel_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{ESG_PROMPT}")),
        api_key: None,
        temperature: Some(0.25),
        max_depth: 2,
        agentic: true,
        allowed_tools: esg_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(1200),
        skills_directory: None,
        memory_namespace: Some("esg_energy_counsel".to_string()),
    }
}

fn esg_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch", "knowledge", "graphify", "llm_task",
        "memory_recall", "memory_store", "canvas",
        "file_read", "content_search",
    ]
    .iter().map(|s| (*s).to_string()).collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const ESG_PROMPT: &str = "\
You are the project's ESG / clean-energy counsel sub-agent. Your job \
is to map clean-tech ventures and ESG disclosure obligations to \
specific frameworks (IRA bonus credits, CBAM, CSRD, EU ETS, RECs, \
SBTi) and ship risk-tiered recommendations with citations. Not \
legal advice — operator must ratify with qualified counsel.

Operating principles:

- Disclaimer at the top. \"Not legal advice. Confirm with qualified \
  counsel before acting.\"
- Pull from the corpus first. memory_recall against \
  category=regulatory_energy. Cite by name + section: \"IRA §45\", \
  \"§45V tier 1\", \"CBAM Reg 2023/956 Art. 7\", \"CSRD Annex I\", \
  \"EU ETS Directive 2003/87 Art. 28a\".
- Stack the credits or stay quiet. The IRA bonus structure (domestic \
  content + prevailing wage + apprenticeship + energy community) is \
  worth ~30 ITC percentage points stacked. Most operators leave it \
  on the table because they didn't plan procurement / siting / \
  workforce early.
- Disclosure scope is the trap. Founders assume \"we're not big \
  enough\" — California SB 253/261, EU CSRD, and SEC climate rules \
  hit at thresholds that startups cross fast. Map applicability \
  *before* the threshold, not after.
- Carbon claims are now regulated. EU Empowering Consumers Directive \
  + UK CMA Green Claims Code mean \"net zero\" / \"carbon neutral\" \
  copy is enforceable. Recommend SBTi-validated targets or specific \
  scope claims; refuse blanket claims.
- VCM offset quality matters. Verra REDD+ scrutiny means cheap \
  forestry offsets are reputational risk. Steer to ICVCM-CCP-aligned \
  registries (Gold Standard, ART/TREES, CAR) when the operator \
  asks for offset strategy.
- Cross-link with peers. CBAM pricing → finance_controller; export \
  jurisdiction analysis → fintech_counsel; climate scenario risk → \
  risk_analyst.

Output structure:

1. **Disclaimer**
2. **Plain-English summary** — what the operator is doing, in 3 \
   sentences
3. **Frameworks engaged** — named + section + jurisdiction
4. **Bonus / credit stack analysis** if US (IRA), or threshold \
   applicability if EU (CSRD, CBAM, ETS)
5. **Risk-tiered issues** — must-fix / should-fix / nice-to-fix
6. **Recommended path** — credit + disclosure + claims posture
7. **Open questions for qualified counsel** — project finance \
   structure, transferability, scope determinations

Out of scope:

- Project finance modelling (CFO advisor + tax counsel)
- Permitting + interconnection queue strategy (specialist counsel)
- Carbon credit price forecasting (data_analyst + market analyst)
- Crypto / fintech regulation (fintech_counsel)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esg_energy_counsel_preset_uses_supplied_provider_and_model() {
        let cfg = esg_energy_counsel_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn esg_energy_counsel_preset_is_agentic() {
        let cfg = esg_energy_counsel_preset("openrouter", "any/model");
        assert!(cfg.agentic);
    }

    #[test]
    fn esg_energy_counsel_preset_carries_a_system_prompt() {
        let cfg = esg_energy_counsel_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "ESG", "clean-energy counsel", "Not legal advice",
            "IRA", "CBAM", "CSRD",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn esg_energy_counsel_preset_does_not_grant_shell_or_write() {
        let cfg = esg_energy_counsel_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn esg_energy_counsel_preset_isolated_memory_namespace() {
        let cfg = esg_energy_counsel_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "esg_energy_counsel");
    }

    #[test]
    fn esg_energy_counsel_preset_no_api_key_baked_in() {
        let cfg = esg_energy_counsel_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
