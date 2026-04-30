//! Knowledge-graph extraction tool.
//!
//! Takes free-form text and asks an LLM to emit a structured set of
//! `(nodes, edges)` JSON suitable for storing in any of the knowledge
//! graph backends (`KnowledgeGraph` SQLite, `PgKnowledgeGraph`,
//! `FalkorDbKnowledgeGraph`).
//!
//! Why not just use `llm_task`?
//!   - The schema for `(nodes, edges)` is cumbersome to repeat at every
//!     call site, and the system prompt has to be very specific about
//!     extraction discipline (don't invent entities, only relate things
//!     present in the text). Encapsulating that in a dedicated tool
//!     gives the agent a one-shot, reliable path to build a graph from
//!     a transcript chunk, a doc snippet, or a pasted email.
//!
//! Output shape — pinned for downstream wiring:
//!
//! ```json
//! {
//!   "nodes": [
//!     {
//!       "node_type": "pattern" | "decision" | "lesson" | "expert" | "technology",
//!       "title":     "short headline",
//!       "content":   "1–3 sentences",
//!       "tags":      ["tag1","tag2"],
//!       "source_project": "optional"
//!     }
//!   ],
//!   "edges": [
//!     {
//!       "from_title": "matches one of nodes[].title",
//!       "to_title":   "matches one of nodes[].title",
//!       "relation":   "uses" | "replaces" | "extends" | "authored_by" | "applies_to"
//!     }
//!   ]
//! }
//! ```
//!
//! Edges reference nodes by `title` (not id) because IDs are assigned
//! by the storage backend after the tool returns. The integrator
//! resolves `title -> id` when persisting.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use zeroclaw_api::provider::Provider;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};

const KG_EXTRACTION_SYSTEM_PROMPT: &str = "\
You are a knowledge-graph extractor. You receive free-form text and \
emit a single JSON object with the following shape and nothing else:\n\
\n\
{\n\
  \"nodes\": [\n\
    {\n\
      \"node_type\": \"pattern\" | \"decision\" | \"lesson\" | \"expert\" | \"technology\",\n\
      \"title\":     \"short headline (≤80 chars)\",\n\
      \"content\":   \"1–3 sentences explaining the node\",\n\
      \"tags\":      [\"tag1\",\"tag2\"],\n\
      \"source_project\": \"optional, omit when unknown\"\n\
    }\n\
  ],\n\
  \"edges\": [\n\
    {\n\
      \"from_title\": \"must match one of nodes[].title\",\n\
      \"to_title\":   \"must match one of nodes[].title\",\n\
      \"relation\":   \"uses\" | \"replaces\" | \"extends\" | \"authored_by\" | \"applies_to\"\n\
    }\n\
  ]\n\
}\n\
\n\
Discipline:\n\
- Only emit entities that are explicitly present in the text. Do NOT \
invent connections or entities that the text does not state.\n\
- Prefer fewer high-quality nodes over many low-quality ones. Cap at \
10 nodes and 15 edges per call.\n\
- `node_type` must be exactly one of the five enum values listed.\n\
- `relation` must be exactly one of the five enum values listed.\n\
- Tags should be lowercase, kebab-case, no spaces.\n\
- Output ONLY the JSON object — no prose, no markdown, no code fence.\n\
- If the text contains nothing extractable, emit `{\"nodes\":[],\"edges\":[]}`.";

/// `kg_extract` tool — see module docs for the wire format.
pub struct KgExtractTool {
    security: Arc<SecurityPolicy>,
    default_provider: String,
    default_model: String,
    default_temperature: f64,
    api_key: Option<String>,
    provider_runtime_options: zeroclaw_providers::ProviderRuntimeOptions,
}

impl KgExtractTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        default_provider: String,
        default_model: String,
        default_temperature: f64,
        api_key: Option<String>,
        provider_runtime_options: zeroclaw_providers::ProviderRuntimeOptions,
    ) -> Self {
        Self {
            security,
            default_provider,
            default_model,
            default_temperature,
            api_key,
            provider_runtime_options,
        }
    }
}

#[async_trait]
impl Tool for KgExtractTool {
    fn name(&self) -> &str {
        "kg_extract"
    }

    fn description(&self) -> &str {
        "Extract a knowledge graph (nodes + edges) from free-form text \
         using the configured LLM. Returns JSON ready to feed into the \
         knowledge tool or a graph backend. Useful for distilling \
         lessons learned from a transcript, decisions from a meeting \
         note, or relationships from a docs snippet."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Source text to extract from. Should fit comfortably in the model's context window — tools/llm_task is the better choice for huge documents."
                },
                "source_project": {
                    "type": "string",
                    "description": "Optional project identifier to stamp on every node."
                },
                "tags_hint": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags to bias the extractor toward (e.g. [\"security\",\"gateway\"])."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override. Defaults to the runtime's default model."
                },
                "temperature": {
                    "type": "number",
                    "description": "Optional sampling temperature. Defaults to 0.2 — extraction is a deterministic task."
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(deny(
                "Rate limit exceeded: too many actions in the last hour",
            ));
        }
        if let Err(reason) = self
            .security
            .enforce_tool_operation(ToolOperation::Read, "kg_extract")
        {
            return Ok(deny(&reason));
        }

        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s,
            _ => return Ok(deny("Missing or empty 'text' parameter")),
        };

