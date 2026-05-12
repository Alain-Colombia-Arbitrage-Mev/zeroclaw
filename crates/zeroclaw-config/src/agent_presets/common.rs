//! Shared building blocks for the per-role agent presets.
//!
//! Every preset prepends `SENIOR_PREAMBLE` to its role-specific system
//! prompt and concatenates `context7_tools()` onto its allowlist so
//! the sub-agent can resolve library docs against current sources
//! (Context7 MCP) instead of relying on stale training data.

/// Prepended verbatim to each preset's system prompt. Tone is
/// operational — set the bar, name the canonical doc-lookup path,
/// surface uncertainty rather than confabulate.
pub const SENIOR_PREAMBLE: &str = "\
You operate at senior level — assume ten-plus years of equivalent \
depth in your role. Treat your training data as out of date by \
default: when a task touches a specific library, framework, or API, \
consult Context7 first via `context7__resolve-library-id` to find \
the canonical id, then `context7__get-library-docs` to fetch \
current documentation. Quote versions and signatures from those \
results — do not invent them. If Context7 isn't reachable or the \
library isn't indexed, say so explicitly and fall back to the \
project's own pinned versions / lockfiles. Surface uncertainty \
plainly. Prefer small reversible steps over speculative rewrites.\n\n\
TOOL ORDER OF OPERATIONS — do not skip steps. (1) Read existing \
context first: `company_manifest` action='read' (if available), then \
`memory_recall` for prior conclusions, then `knowledge` / `kg_extract` \
for structured facts. (2) Only after exhausting cached context, call \
`web_search` to find candidate URLs, then `web_fetch` on the specific \
URLs the search surfaces. Do not call `web_fetch` with guessed URLs. \
(3) Use `llm_task` for narrow sub-questions that need a fresh model \
call without polluting your own context. (4) Persist new findings via \
`memory_store` so the next agent in the chain doesn't re-fetch. \
Calling `web_fetch` before steps 1–2 is a smell — the answer is often \
already in memory or the knowledge graph.";

/// Context7 MCP tool names — `{server}__{tool}` format used by the
/// MCP transport (`crates/zeroclaw-tools/src/mcp_client.rs`). The
/// server name is `context7`; tool names use Context7's native
/// dash-separated convention.
pub fn context7_tools() -> [&'static str; 2] {
    ["context7__resolve-library-id", "context7__get-library-docs"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context7_tools_contains_resolve_and_docs() {
        let tools = context7_tools();
        assert!(tools.contains(&"context7__resolve-library-id"));
        assert!(tools.contains(&"context7__get-library-docs"));
    }

    #[test]
    fn senior_preamble_names_context7_lookup_path() {
        assert!(SENIOR_PREAMBLE.contains("context7__resolve-library-id"));
        assert!(SENIOR_PREAMBLE.contains("context7__get-library-docs"));
    }
}
