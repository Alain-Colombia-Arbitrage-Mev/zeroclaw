# Together embeddings

The `together_embeddings` tool calls the [Together.ai](https://api.together.xyz) embeddings API directly. It serves the agentic use case — when the agent needs embeddings on demand for clustering, classification, deduplication, or ad-hoc semantic search of in-context data.

## Scope and non-scope

**This tool covers**: agent-initiated embeddings of text or batches of texts, returned to the agent as OpenAI-shaped JSON.

**This tool does NOT cover**: configuring Together as the automatic embedding provider for the memory subsystem. That requires extending `embeddings_provider` dispatch in `zeroclaw-memory` (a separate, larger change touching the BM25/vector retrieval pipeline). For now, `embedding_provider = "openrouter"` (or whatever you have today) stays in charge of memory; this tool sits beside it for ad-hoc use.

## Configuration

```toml
[together_embeddings]
enabled = true
base_url = "https://api.together.xyz"
api_key_env = "TOGETHER_API_KEY"
allowed_models = [
  "togethercomputer/m2-bert-80M-8k-retrieval",
  "BAAI/bge-large-en-v1.5",
  "WhereIsAI/UAE-Large-V1",
]
max_batch_size = 32
max_total_chars = 100000
timeout_secs = 30
```

`api_key` can be set directly (kept in the OS keyring via `#[secret]`); if unset, the tool reads the env var named in `api_key_env`.

## Common models

| Model | Dims | Context | Notes |
|---|---|---|---|
| `togethercomputer/m2-bert-80M-2k-retrieval` | 768 | 2k | Cheap, fast, retrieval-tuned |
| `togethercomputer/m2-bert-80M-8k-retrieval` | 768 | 8k | Default — handles paragraphs |
| `togethercomputer/m2-bert-80M-32k-retrieval` | 768 | 32k | Whole-document retrieval |
| `BAAI/bge-large-en-v1.5` | 1024 | 512 | High-quality English |
| `BAAI/bge-base-en-v1.5` | 768 | 512 | Cheaper BGE |
| `WhereIsAI/UAE-Large-V1` | 1024 | 512 | Strong on STS benchmarks |

Verify dimensions and pricing on [api.together.xyz/models](https://api.together.xyz/models) before adding to your allowlist.

## Invocation

Single text:

```jsonc
{
  "model": "togethercomputer/m2-bert-80M-8k-retrieval",
  "input": "Texto a embeber"
}
```

Batch:

```jsonc
{
  "model": "BAAI/bge-large-en-v1.5",
  "input": ["frase uno", "frase dos", "frase tres"]
}
```

Response is the raw Together payload (OpenAI-shaped):

```json
{
  "object": "list",
  "data": [{"object": "embedding", "embedding": [0.123, -0.045, ...], "index": 0}],
  "model": "...",
  "usage": {"prompt_tokens": 12, "total_tokens": 12}
}
```

## Security

- **API key** as `#[secret]` config field with `TOGETHER_API_KEY` env fallback. Never logged.
- **Model allowlist** mandatory — empty allowlist returns a clear configuration error.
- **Batch cap** via `max_batch_size`.
- **Total characters cap** via `max_total_chars` aggregated across the batch.
- **Read-only autonomy** blocks every call; **hourly action budget** enforced.

## Cost considerations

Together embeddings are billed per token. Reference rates:

- M2-BERT 80M: $0.008 per 1M tokens
- BGE Large: $0.016 per 1M tokens

A 100k-character batch (~25k tokens) costs about $0.0002–$0.0004.

## Operational notes

- Together's embeddings API is OpenAI-compatible — response shape identical to OpenAI's `/v1/embeddings`.
- For consistent embedding spaces, pick one model and stick to it. Switching models invalidates similarity scores.
- Combine with `memory_store` — agent embeds, stores `{content, embedding}` for later retrieval.
