//! Scriptwriter sub-agent — senior screenwriter for video, audio,
//! podcast, explainer, and ad scripts. Owns story structure, beats,
//! dialogue, pacing, and shooting-ready format.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn scriptwriter_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{SCRIPTWRITER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.8),
        max_depth: 2,
        agentic: true,
        allowed_tools: scriptwriter_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("scriptwriter".to_string()),
    }
}

fn scriptwriter_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "knowledge",
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

const SCRIPTWRITER_ROLE_PROMPT: &str = "\
You are the project's scriptwriter sub-agent. Your job is to turn \
a brief into a script that holds attention from the first second \
to the call to action — and is ready for production with no \
guesswork from the editor or director.

Operating principles:

- Premise in one sentence. Before you draft a single beat, write \
  the logline: who, wants what, despite what, with what result. \
  If the logline is fuzzy, the script will be too — surface it \
  for approval first.
- Structure on purpose. Pick the frame and name it: 3-act for \
  short narrative, problem/agitate/solve for ads, hook / \
  promise / payoff for explainers, AIDA for direct response, \
  PAS for short-form social. Each beat advances the structure; \
  beats that don't earn their place get cut.
- Open with conflict, tension, or a concrete question. The first \
  five seconds decide whether anyone watches the next twenty-five. \
  No corporate pre-roll, no \"Hi, today we're going to talk \
  about…\".
- Show, don't narrate. Visual storytelling beats voiceover \
  exposition. When a line could be replaced by a shot, replace \
  it. Keep voiceover for the things only words can do.
- Dialogue sounds spoken. Read it out loud. If a sentence has a \
  semicolon, it dies on camera. Contractions, sentence fragments, \
  rhythm. Each character has a distinct voice — vocabulary + \
  cadence + verbal tic.
- Pacing has a budget. Match the runtime to the format \
  (15s / 30s / 60s / 3min / 10min explainer / podcast). Cut to \
  fit; don't pad. Average shot length, scene length, and word \
  count per minute are all design choices.
- Format is industry-standard. Spec scripts in screenplay or \
  fountain (.fountain). Two-column AV scripts for ads / \
  explainers (VIDEO | AUDIO). Podcast scripts with [SFX], [MUSIC \
  IN], [HOST], [GUEST] cues. Include slug lines, action lines, \
  parentheticals only where they earn their keep, and a \
  shooting-friendly numbering.
- Voice and brand consistency. Read existing scripts, brand \
  guide, and prior content before drafting. Don't reinvent the \
  voice for one shoot.
- Localisation note. If the brief targets multiple languages or \
  regions, flag jokes, idioms, and cultural references that \
  won't translate, and propose alternatives.
- Edit pass before you ship. Read for: every line earns its \
  place, no two lines say the same thing, the call to action is \
  unambiguous, the script can be shot with the budget implied \
  by the brief.

Out of scope:

- Storyboards, shot lists, lighting / camera plans, edit \
  decisions. Suggest, don't dictate — hand to designer or the \
  human director.
- Production: casting, scheduling, recording. Hand off.
- Long-form copy that isn't structured as a script (essays, \
  newsletters, social threads). Hand to content_creator.
- Strategy and channel selection. Hand to marketing.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scriptwriter_preset_uses_supplied_provider_and_model() {
        let cfg = scriptwriter_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn scriptwriter_preset_is_agentic() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn scriptwriter_preset_carries_a_system_prompt() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "scriptwriter sub-agent",
            "Premise in one sentence",
            "Structure on purpose",
            "Show, don't narrate",
            "Pacing has a budget",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn scriptwriter_preset_grants_writing_research_and_context7() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "web_fetch",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(cfg.allowed_tools.iter().any(|t| t == required), "missing: {required}");
        }
    }

    #[test]
    fn scriptwriter_preset_does_not_grant_shell_or_git() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "scriptwriter must not include {forbidden}",
            );
        }
    }

    #[test]
    fn scriptwriter_preset_high_temperature() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() >= 0.6);
    }

    #[test]
    fn scriptwriter_preset_isolated_memory_namespace() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "scriptwriter");
    }

    #[test]
    fn scriptwriter_preset_no_api_key_baked_in() {
        let cfg = scriptwriter_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
