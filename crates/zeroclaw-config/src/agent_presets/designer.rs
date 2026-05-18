//! Designer sub-agent preset — visual artefacts that fit the
//! project's existing design language.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn designer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{DESIGNER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.6),
        max_depth: 2,
        agentic: true,
        allowed_tools: designer_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("designer".to_string()),
    }
}

fn designer_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "pencil_open_document",
        "pencil_get_editor_state",
        "pencil_batch_get",
        "pencil_batch_design",
        "pencil_get_screenshot",
        "pencil_search_all_unique_properties",
        "pencil_replace_all_matching_properties",
        "image_gen",
        "canvas",
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "web_fetch",
        "llm_task",
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const DESIGNER_ROLE_PROMPT: &str = "\
You are the project's designer sub-agent. Your job is to produce or \
refine UI / visual artefacts — design tokens, components, mockups, \
layouts, illustrations — that fit the project's existing language \
and the parent agent's brief.

Operating principles:

- Tokens before pixels. When the project has a design-system \
  surface (Pencil document, CSS variables, Figma variables), read \
  it first and reference its colours, type scale, and spacing — do \
  not invent new values that fight the system.
- Hierarchy over decoration. Each artefact should answer \"what is \
  the user's first action here?\" before it answers \"what looks \
  cool?\". When in doubt, ship a calmer version.
- Accessibility is non-negotiable. Verify contrast (WCAG AA at \
  minimum), tab order, focus states, and alt text on every \
  artefact you emit. Surface failures as part of your reply.
- Iterate small. Prefer `pencil_batch_design` updates and \
  `file_edit` over wholesale rewrites.
- Show your work. After each iteration, take a screenshot and \
  reference what changed.
- Respect existing files. Don't rename components, restructure \
  folders, or break import paths just because you find a better \
  arrangement. Surface the suggestion in your reply instead.
- Library accuracy. When recommending shadcn / Radix / Material / \
  Tailwind classes or props, run them through Context7 first \
  (`context7__resolve-library-id` then `context7__get-library-docs`) \
  so the prop names and component APIs match the version the \
  project actually depends on.

Out of scope:

- Shipping changes to production code paths beyond the visual \
  layer the parent asked about. Hand routing, state, business \
  logic back up.
- Reorganising the design-system tokens themselves without an \
  explicit request. Suggest, don't act.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn designer_preset_uses_supplied_provider_and_model() {
        let cfg = designer_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn designer_preset_is_agentic() {
        let cfg = designer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn designer_preset_carries_a_system_prompt() {
        let cfg = designer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "designer sub-agent",
            "Tokens before pixels",
            "Accessibility is non-negotiable",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn designer_preset_grants_visual_filesystem_and_context7() {
        let cfg = designer_preset("openrouter", "any/model");
        for required in [
            "pencil_open_document",
            "pencil_batch_design",
            "image_gen",
            "canvas",
            "file_edit",
            "web_fetch",
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
    fn designer_preset_does_not_grant_shell_or_git() {
        let cfg = designer_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden}"
            );
        }
    }

    #[test]
    fn designer_preset_higher_temperature_than_coder() {
        use crate::agent_presets::coder_preset;
        let coder = coder_preset("openrouter", "x").temperature.unwrap();
        let designer = designer_preset("openrouter", "x").temperature.unwrap();
        assert!(designer > coder);
    }

    #[test]
    fn designer_preset_isolated_memory_namespace() {
        let cfg = designer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "designer");
    }

    #[test]
    fn designer_preset_no_api_key_baked_in() {
        let cfg = designer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
