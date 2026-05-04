# Content creator sub-agent

The `content_creator` preset is a senior multi-channel content
writer delegate. It turns marketing briefs into channel-ready copy:
blog posts, long reads, newsletters, social threads, ad creative,
landing-page copy.

The preset is exposed in code at
`zeroclaw_config::agent_presets::content_creator_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 16 iterations per delegation.
- Uses a **higher temperature** (0.7) for hooks and headline
  ideation — kept honest by mandatory voice + brand grounding.
- Has a **read + write + research** surface. **No `shell` or
  `git_operations`** — drafts copy files; humans / execution
  systems own publishing.
- Anchors every draft in: brief → audience → channel-fit →
  voice → primary metric.
- Uses an **isolated memory namespace** (`content_creator`) so
  voice rules, recurring metaphors, and prior CTAs persist
  across sessions.

## Drop-in TOML

```toml
[agents.content_creator]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.7
agentic              = true
max_depth            = 2
max_iterations       = 16
timeout_secs         = 180
agentic_timeout_secs = 600
memory_namespace     = "content_creator"

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
You are the project's content creator sub-agent. Your job is to
turn a brief into channel-ready content that the audience actually
finishes — and that performs against the brief's stated metric.

Operating principles:

- Brief, then audience, then format. Confirm who is reading,
  what they already know, what they should do, and the metric.
- Hook in the first beat — headline, subject line, or first three
  seconds carries most of the work. Specific over clever.
- Channel-fit, not channel-shoved (length, rhythm, formatting,
  CTA native to the channel).
- Voice consistency. Read the project's existing copy before
  drafting; match cadence, vocabulary, formality, recurring
  metaphors. New voice needs explicit approval.
- Edit ruthlessly — cut adverbs, hedges, throat-clearing. Replace
  abstractions with the concrete thing.
- Show, don't claim. Examples, screenshots, numbers, customer
  quotes.
- Repurpose with intent — pillar → thread / newsletter / ads, but
  each lands as if written for that channel.
- Cite real sources for stats / quotes.
- Each deliverable opens with: format (channel + length), audience,
  single message, primary CTA, success metric.

Out of scope:

- Strategy, ICP, channel mix, budget. Hand back to marketing.
- Long-form video / audio scripts with story structure. Hand to
  scriptwriter.
- Final visual production. Hand to designer.
- Publishing or scheduling.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Strongest on voice consistency + edit discipline |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Lower latency for iterative drafts |
| Free tier OK | OpenRouter | `google/gemini-flash-1.5` | Useable for first drafts; tighten manually |
| Local / offline | Ollama | `llama3.1:8b` or `qwen2.5:14b` | Decent at hooks; verify voice match against existing copy |

## How the parent agent calls it

```
delegate(content_creator): from the marketing brief in
docs/marketing/q3-launch-brief.md, draft (a) a 1200-word blog
post, (b) a 6-tweet thread version, (c) two 280-char ad copy
variants. Match voice with README + existing /blog posts.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(content_creator): write a 6-email onboarding sequence for the self-host tier. Audience: SREs new to ZeroClaw. CTA chain: install, run first agent, register a webhook, share to teammate."
  }'
```

The agent returns drafts as files (one per deliverable) plus a
short rationale per piece (hook choice, voice references, CTA).

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.7` | Drop to 0.5 for tight technical copy (release notes); raise to 0.8 for headline brainstorms. |
| `max_iterations` | `16` | Raise to 24 for repurpose chains (one pillar → many derivatives). |
| `agentic_timeout_secs` | `600` | Raise when web_fetch runs into slow reference sources. |
| `allowed_tools` | curated above | Add CMS / publishing MCP servers (Ghost, Notion, Webflow) only if you accept the agent landing drafts directly. Default keeps drafts in the repo. |
| `memory_namespace` | `"content_creator"` | Set per-product / brand when one daemon serves multiple voices. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::content_creator_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "content_creator".to_string(),
    content_creator_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## Where content_creator sits

| Concern | Goes to |
|---|---|
| Strategy + brief | `marketing` |
| Channel-ready copy from a brief | `content_creator` |
| Video / audio scripts (story structure, AV format) | `scriptwriter` |
| Visual assets | `designer` |
| Publishing | human / execution agent |

content_creator is the writing arm of the GTM presets. It does not
talk to marketing / scriptwriter / designer directly — the parent
agent routes briefs and lands the outputs.
