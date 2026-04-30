# Graphify integration

[Graphify](https://graphify.net) (`pip install graphifyy`) turns any
folder of code, docs, papers, images, or videos into a queryable
knowledge graph. ZeroClaw exposes it as the agent-callable `graphify`
tool so the model can build, query, and explain graphs directly from
chat or voice without you dropping to a terminal.

## How it relates to ZeroClaw's other graph backends

| | What it stores | When to use it |
|---|---|---|
| `graphify` (this page) | **Code/repo structure** — classes, functions, imports, call graphs, semantic relationships extracted by an upstream LLM | "How is this codebase organised?", "Where does X feed into Y?" |
| `KnowledgeGraph` (SQLite — built in) | **Curated** patterns / decisions / lessons / experts the agent or operator captures explicitly | Long-lived team knowledge, decision records |
| `FalkorDbKnowledgeGraph` (Cypher) | Same shape as the SQLite KG, but on a Redis-backed graph DB for larger/shared graphs | Multi-host setups; see [FalkorDB knowledge graph](./falkordb.md) |
| `kg_extract` tool (LLM-driven) | Auto-extracted nodes/edges from arbitrary text | One-shot distillation of a transcript / paper |

These are complementary. Graphify gives you a *map of the
codebase*; the KG backends store *the conclusions you draw from
working with it*.

## 1. Install Graphify

Pick whichever Python install style you already use — the package is
called `graphifyy` (double-y), the CLI is `graphify`:

```bash
# Recommended: uv (Mac / Linux, no PATH gymnastics)
uv tool install graphifyy

# pipx works too
pipx install graphifyy

# Plain pip is fine; you may need to add ~/.local/bin to PATH afterwards
pip install graphifyy
```

Then, **once per project**, register the skill. ZeroClaw is
wire-compatible with the OpenClaw integration layer, so:

```bash
graphify install --platform claw
```

That writes the skill files and updates `AGENTS.md` so any
ZeroClaw-driven assistant in the project can reach the graph
automatically. (You can also use `graphify install --platform claude`
or any of the other supported platforms — Graphify only cares about
where to drop its skill files; the `graphify` CLI itself is shared.)

## 2. Build the first graph

```bash
graphify .
```

Graphify writes a `graphify-out/` folder next to wherever it ran:

```
graphify-out/
├── graph.html       # interactive graph — opens in any browser
├── graph.json       # full graph dump for programmatic queries
├── GRAPH_REPORT.md  # one-page summary: god nodes, communities, surprising connections
└── cache/           # SHA256 cache so re-runs only process changed files
```

It is safe (and often *useful*) to commit `graphify-out/` to git so
teammates start with the same map. Add this to your `.gitignore`:

```
# keep map outputs, skip mtime-dependent + local-only files
graphify-out/manifest.json
graphify-out/cost.json
graphify-out/cache/
```

## 3. Use the `graphify` tool from the agent

Once `graphify` is on PATH, the ZeroClaw runtime exposes a
`graphify` tool with these actions:

| Action | What it runs | When to ask for it |
|---|---|---|
| `init` | `graphify .` in a folder under your workspace | First-time scan, or after big code changes |
| `query` | `graphify query "<question>"` | "Where is auth implemented?", "What modules consume the rate limiter?" |
| `path` | `graphify path <from> <to>` | "How does ChannelSupervisor reach the SQLite session store?" |
| `explain` | `graphify explain "<focus>"` | "Why did the team pick this pattern?" — surfaces design rationale |

Example agent payload:

```json
{
  "action": "query",
  "directory": "crates/zeroclaw-runtime",
  "question": "Which modules emit security audit events?"
}
```

`directory` is interpreted relative to the configured `workspace_dir`
and must resolve inside it — symlinks pointing outside are rejected.

## 4. Privacy and security notes

- Graphify is MIT-licensed, performs no telemetry, and only sends
  semantic descriptions of documents to the upstream LLM (never raw
  source code).
- The upstream LLM is whichever assistant the operator already
  authorised — the `graphify` CLI inherits **its** API key, not
  ZeroClaw's. If you don't want Graphify to use your daemon's
  configured provider, run `graphify` from a shell with that
  provider's environment unset.
- The ZeroClaw wrapper runs the subprocess with a cleared
  environment plus the small allowlist documented in the source
  (`PATH`, `HOME`, `PYTHON*`, etc.) — no secrets leak through.
- The wrapper enforces workspace containment on the `directory`
  arg, the standard rate limit, and the same `ToolOperation`
  approval flow used by the other CLI delegators.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Failed to spawn graphify — is it on PATH?` | Pip script directory not on PATH | `uv tool install graphifyy` (auto-PATH), or add `~/.local/bin` |
| `graphify: command not found` *after* install | Same as above on a fresh shell | Re-source your shell config, or use `python -m graphify` |
| Build is slow on the first run | LLM extraction step | Expected — subsequent runs hit the SHA256 cache and complete in seconds |
| Large repos run out of LLM budget | Each subagent extraction costs tokens | Add a `.graphifyignore` to exclude `vendor/`, `node_modules/`, etc. — same syntax as `.gitignore` |
