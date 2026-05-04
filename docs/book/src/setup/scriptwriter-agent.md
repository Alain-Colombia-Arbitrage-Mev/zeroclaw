# Scriptwriter sub-agent

The `scriptwriter` preset is a senior screenwriter delegate for
video, audio, podcast, explainer, and ad scripts. It owns story
structure, beats, dialogue, pacing, and shooting-ready format —
distinct from `content_creator`, which writes prose for reading.

The preset is exposed in code at
`zeroclaw_config::agent_presets::scriptwriter_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 14 iterations per delegation.
- Uses the **highest temperature** of the GTM lane (0.8) — story
  beats and dialogue need real divergence — anchored by a strict
  format and pacing budget.
- Has a **read + write + research** surface. **No `image_gen` /
  `canvas`** (that's designer's lane), no `shell` / `git` (humans
  own production).
- Outputs in industry-standard formats — Fountain (`.fountain`)
  for screenplay-style, two-column AV scripts (VIDEO | AUDIO) for
  ads / explainers, podcast scripts with `[SFX]` / `[MUSIC IN]` /
  `[HOST]` cues.
- Uses an **isolated memory namespace** (`scriptwriter`) so prior
  characters, voices, and brand-specific scene conventions persist.

## Drop-in TOML

```toml
[agents.scriptwriter]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.8
agentic              = true
max_depth            = 2
max_iterations       = 14
timeout_secs         = 180
agentic_timeout_secs = 600
memory_namespace     = "scriptwriter"

allowed_tools = [
  "file_read",
  "file_write",
  "file_edit",
  "glob_search",
  "content_search",
  "knowledge",
  "llm_task",
  "web_fetch",
  "memory_recall",
  "memory_store",
  "context7__resolve-library-id",
  "context7__get-library-docs",
]

system_prompt = """
You are the project's scriptwriter sub-agent. Your job is to turn
a brief into a script that holds attention from the first second
to the call to action — and is ready for production.

Operating principles:

- Premise in one sentence (logline): who, wants what, despite
  what, with what result.
- Structure on purpose. Pick the frame and name it: 3-act for
  short narrative, problem/agitate/solve for ads, hook /
  promise / payoff for explainers, AIDA for direct response,
  PAS for short-form social.
- Open with conflict, tension, or a concrete question. First five
  seconds decide the next twenty-five.
- Show, don't narrate. When a line could be replaced by a shot,
  replace it.
- Dialogue sounds spoken — read aloud, no semicolons,
  contractions, fragments, distinct voices.
- Pacing has a budget — match runtime to format
  (15s / 30s / 60s / 3min / 10min explainer / podcast). Cut to
  fit; don't pad.
- Format is industry-standard. Spec scripts in fountain. Two-column
  AV scripts for ads / explainers. Podcast scripts with [SFX] /
  [MUSIC] / [HOST] / [GUEST] cues.
- Voice and brand consistency — read existing scripts and brand
  guide first.
- Localisation note: flag jokes, idioms, references that won't
  translate; propose alternatives.
- Edit pass before you ship.

Out of scope:

- Storyboards, shot lists, lighting / camera plans, edit
  decisions. Suggest, don't dictate.
- Production: casting, scheduling, recording.
- Long-form prose. Hand to content_creator.
- Strategy and channel selection. Hand to marketing.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Strong on dialogue rhythm + story structure |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Faster iteration on alternates |
| Free tier OK | OpenRouter | `google/gemini-flash-1.5` | Workable for first-draft beats; revise manually |
| Local / offline | Ollama | `llama3.1:8b` | Decent at structure; weaker on natural dialogue |

## How the parent agent calls it

```
delegate(scriptwriter): from the marketing brief in
docs/marketing/q3-launch-brief.md, write (a) a 30s social ad in
PAS format, (b) a 90s explainer in two-column AV, (c) a 6-min
podcast cold-open. Voice: same as our previous launch ads —
read /content/ads/*.fountain first.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(scriptwriter): write a 60s YouTube pre-roll in problem/agitate/solve, target audience SREs, single message: ZeroClaw runs the agent loop on your own infra. Logline first."
  }'
```

The agent returns scripts as files (one per deliverable) plus a
production note: format, runtime, the structural frame chosen,
and any localisation flags.

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.8` | Drop to 0.6 when matching an existing tightly-defined character voice; raise to 0.9 for early-stage exploration. |
| `max_iterations` | `14` | Raise to 20 for episodic / multi-script briefs. |
| `agentic_timeout_secs` | `600` | Usually adequate; raise only for very long scripts or when web_fetch is slow. |
| `allowed_tools` | curated above | Generally leave it. Add `image_gen` only if you want rough mood-frames embedded in the doc — designer is the right home for that work. |
| `memory_namespace` | `"scriptwriter"` | Set per-show / per-brand when one daemon serves multiple narrative universes. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::scriptwriter_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "scriptwriter".to_string(),
    scriptwriter_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## Scriptwriter vs content_creator

| Concern | Goes to |
|---|---|
| Blog post, newsletter, social thread, ad copy text, landing copy | `content_creator` |
| Anything spoken or filmed (script with structure + format) | `scriptwriter` |
| 30s ad in two-column AV | `scriptwriter` |
| 600-word ad-supporting blog post | `content_creator` |

The split is "is it read or is it performed?". Performed work goes
to scriptwriter for the script; designer or a human team handles
storyboards, recording, and edit.
