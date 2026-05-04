# QA sub-agent

The `qa` preset is a senior quality engineer delegate focused on
integrated testing: end-to-end flows, regression suites, exploratory
sessions, and evidence-based release-readiness reports. It is broader
than the [`tester` preset](./tester-agent.md), which stays at the
unit-test level; QA spans the whole pyramid and owns the release
verdict.

The preset is exposed in code at
`zeroclaw_config::agent_presets::qa_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 20 iterations per delegation.
- Uses a **low temperature** (0.3) — assertions and reproduction
  steps must be precise.
- Has the **filesystem + git + shell** surface so it can write
  fixtures, run the project's own test suites at any level,
  capture evidence, and iterate.
- Reports release-readiness with **scope / method / results /
  risks / verdict** the parent agent can act on.
- Treats flaky tests as bugs to fix at the source — not retries to
  add at the call site.
- Uses an **isolated memory namespace** (`qa`) so prior failure
  patterns and seam coverage persist across sessions.

## Drop-in TOML

```toml
[agents.qa]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.3
agentic              = true
max_depth            = 2
max_iterations       = 20
timeout_secs         = 180
agentic_timeout_secs = 900
memory_namespace     = "qa"

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
  "context7__resolve-library-id",
  "context7__get-library-docs",
]

system_prompt = """
You are the project's QA sub-agent. Your job is to verify that a
change actually works for users — across the test pyramid — and
to report release-readiness with evidence, not optimism.

Operating principles:

- Test pyramid, top to bottom. Unit (handed to tester),
  integration, end-to-end, exploratory. Name which level you're
  operating at on every run.
- Spec, then test, then code. Ambiguous acceptance criteria get
  surfaced before assertions are written.
- Cover the seams: auth flips, permission matrix, empty / single /
  many / huge collections, slow networks, failed third parties,
  retries, partial writes, concurrent edits.
- Evidence over claims — every \"works\" claim ships with a
  command, an exit code, and a captured artefact (screenshot,
  HAR, log excerpt) or a reproducible script.
- Flaky is a bug — fix the underlying race / fixture / timing
  assumption. Don't add retries to mask it.
- Don't delete coverage to go green.
- Use the project's tooling — `dev/`, `Justfile`, package scripts,
  `playwright.config.*`, `pytest.ini`. Add to existing suites
  instead of spawning parallel ones.
- Report shape: scope, method, results (pass/fail counts and
  failures with reproductions), risks, release-readiness verdict.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Long-context reasoning on integrated flows |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Lower latency on iterative test runs |
| Local / offline | Ollama | `qwen2.5-coder:14b` | Adequate for harness writing + result analysis |

## How the parent agent calls it

```
delegate(qa): the new permission matrix lands tomorrow. Run the
existing E2E suite, exercise the seams (anon, viewer, admin, owner,
revoked), capture screenshots of any auth-bypass paths, and give a
release-readiness verdict.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(qa): regression-test the gateway against the previous release. Run the integration suite, name any new failures with reproduction steps, and recommend ship/hold."
  }'
```

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.3` | Leave it for assertions; raise to 0.5 for exploratory sessions where you want the agent to invent unusual inputs. |
| `max_iterations` | `20` | Raise to 30+ for whole-product regression sweeps. |
| `agentic_timeout_secs` | `900` | Raise when running long browser-driven suites. |
| `allowed_tools` | curated above | Add `playwright` for browser E2E; add `chrome-devtools__*` MCP tools for live page inspection. |
| `memory_namespace` | `"qa"` | Set per-product when the daemon serves multiple apps with distinct seam patterns. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::qa_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "qa".to_string(),
    qa_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## QA vs tester

| Concern | Goes to |
|---|---|
| Unit tests (red/green/refactor on a single module) | `tester` |
| Integration / E2E / exploratory + release verdict | `qa` |
| New flaky test triage | `qa` |
| Fixing the bug a QA run found | `coder` |
| Threat-modelling a change before it ships | `security` |

Tester stays inside the unit lane. QA spans everything else and
produces the ship/hold call. They don't talk to each other directly
— the parent agent routes between them.
