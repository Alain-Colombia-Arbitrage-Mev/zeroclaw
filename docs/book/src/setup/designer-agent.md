# Designer sub-agent + Pencil.dev integration

ZeroClaw's `designer` preset is a sub-agent specialised in UI / visual
design tasks — design tokens, components, mockups, layouts. It pairs
with the [Pencil.dev](https://pencil.dev) MCP server when you have one
running, and falls back gracefully to ZeroClaw's built-in `image_gen`
+ `canvas` tools when you don't.

The preset is exposed in code at
`zeroclaw_config::agent_presets::designer_preset(provider, model)`,
mirroring [`coder_preset`](./coder-agent.md).

## What you get

A sub-agent that:

- Runs **agentic** with up to 18 iterations per delegation.
- Uses a **moderately creative temperature** (0.6) — higher than
  the coder (0.2) because design benefits from divergent thinking,
  but still bounded by hard constraints (tokens, accessibility).
- Has a **visual-design tool surface**:
  - **Pencil tools** (`pencil_*`) when the operator has registered
    a Pencil MCP server.
  - **`image_gen`** for raster artefacts and **`canvas`** for ad-hoc
    composition when no Pencil server is available.
  - Filesystem (`file_read` / `_edit` / `_write`, `glob_search`,
    `content_search`) so it can land HTML / CSS / SVG once a design
    is approved.
  - `web_fetch` for component-library lookups (shadcn, Radix,
    Material).
  - `llm_task` for structured copy / a11y review.
  - `memory_recall` / `memory_store` in a dedicated `designer`
    namespace so cross-session design language persists without
    contaminating the parent agent's memory.
- **Does NOT have** `shell` or `git_operations` — those are the
  coder's surface; the parent agent owns the shipping step.
- Carries a system prompt that biases toward design tokens,
  hierarchy over decoration, accessibility checks, small iterative
  edits, and respecting the project's existing brand voice.

## 1. Drop-in TOML

Pegá esto en `~/.zeroclaw/config.toml` debajo del bloque del coder
(o donde tengas tus otros `[agents.*]`):

```toml
[agents.designer]
provider           = "openrouter"
model              = "anthropic/claude-sonnet-4"
temperature        = 0.6
agentic            = true
max_depth          = 2
max_iterations     = 18
timeout_secs       = 180
agentic_timeout_secs = 600
memory_namespace   = "designer"

allowed_tools = [
  # Pencil MCP — exposed only when the server is registered (next section)
  "pencil_open_document",
  "pencil_get_editor_state",
  "pencil_batch_get",
  "pencil_batch_design",
  "pencil_get_screenshot",
  "pencil_search_all_unique_properties",
  "pencil_replace_all_matching_properties",
  # Native generative + composition fallbacks
  "image_gen",
  "canvas",
  # Filesystem — landing HTML / CSS / SVG / component files
  "file_read",
  "file_write",
  "file_edit",
  "glob_search",
  "content_search",
  # Reference fetching
  "web_fetch",
  # Structured sub-tasks (copy / a11y review)
  "llm_task",
  # Cross-session design-language continuity
  "memory_recall",
  "memory_store",
]

system_prompt = """
You are the project's designer sub-agent. Your job is to produce or
refine UI / visual artefacts — design tokens, components, mockups,
layouts, illustrations — that fit the project's existing language
and the parent agent's brief.

Operating principles:

- Tokens before pixels. When the project has a design-system surface
  (Pencil document, CSS variables, Figma variables), read it first
  and reference its colours, type scale, and spacing — do not invent
  new values that fight the system.
- Hierarchy over decoration. Each artefact should answer "what is
  the user's first action here?" before it answers "what looks
  cool?". When in doubt, ship a calmer version.
- Accessibility is non-negotiable. Verify contrast (WCAG AA at
  minimum), tab order, focus states, and alt text on every artefact
  you emit. Surface failures as part of your reply, not buried.
- Iterate small. Prefer pencil_batch_design updates and file_edit
  over wholesale pencil_open_document / file_write rewrites.
- Show your work. After each iteration, take a screenshot
  (pencil_get_screenshot or image_gen for native artefacts) and
  reference what changed.
- Respect existing files. Don't rename components, restructure
  folders, or break import paths just because you find a better
  arrangement. Surface the suggestion in your reply instead.
- Brand voice carries through. If the project has tone-of-voice
  rules in MEMORY.md or a brand guide, copy and microcopy match
  them — never default to generic SaaS prose.
"""
```

## 2. Register Pencil as an MCP server (optional but recommended)

If you already use Pencil with another AI assistant (Claude Code,
Cursor, etc.), the MCP server is already on your machine — ZeroClaw
just needs to know where to find it. Add this section to the same
`config.toml`:

```toml
[[tools.mcp.servers]]
name      = "pencil"
transport = "stdio"
command   = "pencil-mcp"          # or the full path to the binary
args      = []
```

If your Pencil MCP entrypoint is different (e.g. an `npx` invocation
or a Python module), match the working command:

```toml
[[tools.mcp.servers]]
name      = "pencil"
transport = "stdio"
command   = "npx"
args      = ["-y", "@pencil-dev/mcp-server"]
```

ZeroClaw's MCP registry connects on startup and auto-prefixes the
tool names with `pencil_`, which is what the designer preset
expects.

**No Pencil server installed?** The preset still works — the
designer just falls back to `image_gen` and `canvas` for visual
output, and `file_edit` / `file_write` to land assets. Pencil tools
listed in `allowed_tools` that aren't registered at runtime are
silently dropped from the agent's effective surface.

## 3. How the parent agent calls it

```
delegate(designer): redesign the /jarvis settings modal — match the
existing dark gradient, ensure WCAG AA contrast, and propose three
icon variants for the "save" action.
```

Or via the webhook surface:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(designer): produce a Pencil mockup for an empty-state when no cron jobs are scheduled. Use the existing Inter / Geist Mono pair and the project'\''s violet accent."
  }'
