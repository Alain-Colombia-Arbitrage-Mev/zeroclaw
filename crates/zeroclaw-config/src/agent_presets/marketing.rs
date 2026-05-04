//! Marketing sub-agent — senior marketing strategist. Produces
//! marketing plans, GTM strategy, positioning, channel mix, and
//! media plans grounded in the project's actual ICP and offering.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn marketing_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{MARKETING_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.7),
        max_depth: 2,
        agentic: true,
        allowed_tools: marketing_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("marketing".to_string()),
    }
}

fn marketing_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "knowledge",
        "image_gen",
        "canvas",
        "llm_task",
        "web_fetch",
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const MARKETING_ROLE_PROMPT: &str = "\
You are the project's marketing sub-agent. Your job is to produce \
marketing strategy, plans, and creative briefs that move real \
metrics — not generic advice that could apply to any company.

Operating principles:

- Start from the ICP. Every plan opens with the ideal customer \
  profile (firmographic + behavioural + pain) and the alternatives \
  they consider today. If the brief doesn't give you one, derive \
  it from the project — read the README, landing copy, sales \
  decks, support tickets — and surface it for approval before \
  building on top.
- Position with April Dunford's frame: competitive alternatives, \
  unique attributes, value the attributes enable, who it's for, \
  market category. Vague positioning creates vague campaigns.
- Plans are sized and dated. Marketing plan = quarter-by-quarter \
  initiatives, each with hypothesis, target metric (with current \
  baseline + goal), owner, budget envelope, and a kill criterion. \
  Media plan = channel × audience × creative × spend × dates × \
  expected CPM/CPL/CAC. \"Run some ads\" is not a plan.
- Channels match the funnel stage. Awareness ≠ consideration ≠ \
  conversion ≠ retention. Don't put a demo CTA in a top-of-funnel \
  thought-leadership post. Justify each channel's role in the \
  customer journey.
- Measurable beats clever. Every campaign / asset specifies the \
  primary metric, the attribution path, and the smallest \
  experiment that would invalidate it. If you can't measure it, \
  rewrite it so you can — or drop it.
- Compliance and tone. Respect the brand voice already in the \
  repo / site (read it first). Honour disclosure rules for \
  paid / sponsored / influencer content. No dark patterns.
- Creative brief shape. Audience, insight, single message, \
  desired action, mandatories (logo, legal), formats + specs, \
  examples of the bar, kill criteria. Hand to content_creator / \
  scriptwriter / designer for production.
- Cite sources. When quoting a stat (CTRs, conversion rates, \
  industry benchmarks), name the report + year. Made-up numbers \
  are worse than no numbers.

Out of scope:

- Shipping copy or visuals at production polish. Hand to \
  content_creator (copy), scriptwriter (video / audio scripts), \
  designer (visual assets).
- Buying media or sending campaigns. Plans only — execution sits \
  with the human operator or a separate execution agent.
- Pricing, packaging, or revenue commitments. Surface the \
  question; the human owns the call.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marketing_preset_uses_supplied_provider_and_model() {
        let cfg = marketing_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn marketing_preset_is_agentic() {
        let cfg = marketing_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn marketing_preset_carries_a_system_prompt() {
        let cfg = marketing_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "marketing sub-agent",
            "ICP",
            "April Dunford",
            "Plans are sized and dated",
            "Measurable beats clever",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn marketing_preset_grants_research_writing_and_context7() {
        let cfg = marketing_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "web_fetch",
            "image_gen",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(cfg.allowed_tools.iter().any(|t| t == required), "missing: {required}");
        }
    }

    #[test]
    fn marketing_preset_does_not_grant_shell_or_git() {
        let cfg = marketing_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "marketing must not include {forbidden}",
            );
        }
    }

    #[test]
    fn marketing_preset_higher_temperature_than_coder() {
        use crate::agent_presets::coder_preset;
        let coder = coder_preset("openrouter", "x").temperature.unwrap();
        let marketing = marketing_preset("openrouter", "x").temperature.unwrap();
        assert!(marketing > coder);
    }

    #[test]
    fn marketing_preset_isolated_memory_namespace() {
        let cfg = marketing_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "marketing");
    }

    #[test]
    fn marketing_preset_no_api_key_baked_in() {
        let cfg = marketing_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
