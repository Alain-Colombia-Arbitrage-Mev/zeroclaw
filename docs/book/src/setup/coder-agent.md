# Coder sub-agent

ZeroClaw can delegate coding work to a specialised sub-agent through
the existing `[agents.<name>]` mechanism. This page documents the
`coder` preset — a curated `DelegateAgentConfig` that gives you a
ready-to-use coding agent without having to design the system prompt
and tool allowlist from scratch.

The preset is also exposed in code at
`zeroclaw_config::agent_presets::coder_preset(provider, model)` for
extension authors who want to register the agent programmatically.

## What you get

A sub-agent that:

- Runs **agentic** (multi-turn loop), so it can read code, edit it,
  run tests, and iterate inside a single delegation.
- Uses a **low temperature** (0.2) for deterministic codegen.
- Has a **tight tool allowlist** — filesystem, git, shell, knowledge
  graph, LLM helpers — but **not** the broader integration surface
  (no Slack, no Telegram, no canvas).
- Uses an **isolated memory namespace** (`coder`) so its
  recall/store calls don't pollute the parent agent's main namespace.
- Carries a **role-specific system prompt** that biases toward small
  reviewable diffs, exact-string `file_edit` over `file_write`, and
  running the project's own checks before declaring success.

## Drop-in TOML

Paste this into `~/.zeroclaw/config.toml`. Pick a model that handles
codegen well — recommended starting points are listed under
[Choosing a model](#choosing-a-model).

```toml
[agents.coder]
provider           = "openrouter"
model              = "anthropic/claude-sonnet-4"
temperature        = 0.2
agentic            = true
max_depth          = 2
max_iterations     = 24
timeout_secs       = 180
agentic_timeout_secs = 900
memory_namespace   = "coder"

# Tools the sub-agent is allowed to call. The shell tool is still
# gated by your top-level [autonomy] command allowlist; this list is
# the *additional* surface the sub-agent can reach inside the parent
# delegation.
allowed_tools = [
  "file_read",
  "file_write",
  "file_edit",
  "glob_search",
  "content_search",
  "git_operations",
  "shell",
  "tool_search",
  "knowledge",
  "graphify",
  "llm_task",
  "web_fetch",
  "memory_recall",
  "memory_store",
]

system_prompt = """
You are the project's coder sub-agent. Your job is to make small,
reviewable changes to the codebase that satisfy the parent agent's
request and pass the project's own checks before you report done.

Operating principles:

- Read before you write. Use file_read, glob_search, and
  content_search to find the relevant code. When the project has a
  knowledge graph, prefer `graphify query` to grepping blindly.
- Diffs over rewrites. Prefer file_edit (exact-string replace) over
  file_write for edits to existing files. Three similar lines is
  better than a premature abstraction.
- One concern per delegation. If the request mixes refactor + feature
  + infrastructure, ask the parent to split it before you start.
- Don't speculate. No new config keys, abstractions, or backwards-
  compatibility shims unless the request demands them.
- Verify locally. After your change, run the project's own check
  suite (cargo test / npm test / pytest / etc.) through `shell`. If
  the project has a `dev/ci.sh`, prefer that. Report the actual
  command and exit code.
- Style matches the file. Don't introduce comments unless the *why*
  is non-obvious. Don't reformat code you didn't need to touch.
- When you're stuck, surface it. Return a short summary of what you
  tried and why it didn't work — don't loop the same approach.

Out of scope:

- Pushing branches, opening PRs, or anything that affects shared
  state. The parent agent owns those steps.
- Changing security/policy boundaries unless the request is
  explicitly about that.
"""
```

The block above is what `coder_preset()` produces — copy-pastable TOML
and Rust constructor stay in lock-step (the unit tests in
`agent_presets.rs` enforce that the prompt and tool list don't drift
silently).

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Strong on long-context refactors, deep diff intent |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Lower latency, broad library knowledge |
| Free tier OK | OpenRouter | `qwen/qwen-2.5-coder-32b-instruct:free` | Coder-tuned; surprisingly competent on Rust + JS |
| Local / offline | Ollama | `qwen2.5-coder:14b` | Pulls cleanly with `ollama pull qwen2.5-coder`; needs ~16 GB RAM for 14B variant |

Switch by editing the `provider` and `model` fields above. The shape
of the agent doesn't change.

## How the parent agent calls it

Once registered, the parent agent can invoke the coder via the
`delegate` tool. From the agent chat or `/jarvis`:

> *"Delegate to the coder sub-agent: add a `--dry-run` flag to the
> `cron add` CLI subcommand and verify with cargo test"*

Or from a webhook payload:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(coder): refactor the gateway rate-limit constants behind ZEROCLAW_AUTH_* env vars; run cargo test -p zeroclaw-gateway and report"
  }'
```

The `delegate` tool returns the sub-agent's final message plus a
trace of the tool calls it made.

## Tuning knobs to know

| Field | Default in preset | When to change |
|---|---|---|
| `temperature` | `0.2` | Bump to ~0.4 if the model gets stuck producing the same wrong fix on retries |
| `max_iterations` | `24` | Lower (10–12) when delegating tightly-scoped fixes; raise (40+) for "rewrite this module" tasks |
| `agentic_timeout_secs` | `900` | Raise for long test suites; lower for triage-style queries |
| `allowed_tools` | curated list above | Add `playwright` for UI codegen, drop `shell` for read-only audits |
| `memory_namespace` | `"coder"` | Set to `"coder-{project}"` if you run multiple repos through the same daemon and want isolated recall per project |

## Programmatic registration

For extension code that injects the agent at runtime instead of via
TOML:

```rust
use zeroclaw_config::agent_presets::coder_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "coder".to_string(),
    coder_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

`coder_preset` returns a plain `DelegateAgentConfig` — every field
is `pub`, so the caller can mutate the result before inserting it.
