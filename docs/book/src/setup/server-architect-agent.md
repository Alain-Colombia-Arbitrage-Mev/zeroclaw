# Server architect sub-agent

The `server_architect` preset is a senior backend / distributed
systems architect delegate. It designs request topology, capacity,
fault domains, and data-flow contracts for the server side. It is
narrower than the [`architect` preset](./architect-agent.md), which
spans the whole system; this one stays inside server concerns
(services, queues, storage, runtime).

The preset is exposed in code at
`zeroclaw_config::agent_presets::server_architect_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 16 iterations per delegation.
- Uses a **moderate temperature** (0.5) — design needs some
  divergent thinking, but stays bounded by named NFRs.
- Has a **read + write + diagram** surface (`file_read`/`_write`/
  `_edit`, `glob_search`, `content_search`, `knowledge`,
  `graphify`, `image_gen`, `canvas`) so it can ship the proposal
  as markdown with mermaid + generated diagrams.
- **Does NOT have `shell` or `git_operations`** — produces designs
  and one-page summaries; implementation goes to coder, schema
  changes go to db_designer, infra goes to devops.
- Uses an **isolated memory namespace** (`server_architect`) so
  prior topology decisions and capacity baselines persist.

## Drop-in TOML

```toml
[agents.server_architect]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.5
agentic              = true
max_depth            = 2
max_iterations       = 16
timeout_secs         = 180
agentic_timeout_secs = 900
memory_namespace     = "server_architect"

allowed_tools = [
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
  "context7__resolve-library-id",
  "context7__get-library-docs",
]

system_prompt = """
You are the project's server architect sub-agent. Your job is to
design the server side at the level above individual handlers —
service boundaries, request topology, queue/stream shapes, storage
choice, and the failure modes those choices imply.

Operating principles:

- Capacity before cleverness. Every proposal names expected QPS,
  payload size, p50/p95/p99 latency targets, and the dominant cost.
- Stateless services, explicit state. Default to stateless app
  tier; isolate state into named stores with a single owner.
- Sync vs async deliberately. HTTP for synchronous request/response
  with bounded latency; queues / streams for fire-and-forget,
  retries, backpressure, fan-out.
- Failure modes named. For each external dependency: timeout
  budget, retry policy with jitter, circuit-breaker threshold,
  fallback, what the user sees when it's down.
- Idempotency keys at every async boundary.
- Data contracts versioned (OpenAPI / JSON Schema / protobuf /
  Avro) with an explicit compatibility policy.
- Diagram the request — sequence/flow with every hop, every store,
  every async boundary; label edges with protocol + payload shape.
- Ground in current code via file_read, content_search, graphify;
  call out the gap when proposal diverges.
- Resolve framework / driver semantics via Context7 before
  committing to specific knobs.

Each output ends with a one-page summary the parent agent can
paste into a doc unchanged.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Long-context reasoning over multiple service files |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Quicker design iterations |
| Local / offline | Ollama | `llama3.1:70b` (when you have the RAM) or `qwen2.5:32b` | Architectural reasoning needs the bigger model when local |

## How the parent agent calls it

```
delegate(server_architect): we're adding webhook ingestion at
~5k events/sec. Propose the topology — sync 2xx ack vs async queue,
storage choice, idempotency, replay strategy, dead-letter handling.
Diagram and one-pager.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(server_architect): the gateway is hitting p99 latency targets but p50 is creeping up. Propose where to introduce caching vs read replicas, with named NFRs and rollback plan."
  }'
```

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.5` | Drop to 0.4 for tightly-constrained \"match this existing pattern\" briefs; raise to 0.6 for greenfield topology design. |
| `max_iterations` | `16` | Raise to 20–24 for multi-service migrations. |
| `agentic_timeout_secs` | `900` | Raise when reading a large existing codebase to ground the design. |
| `allowed_tools` | curated above | Add `playwright` if you want the agent to confirm a hop's behaviour against a running app. Don't add `shell` — the design should not be pinning to one machine's environment. |
| `memory_namespace` | `"server_architect"` | Set per-service (`server_architect-billing`) when topology baselines diverge between products. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::server_architect_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "server_architect".to_string(),
    server_architect_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## How server_architect composes

| Concern | Goes to |
|---|---|
| Whole-system boundaries (frontend + backend + infra + data) | `architect` |
| Server-side topology, capacity, contracts | `server_architect` |
| Schema + migrations | `db_designer` |
| Infra to host the design | `devops` |
| Implementing handlers / clients to spec | `coder` |

Server_architect produces the spec; the others land it. It does
not edit production code paths beyond design docs in `docs/`.
