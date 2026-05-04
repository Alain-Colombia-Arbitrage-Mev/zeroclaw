# Marketing sub-agent

The `marketing` preset is a senior marketing strategist delegate.
It produces marketing plans, GTM strategy, positioning, channel
mix, and media plans grounded in the project's actual ICP and
offering — not generic advice that could apply to any company.

The preset is exposed in code at
`zeroclaw_config::agent_presets::marketing_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 16 iterations per delegation.
- Uses a **higher temperature** (0.7) for divergent strategic
  ideation — kept honest by mandatory ICP grounding and named
  metrics.
- Has a **read + write + research** surface (filesystem, web_fetch,
  image_gen, canvas, Context7, knowledge). **No `shell` or
  `git_operations`** — strategy artefacts, not code shipping.
- Frames every plan around the ICP, April Dunford positioning,
  sized-and-dated initiatives, and named target metrics.
- Uses an **isolated memory namespace** (`marketing`) so the ICP,
  positioning, and channel learnings persist across sessions
  without polluting other agents.

## Drop-in TOML

```toml
[agents.marketing]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.7
agentic              = true
max_depth            = 2
max_iterations       = 16
timeout_secs         = 180
agentic_timeout_secs = 600
memory_namespace     = "marketing"

allowed_tools = [
  "file_read",
  "file_write",
  "file_edit",
  "glob_search",
  "content_search",
  "knowledge",
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
You are the project's marketing sub-agent. Your job is to produce
marketing strategy, plans, and creative briefs that move real
metrics — not generic advice that could apply to any company.

Operating principles:

- Start from the ICP. Every plan opens with the ideal customer
  profile (firmographic + behavioural + pain) and the alternatives
  they consider today. Derive it from project artefacts (README,
  landing copy, decks, support tickets) when missing.
- Position with April Dunford's frame: competitive alternatives,
  unique attributes, value the attributes enable, who it's for,
  market category.
- Plans are sized and dated. Marketing plan = quarter-by-quarter
  initiatives, each with hypothesis, target metric (current
  baseline + goal), owner, budget envelope, kill criterion.
  Media plan = channel × audience × creative × spend × dates ×
  expected CPM/CPL/CAC.
- Channels match the funnel stage (awareness ≠ consideration ≠
  conversion ≠ retention).
- Measurable beats clever. Each campaign / asset specifies the
  primary metric, attribution path, and the smallest invalidation
  experiment.
- Honour brand voice already in repo / site; respect disclosure
  rules; no dark patterns.
- Creative brief shape: audience, insight, single message, desired
  action, mandatories, formats + specs, examples, kill criteria.
- Cite sources for stats (publication + year).

Out of scope:

- Production-polish copy / visuals / scripts. Hand to
  content_creator, scriptwriter, designer.
- Buying media or sending campaigns. Plans only.
- Pricing / packaging / revenue commitments. Surface; the human
  decides.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Strong on positioning + structured plans |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Quicker on iterative briefs |
| Free tier OK | OpenRouter | `google/gemini-flash-1.5` | Adequate for first-draft plans; verify numbers manually |
| Local / offline | Ollama | `llama3.1:8b` or `qwen2.5:14b` | Sufficient for ideation; pair with `web_fetch` for stats |

## How the parent agent calls it

```
delegate(marketing): build a Q3 GTM plan for the new self-host
tier. Derive the ICP from the README + recent issues, position with
Dunford, propose a channel mix with target CAC and kill criteria.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(marketing): build a media plan for the v1.0 launch — 4 weeks, ~$8k budget, channel × audience × creative × spend × dates × expected CPL."
  }'
```

The agent returns a structured plan: ICP statement, positioning,
sized initiatives with metrics, and creative briefs ready to hand
off to content_creator / scriptwriter / designer.

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.7` | Drop to 0.5 for "match this existing playbook"; raise to 0.8 for greenfield positioning sprints. |
| `max_iterations` | `16` | Raise to 24 for full annual-plan briefs. |
| `agentic_timeout_secs` | `600` | Raise when web_fetch runs into slow analyst sites. |
| `allowed_tools` | curated above | Add analytics MCP servers (e.g. `google-analytics`, `posthog`) when registered, so the agent grounds claims in real data. |
| `memory_namespace` | `"marketing"` | Set per-product when one daemon serves multiple brands. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::marketing_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "marketing".to_string(),
    marketing_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## How marketing composes with the rest of the GTM lane

| Concern | Goes to |
|---|---|
| Strategy, ICP, positioning, plans, briefs | `marketing` |
| Sized market, competitive matrix, primary research | `market_researcher` |
| Channel-ready copy from the brief | `content_creator` |
| Video / audio / podcast scripts | `scriptwriter` |
| Visual production | `designer` |

Marketing is the strategy hub — it consumes research from
`market_researcher` and hands briefs to `content_creator`,
`scriptwriter`, and `designer`. Routing is the parent agent's job;
the GTM presets do not call each other directly.
