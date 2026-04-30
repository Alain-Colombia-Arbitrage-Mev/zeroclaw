//! FalkorDB-backed `knowledge` tool variant.
//!
//! Mirrors a subset of `KnowledgeTool` (capture / search / relate /
//! graph_stats) but talks to a `FalkorDbKnowledgeGraph` instead of the
//! SQLite-backed `KnowledgeGraph`. The runtime selects between the two
//! at startup based on `[knowledge] backend = "sqlite" | "falkordb"`.
//!
//! Why a parallel struct rather than a refactor of `KnowledgeTool`?
//!   - Surface-stable. The SQLite tool is hot-tested production code;
//!     replumbing it to a trait would require touching every action
//!     and its tests.
//!   - The two backends differ in sync vs. async semantics. SQLite is
//!     blocking (rusqlite); FalkorDB is async (redis-rs). A single
//!     enum dispatch would have to choose one shape and adapt the
//!     other — extra ceremony for no behavioural gain.
//!
//! Coverage today: capture (add_node), search (query_by_tags),
//! relate (add_edge), graph_stats. Suggest / expert_find /
//! lessons_extract are deferred — the operator can fall back to the
//! SQLite backend if those are needed before this gap is closed.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_memory::knowledge_graph::{NodeType, Relation};
use zeroclaw_memory::knowledge_graph_falkordb::FalkorDbKnowledgeGraph;

/// FalkorDB-backed `knowledge` tool. Drop-in replacement for
/// `KnowledgeTool` in the runtime tool registry when
/// `[knowledge] backend = "falkordb"`.
pub struct KnowledgeToolFalkor {
    graph: Arc<FalkorDbKnowledgeGraph>,
}

impl KnowledgeToolFalkor {
    pub fn new(graph: Arc<FalkorDbKnowledgeGraph>) -> Self {
        Self { graph }
    }
}

#[async_trait]
impl Tool for KnowledgeToolFalkor {
    fn name(&self) -> &str {
        "knowledge"
    }

    fn description(&self) -> &str {
        "Manage a FalkorDB knowledge graph of architecture decisions, solution patterns, \
         lessons learned, and experts. Actions: capture, search, relate, graph_stats."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["capture", "search", "relate", "graph_stats"]
                },
                "node_type": {
                    "type": "string",
                    "enum": ["pattern", "decision", "lesson", "expert", "technology"]
                },
                "title":          { "type": "string" },
                "content":        { "type": "string" },
                "tags":           { "type": "array", "items": { "type": "string" } },
                "source_project": { "type": "string" },
                "from_id":        { "type": "string" },
                "to_id":          { "type": "string" },
                "relation": {
                    "type": "string",
                    "enum": ["uses", "replaces", "extends", "authored_by", "applies_to"]
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let Some(action) = args.get("action").and_then(|v| v.as_str()) else {
            return Ok(deny("Missing 'action' parameter"));
        };

        match action {
            "capture" => self.capture(&args).await,
            "search" => self.search(&args).await,
            "relate" => self.relate(&args).await,
            "graph_stats" => self.stats().await,
            other => Ok(deny(&format!(
                "Unsupported action '{other}'. The FalkorDB backend currently \
                 implements: capture, search, relate, graph_stats. Switch to \
                 backend = \"sqlite\" for suggest / expert_find / lessons_extract."
            ))),
        }
    }
}

impl KnowledgeToolFalkor {
    async fn capture(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let node_type = parse_node_type(args)?;
        let title = required_str(args, "title")?;
        let content = required_str(args, "content")?;
        let tags = parse_tags(args);
        let source_project = args
            .get("source_project")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        match self
            .graph
            .add_node(node_type, title, content, &tags, source_project)
            .await
        {
            Ok(id) => Ok(ToolResult {
                success: true,
                output: json!({ "id": id, "title": title }).to_string(),
                error: None,
            }),
            Err(e) => Ok(deny(&format!("FalkorDB capture failed: {e}"))),
        }
    }

    async fn search(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let tags = parse_tags(args);
        if tags.is_empty() {
            return Ok(deny(
                "FalkorDB search requires at least one tag in the `tags` array",
            ));
        }
        match self.graph.query_by_tags(&tags).await {
            Ok(nodes) => {
                let payload = json!({
                    "count": nodes.len(),
                    "nodes": nodes.iter().map(|n| json!({
                        "id": n.id,
                        "node_type": n.node_type.as_str(),
                        "title": n.title,
                        "tags": n.tags,
                    })).collect::<Vec<_>>(),
                });
                Ok(ToolResult {
                    success: true,
                    output: payload.to_string(),
                    error: None,
                })
            }
            Err(e) => Ok(deny(&format!("FalkorDB search failed: {e}"))),
        }
    }

    async fn relate(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let from_id = required_str(args, "from_id")?;
        let to_id = required_str(args, "to_id")?;
        let relation_str = required_str(args, "relation")?;
        let relation = Relation::parse(relation_str)
            .map_err(|e| anyhow::anyhow!("Invalid relation '{relation_str}': {e}"))?;
        match self.graph.add_edge(from_id, to_id, relation).await {
            Ok(()) => Ok(ToolResult {
                success: true,
                output: json!({ "from_id": from_id, "to_id": to_id, "relation": relation_str })
                    .to_string(),
                error: None,
            }),
            Err(e) => Ok(deny(&format!("FalkorDB relate failed: {e}"))),
        }
    }

    async fn stats(&self) -> anyhow::Result<ToolResult> {
        match self.graph.stats().await {
            Ok(stats) => Ok(ToolResult {
                success: true,
                output: json!({
                    "total_nodes": stats.total_nodes,
                    "total_edges": stats.total_edges,
                    "nodes_by_type": stats.nodes_by_type,
                })
                .to_string(),
                error: None,
            }),
            Err(e) => Ok(deny(&format!("FalkorDB stats failed: {e}"))),
        }
    }
}

fn deny(reason: &str) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(reason.to_string()),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing or non-string '{key}' parameter"))
}

fn parse_tags(args: &Value) -> Vec<String> {
    args.get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_node_type(args: &Value) -> anyhow::Result<NodeType> {
    let s = required_str(args, "node_type")?;
    NodeType::parse(s).map_err(|e| anyhow::anyhow!("Invalid node_type '{s}': {e}"))
}