        let source_project = args
            .get("source_project")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let tags_hint: Vec<&str> = args
            .get("tags_hint")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let model = args
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_model);
        // Extraction is deliberately low-temperature — we want consistent
        // structured output, not creative interpretations.
        let temperature = args
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.2_f64.min(self.default_temperature));

        // Build the user prompt. The system prompt is concatenated by
        // simple_chat-callers internally, but to stay portable we feed
        // everything through the user message: many providers' simple
        // chat path does not let us pass a separate system role.
        let mut user_prompt = String::new();
        user_prompt.push_str(KG_EXTRACTION_SYSTEM_PROMPT);
        user_prompt.push_str("\n\n--- BEGIN INPUT ---\n");
        user_prompt.push_str(text);
        user_prompt.push_str("\n--- END INPUT ---\n");
        if let Some(p) = source_project {
            user_prompt.push_str(&format!("\nUse source_project=\"{p}\" on every node.\n"));
        }
        if !tags_hint.is_empty() {
            user_prompt.push_str(&format!(
                "\nFavour these tags when relevant (do NOT force them): {}\n",
                tags_hint.join(", ")
            ));
        }

        let provider: Box<dyn Provider> = match zeroclaw_providers::create_provider_with_options(
            &self.default_provider,
            self.api_key.as_deref(),
            &self.provider_runtime_options,
        ) {
            Ok(p) => p,
            Err(e) => return Ok(deny(&format!("Failed to create provider: {e}"))),
        };

        let raw = match provider
            .simple_chat(&user_prompt, model, Some(temperature))
            .await
        {
            Ok(t) => t,
            Err(e) => return Ok(deny(&format!("LLM call failed: {e}"))),
        };

        // Strip optional code fences the LLM may have added even after
        // being told not to.
        let cleaned = strip_code_fences(raw.trim());

        // Parse and validate. We're lenient: if the LLM produced extra
        // fields we drop them; if it omitted optional fields we fill
        // defaults. The hard requirements are `nodes` and `edges` as
        // arrays of objects with at minimum the title/relation fields.
        let parsed: Value = match serde_json::from_str(cleaned) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: raw,
                    error: Some(format!(
                        "LLM did not return valid JSON ({e}). Raw response is in `output` for inspection."
                    )),
                });
            }
        };

        let normalised = normalise_extraction(&parsed, source_project)
            .map_err(|e| anyhow::anyhow!("Validation failed: {e}"))?;

        Ok(ToolResult {
            success: true,
            output: normalised.to_string(),
            error: None,
        })
    }
}

fn deny(reason: &str) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(reason.to_string()),
    }
}

fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.trim_start().trim_end_matches("```").trim();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.trim_start().trim_end_matches("```").trim();
    }
    s
}

