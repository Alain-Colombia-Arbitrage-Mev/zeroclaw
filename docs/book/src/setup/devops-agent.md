# DevOps sub-agent

The `devops` preset is a senior site-reliability / platform
engineer delegate. It owns infrastructure-as-code, container and
Kubernetes work, observability wiring, and deployment safety. It is
broader than the [`cicd` preset](./cicd-agent.md), which is scoped
to the pipeline itself.

The preset is exposed in code at
`zeroclaw_config::agent_presets::devops_preset(provider, model)`.

## What you get

A sub-agent that:

- Runs **agentic** with up to 20 iterations per delegation.
- Uses a **low temperature** (0.3) — infra changes should be
  predictable, not creative.
- Has the **full filesystem + git + shell surface** so it can
  draft Terraform / Helm / Kustomize / Ansible / Nix changes,
  run `terraform plan`, `kubectl --dry-run`, `helm diff`, and
  iterate.
- Pins everything that can move (image digests, chart versions,
  provider versions, GitHub Actions SHAs).
- Treats observability (logs, metrics, traces, SLO + alert) as
  a deliverable, not a follow-up.
- Uses an **isolated memory namespace** (`devops`) so prior
  infra context (cluster names, env conventions, drift findings)
  persists across sessions.

## Drop-in TOML

```toml
[agents.devops]
provider             = "openrouter"
model                = "anthropic/claude-sonnet-4"
temperature          = 0.3
agentic              = true
max_depth            = 2
max_iterations       = 20
timeout_secs         = 180
agentic_timeout_secs = 900
memory_namespace     = "devops"

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
You are the project's DevOps / platform sub-agent. Your job is to
keep infrastructure, deployments, and operational tooling reliable,
reproducible, and observable — without taking destructive actions
on shared environments.

Operating principles:

- Infrastructure-as-code, always. Every change shows up as a diff
  in Terraform / Pulumi / Helm / Kustomize / Ansible / Nix.
- Idempotency is the contract. Running the same playbook twice in
  a row produces zero changes.
- Pin everything that can move (image digests, chart versions,
  provider versions, GitHub Actions SHAs).
- Twelve-factor by default — config from env, secrets from the
  secret manager (never the repo), logs to stdout.
- Observability is a deliverable: structured logs, golden-signal
  metrics, traces, at least one SLO + alert.
- Blast radius before action: name what this affects, the rollback,
  who pages, can it be flagged or canaried? Run `terraform plan` /
  `kubectl --dry-run` / `helm diff` and report output before apply.
- Use the project's tooling — `dev/`, `Justfile`, `scripts/`,
  `.github/workflows/`, `deploy-k8s/` — match conventions found.
- Use Context7 to confirm cloud-SDK flags, k8s resource shapes,
  and chart values before committing.
"""
```

## Choosing a model

| Goal | Provider | Model | Why |
|---|---|---|---|
| Best quality, paid | OpenRouter | `anthropic/claude-sonnet-4` | Long context for whole-stack reviews; precise on YAML/HCL |
| Fast + capable, paid | OpenRouter | `openai/gpt-4o` | Lower latency on iterative `plan` / `diff` runs |
| Local / offline | Ollama | `qwen2.5-coder:14b` | Adequate for HCL / Helm chart edits with the project's existing files for grounding |

## How the parent agent calls it

```
delegate(devops): split the existing monolithic Helm chart in
deploy-k8s/ into chart-per-service, pin all images by digest,
add SLO + alert for the api service, run helm diff and report.
```

Or via webhook:

```bash
curl -X POST http://localhost:42617/webhook \
  -H "Authorization: Bearer $ZEROCLAW_BEARER" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "delegate(devops): audit deploy-k8s/ for floating image tags, missing resource limits, and missing readiness probes. Propose fixes and run helm diff."
  }'
```

## Tuning knobs

| Field | Default | When to change |
|---|---|---|
| `temperature` | `0.3` | Leave it. Bump to 0.4 only when the agent gets stuck on the same wrong shape on retries. |
| `max_iterations` | `20` | Raise to 30+ for multi-cluster / multi-env migrations; lower to 10–12 for targeted edits. |
| `agentic_timeout_secs` | `900` | Raise when `terraform plan` against large states is slow. |
| `allowed_tools` | curated above | Add cloud-vendor MCP tools (e.g. `aws_*`, `gcp_*`) if you've registered those servers. Drop `shell` for read-only drift audits. |
| `memory_namespace` | `"devops"` | Set per-environment (`devops-prod`, `devops-staging`) when one daemon serves multiple clusters. |

## Programmatic registration

```rust
use zeroclaw_config::agent_presets::devops_preset;

let mut config = load_my_config()?;
config.agents.insert(
    "devops".to_string(),
    devops_preset("openrouter", "anthropic/claude-sonnet-4"),
);
```

## DevOps vs CICD

| Concern | Goes to |
|---|---|
| Pipeline YAML, action pinning, build/test/deploy stages | `cicd` |
| Infra outside the pipeline (charts, modules, manifests, observability, secrets manager) | `devops` |
| New service rollout (chart + pipeline + alert) | `devops` (chart + alerting) → `cicd` (pipeline) |
| Drift between repo IaC and live infra | `devops` |

The two presets share `shell` + git, so both can run scanners and
plans, but their scopes don't overlap. Route based on whether the
artefact lives inside `.github/workflows/` (cicd) or anywhere else
infra-related (devops).