```

The designer returns its final artefact (image URL, Pencil document
ID, written files) plus a concise change log of what it did and
which a11y checks it ran.

## 4. Tuning knobs

| Field | Default | Bump when… |
|---|---|---|
| `temperature` | `0.6` | You want more divergent ideation (e.g. mood-board phase) — try `0.8`. Drop to `0.4` for "match this existing pattern exactly". |
| `max_iterations` | `18` | The designer keeps "almost there" — raise to 24–30 for ambitious mockups; lower to 8–10 for tweaks. |
| `agentic_timeout_secs` | `600` | Pencil document edits + screenshots can be slow on large boards. |
| `allowed_tools` | curated list | Add `playwright` if you want the designer to verify against a running site (visual diff). Remove the `pencil_*` block if you don't run a Pencil server (it's harmless but the agent's tool list is shorter). |
| `memory_namespace` | `"designer"` | Set per-project (e.g. `"designer-marketing"`) when one daemon serves multiple brand systems. |

## 5. Programmatic registration

```rust
use zeroclaw_config::agent_presets::designer_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "designer".to_string(),
    designer_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## How designer + coder compose

The two presets cover complementary lanes:

| Concern | Goes to | Why |
|---|---|---|
| New visual artefact | `designer` | Token-aware, accessibility-checked output; Pencil-native when available |
| Landing the artefact in code (HTML/CSS) | `designer` (small) → `coder` (refactor / cleanup) | Designer drops the file; coder makes sure tests still pass |
| Implementing functionality behind the visual | `coder` | Designer lacks `shell` / `git_operations` |
| Updating design tokens (CSS variables) | `designer` first, `coder` second | Designer proposes token changes; coder runs the type-check + tests after the swap |

Use the parent agent (or you, manually) to route between them. They
don't talk to each other directly — that's by design, so each lane
stays narrow and reviewable.
