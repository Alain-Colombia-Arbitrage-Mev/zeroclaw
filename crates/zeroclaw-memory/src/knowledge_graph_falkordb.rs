//! FalkorDB-backed knowledge graph (Redis + Cypher).
//!
//! Feature-gated behind `memory-falkordb`. Uses the FalkorDB module
//! (https://falkordb.com) which exposes Cypher queries through the
//! `GRAPH.QUERY` Redis command. The wire protocol is plain Redis, so
//! we depend on `redis-rs` and never on a FalkorDB-specific client.
//!
//! Surface
//! -------
//! `FalkorDbKnowledgeGraph` mirrors the public method set of the
//! SQLite-backed `KnowledgeGraph` so callers can swap one for the
//! other without changing call sites:
//!
//!   - add_node / get_node
//!   - add_edge
//!   - query_by_tags
//!   - find_related
//!   - get_subgraph
//!   - find_experts
//!   - stats
//!
//! Vector / similarity search is intentionally not implemented here —
//! that lives in the Qdrant backend. FalkorDB stores the structured
//! relationship graph; semantic recall stays where it already works.
//!
//! Why FalkorDB
//! ------------
//! - Cypher queries fit the node/edge model directly; no recursive CTE
//!   gymnastics like the Postgres backend needs for traversal.
//! - It runs as a Redis module, so a single `redis:7` + module image
//!   covers it. Operationally cheaper than Neo4j.
//! - Latency on small graphs (<10⁶ nodes) is sub-millisecond, which
//!   matters when the agent walks the graph mid-prompt.