fn normalise_extraction(v: &Value, source_project: Option<&str>) -> Result<Value, String> {
    let nodes_in = v
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let edges_in = v
        .get("edges")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    let allowed_node_types = ["pattern", "decision", "lesson", "expert", "technology"];
    let allowed_relations = ["uses", "replaces", "extends", "authored_by", "applies_to"];

    let mut nodes_out: Vec<Value> = Vec::new();
    for n in &nodes_in {
        let Some(obj) = n.as_object() else { continue };
        let node_type = obj
            .get("node_type")
            .and_then(|v| v.as_str())
            .map(str::to_ascii_lowercase)
            .filter(|s| allowed_node_types.contains(&s.as_str()));
        let title = obj
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let content = obj
            .get("content")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .unwrap_or("");
        let tags: Vec<String> = obj
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_ascii_lowercase()))
                    .collect()
            })
            .unwrap_or_default();
        let project = obj
            .get("source_project")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| source_project.map(str::to_string));

        let (Some(node_type), Some(title)) = (node_type, title) else {
            continue;
        };
        nodes_out.push(json!({
            "node_type": node_type,
            "title": title,
            "content": content,
            "tags": tags,
            "source_project": project,
        }));
        if nodes_out.len() >= 10 {
            break;
        }
    }

    // Build a set of node titles to validate edge endpoints against.
    let titles: std::collections::HashSet<String> = nodes_out
        .iter()
        .filter_map(|n| n.get("title").and_then(|t| t.as_str()).map(str::to_string))
        .collect();

    let mut edges_out: Vec<Value> = Vec::new();
    for e in &edges_in {
        let Some(obj) = e.as_object() else { continue };
        let from = obj
            .get("from_title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let to = obj
            .get("to_title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let rel = obj
            .get("relation")
            .and_then(|v| v.as_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if from.is_empty()
            || to.is_empty()
            || !titles.contains(from)
            || !titles.contains(to)
            || !allowed_relations.contains(&rel.as_str())
        {
            // Silently drop malformed edges — better to lose one edge
            // than to corrupt the downstream graph.
            continue;
        }
        edges_out.push(json!({
            "from_title": from,
            "to_title": to,
            "relation": rel,
        }));
        if edges_out.len() >= 15 {
            break;
        }
    }

    Ok(json!({
        "nodes": nodes_out,
        "edges": edges_out,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::policy::AutonomyLevel;

    fn security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        })
    }

    fn make_tool() -> KgExtractTool {
        KgExtractTool::new(
            security(),
            "openrouter".into(),
            "openai/gpt-4o-mini".into(),
            0.2,
            None,
            zeroclaw_providers::ProviderRuntimeOptions::default(),
        )
    }

    #[test]
    fn name_and_description_present() {
        let t = make_tool();
        assert_eq!(t.name(), "kg_extract");
        assert!(!t.description().is_empty());
    }

    #[test]
    fn schema_requires_text() {
        let t = make_tool();
        let schema = t.parameters_schema();
        let req = schema["required"].as_array().expect("required is array");
        assert!(req.iter().any(|v| v == "text"));
    }

    #[test]
    fn strip_fences_handles_json_fence() {
        let s = "```json\n{\"a\":1}\n```";
        assert_eq!(strip_code_fences(s), "{\"a\":1}");
    }

    #[test]
    fn strip_fences_handles_plain_fence() {
        let s = "```\n{\"a\":1}\n```";
        assert_eq!(strip_code_fences(s), "{\"a\":1}");
    }

    #[test]
    fn strip_fences_passthrough_when_unfenced() {
        assert_eq!(strip_code_fences("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn normalise_drops_invalid_node_types() {
        let v = json!({
            "nodes": [
                { "node_type": "pattern", "title": "Good", "content": "ok", "tags": [] },
                { "node_type": "ghost",   "title": "Bad",  "content": "no", "tags": [] }
            ],
            "edges": []
        });
        let out = normalise_extraction(&v, None).unwrap();
        assert_eq!(out["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(out["nodes"][0]["title"], "Good");
    }

    #[test]
    fn normalise_drops_edges_with_unknown_endpoints() {
        let v = json!({
            "nodes": [
                { "node_type": "pattern", "title": "A", "content": "", "tags": [] }
            ],
            "edges": [
                { "from_title": "A", "to_title": "B", "relation": "uses" },
                { "from_title": "A", "to_title": "A", "relation": "uses" }
            ]
        });
        let out = normalise_extraction(&v, None).unwrap();
        let edges = out["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["from_title"], "A");
        assert_eq!(edges[0]["to_title"], "A");
    }

    #[test]
    fn normalise_caps_node_and_edge_counts() {
        let mut nodes = vec![];
        let mut edges = vec![];
        for i in 0..30 {
            let title = format!("N{i}");
            nodes.push(json!({
                "node_type": "pattern",
                "title": title,
                "content": "",
                "tags": []
            }));
            if i < 25 {
                edges.push(json!({
                    "from_title": format!("N{i}"),
                    "to_title": format!("N{i}"),
                    "relation": "uses"
                }));
            }
        }
        let out = normalise_extraction(&json!({ "nodes": nodes, "edges": edges }), None).unwrap();
        assert_eq!(out["nodes"].as_array().unwrap().len(), 10);
        // Edges referencing dropped nodes are filtered first; remaining
        // valid ones are then capped at 15.
        assert!(out["edges"].as_array().unwrap().len() <= 15);
    }

    #[test]
    fn normalise_stamps_source_project_default() {
        let v = json!({
            "nodes": [
                { "node_type": "pattern", "title": "X", "content": "", "tags": [] }
            ],
            "edges": []
        });
        let out = normalise_extraction(&v, Some("zeroclaw")).unwrap();
        assert_eq!(out["nodes"][0]["source_project"], "zeroclaw");
    }

    #[tokio::test]
    async fn execute_blocks_when_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            max_actions_per_hour: 0,
            ..SecurityPolicy::default()
        });
        let tool = KgExtractTool::new(
            security,
            "openrouter".into(),
            "openai/gpt-4o-mini".into(),
            0.2,
            None,
            zeroclaw_providers::ProviderRuntimeOptions::default(),
        );
        let res = tool.execute(json!({ "text": "anything" })).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap_or_default().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_rejects_empty_text() {
        let tool = make_tool();
        let res = tool.execute(json!({ "text": "" })).await.unwrap();
        assert!(!res.success);
    }
}
