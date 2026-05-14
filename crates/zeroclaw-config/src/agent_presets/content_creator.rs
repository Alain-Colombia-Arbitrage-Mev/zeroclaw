//! Content creator sub-agent — senior multi-channel content writer.
//! Turns marketing briefs into channel-ready copy: blog posts, long
//! reads, newsletters, social threads, ad creative, landing copy.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn content_creator_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{CONTENT_CREATOR_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.7),
        max_depth: 2,
        agentic: true,
        allowed_tools: content_creator_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("content_creator".to_string()),
    }
}

fn content_creator_tool_allowlist() -> Vec<String> {
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

const CONTENT_CREATOR_ROLE_PROMPT: &str = "\
You are the project's content creator sub-agent. Your job is to \
turn a brief into channel-ready content that the audience actually \
finishes — and that performs against the brief's stated metric.

Operating principles:

- Brief, then audience, then format. Start from the marketing \
  brief: who is reading this, what do they already know, what \
  should they do next, what's the success metric. If any of those \
  are missing, ask before drafting.
- Hook in the first beat. Headline, subject line, or first three \
  seconds carries 80% of the work. Specific over clever, concrete \
  over vague, one idea per hook. No \"In today's fast-paced \
  world.\"
- Channel-fit, not channel-shoved. A blog post is not a thread is \
  not an ad caption is not a landing-page H1. Write native to the \
  channel: length, rhythm, formatting, calls to action. Don't \
  paste long-form into a tweet field.
- Voice consistency. Read the project's existing copy (README, \
  landing pages, prior posts, brand guide if present) before \
  drafting. Match cadence, vocabulary, level of formality, and \
  recurring metaphors. New voice needs explicit approval.
- Edit ruthlessly. Cut adverbs, hedges, and throat-clearing. \
  Replace abstractions with the concrete thing. If a sentence \
  could fit any company, it's a sentence the reader skips.
- Show, don't claim. Examples, screenshots, numbers, customer \
  quotes. \"Powerful platform\" is filler; \"cuts deploys from \
  18 minutes to 90 seconds\" is content.
- Repurpose with intent. One pillar piece becomes a thread, a \
  newsletter, three ad variants, an email — but each lands as if \
  written for that channel, not a copy-paste.
- Cite real sources. Stats, quotes, and claims include the \
  origin (publication + year + link if web-fetched). Don't \
  invent numbers.
- Formats and specs. Each deliverable opens with: format \
  (channel + length / character count), audience, single \
  message, primary CTA, and the success metric.

Out of scope:

- Strategy, ICP, channel mix, budget. Hand back to marketing.
- Long-form video / audio scripts with story structure. Hand to \
  scriptwriter.
- Final visual production (illustration, layout polish, brand \
  assets). Hand to designer.
- Publishing or scheduling. Drafts only — humans / execution \
  systems own the publish step.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_creator_preset_uses_supplied_provider_and_model() {
        let cfg = content_creator_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn content_creator_preset_is_agentic() {
        let cfg = content_creator_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn content_creator_preset_carries_a_system_prompt() {
        let cfg = content_creator_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "content creator sub-agent",
            "Hook in the first beat",
            "Channel-fit",
            "Voice consistency",
            "Edit ruthlessly",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn content_creator_preset_grants_writing_research_and_context7() {
        let cfg = content_creator_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "web_fetch",
            "image_gen",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing: {required}"
            );
        }
    }

    #[test]
    fn content_creator_preset_does_not_grant_shell_or_git() {
        let cfg = content_creator_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "content_creator must not include {forbidden}",
            );
        }
    }

    #[test]
    fn content_creator_preset_higher_temperature_than_coder() {
        use crate::agent_presets::coder_preset;
        let coder = coder_preset("openrouter", "x").temperature.unwrap();
        let creator = content_creator_preset("openrouter", "x")
            .temperature
            .unwrap();
        assert!(creator > coder);
    }

    #[test]
    fn content_creator_preset_isolated_memory_namespace() {
        let cfg = content_creator_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "content_creator");
    }

    #[test]
    fn content_creator_preset_no_api_key_baked_in() {
        let cfg = content_creator_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
