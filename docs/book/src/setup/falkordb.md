# FalkorDB knowledge-graph backend

ZeroClaw can store its **structured knowledge graph** — patterns,
decisions, lessons, experts, and the relationships between them — in
[FalkorDB](https://falkordb.com), a graph database that runs as a
Redis module. The FalkorDB backend coexists with the default SQLite
graph and the optional Postgres one; pick the backend that matches
your scale.

| Backend | Best for | Trade-off |
|---|---|---|
| SQLite (default) | Single-user / single-host | Zero ops, but recursive traversal is hand-written CTEs |
| Postgres | Shared team graph | Needs a server; Cypher emulated via SQL |
| **FalkorDB** | Cypher-native, low-latency, larger graphs | Requires running a Redis container |

The FalkorDB backend is **feature-gated** behind the
`memory-falkordb` Cargo feature so the default `cargo build` doesn't
pull in `redis-rs` or any networking infra you don't need.

## 1. Start FalkorDB

```bash
docker compose -f docker-compose.falkordb.yml up -d
```

That brings up a FalkorDB instance on the standard Redis port and
the FalkorDB Browser web UI:

| | |
|---|---|
| Redis (Cypher over `GRAPH.QUERY`) | `localhost:6379` |
| FalkorDB Browser | <http://localhost:3000> |

Health check:

```bash
docker exec zeroclaw-falkordb redis-cli ping        # → PONG
```

To stop and wipe the persistent volume:

```bash
docker compose -f docker-compose.falkordb.yml down -v
```

## 2. Build the daemon with the feature on

```bash
cargo run --bin zeroclaw --features memory-falkordb -- daemon
```

If the feature flag is missing, the daemon compiles fine but cannot
connect to FalkorDB at runtime — the module just isn't linked in.

## 3. Use the backend from your integration

The schema-level dispatch under `[memory.knowledge_graph]` lands in a
follow-up PR. For now, instantiate the backend directly from your
integration code:

```rust
use zeroclaw_memory::knowledge_graph_falkordb::FalkorDbKnowledgeGraph;
use zeroclaw_memory::knowledge_graph::{NodeType, Relation};

let kg = FalkorDbKnowledgeGraph::connect(
    "redis://localhost:6379",
    "zeroclaw_kg",
)
.await?;

let pattern_id = kg
    .add_node(
        NodeType::Pattern,
        "Sliding-window rate limiter",
        "Per-IP sliding-window with lockout, used in gateway auth.",
        &["security".into(), "gateway".into()],
        Some("zeroclaw"),
    )
    .await?;

let lesson_id = kg
    .add_node(
        NodeType::Lesson,
        "Defaults too restrictive for paired CLI tests",
        "MAX_ATTEMPTS=10 broke smoke tests; raised to 60.",
        &["security".into(), "rate-limit".into()],
        Some("zeroclaw"),
    )
    .await?;

kg.add_edge(&pattern_id, &lesson_id, Relation::AppliesTo).await?;
```

The graph then supports:

| Method | Cypher equivalent |
|---|---|
| `add_node` | `CREATE (n:Node {...})` |
| `add_edge` | `MATCH (a),(b) CREATE (a)-[:REL]->(b)` |
| `get_node` | `MATCH (n {id:'…'}) RETURN n LIMIT 1` |
| `query_by_tags` | `MATCH (n) WHERE n.tags CONTAINS '…' RETURN n` |
| `find_related` | `MATCH (a {id:'…'})-[r]->(b) RETURN b, type(r)` |
| `get_subgraph` | `MATCH (start {id:'…'})-[*1..k]->(n) RETURN ...` |
| `find_experts` | `MATCH (e:Node)-[:AUTHORED_BY]-(p) WHERE e.node_type='expert' AND ... ORDER BY count(p)` |
| `stats` | aggregate node/edge counts |

## 4. What stays where

| Substrate | Backend |
|---|---|
| Vector / semantic recall | Qdrant (see [Qdrant memory backend](./qdrant.md)) |
| Structured graph | FalkorDB (this page) — or SQLite / Postgres |
| Conversation history | unaffected — still SQLite session store |
| Daily memory files | unaffected — still `~/.zeroclaw/workspace/memory/*.md` |

You can run **all three** at once: Qdrant for "what did the user
say?", FalkorDB for "how is concept X connected to concept Y?", and
the SQLite session store for the verbatim chat log.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Failed to open Redis connection` | FalkorDB not running | `docker compose -f docker-compose.falkordb.yml ps` |
| `unknown command 'GRAPH.QUERY'` | Connected to a vanilla Redis instead of FalkorDB | Make sure you used the `falkordb/falkordb` image, not `redis` |
| Symbol `FalkorDbKnowledgeGraph` not found at compile time | Built without the feature flag | Rebuild with `--features memory-falkordb` |
| Slow `find_related` on large graphs | Missing index on `id` | Connect with `redis-cli` and run `GRAPH.QUERY zeroclaw_kg "CREATE INDEX FOR (n:Node) ON (n.id)"` |
