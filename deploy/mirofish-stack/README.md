# MiroFish + ZeroClaw + Observability — single-host stack

This directory holds the docker-compose stack that brings up the
full sentiment-simulation pipeline (MiroFish + Zep) alongside the
ZeroClaw daemon and an audit-grade observability stack (OTel
collector → Loki → Grafana).

Designed for a Hetzner `cax21` (4 vCPU ARM, 8 GB RAM) or
equivalent. Smaller hosts (`cax11`, 2 vCPU / 4 GB) work if you
drop Grafana and shrink Loki retention to 7 days.

## Quick start

```bash
cd deploy/mirofish-stack/
cp .env.example .env
$EDITOR .env       # fill OPENROUTER_API_KEY at minimum
docker compose up -d

# Verify everything came up:
docker compose ps
curl -s localhost:5001/api/graph/project/list | jq .   # MiroFish
curl -s localhost:42617/health                          # ZeroClaw
curl -s localhost:3100/ready                            # Loki
curl -s localhost:3000/api/health                       # Grafana
```

Then point a browser at:

- **MiroFish UI**: `http://localhost:8002` (inspect graph builds, runs, reports)
- **Grafana**: `http://localhost:3000` (admin / `${GRAFANA_ADMIN_PASSWORD}`)
- **ZeroClaw**: `http://localhost:42617` (paired — pair via Tauri or set `ZEROCLAW_AUTO_PAIR_TOKEN`)

## What runs and why

| Service | Port | Purpose |
|---------|------|---------|
| `mirofish-backend` | 5001 | The simulation engine ZeroClaw calls over HTTP |
| `mirofish-frontend` | 8002 | Web UI to inspect graph builds, simulations, reports |
| `zep` | 8000 | Long-term memory for MiroFish personas (gives agents biography) |
| `postgres-zep` | 5432 | Zep's database (internal only) |
| `zeroclaw` | 42617 | The daemon — orchestrator + agents + memory |
| `falkordb` | 6379 | Knowledge graph for ZeroClaw memory (Redis-compatible) |
| `otel-collector` | 4317/4318 | OTLP ingest, fans out to Loki + stdout |
| `loki` | 3100 | Stores audit logs with 30-day retention |
| `promtail` | — | Sidecar tailing container stdout + audit JSONL → Loki |
| `grafana` | 3000 | Dashboards over Loki logs, OTel traces |

All inter-service traffic stays on the `mirofish_internal` Docker
network. Only the public-facing services bind to the host
(`mirofish-frontend`, `zeroclaw`, `grafana`, `mirofish-backend`).
For production, put a reverse proxy (Caddy / nginx) in front and
remove the host-binding from the rest.

## Zep activation

MiroFish supports two modes:

1. **Stateless personas** (default in the bare repo): each round
   the agents have no memory of prior rounds. Cheap but produces
   shallower sentiment dynamics.
2. **Persistent personas via Zep**: each agent's biography and
   reactions persist across rounds and across simulations. The
   sentiment evolution feels more realistic, the spread / damping
   curves track better against real-world launches, and the cost
   is one Postgres + one Zep container.

This stack defaults to Zep ON. The `mirofish-backend` container
receives `ZEP_API_URL=http://zep:8000` and `ZEP_API_KEY` (the
matching secret from `.env`). If MiroFish's backend doesn't see a
working Zep endpoint at boot, it falls back to stateless mode
silently — check `docker compose logs mirofish-backend` for a line
like `zep connection established` to verify.

To run MiroFish stateless (cheaper, drops Zep):

```bash
# Comment out the `zep` and `postgres-zep` services in
# docker-compose.yml AND remove the ZEP_* env vars on
# mirofish-backend. Then:
docker compose up -d mirofish-backend mirofish-frontend zeroclaw \
    falkordb otel-collector loki promtail grafana
```

## Forensic generation logging

When `[mirofish.logging] enabled = true` in `config/config.toml`,
the `market_sentiment_analyst` preset writes one JSON file per
HTTP call to `<workspace>/mirofish-logs/<date>/<simulation_id>/`.
Files:

- `00-health.json` — initial health-check
- `01-build-request.json` / `01-build-response.json`
- `02-entities-request.json` / `02-entities-response.json`
- `03-run-request.json` / `03-run-response.json`
- `04-generate-request.json` / `04-generate-response.json`
- `05-poll-NNN-status.json` — one per poll
- `06-report.json` — final report body

Promtail tails this directory automatically and ships it to Loki
with labels `service=market_sentiment_analyst`, `sim_id=`,
`stage=`. In Grafana:

```
{service="market_sentiment_analyst", sim_id="<id>"} | line_format "{{.stage}}"
```

gives you the full transcript of a simulation in chronological
order.

## Cost reality

Per simulation (3 seed variants × 50 agents × 10 rounds, smoke-
test defaults, on DeepSeek via OpenRouter):

| Item | Cost (USD) |
|------|------------|
| OpenRouter token spend | 0.60 – 1.80 |
| Hetzner cax21 monthly | 7.50 |
| OpenRouter monthly minimum credit | 5.00 |
| **Per-simulation upper bound** | **~$2.00** |
| **Monthly fixed (host + OpenRouter floor)** | **~$12.50** |

Production-scale runs (500 agents × 30 rounds × 3 variants) cost
$10–30 per simulation depending on the model. Use frugal defaults
for smoke tests, scale only after the persona-mix audit confirms
the seed dossier is right.

## Backup

The `loki-data`, `falkordb-data`, `postgres-zep-data`,
`zeroclaw-workspace`, and `grafana-data` volumes are the state.
Hetzner Storage Box is the cheapest off-host backup target —
~€3.20/mo for 1 TB. Cron a `restic` job:

```cron
0 4 * * *   restic -r sftp:u123@u123.your-storagebox.de:./mirofish-stack backup \
              /var/lib/docker/volumes
```

## Updates

The `:latest` tags pin to the upstream's mainline. For production,
swap to explicit `:vX.Y.Z` tags after smoke-testing each upgrade.
The MiroFish project (`github.com/666ghj/MiroFish`) is pre-1.0 —
expect breaking API changes; pin and test before rolling forward.

## Troubleshooting

- **MiroFish backend won't come up**: check `docker compose logs
  mirofish-backend`. Most common: `OPENROUTER_API_KEY` is unset
  or wrong, or OpenRouter has zero credit balance.
- **Zep healthcheck failing**: `docker compose logs zep`. If
  `POSTGRES_ZEP_PASSWORD` was changed after the volume was
  initialized, the postgres user won't auth. Wipe the volume:
  `docker compose down -v` and bring back up.
- **ZeroClaw can't reach MiroFish**: confirm both are on
  `mirofish_internal` network. From inside the `zeroclaw`
  container: `docker compose exec zeroclaw curl
  http://mirofish-backend:5001/api/graph/project/list`. If it
  works inside but not from the host, that's expected — host only
  sees mirofish via `127.0.0.1:5001`, ZeroClaw uses internal DNS.
- **Loki rejects logs as too old**: clock drift between host and
  containers. `docker compose restart` usually fixes it; for
  persistent drift install `chrony` on the host.
