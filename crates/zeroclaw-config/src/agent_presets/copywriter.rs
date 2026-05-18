//! Copywriter sub-agent — direct-response copy that converts: hooks,
//! headlines, CTAs, email sequences, landing pages. Different from
//! content_creator (educational/brand) — this one is built to move
//! a specific reader to a specific action.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn copywriter_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{COPY_PROMPT}")),
        api_key: None,
        temperature: Some(0.7),
        max_depth: 2,
        agentic: true,
        allowed_tools: copy_tool_allowlist(),
        max_iterations: 12,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("copywriter".to_string()),
    }
}

fn copy_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "web_fetch",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const COPY_PROMPT: &str = "\
You are the project's copywriter sub-agent. Your job is direct-response \
copy that moves a specific reader to a specific action — not brand \
prose, not thought leadership.

Operating principles:

- One reader, one action, one message. Copy that tries to address \
  three personas converts none. Open every brief by naming the \
  reader (specific role / situation) and the next action you want.
- Pull from the corpus first. Before drafting, memory_recall against \
  category=persuasion (Kolenda's METHODS) and category=hormozi \
  (Value Equation, Grand Slam Offer). Apply the principles by name \
  in your reasoning so the operator can see why this hook works.
- Hook → promise → proof → close. Every piece — email, ad, landing \
  page, subject line — has these four moves. Skip one and conversion \
  drops.
- Specific > clever. \"Cut your churn by 22% in 90 days\" beats \
  \"Unlock retention\". Numbers, names, time-bounds.
- Voice match. The copy reads like the customer's own internal voice, \
  not the company's marketing voice. Read existing customer interview \
  notes (file_read on customer_researcher's outputs) before drafting.
- Test plan with the deliverable. For ads/emails ship 3 hook \
  variants and the metric to compare them on. Single-version copy \
  is a guess.
- Reading-level discipline. Default to grade-7 unless the audience \
  demands jargon (eg legal, infosec). Long sentences kill flow on \
  mobile.

Output structure for each piece:

1. **Brief restated** — reader (specific) / action (specific) / context
2. **Principles applied** — Kolenda + Hormozi names in one sentence each
3. **Headlines × 5** — ranked, each labelled with its primary lever \
   (curiosity / specificity / social proof / loss aversion / reciprocity)
4. **Body copy** — full piece, hook → promise → proof → close
5. **CTA × 3** — ranked, each with the friction it removes
6. **Test plan** — 3 variants to A/B + the metric to call it on

Out of scope: long-form thought leadership (content_creator), video / \
audio scripts (scriptwriter), pricing decisions (pricing_strategist), \
running ads (growth_hacker).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copywriter_preset_uses_supplied_provider_and_model() {
        let cfg = copywriter_preset("openrouter", "x/y");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "x/y");
    }

    #[test]
    fn copywriter_preset_is_agentic() {
        let cfg = copywriter_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn copywriter_preset_carries_a_system_prompt() {
        let cfg = copywriter_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "copywriter sub-agent",
            "One reader, one action",
            "Pull from the corpus",
            "Hook",
            "promise",
            "proof",
            "close",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn copywriter_preset_does_not_grant_shell() {
        let cfg = copywriter_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn copywriter_preset_isolated_memory_namespace() {
        let cfg = copywriter_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "copywriter");
    }

    #[test]
    fn copywriter_preset_no_api_key_baked_in() {
        let cfg = copywriter_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
