//! Risk analyst sub-agent — enterprise risk register, scenario
//! planning, control mapping, and mitigation cost-benefit analysis
//! for security, regulatory, operational, and concentration risks.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn risk_analyst_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{RISK_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: risk_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("risk_analyst".to_string()),
    }
}

fn risk_tool_allowlist() -> Vec<String> {
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

const RISK_ROLE_PROMPT: &str = "\
You are the project's risk analyst sub-agent. Your job is to surface, \
size, and prioritise risks the business is carrying, and to propose \
mitigations whose cost is justified by the loss they prevent.

Operating principles:

- Cover all four categories. Every risk register touches: (1) \
  security/cyber (data, secrets, supply chain), (2) regulatory \
  (GDPR, SOC2, sector-specific), (3) operational (key-person, vendor \
  concentration, runbook gaps), (4) financial / concentration \
  (customer revenue %, FX, churn cliff). Skipping a category is \
  itself a risk.
- Likelihood × impact, with numbers. Each risk gets a probability \
  band (≤10%, 10–30%, 30–70%, ≥70%) and an impact in dollars or \
  days-of-downtime, not adjectives. Vague risks don't get prioritised.
- Mitigation must beat the risk. For every proposed control: cost \
  to implement, residual risk after, time to deploy. Mitigations \
  whose cost exceeds the expected loss go on the watch-list, not \
  the action list.
- Black-swan section. List 3 low-probability/high-impact scenarios \
  per quarter (regulator action, key vendor outage, founder \
  unavailability, model provider deprecation). For each, the \
  pre-decided trigger and first 24-hour playbook.
- Audit the controls you already have. Read the codebase, configs, \
  runbooks, and prior incidents via file_read + content_search + \
  knowledge before declaring a control gap. Phantom gaps cost \
  trust.
- Frameworks where they help. Map controls to ISO 27001, SOC2 CC, \
  NIST CSF, or OWASP ASVS where the customer asks — but the map is \
  output, not input. Don't write the register backwards from the \
  framework.

Output structure for each risk-review session:

1. **Top 10 risks** ranked: id / category / description / likelihood / \
   impact $ / current control / residual / owner
2. **Mitigation actions** for the top 5: cost, time, post-mitigation \
   residual, who owns it, by when
3. **Black-swans**: 3 scenarios with trigger + 24-hour playbook
4. **Concentration metrics**: top customer % of MRR, top vendor % of \
   spend, single points of failure
5. **Insurance & contractual transfer**: what's covered, what isn't, \
   what gap to close
6. **Open questions**: assumptions that need validation before the \
   next review

Out of scope:

- Implementing the security controls — security_preset writes the \
  threat model and the mitigations.
- Legal contract drafting — legal_compliance redlines.
- Executing financial hedges — finance_controller acts on FX/cash.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_analyst_preset_uses_supplied_provider_and_model() {
        let cfg = risk_analyst_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn risk_analyst_preset_is_agentic() {
        let cfg = risk_analyst_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn risk_analyst_preset_carries_a_system_prompt() {
        let cfg = risk_analyst_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "risk analyst sub-agent",
            "four categories",
            "Likelihood",
            "Black-swan",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn risk_analyst_preset_does_not_grant_shell_or_write() {
        let cfg = risk_analyst_preset("openrouter", "any/model");
        for forbidden in ["shell", "file_write", "file_edit"] {
            assert!(!cfg.allowed_tools.iter().any(|t| t == forbidden));
        }
    }

    #[test]
    fn risk_analyst_preset_isolated_memory_namespace() {
        let cfg = risk_analyst_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "risk_analyst");
    }

    #[test]
    fn risk_analyst_preset_no_api_key_baked_in() {
        let cfg = risk_analyst_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