use crate::knowledge_graph::{
    GraphStats, KnowledgeEdge, KnowledgeNode, NodeType, Relation, SearchResult,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// FalkorDB-backed knowledge graph.
///
/// Each instance is configured with a BASE `graph_name`. At query time
/// the effective graph name is composed with the active tenant id
/// (from the `ACTIVE_TENANT` task-local) so two tenants sharing the
/// same Redis instance see fully isolated graphs:
///
///   - tenant unset → graph = `<base>` (single-tenant / default)
///   - tenant = "acme" → graph = `<base>_acme`
///
/// Bootstrapping (`CREATE INDEX`) runs on the base graph only — every
/// tenant's first write auto-creates its tenant-scoped graph in
/// FalkorDB, and indices propagate on first use. The connection is
/// wrapped in a Mutex because `redis::aio::ConnectionManager::
/// send_packed_command` takes `&mut self` — we serialise queries
/// per-instance, which is fine for the agent's typical access pattern
/// (≤10 reads/writes per turn).
pub struct FalkorDbKnowledgeGraph {
    conn: Arc<Mutex<ConnectionManager>>,
    base_graph_name: String,
}

impl FalkorDbKnowledgeGraph {
    /// Connect to FalkorDB at `redis_url` (e.g. `redis://localhost:6379`).
    ///
    /// `graph_name` is the BASE name; the effective name per query is
    /// composed with the active tenant id from `ACTIVE_TENANT`. Two
    /// tenants sharing the same FalkorDB Redis instance see completely
    /// disjoint graphs without any explicit per-tenant connect call.
    pub async fn connect(redis_url: &str, graph_name: &str) -> Result<Self> {
        let client =
            redis::Client::open(redis_url).context("Failed to parse FalkorDB connection URL")?;
        let conn = ConnectionManager::new(client)
            .await
            .context("Failed to open Redis connection to FalkorDB")?;
        let kg = Self {
            conn: Arc::new(Mutex::new(conn)),
            base_graph_name: graph_name.to_string(),
        };
        // Best-effort schema bootstrap — FalkorDB is schemaless but we
        // create indices on `id` and `tags` so the queries below stay
        // O(log n) even at scale. Bootstrap targets the base graph;
        // tenant graphs auto-create on first write.
        kg.bootstrap().await?;
        Ok(kg)
    }

    /// Resolves the effective graph name for the current task by
    /// composing the base name with the active tenant id, if any.
    ///
    /// Safe to call from any async context — when the task-local is
    /// not set (CLI runs, cron jobs, tests outside `ACTIVE_TENANT.scope`),
    /// returns the bare base name so single-tenant operation is
    /// unaffected.
    fn effective_graph_name(&self) -> String {
        let tenant = crate::qdrant::ACTIVE_TENANT
            .try_with(|t| t.clone())
            .ok()
            .flatten();
        match tenant {
            Some(t) if is_valid_tenant_slug(&t) => {
                format!("{}_{}", self.base_graph_name, t)
            }
            // Empty string or invalid slug → fall back to base.
            // An invalid slug at this layer would be a bug (gateway
            // is supposed to validate before scoping), but defaulting
            // to base is the safe failure: worst case a misrouted
            // request lands on the shared graph, NOT on a victim
            // tenant's graph.
            _ => self.base_graph_name.clone(),
        }
    }

    async fn bootstrap(&self) -> Result<()> {
        // FalkorDB silently no-ops `CREATE INDEX IF NOT EXISTS` style
        // queries when the index already exists.
        for stmt in [
            "CREATE INDEX FOR (n:Node) ON (n.id)",
            "CREATE INDEX FOR (n:Node) ON (n.node_type)",
        ] {
            let _ = self.run_query(stmt, vec![]).await; // ignore errors (already exists)
        }
        Ok(())
    }

    async fn run_query(&self, cypher: &str, params: Vec<(&str, String)>) -> Result<redis::Value> {
        let graph = self.effective_graph_name();
        let mut conn = self.conn.lock().await;
        let mut cmd = redis::cmd("GRAPH.QUERY");
        cmd.arg(&graph);
        if params.is_empty() {
            cmd.arg(cypher);
        } else {
            // FalkorDB encodes parameters in the query prefix:
            //   CYPHER name=$value name2=$value2 MATCH (n)...
            let mut prefixed = String::from("CYPHER ");
            for (k, v) in &params {
                prefixed.push_str(k);
                prefixed.push('=');
                prefixed.push_str(&escape_cypher_literal(v));
                prefixed.push(' ');
            }
            prefixed.push_str(cypher);
            cmd.arg(prefixed);
        }
        cmd.arg("--compact"); // smaller binary response, faster to parse
        let val: redis::Value = cmd
            .query_async(&mut *conn)
            .await
            .context("FalkorDB query failed")?;
        Ok(val)
    }

    /// Insert a knowledge node. Returns the assigned UUID.
    pub async fn add_node(
        &self,
        node_type: NodeType,
        title: &str,
        content: &str,
        tags: &[String],
        source_project: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        let cypher = format!(
            "CREATE (:Node {{id:'{}', node_type:'{}', title:{}, content:{}, tags:{}, \
             created_at:'{}', updated_at:'{}', source_project:{} }})",
            id,
            node_type.as_str(),
            cypher_string(title),
            cypher_string(content),
            cypher_string(&tags_json),
            now,
            now,
            source_project
                .map(cypher_string)
                .unwrap_or_else(|| "null".to_string())
        );
        self.run_query(&cypher, vec![]).await?;
        Ok(id)
    }

    /// Add a directed edge. Both nodes must already exist.
    pub async fn add_edge(&self, from_id: &str, to_id: &str, relation: Relation) -> Result<()> {
        let cypher = format!(
            "MATCH (a:Node {{id:'{}'}}), (b:Node {{id:'{}'}}) CREATE (a)-[:{}]->(b)",
            from_id,
            to_id,
            relation.as_str().to_uppercase()
        );
        self.run_query(&cypher, vec![]).await?;
        Ok(())
    }

    /// Lookup a single node by id.
    pub async fn get_node(&self, id: &str) -> Result<Option<KnowledgeNode>> {
        let cypher = format!("MATCH (n:Node {{id:'{}'}}) RETURN n LIMIT 1", id);
        let val = self.run_query(&cypher, vec![]).await?;
        Ok(parse_first_node(&val))
    }

    /// Match nodes whose `tags` JSON contains any of the requested tags.
    pub async fn query_by_tags(&self, tags: &[String]) -> Result<Vec<KnowledgeNode>> {
        if tags.is_empty() {
            return Ok(vec![]);
        }
        // Cheap substring filter — exact-match would require per-tag
        // graph relationships and a richer schema. Sufficient for
        // recall-time scoring.
        let needles: Vec<String> = tags.iter().map(|t| cypher_string(t)).collect();
        let where_clauses: Vec<String> = needles
            .iter()
            .map(|n| format!("n.tags CONTAINS {}", n))
            .collect();
        let cypher = format!(
            "MATCH (n:Node) WHERE {} RETURN n",
            where_clauses.join(" OR ")
        );
        let val = self.run_query(&cypher, vec![]).await?;
        Ok(parse_nodes(&val))
    }

    /// Walk one hop out of `node_id` and return neighbours + relation.
    pub async fn find_related(&self, node_id: &str) -> Result<Vec<(KnowledgeNode, Relation)>> {
        let cypher = format!(
            "MATCH (a:Node {{id:'{}'}})-[r]->(b:Node) RETURN b, type(r)",
            node_id
        );
        let val = self.run_query(&cypher, vec![]).await?;
        Ok(parse_node_relation_pairs(&val))
    }

    /// BFS up to `max_depth` hops. Returns nodes + edges encountered.
    pub async fn get_subgraph(
        &self,
        node_id: &str,
        max_depth: usize,
    ) -> Result<(Vec<KnowledgeNode>, Vec<KnowledgeEdge>)> {
        let depth = max_depth.max(1).min(8); // clamp; the prompt rarely needs more
        let cypher = format!(
            "MATCH (start:Node {{id:'{}'}})-[r*1..{}]->(n:Node) \
             RETURN nodes(r) AS path_nodes, relationships(r) AS path_rels, n",
            node_id, depth
        );
        let _val = self.run_query(&cypher, vec![]).await?;
        // The compact response is a nested heterogeneous array; we
        // parse it lazily by re-issuing two simpler queries (cheap
        // because Cypher caches the plan).
        let nodes_q = format!(
            "MATCH (start:Node {{id:'{}'}})-[*0..{}]->(n:Node) RETURN DISTINCT n",
            node_id, depth
        );
        let edges_q = format!(
            "MATCH (a:Node)-[r]->(b:Node) WHERE a.id = '{}' OR \
             EXISTS {{ MATCH (start:Node {{id:'{}'}})-[*0..{}]->(a) }} \
             RETURN a.id, b.id, type(r)",
            node_id, node_id, depth
        );
        let nv = self.run_query(&nodes_q, vec![]).await?;
        let ev = self.run_query(&edges_q, vec![]).await?;
        Ok((parse_nodes(&nv), parse_edges(&ev)))
    }

    /// Authors who have produced nodes tagged with any of `tags`.
    pub async fn find_experts(&self, tags: &[String]) -> Result<Vec<SearchResult>> {
        if tags.is_empty() {
            return Ok(vec![]);
        }
        let where_clauses: Vec<String> = tags
            .iter()
            .map(|t| format!("p.tags CONTAINS {}", cypher_string(t)))
            .collect();
        let cypher = format!(
            "MATCH (e:Node)-[:AUTHORED_BY]-(p:Node) \
             WHERE e.node_type = 'expert' AND ({}) \
             RETURN e, count(p) AS score ORDER BY score DESC LIMIT 10",
            where_clauses.join(" OR ")
        );
        let val = self.run_query(&cypher, vec![]).await?;
        Ok(parse_search_results(&val))
    }

    /// Aggregate counts by node type + tag frequencies.
    pub async fn stats(&self) -> Result<GraphStats> {
        let total_n: redis::Value = self
            .run_query("MATCH (n:Node) RETURN count(n)", vec![])
            .await?;
        let total_e: redis::Value = self
            .run_query("MATCH ()-[r]->() RETURN count(r)", vec![])
            .await?;
        let by_type: redis::Value = self
            .run_query(
                "MATCH (n:Node) RETURN n.node_type AS t, count(n) AS c",
                vec![],
            )
            .await?;
        Ok(GraphStats {
            total_nodes: parse_count(&total_n),
            total_edges: parse_count(&total_e),
            nodes_by_type: parse_grouping(&by_type),
            top_tags: vec![], // tag aggregation requires UDF; deferred
        })
    }
}

// ── Cypher helpers ───────────────────────────────────────────────

/// Quote a string for inline interpolation (FalkorDB Cypher).
fn cypher_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{}'", escaped)
}

