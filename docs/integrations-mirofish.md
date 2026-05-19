# MiroFish — pre-launch market sentiment

[MiroFish](https://github.com/666ghj/MiroFish) is an external
multi-agent simulation engine that builds a digital parallel world
from seed materials, spawns thousands of persona-rich agents with
long-term memory, and lets them socially evolve around a proposed
event. Use it to predict public reaction to a launch, a policy, or
a press release **before** it ships.

ZeroClaw integrates with it via the `market_sentiment_analyst` agent
preset, which orchestrates MiroFish over its HTTP API and synthesises
the simulation output into a launch decision with audit receipts.

---

## 1. What you get

A new sub-agent in the bench that:

1. Assembles a seed dossier from the tenant's materials (landing
   copy, comparable launches, target-market context, known risks).
2. Calls MiroFish's HTTP API to build a graph, run the simulation,
   and generate a report.
3. Runs **three variant seed sets** (baseline / failure-mode
   amplified / competitor counter-move) so single-shot noise is
   visible as variance rather than mistaken for signal.
4. Captures the three asset IDs every simulation produces
   (`graph_id`, `simulation_id`, `report_id`) so another analyst
   can replay the work.
5. Triangulates against `idea_validator`, `red_teamer`,
   `forensic_auditor`, and `market_researcher` before recommending
   go / hold / iterate / kill.

It does **not** make the launch decision — it produces the evidence
file the operator decides against.

---

## 2. Architecture

```
   ┌──────────────────────────────┐
   │  ZeroClaw daemon             │
   │  market_sentiment_analyst    │
   │  └─ uses tool: http_request  │
   └─────────────┬────────────────┘
                 │  HTTP (JSON)
                 ▼
   ┌──────────────────────────────┐
   │  MiroFish backend  :5001     │
   │  ├─ /api/graph/build         │
   │  ├─ /api/simulation/run      │
   │  ├─ /api/simulation/entities │
   │  ├─ /api/report/generate     │
   │  └─ /api/report/{id}         │
   │                              │
   │  Calls upstream:             │
   │  ├─ LLM API (Qwen / OpenAI / │
   │  │   any OpenAI-compat URL)  │
   │  └─ Zep Cloud (long memory)  │
   └──────────────────────────────┘
```

MiroFish runs as a **sidecar service**. Same host, different
process. ZeroClaw doesn't fork or embed it; the integration is a
pure HTTP call.

---

## 3. Hosting MiroFish alongside ZeroClaw

### 3.1 Same VPS (recommended for solo operators)

If your `cax21` Hetzner box has 8 GB RAM, MiroFish + ZeroClaw fit
comfortably. The backend is Python 3.11+ with `uv`; the frontend is
Node.js. Both ship as Docker images so the cleanest install is
docker-compose.

Create `/opt/mirofish/`:

```bash
sudo adduser --system --group --home /var/lib/mirofish --shell /bin/bash mirofish
sudo mkdir -p /opt/mirofish && cd /opt/mirofish
sudo -u mirofish git clone https://github.com/666ghj/MiroFish.git src
cd src
sudo -u mirofish cp .env.example .env
sudo -u mirofish $EDITOR .env     # see §3.3 for required vars
sudo -u mirofish docker compose up -d
```

Maps:

- `:3000` — MiroFish frontend (you can either expose it publicly via
  Caddy on a sub-path, or keep it loopback-only and only let the
  agent talk to the API).
- `:5001` — MiroFish backend API (this is the surface ZeroClaw
  hits).

Lock the `:5001` port to loopback in MiroFish's `docker-compose.yml`
so it's not reachable from outside the box:

```yaml
services:
  backend:
    ports:
      - "127.0.0.1:5001:5001"   # was: "5001:5001"
  frontend:
    ports:
      - "127.0.0.1:3000:3000"
```

If you want the MiroFish UI on the public web too, add a Caddy
block on a sub-domain (`mirofish.example.com`) or sub-path
(`octopus.example.com/mirofish/`). The ZeroClaw agent only needs
the API port, not the UI.

