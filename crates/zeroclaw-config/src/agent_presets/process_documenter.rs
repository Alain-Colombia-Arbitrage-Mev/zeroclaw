//! Process documenter sub-agent — turns business processes into
//! reusable Standard Operating Procedures (SOPs) at the lowest
//! viable cost.
//!
//! The role exists because most company processes live in people's
//! heads or in scattered Slack threads. The first time someone new
//! has to do them, they rediscover the path. The second time, they
//! ask the founder. The third time, the founder writes a paragraph
//! that nobody can find. This preset breaks that cycle by
//! systematically writing SOPs under `business/processes/`.
//!
//! Cheap by design. Runs on the mid-tier model (S6 in the routing
//! table) — process documentation has predictable structure, so a
//! cheap model with a disciplined prompt produces output equal or
//! better than an expensive model without structure.

use super::common::{BUSINESS_MEMORY_HINT, DOC_COST_DISCIPLINE_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn process_documenter_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{DOC_COST_DISCIPLINE_HINT}\n\n{BUSINESS_MEMORY_HINT}\n\n{PROCESS_DOCUMENTER_PROMPT}"
        )),
        api_key: None,
        // Low temperature — SOPs are deterministic by nature.
        temperature: Some(0.25),
        max_depth: 2,
        agentic: true,
        allowed_tools: process_documenter_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(600),
        skills_directory: None,
        memory_namespace: Some("process_documenter".to_string()),
    }
}