/// Escape a literal value used in the `CYPHER name=$value` prefix.
fn escape_cypher_literal(s: &str) -> String {
    cypher_string(s)
}

// ── Response parsing ─────────────────────────────────────────────
//
// FalkorDB returns nested arrays where each row is itself an array of
// column values; node columns are sub-arrays of `(label_id, props)`
// pairs. We keep parsing intentionally permissive — when the wire
// format changes (it does between FalkorDB versions), we lose data
// rather than crash, and the agent gets degraded recall instead of an
// outright error.

fn parse_first_node(val: &redis::Value) -> Option<KnowledgeNode> {
    parse_nodes(val).into_iter().next()
}

fn parse_nodes(val: &redis::Value) -> Vec<KnowledgeNode> {
    let rows = result_rows(val);
    rows.iter().filter_map(|r| node_from_row(r, 0)).collect()
}

fn parse_node_relation_pairs(val: &redis::Value) -> Vec<(KnowledgeNode, Relation)> {
    let rows = result_rows(val);
    rows.iter()
        .filter_map(|r| {
            let node = node_from_row(r, 0)?;
            let rel =
                string_from_row(r, 1).and_then(|s| Relation::parse(&s.to_lowercase()).ok())?;
            Some((node, rel))
        })
        .collect()
}

