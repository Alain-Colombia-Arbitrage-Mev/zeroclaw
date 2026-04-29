# Qdrant memory backend

ZeroClaw can store conversation memory in [Qdrant](https://qdrant.tech),
a dedicated vector database. The Qdrant backend lives **alongside** the
default SQLite store — you opt in per-deployment by editing
`[memory] backend = "qdrant"` in your config.

The wire-up has three pieces:

1. A running Qdrant instance.
2. An `[memory.qdrant]` section pointing at it.
3. An embedding provider that turns memory text into vectors.

## 1. Start Qdrant

The repository ships a standalone compose file that brings up Qdrant on
the default ports (`6333` REST, `6334` gRPC) with persistent volumes:

```bash
docker compose -f docker-compose.qdrant.yml up -d
```

To verify it's reachable:

```bash
curl http://localhost:6333/healthz   # → "healthz check passed"
```

The Qdrant dashboard is at <http://localhost:6333/dashboard>.

To stop Qdrant and wipe its data:

```bash
docker compose -f docker-compose.qdrant.yml down -v
```

If you'd rather use **Qdrant Cloud**, skip the compose entirely — just
copy your cluster URL and API key into the config below.

## 2. Wire it into ZeroClaw

Edit `~/.zeroclaw/config.toml`:

```toml
[memory]
backend = "qdrant"
embedding_provider = "openai"
embedding_model = "text-embedding-3-small"

[memory.qdrant]
url = "http://localhost:6333"
collection = "zeroclaw_memories"
# api_key = "..."        # only needed for Qdrant Cloud
```

The collection is created on first use with the right vector dimension
(1536 for `text-embedding-3-small`). Switching models later means
either picking a new collection name or wiping the existing one.

Restart the daemon and the new backend is live:

```bash
zeroclaw daemon
```

## 3. Embeddings

`embedding_provider` and `embedding_model` decide how text becomes a
vector before it's stored or queried. Three sensible setups:

| Goal | `embedding_provider` | `embedding_model` | Notes |
|---|---|---|---|
| Reuse OpenRouter (recommended) | `openai` | `text-embedding-3-small` | OpenRouter speaks the OpenAI `/v1/embeddings` shape; your existing `OPENROUTER_API_KEY` works. 1536 dims. |
| Higher quality | `openai` | `text-embedding-3-large` | 3072 dims, ~6.5× the price, ~2× the storage. Better recall on dense topics. |
| Local / offline | `ollama` | `nomic-embed-text` | Requires Ollama running locally with the model pulled (`ollama pull nomic-embed-text`). 768 dims, free. |

Once configured, vector recall happens automatically via the existing
`memory_recall` and `memory_search` tools — no changes to skills or
prompts.

## Where data is stored

| | |
|---|---|
| Qdrant memory | Qdrant collection (`zeroclaw_memories` by default) |
| Conversation history | unaffected — still SQLite session store |
| Daily memory files | unaffected — still `~/.zeroclaw/workspace/memory/*.md` |

The Qdrant backend replaces the *recall* substrate, not the chat
session log. Existing markdown / SQLite stores keep working as
secondary read sources, so you can flip between backends without
losing prior memories — only the *new* writes go to Qdrant.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `connection refused` on startup | Qdrant container not running | `docker compose -f docker-compose.qdrant.yml ps` and start if needed |
| `Embedding provider not configured` | `embedding_provider` is empty or `none` | Set it to `openai` (or any working provider) |
| `Vector dimension mismatch` | Switched embedding models without changing the collection | Change `[memory.qdrant] collection` to a new name, or delete the old one |
| Qdrant Cloud `401 Unauthorized` | Missing API key | Set `[memory.qdrant] api_key` or export `QDRANT_API_KEY` |