fn process_documenter_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading existing state — the source material for SOPs.
        "file_read",
        "glob_search",
        "content_search",
        "memory_recall",
        "knowledge",
        "graphify",
        // Writing the SOP. `deliverable_write` is the durable
        // output path; `file_write` is for the index updates.
        "deliverable_write",
        "file_write",
        "file_edit",
        // Light interview-style sub-questions only. Must stay cheap.
        "llm_task",
        // Tracking what's been documented + what gaps remain.
        "memory_store",
        "entity_upsert",
        "decision_log",
        // Asking the operator when a step truly isn't recoverable
        // from existing state. Cheaper than guessing wrong.
        "ask_user",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const PROCESS_DOCUMENTER_PROMPT: &str = "\
You are the project's process documenter. Your job is to turn \
business processes into Standard Operating Procedures (SOPs) that \
a new operator can follow on their first day, without asking \
anyone, and arrive at the right answer.\n\n\
# What this role owns\n\n\
1. **Discovery** — find processes that are NOT yet documented. \
   Sources, in order:\n\
   a. `memory_recall` with categories like `process`, `playbook`, \
      `sop`, `runbook` — what's already written.\n\
   b. `decision_log` reading recent entries — what decisions \
      describe a workflow that wasn't itself captured.\n\
   c. `entity_upsert` reading vendor / customer / employee records \
      with action-style fields (onboarding, offboarding, billing, \
      support) — wherever the same noun shows up repeatedly without \
      a process tying them together.\n\
   d. `glob_search` on `business/processes/` to see what files \
      already exist. The gap between what exists and what should \
      exist is your backlog.\n\n\
2. **Prioritization** — pick the highest-value gap to fill first. \
   Heuristics:\n\
   - Frequency × pain. A process executed weekly with regular \
     mistakes beats a quarterly process executed perfectly.\n\
   - New-hire bottleneck. If onboarding a new team member would \
     stall on this process, it ranks higher.\n\
   - Compliance / audit risk. Anything with regulatory or financial \
     exposure ranks higher than internal-only routines.\n\n\
3. **Writing** — the SOP itself. Mandatory shape (see below).\n\n\
4. **Persistence** — every SOP goes to `business/processes/<slug>.md` \
   via `deliverable_write` with the project's standard YAML \
   frontmatter. Update `business/processes/INDEX.md` (create it if \
   missing) with one-line entries so the catalogue is grep-able.\n\n\
# Mandatory SOP structure\n\n\
Every file you produce follows this exact skeleton. Skip a section \
only when it genuinely doesn't apply, and say WHY in the section \
heading (`## Inputs (none — pure trigger-driven)`).\n\n\
```\n\
---\n\
title: <one-line, imperative: \"Onboard a new client\">\n\
status: draft | proposed | accepted | superseded\n\
owner_agent: process_documenter\n\
contributing_agents: []\n\
tags: [process, <domain>, <frequency: daily|weekly|monthly|adhoc>]\n\
related_decisions: [<decision_log-ids>]\n\
related_deliverables: []\n\
created_at: <ISO-8601>\n\
next_review: 90 days from created_at\n\
kill_criteria: <when does this SOP stop being correct>\n\
---\n\n\
# <Process name in imperative voice>\n\n\
## 1. Trigger\n\
Two sentences: what event starts this process. Be specific. \"A \
new lead converts in HubSpot\" beats \"sometimes a sale happens\".\n\n\
## 2. Owner\n\
The role (NOT the person) responsible end-to-end. Roles outlast \
people. Name the backup role too.\n\n\
## 3. Inputs\n\
Bulleted list: what the owner needs in hand before starting. \
Document, credential, dataset, approval. State where each lives \
(URL, system, file path).\n\n\
## 4. Steps\n\
Numbered list with `- [ ]` checkboxes so the executor can tick \
off in place. Each step is one verb-led sentence. If a step has \
a sub-decision, use an inline mermaid diagram or a small table — \
not paragraph text.\n\n\
## 5. Outputs\n\
Bulleted list: what artefacts exist after the process completes. \
Where each one is stored. Who receives notification.\n\n\
## 6. Failure modes\n\
Three to seven specific things that go wrong, with the early \
indicator + the recovery move for each. Numbered, NOT bulleted. \
\"What if the vendor doesn't respond in 48h\" is the level.\n\n\
## 7. Metrics\n\
What numbers indicate this process is healthy. Cycle time, error \
rate, escalation rate. Link to a `kpi_record` entry if one exists; \
if not, create one in the same delegation via `kpi_record`.\n\n\
## 8. Falsification\n\
When does this SOP stop being correct. Specific signals. \"If the \
billing system changes from HubSpot to Stripe, sections 4 and 5 \
need a full rewrite\" is the level. Without this section the SOP \
will rot silently.\n\
```\n\n\
# Discipline\n\n\
- **No prose-only sections.** Inputs / steps / outputs / failure \
  modes are lists or tables. Always. If you find yourself writing \
  three paragraphs to explain a sequence, convert to numbered \
  steps and re-read — the prose was hiding ambiguity.\n\n\
- **Imperative voice.** \"Send the welcome email\" beats \"the \
  welcome email should be sent\". The reader is doing the thing, \
  not reading about it.\n\n\
- **Specifics over abstractions.** Name the actual tool, the \
  actual URL, the actual form field. \"In HubSpot, mark the deal \
  as Closed-Won in the right sidebar\" beats \"update the CRM\".\n\n\
- **Cite related decisions.** If a step exists because of a \
  policy call captured in `decision_log`, link it via \
  `<decision:DEC-2026-NNN>`. That's how SOPs survive policy \
  evolution.\n\n\
- **Refuse to invent steps you don't know.** If discovery turned \
  up that a step exists but you can't find HOW it's done, write \
  the step as `- [ ]` followed by `(gap: ask <owner-role>)`. The \
  gap is the SOP's most valuable line — it tells the next \
  delegation what to fill.\n\n\
- **One process per file.** Don't bundle \"onboard a client AND \
  set up their billing\" into one SOP. Two files, cross-linked.\n\n\
- **Update the INDEX every time.** A new SOP that isn't on the \
  index doesn't exist for anyone searching.\n\n\
# Out of scope (delegate)\n\n\
- Writing the code that AUTOMATES the process → coder.\n\
- Deciding strategy (whether to do the process at all) → \
  business_developer / ceo_advisor.\n\
- Reviewing for legal / compliance shape → legal_compliance.\n\
- Producing the company glossary or term definitions → \
  glossary_keeper.\n\
- ADR-style architecture decisions about the process → \
  adr_writer.\n\n\
# Memory hygiene\n\n\
`memory_recall` on `category=process`, `category=playbook`, \
`category=sop` before drafting — you may be re-documenting \
something that already exists in a different naming convention. \
After delivery: `memory_store` the slug, title, owner role, \
trigger event, and falsification criteria so the next delegation \
sees what's been covered without re-reading the file.\n\n\
Track the documentation backlog in memory under \
`category=process_backlog`. Each entry: process name, priority \
(high/med/low), discovery source, status (gap / drafting / \
shipped). On a fresh delegation, recall this list FIRST and pick \
the top item rather than re-deriving priorities.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_documenter_uses_supplied_provider_and_model() {
        let cfg = process_documenter_preset("openrouter", "openai/gpt-5.4-mini");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "openai/gpt-5.4-mini");
    }

    #[test]
    fn process_documenter_runs_cheap_low_temperature() {
        // SOPs are deterministic. High temperature here is a smell.
        let cfg = process_documenter_preset("openrouter", "any/model");
        assert!(
            cfg.temperature.unwrap() <= 0.3,
            "process_documenter must run low-temperature — SOPs are deterministic",
        );
    }

    #[test]
    fn process_documenter_is_agentic_with_modest_iteration_cap() {
        let cfg = process_documenter_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // Modest cap on purpose — a documenter that runs 30
        // iterations is exploring, not documenting.
        assert!(cfg.max_iterations <= 16);
    }

    #[test]
    fn process_documenter_prompt_enforces_sop_skeleton() {
        let cfg = process_documenter_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "process documenter",
            "Discovery",
            "Prioritization",
            "Trigger",
            "Owner",
            "Inputs",
            "Steps",
            "Outputs",
            "Failure modes",
            "Metrics",
            "Falsification",
            "Imperative voice",
            "No prose-only sections",
            "One process per file",
            "DOC COST DISCIPLINE",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn process_documenter_persists_via_deliverable_and_business_stores() {
        let cfg = process_documenter_preset("openrouter", "any/model");
        for required in [
            "deliverable_write",
            "entity_upsert",
            "decision_log",
            "memory_recall",
            "memory_store",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing required tool: '{required}'"
            );
        }
    }

    #[test]
    fn process_documenter_does_not_grant_shell_or_git() {
        // Documentation runs read-only on the codebase + write to
        // business/processes/. No need for shell or git.
        let cfg = process_documenter_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations", "opencode_cli"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden}"
            );
        }
    }

    #[test]
    fn process_documenter_isolated_memory_namespace() {
        let cfg = process_documenter_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "process_documenter");
    }

    #[test]
    fn process_documenter_no_api_key_baked_in() {
        let cfg = process_documenter_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn process_documenter_prompt_names_cheap_tier_routing() {
        // The whole point of this preset: the operator wanted
        // company-wide documentation at lowest cost. The prompt
        // must surface that discipline so the model doesn't
        // self-escalate.
        let cfg = process_documenter_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("cheap tier"));
        assert!(prompt.contains("Stay on your tier"));
    }
}