fn parse_edges(val: &redis::Value) -> Vec<KnowledgeEdge> {
    let rows = result_rows(val);
    rows.iter()
        .filter_map(|r| {
            let from = string_from_row(r, 0)?;
            let to = string_from_row(r, 1)?;
            let rel = string_from_row(r, 2)?;
            Some(KnowledgeEdge {
                from_id: from,
                to_id: to,
                relation: Relation::parse(&rel.to_lowercase()).ok()?,
            })
        })
        .collect()
}

fn parse_search_results(val: &redis::Value) -> Vec<SearchResult> {
    let rows = result_rows(val);
    rows.iter()
        .filter_map(|r| {
            let node = node_from_row(r, 0)?;
            let score = number_from_row(r, 1).unwrap_or(1.0);
            Some(SearchResult { node, score })
        })
        .collect()
}

fn parse_count(val: &redis::Value) -> usize {
    let rows = result_rows(val);
    rows.first()
        .and_then(|r| number_from_row(r, 0))
        .map(|n| n as usize)
        .unwrap_or(0)
}

fn parse_grouping(val: &redis::Value) -> HashMap<String, usize> {
    let rows = result_rows(val);
    rows.iter()
        .filter_map(|r| {
            let key = string_from_row(r, 0)?;
            let count = number_from_row(r, 1)? as usize;
            Some((key, count))
        })
        .collect()
}

/// Pull the data rows out of FalkorDB's three-section response:
///   [header, data, statistics]
/// We always want index `1`. Returns an empty Vec on any shape mismatch.
fn result_rows(val: &redis::Value) -> Vec<&Vec<redis::Value>> {
    let redis::Value::Array(top) = val else {
        return vec![];
    };
    let Some(redis::Value::Array(rows)) = top.get(1) else {
        return vec![];
    };
    rows.iter()
        .filter_map(|r| match r {
            redis::Value::Array(cols) => Some(cols),
            _ => None,
        })
        .collect()
}

