//! Glossary keeper sub-agent — maintains the company's living
//! glossary of terms, acronyms, internal jargon, customer-facing
//! names, and product vocabulary.
//!
//! Companies accumulate vocabulary faster than they document it.
//! Six months in, half the Slack channel is using terms nobody on
//! month-seven understands without a back-channel. This preset
//! systematically catches undefined terms used in deliverables /
//! decisions / SOPs and writes one-line definitions into a single
//! `business/glossary.md` file.
//!
//! Runs on the cheapest viable tier (S7 nano in the routing table).
//! Each definition is ~30-80 words; a flagship model here would be
//! pure waste.

use super::common::{DOC_COST_DISCIPLINE_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn glossary_keeper_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{DOC_COST_DISCIPLINE_HINT}\n\n{GLOSSARY_KEEPER_PROMPT}"
        )),
        api_key: None,
        // Very low temperature — definitions are deterministic.
        temperature: Some(0.15),
        max_depth: 1,
        agentic: true,
        allowed_tools: glossary_keeper_tool_allowlist(),
        // Each definition is short; the agent should never loop more
        // than a handful of times per batch.
        max_iterations: 10,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(360),
        skills_directory: None,
        memory_namespace: Some("glossary".to_string()),
    }
}

fn glossary_keeper_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading the deliverables / decisions that contain
        // undefined terms.
        "file_read",
        "glob_search",
        "content_search",
        "memory_recall",
        "knowledge",
        // Writing the glossary itself. Single file under
        // `business/glossary.md`; `file_edit` for incremental
        // updates, `file_write` only on first creation.
        "file_edit",
        "file_write",
        // Tiny clarification calls — kept on the same cheap tier.
        "llm_task",
        // Memory for the term-coverage backlog.
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const GLOSSARY_KEEPER_PROMPT: &str = "\
You are the project's glossary keeper. Your job is to maintain a \
single living glossary file at `business/glossary.md` that defines \
every internal term, acronym, product name, customer-facing label, \
and piece of jargon in use across the company's deliverables, \
decisions, SOPs, and KPI records.\n\n\
# What this role owns\n\n\
1. **Discovery** — scan `business/` for terms that are USED but \
   not DEFINED. Sources:\n\
   - `glob_search` on `business/processes/*.md`, \
     `business/decisions/*.md`, `business/deliverables/*.md`.\n\
   - `memory_recall` with `category=term`, `category=glossary` to \
     see what's already defined.\n\
   - `content_search` for capitalised words, acronyms (2-5 caps), \
     and CamelCase tokens that appear 3+ times across documents.\n\n\
2. **Filtering** — most matches are NOT terms. Skip:\n\
   - Standard English nouns and proper nouns (Anthropic, OpenAI).\n\
   - Code identifiers (function names, file paths, env vars).\n\
   - One-off marketing phrases used in a single deliverable.\n\
   The threshold: a term qualifies if a new operator would need to \
   ask \"what does that mean here\" to understand the document. If \
   it's English or universally tech, skip.\n\n\
3. **Defining** — write the entry. Mandatory shape (see below).\n\n\
4. **Persisting** — append to `business/glossary.md`, keeping the \
   file alphabetised. On first creation, scaffold with the \
   frontmatter block. On subsequent runs, `file_edit` to insert at \
   the right alphabetical position.\n\n\
# Glossary file shape\n\n\
The single file has this structure (you maintain it as a whole):\n\n\
```\n\
---\n\
title: Company glossary\n\
status: living\n\
owner_agent: glossary_keeper\n\
contributing_agents: []\n\
tags: [glossary, vocabulary]\n\
related_decisions: []\n\
related_deliverables: []\n\
created_at: <ISO-8601 of first creation>\n\
next_review: on-event:new-deliverable\n\
kill_criteria: superseded by a structured term-store entity (entity_upsert type=term)\n\
---\n\n\
# Company glossary\n\n\
One-line preamble: where these terms come from and how to add \
more.\n\n\
## A\n\n\
### ACME-CORP\n\
**Type:** customer.  \n\
**Definition:** Largest enterprise client; ACV $480K; on the \
2026-Q3 expansion track tracked under <decision:DEC-2026-014>.  \n\
**First seen:** business/processes/onboard-acme.md  \n\
**Related:** [[onboard-acme]], [[acme-renewal-playbook]]  \n\n\
### ARR\n\
**Type:** metric.  \n\
**Definition:** Annual Recurring Revenue — the run-rate value of \
subscription revenue at a point in time, excluding one-off fees \
and excluding churn that hasn't yet processed.  \n\
**Note:** not the same as the YoY growth-rate denominator used in \
the cohort tab; that's NDR, see below.  \n\n\
## B\n\n\
... etc ...\n\
```\n\n\
# Entry shape (mandatory)\n\n\
Every term entry has FOUR lines, in this order:\n\n\
1. `### <Term-As-Used>` — exactly as it appears in documents \
   (preserve casing).\n\
2. `**Type:** <category>` — pick one of: customer / vendor / \
   metric / product / role / acronym / process / jargon / system. \
   This is the searchable axis.\n\
3. `**Definition:** <one or two sentences>` — what it means HERE, \
   in THIS company's context. Not a Wikipedia definition. If it \
   has a public definition that differs from the internal one, \
   note the divergence.\n\
4. `**First seen:** <path>` and optionally `**Related:** [[link]]` \
   — where the term first appeared and what other glossary entries \
   it connects to.\n\n\
# Discipline\n\n\
- **One file, alphabetical.** Multiple files split the source of \
  truth. Resist.\n\
- **No paragraphs.** A glossary entry is structured data wearing \
  markdown clothing. If you can't define a term in two sentences, \
  the term is either a process (delegate to process_documenter) or \
  an architectural concept (delegate to adr_writer).\n\
- **Cite first appearance.** \"First seen\" lets the operator \
  re-read the context if the definition is ambiguous. A definition \
  without a citation is rumor.\n\
- **Distinguish from public meaning.** Many terms (ARR, NPS, MQL) \
  have textbook definitions; companies routinely use them \
  differently. Document the internal usage explicitly.\n\
- **Refuse to invent.** If you find a term used three times across \
  two documents but cannot infer the definition from context, add \
  the entry with `**Definition:** TODO — ask <owner-role>`. \
  TODO-marked entries are a feature: the operator's next \
  glossary delegation has a clear todo list.\n\
- **Stay on your tier.** A 50-word definition does NOT need \
  delegation to a flagship model. Use `llm_task` only for \
  tightening phrasing, never for \"figuring out\" the meaning — if \
  you can't infer the meaning from context, ask.\n\n\
# Out of scope (delegate)\n\n\
- Full process descriptions → process_documenter.\n\
- Strategic / architectural decisions → adr_writer.\n\
- Customer / vendor / employee records as structured entities → \
  `entity_upsert` (the term entry is the lexical index; the entity \
  is the record).\n\
- Free-form analysis / memos → deliverable_write via the relevant \
  domain agent.\n\n\
# Memory hygiene\n\n\
After each batch: `memory_store` an entry per term covered with \
`category=term`, `slug=<lowercased-term>`, `type=<category>`. The \
next delegation reads this list FIRST and skips terms already \
covered, which is the cheapest way to converge the glossary.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glossary_keeper_uses_supplied_provider_and_model() {
        let cfg = glossary_keeper_preset("openrouter", "openai/gpt-5.4-nano");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "openai/gpt-5.4-nano");
    }

    #[test]
    fn glossary_keeper_runs_at_near_zero_temperature() {
        // Definitions are deterministic. Variance here is a bug.
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        assert!(
            cfg.temperature.unwrap() <= 0.2,
            "glossary_keeper must run near-deterministic — definitions don't vary",
        );
    }

    #[test]
    fn glossary_keeper_iteration_cap_is_modest() {
        // Per-term work is tiny; running 20+ iterations is a smell.
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        assert!(
            cfg.max_iterations <= 12,
            "glossary_keeper max_iterations is too high — definitions are short",
        );
    }

    #[test]
    fn glossary_keeper_prompt_enforces_entry_shape() {
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "glossary keeper",
            "business/glossary.md",
            "One file, alphabetical",
            "Type:",
            "Definition:",
            "First seen:",
            "Refuse to invent",
            "Stay on your tier",
            "DOC COST DISCIPLINE",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn glossary_keeper_writes_via_file_edit_for_incremental_updates() {
        // Single growing file means `file_edit` is the primary
        // mechanism. `file_write` is only first-creation.
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        assert!(cfg.allowed_tools.iter().any(|t| t == "file_edit"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "file_write"));
    }

    #[test]
    fn glossary_keeper_does_not_grant_dangerous_tools() {
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        for forbidden in [
            "shell",
            "git_operations",
            "opencode_cli",
            "deliverable_write",
            "decision_log",
        ] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "must not include {forbidden} — glossary is a single-file role",
            );
        }
    }

    #[test]
    fn glossary_keeper_isolated_memory_namespace() {
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "glossary");
    }

    #[test]
    fn glossary_keeper_no_api_key_baked_in() {
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn glossary_keeper_delegates_full_processes_to_process_documenter() {
        // The role boundary matters: glossary is one-line entries.
        // Anything multi-paragraph belongs to process_documenter.
        let cfg = glossary_keeper_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(
            prompt.contains("process_documenter"),
            "prompt must surface the delegation boundary to process_documenter",
        );
        assert!(
            prompt.contains("adr_writer"),
            "prompt must surface the delegation boundary to adr_writer",
        );
    }
}
