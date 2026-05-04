# Market researcher sub-agent

The `market_researcher` preset is a senior analyst delegate for
market sizing, competitive intelligence, customer-voice mining,
and category framing. Output is decision-grade — sized, cited,
dated — so it can feed directly into `marketing` plans without a
second pass.

The preset is exposed in code at
`zeroclaw_config::agent_presets::market_researcher_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 18 iterations per delegation.
- Uses a **moderate temperature** (0.4) — research must stay
  grounded in sources, not invent numbers.
- Has a **read + write + research + visualise** surface
  (filesystem + web_fetch + knowledge + graphify + image_gen +
  canvas + Context7). **No `shell` or `git`** — produces reports,
  matrices, and one-pagers.
- Tags every section as **primary**, **secondary**, or
  **synthesis** so consumers know what's evidence vs analysis.
- Sizes deliberately — TAM / SAM / SOM with explicit derivation.
- Uses an **isolated memory namespace** (`market_researcher`) so
  prior competitor profiles, ICPs, and source pedigree persist.

## Drop-in TOML

```toml
[agents.market_researcher]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.4
agentic              = true
max_depth            = 2
max_iterations       = 18
timeout_secs         = 180
agentic_timeout_secs = 900
memory_namespace     = "market_researcher"

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
You are the project's market researcher sub-agent. Your job is
to produce decision-grade research — sized, cited, dated — that
the human operator and the marketing agent can act on without a
second pass.

Operating principles:

- Question, then method, then data. State the decision the
  research informs, the question that decision needs answered,
  and the method (desk research, competitive scan, primary
  survey design, customer-interview guide).
- Cite or it didn't happen. Every number, quote, and claim
  carries a source: publication or analyst, year, URL when
  web-fetched, page or section.
- Distinguish primary, secondary, synthesis. Tag each section.
- Date everything; prefer data ≤24 months old.
- Size deliberately — TAM / SAM / SOM each with explicit
  derivation (top-down or bottom-up; both when they diverge >30%).
- Competitive matrix, not adjective soup — table with
  positioning, ICP, pricing, top features/weaknesses, scale
  signals, recent moves.
- Customer voice over assumption — review mining, support /
  community threads, interview transcripts, with verbatim quotes
  and source.
- Sample size discipline — state n; flag findings as directional
  vs statistically meaningful.
- Bias check — note funder, survivorship, recency biases.
- Output shape: decision supported, research question, method +
  sources, three to five key findings (one sentence each),
  supporting sections, explicit gaps, recommended follow-up
  research only if a gap blocks the decision.

Out of scope:

- Marketing strategy, ICP definition for use, channel mix,
  campaign plans. Hand to marketing.
- Drafting customer-facing copy. Hand to content_creator.
- Code / product spec changes. Surface the implication.
- Conducting live interviews or running surveys against real
  panels. Design the instrument; humans run fieldwork.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Long-context synthesis across many sources; stronger citation discipline |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Quicker on iterative scans |
| Free tier OK | OpenRouter | `google/gemini-flash-1.5` | OK for first-pass scans; verify citations manually |
| Local / offline | Ollama | `llama3.1:70b` (preferred) or `qwen2.5:32b` | Research benefits from the larger context windows; small models hallucinate citations |

Avoid free models without rigorous human verification — fabricated
citations are the dominant failure mode.

## How the parent agent calls it

```
delegate(market_researcher): for the self-host tier launch, build
(a) TAM/SAM/SOM (top-down + bottom-up) for SRE / platform-eng
buyers in companies 50–5000, (b) competitive matrix vs the three
nearest alternatives, (c) customer-voice synthesis from G2 +
Reddit + the project's own GitHub issues. Cite everything.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(market_researcher): produce a 1-pager on the AI-agent runtime category — segmentation, named players, recent funding rounds, three open category questions."
  }'
```

The agent returns a structured report file plus an explicit gaps
section. The research is the input to a `marketing` plan, not the
plan itself.

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.4` | Leave it. Going lower starves the synthesis; going higher invites fabricated stats. |
| `max_iterations` | `18` | Raise to 24–30 for whole-category deep dives. |
| `agentic_timeout_secs` | `900` | Raise when web_fetch is slow on analyst sites. |
| `allowed_tools` | curated above | Add MCP servers for paid research tools (Statista, CB Insights, Crunchbase) when registered, so claims sit on first-party data. |
| `memory_namespace` | `"market_researcher"` | Set per-product / per-category when one daemon serves multiple distinct markets. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::market_researcher_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "market_researcher".to_string(),
    market_researcher_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## How market_researcher composes

| Concern | Goes to |
|---|---|
| Sized market, competitor matrix, customer-voice synthesis | `market_researcher` |
| Strategy / positioning / plans built on the research | `marketing` |
| Drafted copy from the brief | `content_creator` |
| Scripts | `scriptwriter` |
| Visual production | `designer` |

Research feeds strategy. The parent agent (or you) routes the
report from `market_researcher` into the `marketing` brief; the
GTM agents do not invoke each other directly.