### 3.2 Separate box

If you outgrow the shared host (or want clean cost accounting),
move MiroFish to a second Hetzner box and point ZeroClaw at it via
a private network address. Both boxes in the same Hetzner Cloud
network (`hcloud network create --name zc-private --ip-range
10.0.0.0/16`) gives you free internal traffic and a stable internal
IP for the API URL.

### 3.3 Required env vars

MiroFish's `.env` (copy from `.env.example`):

```env
# OpenAI-compatible LLM endpoint. Any provider works:
#  - Alibaba Bailian (Qwen): high quality + cheaper than GPT-4
#  - OpenRouter: same key as ZeroClaw, model selectable
#  - Ollama: local, near-zero cost, lower quality
LLM_API_KEY=sk-...
LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
LLM_MODEL_NAME=qwen-plus

# Zep Cloud for long-term agent memory. Free tier is enough for
# small simulations (<50 agents). Paid only if you scale persona
# count.
ZEP_API_KEY=z_...
```

If you want to keep credentials inside ZeroClaw's secret store
instead of the MiroFish container, mount them as Docker secrets
or use systemd `EnvironmentFile=` and reference them at compose
time.

### 3.4 Wire ZeroClaw to MiroFish

Tell the agent where MiroFish lives. Two options:

**Option A — agent picks it up from a known env var (recommended):**

In `/etc/systemd/system/zeroclaw.service`:

```ini
[Service]
Environment=MIROFISH_BASE_URL=http://127.0.0.1:5001
```

Then `systemctl restart zeroclaw`. The agent's prompt already
defaults to that URL when nothing is supplied; the env var lets you
override per-environment without editing the prompt.

**Option B — pass it explicitly in every delegation:**

When invoking the agent through the orchestrator, include the URL
in the task: "Run a sentiment simulation against MiroFish at
http://10.0.0.5:5001 for this launch material…". More verbose, but
useful when you have multiple MiroFish instances per market.

---

## 4. Cost model

MiroFish's variable cost is dominated by LLM calls. Rough numbers
on a Qwen-plus run at default settings:

| Run size | Agents | Rounds | LLM tokens | Wall time | Cost |
|---|---|---|---|---|---|
| Smoke test | 50 | 10 | ~150 k | 3-5 min | ~$0.30 |
| First-pass | 500 | 30 | ~3 M | 30-60 min | ~$6 |
| Production | 2000 | 60 | ~25 M | 4-8 h | ~$50 |
| Stress run | 5000 | 100 | ~100 M | 24+ h | ~$200 |

The agent preset defaults to **<40 rounds + <500 agents on the first
pass** specifically to keep this bounded. Scale only after the small
run shows the persona mix is right.

Three variant seeds × first-pass = ~$18 for a complete
pre-launch sentiment read with variance bracketing. That's
order-of-magnitude cheaper than the alternative (a 4-week beta
test program), with weeks shaved off the timeline.

Zep Cloud free tier covers ~10 simulations/month at small scale.
Above that, the paid plan starts at $50/mo.

---

## 5. When to use it (and when not)

**Use it when:**

- You're about to ship a launch with high reputational stakes
  (founding-team-public reaction, regulatory framing, sensitive
  segment).
- You have at least 3-5 comparable launches with public reaction
  data to anchor the persona mix.
- The launch is in a market segment where social signal moves
  outcomes (consumer, B2C, founder-led narrative, policy).
- You want a structured pre-mortem against simulated public
  reaction, not just a `red_teamer` paper exercise.

**Skip it when:**

- The launch is purely B2B procurement and the buying centre is
  3 people you can just call.
- The seed dossier would be entirely speculative — there are no
  comparable launches and no real customer interviews.
- You haven't run `idea_validator` or `customer_researcher` yet.
  Sentiment simulation is downstream of those, not a substitute.
