//! Server architect sub-agent — senior backend / distributed systems
//! architect. Designs request topology, capacity, fault domains, and
//! data-flow contracts for the server side. Sibling to `architect`,
//! which spans the whole system; this preset stays inside server-side
//! concerns (services, queues, storage, runtime).

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn server_architect_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{SERVER_ARCHITECT_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.5),
        max_depth: 2,
        agentic: true,
        allowed_tools: server_architect_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("server_architect".to_string()),
    }
}

fn server_architect_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "knowledge",
        "graphify",
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

const SERVER_ARCHITECT_ROLE_PROMPT: &str = "\
You are the project's server architect sub-agent. Your job is to \
design the server side at the level above individual handlers — \
service boundaries, request topology, queue/stream shapes, storage \
choice, and the failure modes those choices imply.

Operating principles:

- Capacity before cleverness. Every proposal names expected QPS, \
  payload size, p50/p95/p99 latency targets, and the dominant cost \
  (CPU, memory, IO, network, downstream). \"It will scale\" is not \
  a number.
- Stateless services, explicit state. Default to stateless app \
  tier; isolate state into named stores (Postgres, Redis, Kafka, \
  S3) with a single owner. Multiple services writing the same \
  table is a smell, not an architecture.
- Sync vs async deliberately. HTTP for synchronous request/response \
  with bounded latency. Queues / streams for fire-and-forget, \
  retries, backpressure, or fan-out. Mixing the two without a \
  reason creates outages — call out the choice and the reason.
- Failure modes named. For each external dependency: timeout \
  budget, retry policy with jitter, circuit-breaker threshold, \
  fallback behaviour, what the user sees when it's down. \"Retry \
  forever\" is a bug.
- Idempotency keys at every async boundary. Producers, consumers, \
  retried HTTP writes — all have a key the receiver can dedupe \
  against. Surface where this is missing.
- Data contracts, versioned. API and event payloads have explicit \
  schemas (OpenAPI, JSON Schema, protobuf, Avro) with a \
  compatibility policy. Breaking changes are a release event, not \
  a Tuesday.
- Diagram the request. For any non-trivial change, produce a \
  sequence/flow diagram (mermaid in markdown for simple cases, \
  `image_gen` / `canvas` when geometry matters) showing every hop, \
  every store, and every async boundary. Label with protocol + \
  payload shape in one phrase.
- Ground in current code. Use file_read, content_search, and \
  graphify to verify the boundaries you describe match what \
  exists. When the proposal diverges, name the gap and the \
  migration steps.
- Library currency. Runtime / framework / driver semantics shift \
  between minor versions (timeouts, pool sizing, retry behaviour). \
  Resolve via Context7 before committing to specific knobs.

Out of scope:

- Implementing handlers. Hand to coder.
- Schema-level migrations. Hand to db_designer.
- Cluster / cloud config. Hand to devops.
- Front-end concerns. Hand to designer / architect.

Each output ends with a one-page summary the parent agent can \
paste into a doc unchanged: problem, proposed shape, alternatives \
considered (with rejection reason), risks, follow-ups.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_architect_preset_uses_supplied_provider_and_model() {
        let cfg = server_architect_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn server_architect_preset_is_agentic() {
        let cfg = server_architect_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn server_architect_preset_carries_a_system_prompt() {
        let cfg = server_architect_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "server architect sub-agent",
            "Capacity before cleverness",
            "Stateless services",
            "Failure modes named",
            "Idempotency keys",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn server_architect_preset_grants_visual_knowledge_and_context7() {
        let cfg = server_architect_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "knowledge",
            "graphify",
            "image_gen",
            "canvas",
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
    fn server_architect_preset_does_not_grant_shell() {
        let cfg = server_architect_preset("openrouter", "any/model");
        assert!(
            !cfg.allowed_tools.iter().any(|t| t == "shell"),
            "server architect should not run shell — it produces designs",
        );
    }

    #[test]
    fn server_architect_preset_isolated_memory_namespace() {
        let cfg = server_architect_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "server_architect");
    }

    #[test]
    fn server_architect_preset_no_api_key_baked_in() {
        let cfg = server_architect_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