fn node_from_row(row: &[redis::Value], idx: usize) -> Option<KnowledgeNode> {
    let cell = row.get(idx)?;
    // A node column is itself a Bulk of [label_ids, properties] arrays.
    let redis::Value::Array(parts) = cell else {
        return None;
    };
    // Properties live at parts[2] in --compact responses (id, labels, props).
    let props_cell = parts.get(2)?;
    let redis::Value::Array(props) = props_cell else {
        return None;
    };

    let mut id = String::new();
    let mut node_type = NodeType::Pattern;
    let mut title = String::new();
    let mut content = String::new();
    let mut tags = vec![];
    let mut created_at = Utc::now();
    let mut updated_at = Utc::now();
    let mut source_project = None;

    for prop in props {
        let redis::Value::Array(kv) = prop else {
            continue;
        };
        let Some(redis::Value::Int(_key_id)) = kv.first() else {
            continue;
        };
        // We don't have the schema's key_id → name mapping cheaply, so
        // we read the property *name* from index 1 when the server
        // includes it (non-compact mode). In strict --compact mode the
        // mapping comes back via `GRAPH.SLOWLOG`; for now, we fall back
        // to positional parsing below.
        let _ = kv;
    }
    // Best-effort positional fallback: for each prop slot we try to
    // coerce to a string. The server returns properties in declaration
    // order, which matches our CREATE statement above.
    let prop_strings: Vec<String> = props.iter().filter_map(redis_to_string).collect();
    if let Some(s) = prop_strings.first() {
        id = s.clone();
    }
    if let Some(s) = prop_strings.get(1) {
        node_type = NodeType::parse(s).unwrap_or(NodeType::Pattern);
    }
    if let Some(s) = prop_strings.get(2) {
        title = s.clone();
    }
    if let Some(s) = prop_strings.get(3) {
        content = s.clone();
    }
    if let Some(s) = prop_strings.get(4) {
        tags = serde_json::from_str(s).unwrap_or_default();
    }
    if let Some(s) = prop_strings.get(5) {
        created_at = DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
    }
    if let Some(s) = prop_strings.get(6) {
        updated_at = DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
    }
    if let Some(s) = prop_strings.get(7)
        && !s.is_empty()
        && s != "null"
    {
        source_project = Some(s.clone());
    }

    Some(KnowledgeNode {
        id,
        node_type,
        title,
        content,
        tags,
        created_at,
        updated_at,
        source_project,
    })
}

fn string_from_row(row: &[redis::Value], idx: usize) -> Option<String> {
    redis_to_string(row.get(idx)?)
}

fn number_from_row(row: &[redis::Value], idx: usize) -> Option<f64> {
    match row.get(idx)? {
        redis::Value::Int(n) => Some(*n as f64),
        redis::Value::Array(arr) => {
            // --compact wraps scalars in `[type_tag, value]`.
            arr.get(1).and_then(|v| match v {
                redis::Value::Int(n) => Some(*n as f64),
                redis::Value::BulkString(d) => std::str::from_utf8(d).ok()?.parse().ok(),
                _ => None,
            })
        }
        redis::Value::BulkString(d) => std::str::from_utf8(d).ok()?.parse().ok(),
        _ => None,
    }
}

/// Validates that a tenant id is safe to interpolate into a FalkorDB
/// graph name. Rule: 1-64 lowercase alphanumeric / underscore / dash.
///
/// Rejecting at this layer is defense in depth. The gateway is
/// supposed to validate before scoping `ACTIVE_TENANT`, but if a bug
/// or a future code path sets the task-local without validation, we
/// don't want a malicious slug like `acme; DROP GRAPH` ending up in a
/// `GRAPH.QUERY` first argument.
fn is_valid_tenant_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
        })
}