- The cost of being wrong is recoverable (cheap pivots, test-and-
  iterate channels). A live A/B is cheaper than a 3-variant
  simulation here.

---

## 6. Failure modes

**Garbage seed → garbage reaction.** MiroFish faithfully simulates
whatever persona mix you seed. If you feed only Hacker News
threads, you get a Hacker News simulation, not a market simulation.
The preset's "persona-mix audit" step is the gate that catches this.

**LLM model bias.** The simulated personas are generated by the
LLM behind MiroFish. A model trained heavily on US English data
will reproduce US English consensus regardless of the locale you
asked for. Run with a locale-appropriate model when target market
is Spanish-speaking (Qwen-plus handles zh/en/es well; some Western
models are weaker at non-English personas).

**Single-seed false confidence.** A 78% positive single-seed run
is 78% positive in *that* draw of the noisy distribution. The
preset enforces 3 variant seeds explicitly; if you skip that step
you're rolling a single die.

**Cost runaway.** Default to the smoke-test settings until persona
mix is verified. A 5000-agent / 100-round run can blow $200 before
you notice if you set it as the first attempt.

---

## 7. Verifying the integration

After both services are up:

```bash
# 1. MiroFish reachable from the daemon's view
ssh deploy@your-vps "curl -fsS http://127.0.0.1:5001/api/graph/project/list"

# 2. Trigger the agent (assuming you have a paired bearer token):
curl -X POST https://octopus.example.com/webhook \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"message": "delegate to market_sentiment_analyst: predict reception of a $499 home solar monitor pre-order launch in California, mainstream consumer segment, comparable launches Tesla Powerwall waitlist 2015 and Span Panel 2021, target locale en-US"}'
```

If MiroFish isn't running, the agent will surface an actionable
error: "MiroFish not running on http://127.0.0.1:5001. Start it
with `docker compose up -d` from the MiroFish repository."

---

## 8. Ops checklist

- [ ] MiroFish containers running, `docker compose ps` shows healthy
- [ ] Backend port `:5001` bound to loopback only on the host
- [ ] `LLM_API_KEY` and `ZEP_API_KEY` set, neither committed
- [ ] `MIROFISH_BASE_URL` set in ZeroClaw's systemd unit
- [ ] First smoke test run completed — agent returns the three
      receipts (graph_id, simulation_id, report_id)
- [ ] Backups include MiroFish's volume mounts (graph data,
      simulation history)
- [ ] Cost-cap on the upstream LLM provider set so a runaway
      simulation can't bankrupt you

---

## Activating Zep for persistent personas

MiroFish ships in two modes:

1. **Stateless personas** (default in the bare repo). Each round
   the simulated agents have no memory of prior rounds. Cheap
   but produces shallower sentiment dynamics — the same agent
   can flip-flop on consecutive rounds without anyone noticing.

2. **Persistent personas via Zep**. Each agent's biography and
   prior reactions persist across rounds and across simulations.
   The sentiment-evolution curves track closer to real-world
   launches, and contrarian voices stay contrarian rather than
   regressing to mean. Cost is one Postgres container + one Zep
   container (each ~200 MB RAM at idle).

### How to activate (`deploy/mirofish-stack/`)

Zep is **already wired** in the bundled docker-compose. You only
need to supply credentials in `.env`:

```bash
openssl rand -hex 32     # → ZEP_AUTH_SECRET
openssl rand -hex 32     # → ZEP_API_KEY (must equal ZEP_AUTH_SECRET in this config)
openssl rand -hex 16     # → POSTGRES_ZEP_PASSWORD
```

Set the values in `.env`. `docker compose up -d` brings up `zep`
and `postgres-zep` next to the MiroFish backend. The backend
receives `ZEP_API_URL=http://zep:8000` and `ZEP_API_KEY` via the
compose environment — no code change needed.

### Verify Zep is actually being used

