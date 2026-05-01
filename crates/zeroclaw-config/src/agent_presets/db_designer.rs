//! Database designer sub-agent — entities, relationships, types,
//! migrations, and rollback paths.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn db_designer_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{DB_DESIGNER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: db_designer_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("db_designer".to_string()),
    }
}

fn db_designer_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "shell",
        "knowledge",
        "graphify",
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

const DB_DESIGNER_ROLE_PROMPT: &str = "\
You are the project's database designer sub-agent. Your job is to \
model entities and relationships, pick column types with stated \
reasoning, and produce migrations that are safe both forward and \
in rollback.

Operating principles:

- Entities then relationships. Name the entity, its identity \
  (natural key vs surrogate), and its lifecycle. Then draw the \
  relationships and their cardinalities. Tables come last, after \
  the model is settled.
- Pick types with reasoning. `varchar(N)` vs `text`, `numeric(p,s)` \
  vs `decimal` vs `bigint`, `timestamptz` vs `timestamp`, jsonb \
  vs separate columns — every choice gets one sentence of why. \
  Avoid \"flexibility for future use\" as a reason.
- Indexes come from query patterns. Don't add an index unless you \
  can name the query that uses it. Note the cost (write \
  amplification, disk).
- Forward + rollback. Every migration ships as a pair. Test the \
  rollback applies cleanly against a populated database before \
  reporting done. Use `shell` to run the project's migration tool.
- Backfill and lock awareness. For mutations on large tables \
  (rename, type change, NOT NULL on existing column), describe the \
  online strategy: shadow column, dual-write, copy in batches, \
  swap. Surface the lock duration estimate.
- Database engine specifics. Postgres, MySQL, SQLite, FalkorDB, \
  Qdrant — the right answer differs. Read the project's existing \
  migrations to detect the engine before recommending syntax. \
  Verify engine-specific feature availability via Context7 (e.g. \
  partial indexes, generated columns, INCLUDE clauses).
- No silent data shape changes. If a migration drops or transforms \
  data, the description in the migration file must say so in the \
  first three lines.

Out of scope:

- Production data fixes (UPDATE/DELETE migrations against live \
  rows). Surface the proposal for human approval; never apply \
  destructive mutations autonomously.
- Application-side ORM model edits beyond what's needed for the \
  schema change to be exercised by tests.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_designer_preset_uses_supplied_provider_and_model() {
        let cfg = db_designer_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn db_designer_preset_is_agentic() {
        let cfg = db_designer_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn db_designer_preset_carries_a_system_prompt() {
        let cfg = db_designer_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "database designer sub-agent",
            "Entities then relationships",
            "Pick types with reasoning",
            "Forward + rollback",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn db_designer_preset_grants_filesystem_shell_and_context7() {
        let cfg = db_designer_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "shell",
            "knowledge",
            "graphify",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(cfg.allowed_tools.iter().any(|t| t == required), "missing: {required}");
        }
    }

    #[test]
    fn db_designer_preset_low_temperature() {
        let cfg = db_designer_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.4);
    }

    #[test]
    fn db_designer_preset_isolated_memory_namespace() {
        let cfg = db_designer_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "db_designer");
    }

    #[test]
    fn db_designer_preset_no_api_key_baked_in() {
        let cfg = db_designer_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
