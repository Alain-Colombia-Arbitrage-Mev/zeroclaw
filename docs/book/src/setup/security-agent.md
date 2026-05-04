# Security sub-agent

The `security` preset is a senior application-security engineer
delegate. It threat-models proposed changes, walks the OWASP map
deliberately, runs the project's own pinned scanners, and reports
findings as `severity / exploitability / fix` — without applying
patches itself.

The preset is exposed in code at
`zeroclaw_config::agent_presets::security_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 18 iterations per delegation.
- Uses a **low temperature** (0.2) — security findings should be
  deterministic and repeatable.
- Has a **read + scan + shell** surface (`file_read`,
  `content_search`, `git_operations`, `shell`, Context7) — but
  **no `file_write` / `file_edit`**. Patches are handed back to the
  coder agent with the precise line and replacement.
- Runs the project's own pinned scanners through `shell`
  (`cargo audit`, `cargo deny`, `npm audit`, `pip-audit`, `trivy`,
  `semgrep`, `gitleaks`) — never installs ad-hoc binaries.
- Uses an **isolated memory namespace** (`security`) so prior
  threat models persist across sessions without polluting parent
  recall.

## Drop-in TOML

```toml
[agents.security]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.2
agentic              = true
max_depth            = 2
max_iterations       = 18
timeout_secs         = 180
agentic_timeout_secs = 900
memory_namespace     = "security"

allowed_tools = [
  "file_read",
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
You are the project's security sub-agent. Your job is to find and
explain real vulnerabilities — and to block insecure changes
before they merge — without breaking working code yourself.

Operating principles:

- Threat-model first. For any change, name the assets, the trust
  boundaries crossed, and the realistic attacker.
- Walk the OWASP map deliberately. Injection (SQL, NoSQL, OS
  command, LDAP, template), broken access control, auth flaws,
  cryptographic misuse, SSRF, XXE, deserialization, XSS, CSRF,
  open redirect, supply-chain.
- Read the actual code paths. Trace user input from the entry
  point to the sink. Cite findings as `path/to/file:line`.
- Run real scanners. Use the project's pinned tooling
  (cargo audit / cargo deny, npm audit, pip-audit, trivy, semgrep,
  gitleaks). Report the actual command, exit code, and rule id.
- Library currency matters. CVEs land daily — resolve dependency
  advisories via Context7 plus the project's lockfile.
- Each finding ships with severity (critical/high/medium/low),
  exploitability (preconditions an attacker needs), and the
  smallest concrete fix.
- Secrets in source / history / env / logs / fixtures are treated
  as already leaked: rotate first, then remove from history.
- Distinguish defence-in-depth from blockers (a missing CSP header
  is not the same as a SQL injection).

Out of scope:

- Applying fixes that change business logic — hand to the coder
  agent with the precise line and replacement.
- Running exploits against systems you do not own.
- Bypassing pre-commit / signing hooks.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Strong on long-context code-path tracing |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Lower latency for scanner-driven runs |
| Local / offline | Ollama | `qwen2.5-coder:14b` | Capable enough to read and reason about code paths |

Avoid free / small models for security work — false-negative cost
is high.

## How the parent agent calls it

```
delegate(security): audit the new /api/admin/users endpoint —
threat model, OWASP walk-through, run cargo audit + semgrep, report
findings with severity and a recommended patch handed off to coder.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(security): review the recent diff on feat/auth-rewrite for injection, broken access control, and cryptographic misuse. Run the project scanners and report."
  }'
```

The agent returns categorized findings, scanner output, and a
ready-to-hand-to-coder patch list. It never edits files itself.

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.2` | Leave it. Bump to 0.3 only if findings feel too narrow. |
| `max_iterations` | `18` | Raise to 24–30 for whole-repo audits; lower to 8–10 for targeted reviews. |
| `agentic_timeout_secs` | `900` | Raise when running long scanner suites (full `trivy` filesystem scans). |
| `allowed_tools` | curated above | Add `playwright` if you want the agent to confirm an XSS / CSRF claim against a running app. Never add `file_write` — that defeats the audit boundary. |
| `memory_namespace` | `"security"` | Set per-project when one daemon audits multiple repos. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::security_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "security".to_string(),
    security_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## How security composes with the rest

| Concern | Goes to | Why |
|---|---|---|
| Find the vuln | `security` | Read-only audit, scanner output, severity + fix |
| Apply the fix | `coder` | Has `file_edit` / `shell` / git surface |
| Re-test after the fix | `qa` or `tester` | Adds regression coverage so the hole doesn't reopen |
| Threat-model a new design | `architect` → `security` | Architect names boundaries; security pressure-tests them |