```bash
# Zep healthy:
docker compose exec zep curl -s http://localhost:8000/healthz
# expected: {"status":"ok"}

# After a 50-agent smoke test, Zep should have 50 sessions:
docker compose exec zep curl -s \
    -H "Authorization: Bearer ${ZEP_API_KEY}" \
    http://localhost:8000/api/v1/sessions | jq '.sessions | length'
# 0 = MiroFish is silently running stateless. Check logs:
docker compose logs mirofish-backend | grep -i zep
```

### What Zep stores per agent

- **User record** — biography + persona traits from seed dossier.
- **Session messages** — reactions across rounds; round N sees
  round N-1.
- **Summary** — periodic compression so context doesn't blow up
  at round 50.

Retention defaults to 90 days (`ZEP_RETENTION_DAYS` env). Bump
for audit-grade retention or back up the `postgres-zep-data`
volume.

### Cost impact

- Memory: ~400 MB extra (Postgres + Zep). Fits 4 GB host.
- LLM tokens: embeddings via OpenRouter `text-embedding-3-small`
  at $0.02/1M. A 500×30 simulation ≈ 150K tokens ≈ $0.003.
  Negligible.
- Disk: ~1 MB per agent-simulation. 100 sims × 500 agents = ~50
  GB. Plan accordingly.

### When NOT to use Zep

- Smoke tests where persona realism doesn't matter. Drop the
  `zep` + `postgres-zep` services to save 400 MB RAM.
- Stateless A/B simulations where rounds SHOULD reset.

---

## Forensic generation logging

The `[mirofish.logging]` config block lets the daemon record every
HTTP call to MiroFish in addition to the three asset receipts.

```toml
[mirofish.logging]
enabled = true
generation_log_dir = "mirofish-logs"      # relative to workspace
response_truncate_bytes = 5_242_880       # 5 MiB cap per response
retention_days = 90                       # cleanup cron threshold
```

When enabled, the `market_sentiment_analyst` preset writes one
JSON file per HTTP call under
`<workspace>/<generation_log_dir>/<YYYY-MM-DD>/<simulation_id>/`:

| File | Content |
|------|---------|
| `00-health.json` | Health-check response |
| `01-build-request.json` + `01-build-response.json` | Graph build |
| `02-entities-request.json` + `02-entities-response.json` | Persona audit |
| `03-run-request.json` + `03-run-response.json` | Simulation kickoff |
| `04-generate-request.json` + `04-generate-response.json` | Report request |
| `05-poll-NNN-status.json` | One per polling iteration |
| `06-report.json` | Final report |

Pretty-printed, 2-space indent. No API keys in any body — the
preset prompt enforces this explicitly.

`promtail-config.yaml` in `deploy/mirofish-stack/` auto-tails
this directory into Loki with labels `service=market_sentiment_
analyst`, `sim_id=`, `stage=`. In Grafana:

```
{service="market_sentiment_analyst", sim_id="<id>"} | line_format "{{.stage}}"
```

gives the full transcript chronologically.

### When to enable

- Post-launch retros — replay against ZeroClaw's trail without
  needing MiroFish.
- Disputed predictions — ground-truth of what the model saw vs
  what the operator interpreted.
- Regulated tenants requiring full transcripts.

### When NOT to enable

- High-volume tenants. 500×30×3 ≈ 50-200 MB per simulation.
  10/day = 1-2 GB/day. Size disk for it or leave off.
- Smoke tests at 50×10 produce ~5-10 MB; off is reasonable.

### Cleanup

Files older than `retention_days` are deleted by the daemon's
cleanup pass (nightly). Pure filesystem, no DB.

---

## See also

- MiroFish repo — [github.com/666ghj/MiroFish](https://github.com/666ghj/MiroFish)
- OASIS (the engine MiroFish wraps) — [github.com/camel-ai/oasis](https://github.com/camel-ai/oasis)
- `crates/zeroclaw-config/src/agent_presets/market_sentiment_analyst.rs`
  — the preset
- `docs/hosting-hetzner.md` — VPS the MiroFish sidecar
- `deploy/mirofish-stack/` — single-host docker-compose