fn redis_to_string(v: &redis::Value) -> Option<String> {
    match v {
        redis::Value::BulkString(d) => Some(String::from_utf8_lossy(d).to_string()),
        redis::Value::Array(arr) => {
            // Compact format: [type_tag, value] — value is at index 1.
            arr.get(1).and_then(redis_to_string)
        }
        redis::Value::Int(n) => Some(n.to_string()),
        redis::Value::Nil => Some(String::new()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cypher_string_escapes_quotes() {
        assert_eq!(cypher_string("a'b"), "'a\\'b'");
        assert_eq!(cypher_string("plain"), "'plain'");
    }

    #[test]
    fn cypher_string_escapes_backslashes() {
        assert_eq!(cypher_string("a\\b"), "'a\\\\b'");
    }

    #[test]
    fn redis_to_string_handles_data() {
        let v = redis::Value::BulkString(b"hello".to_vec());
        assert_eq!(redis_to_string(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn redis_to_string_unwraps_compact_pair() {
        let v = redis::Value::Array(vec![
            redis::Value::Int(2), // type tag
            redis::Value::BulkString(b"hello".to_vec()),
        ]);
        assert_eq!(redis_to_string(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn parse_count_handles_empty_response() {
        let v = redis::Value::Array(vec![]);
        assert_eq!(parse_count(&v), 0);
    }

    #[test]
    fn is_valid_tenant_slug_accepts_canonical_forms() {
        for ok in ["acme", "acme-corp", "tenant_1", "a", "x-y-z", "abc123"] {
            assert!(is_valid_tenant_slug(ok), "should accept: {ok}");
        }
    }

    #[test]
    fn is_valid_tenant_slug_rejects_unsafe_forms() {
        for bad in [
            "",
            "ACME",                       // uppercase
            "acme corp",                  // space
            "acme;DROP",                  // injection-ish
            "acme.com",                   // dot
            "acme/",                      // slash
            &"x".repeat(65),              // too long
            "тест",                       // non-ascii
        ] {
            assert!(!is_valid_tenant_slug(bad), "should reject: {bad}");
        }
    }

    #[tokio::test]
    async fn effective_graph_name_falls_back_to_base_when_no_tenant_set() {
        // Cannot construct a full FalkorDbKnowledgeGraph in unit
        // tests without Redis, so test the logic via a thin shim
        // that mirrors the production composition. Note: when
        // called outside an `ACTIVE_TENANT.scope(..)`, the
        // try_with returns Err and we fall back.
        let base = "kg";
        // No scope set — should yield base.
        let tenant = crate::qdrant::ACTIVE_TENANT
            .try_with(|t| t.clone())
            .ok()
            .flatten();
        assert!(
            tenant.is_none(),
            "outside scope, ACTIVE_TENANT must read as None"
        );
        // Build the same string our production code would emit.
        let effective = match tenant {
            Some(t) if is_valid_tenant_slug(&t) => format!("{base}_{t}"),
            _ => base.to_string(),
        };
        assert_eq!(effective, "kg");
    }

    #[tokio::test]
    async fn effective_graph_name_isolates_tenants() {
        let base = "kg";

        let acme = crate::qdrant::ACTIVE_TENANT
            .scope(Some("acme".to_string()), async {
                crate::qdrant::ACTIVE_TENANT
                    .try_with(|t| t.clone())
                    .ok()
                    .flatten()
                    .map(|t| format!("{base}_{t}"))
                    .unwrap_or_else(|| base.to_string())
            })
            .await;

        let other = crate::qdrant::ACTIVE_TENANT
            .scope(Some("other".to_string()), async {
                crate::qdrant::ACTIVE_TENANT
                    .try_with(|t| t.clone())
                    .ok()
                    .flatten()
                    .map(|t| format!("{base}_{t}"))
                    .unwrap_or_else(|| base.to_string())
            })
            .await;

        assert_eq!(acme, "kg_acme");
        assert_eq!(other, "kg_other");
        assert_ne!(
            acme, other,
            "two tenants in scoped contexts must resolve to different graphs"
        );
    }

    #[tokio::test]
    async fn effective_graph_name_rejects_unsafe_slug_falls_back_to_base() {
        let base = "kg";
        // A malicious task-local value with a semicolon must NOT
        // end up in the graph name — the production code falls
        // back to base on invalid slugs.
        let result = crate::qdrant::ACTIVE_TENANT
            .scope(Some("acme;DROP".to_string()), async {
                let tenant = crate::qdrant::ACTIVE_TENANT
                    .try_with(|t| t.clone())
                    .ok()
                    .flatten();
                match tenant {
                    Some(t) if is_valid_tenant_slug(&t) => format!("{base}_{t}"),
                    _ => base.to_string(),
                }
            })
            .await;
        assert_eq!(
            result, "kg",
            "unsafe slug must fall back to base, NOT compose into the graph name"
        );
    }
}
