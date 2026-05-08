# Self-hosting on Fly.io (serverless)

Fly.io is the easiest path to a public Octopus Labs instance: one
`fly launch`, persistent volumes for state, and TLS / global edge
included. No VPS to maintain.

This guide covers a single-region deployment with Qdrant and FalkorDB
running as separate Fly machines on the same internal `.flycast`
network.

> **Heads up:** Fly's free tier is gone (Sep 2024). Budget ~US$8–15/mo
> for a 1-CPU shared / 1 GB Octopus machine + small Qdrant/FalkorDB
> sidecars. Compute scales to zero when idle if you set
> `auto_stop_machines = "stop"`.

## Prerequisites

- A [Fly.io account](https://fly.io/) with billing on file.
- `flyctl` installed: `curl -L https://fly.io/install.sh | sh` (or `iwr -useb fly.io/install.ps1 | iex` on Windows).
- An LLM provider API key.

## 1. Three apps, one organisation

We deploy three Fly apps so each can scale and reboot independently:

| App | Purpose | Memory |
|---|---|---|
| `octopus-app` | Gateway + daemon | 1 GB (scale up if you onboard SOPs) |
| `octopus-qdrant` | Vector memory | 512 MB |
| `octopus-falkordb` | Knowledge graph | 512 MB |

```bash
flyctl auth login
flyctl orgs create octopus    # or use an existing org
```

## 2. Qdrant

```bash
mkdir -p deploy/fly/qdrant && cd deploy/fly/qdrant
flyctl launch --name octopus-qdrant --org octopus --no-deploy --image qdrant/qdrant:latest
```

`fly.toml`:

```toml
app = "octopus-qdrant"
primary_region = "iad"

[build]
  image = "qdrant/qdrant:latest"

[[mounts]]
  source = "qdrant_data"
  destination = "/qdrant/storage"

[[services]]
  internal_port = 6333
  protocol = "tcp"
  auto_stop_machines = "off"   # always-on so the daemon never sees ECONNREFUSED
  auto_start_machines = true
  min_machines_running = 1

  [[services.ports]]
    port = 6333

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory = "512mb"
```

```bash
flyctl volumes create qdrant_data --region iad --size 3
flyctl deploy
```

Internal address (no public IP needed): `octopus-qdrant.flycast:6333`.

## 3. FalkorDB

```bash
cd ../.. && mkdir -p deploy/fly/falkordb && cd deploy/fly/falkordb
flyctl launch --name octopus-falkordb --org octopus --no-deploy --image falkordb/falkordb:latest
```

`fly.toml`:

```toml
app = "octopus-falkordb"
primary_region = "iad"

[build]
  image = "falkordb/falkordb:latest"

[env]
  FALKORDB_ARGS = "MAX_QUEUED_QUERIES 50"

[[mounts]]
  source = "falkordb_data"
  destination = "/data"

[[services]]
  internal_port = 6379
  protocol = "tcp"
  auto_stop_machines = "off"
  auto_start_machines = true
  min_machines_running = 1

  [[services.ports]]
    port = 6379

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory = "512mb"
```

```bash
flyctl volumes create falkordb_data --region iad --size 1
flyctl deploy
```

Internal address: `octopus-falkordb.flycast:6379`.

## 4. The Octopus app

```bash
cd ../.. && mkdir -p deploy/fly/app && cd deploy/fly/app
```

`fly.toml`:

```toml
app = "octopus-app"
primary_region = "iad"

[build]
  image = "ghcr.io/zeroclaw-labs/zeroclaw:debian"

[env]
  ZEROCLAW_ALLOW_PUBLIC_BIND = "true"
  ZEROCLAW_GATEWAY_PORT = "42617"
  ZEROCLAW_PAIRING_REQUIRED = "true"
  PROVIDER = "openrouter"

[[mounts]]
  source = "octopus_state"
  destination = "/root/.zeroclaw"

[http_service]
  internal_port = 42617
  force_https = true
  auto_stop_machines = "stop"   # scale to zero when idle
  auto_start_machines = true
  min_machines_running = 0

  [http_service.concurrency]
    type = "connections"
    soft_limit = 50
    hard_limit = 80

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory = "1gb"
```

`Dockerfile.fly` (optional — only needed if you want to bake the
config in; usually we mount it via the volume on first run):

```dockerfile
FROM ghcr.io/zeroclaw-labs/zeroclaw:debian
COPY config.seed.toml /root/.zeroclaw/config.seed.toml
```

### Set secrets

```bash
flyctl secrets set \
  API_KEY=sk-or-v1-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx \
  OPENROUTER_API_KEY=sk-or-v1-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

### Volume + first deploy

```bash
flyctl volumes create octopus_state --region iad --size 2
flyctl deploy
```

### Seed the config

SSH into the machine and edit:

```bash
flyctl ssh console
zeroclaw onboard      # walks you through provider/model
nano /root/.zeroclaw/config.toml
```

Point memory and knowledge at the sidecar machines:

```toml
[memory.qdrant]
url = "http://octopus-qdrant.flycast:6333"
collection = "octopus_memories"

[knowledge]
enabled = true
backend = "falkordb"
auto_capture = true

[knowledge.falkordb]
url = "redis://octopus-falkordb.flycast:6379"
graph = "octopus_kg"
```

Restart the machine: `flyctl machine restart <id>` (or
`flyctl deploy --strategy immediate`).

## 5. Custom domain + TLS

```bash
flyctl certs create octopus.example.com
# Add the AAAA + A records Fly prints, then:
flyctl certs check octopus.example.com
```

After the cert flips to `Verified`, your dashboard is at
`https://octopus.example.com/`.

## 6. Pair from the dashboard

```bash
flyctl ssh console -C "zeroclaw gateway get-paircode --new"
```

Paste into the pairing screen. The bearer token in localStorage will
persist — only re-pair when you wipe browser state.

## 7. Scale up / down

```bash
# Vertical
flyctl scale memory 2048 -a octopus-app

# Horizontal — for read-heavy dashboards, run the gateway in 2 regions
flyctl machine clone <machine-id> --region fra
```

Note: scaling the daemon horizontally means each instance has its own
runtime state. Use this only for the gateway/dashboard layer; pin the
agent loop to a single region until we ship the multi-leader consensus
work.

## 8. Costs (rough)

| Component | Tier | Monthly |
|---|---|---|
| `octopus-app` | shared 1× / 1 GB, scale-to-zero | $3–8 |
| `octopus-qdrant` | shared 1× / 512 MB, always-on | $2–4 |
| `octopus-falkordb` | shared 1× / 512 MB, always-on | $2–4 |
| Volumes (6 GB) | persistent storage | $0.90 |
| Total | | **~$8–17/mo** |

## Troubleshooting

| Symptom | Fix |
|---|---|
| `flyctl deploy` exits with `image not found` | Pull manually first: `docker pull ghcr.io/zeroclaw-labs/zeroclaw:debian` |
| Daemon can't reach Qdrant | Use `*.flycast`, not `*.fly.dev`. The latter is the public edge. |
| `dial tcp: missing port in address` | URL must include the port (`:6333` for Qdrant, `:6379` for FalkorDB) |
| Cold start is slow | Lift `min_machines_running = 1` on `octopus-app` — costs ~$3/mo more |
| Cert stuck in `Awaiting configuration` | DNS not propagated; `dig +short octopus.example.com` should resolve to `fly.io` |
