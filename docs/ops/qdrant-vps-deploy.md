# Qdrant on a Hetzner VPS (multi-tenant memory backend)

Step-by-step to run Qdrant as a separate service alongside the ZeroClaw
daemon on the same Hetzner box. Same procedure applies to any Linux VPS
(DigitalOcean, OVH, AWS Lightsail, bare metal Ubuntu/Debian).

## Why a separate service?

- The daemon and the vector store have different blast radii: a Rust
  panic shouldn't take the memory layer with it, and a Qdrant OOM
  shouldn't reboot every agent loop.
- Multi-tenant isolation lives in payload filters today (`tenant_id`
  field, see `crates/zeroclaw-memory/src/qdrant.rs`). Running Qdrant on
  its own port lets you scale it horizontally later without touching
  the daemon — point the daemon at a remote Qdrant cluster instead of
  localhost when the active tenants outgrow one box.
- Backups, restarts, and version upgrades happen independently.

## Sizing guidance

| Hetzner SKU | vCPU / RAM | What it fits |
|---|---|---|
| CX22 | 2 / 4 GB | Single tenant, <50k chunks |
| CX32 | 2 / 8 GB | 2–5 tenants, ~250k chunks |
| CPX31 | 4 / 8 GB AMD | 5–15 tenants, ~1M chunks, hybrid BM25 + dense |
| CPX41 | 8 / 16 GB AMD | Production multi-tenant, regular snapshots, room for re-index |

Vectors at 1536 dimensions (OpenAI text-embedding-3-small) cost
~6 KB on disk plus payload. RAM is the constraint, not CPU.

## 0. Prereqs on a fresh Ubuntu 24.04 Hetzner box

```bash
sudo apt update && sudo apt install -y docker.io docker-compose-plugin curl ufw fail2ban
sudo systemctl enable --now docker
sudo usermod -aG docker $USER
newgrp docker

# Lock the box down BEFORE exposing services.
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 22/tcp     # ssh
sudo ufw allow 42617/tcp  # zeroclaw gateway (or behind reverse proxy)
sudo ufw enable
# NOTE: 6333 stays closed. Qdrant is loopback-only by default.
```

## 1. Clone ZeroClaw + bring up Qdrant

```bash
git clone https://github.com/zeroclaw-labs/zeroclaw /opt/zeroclaw
cd /opt/zeroclaw

# Generate a strong API key. Lose this and you lose access; write it
# down somewhere offline before running the next step.
QDRANT_API_KEY=$(openssl rand -hex 32)
echo "QDRANT_API_KEY=$QDRANT_API_KEY" >> .env
echo "QDRANT_BIND_HOST=127.0.0.1" >> .env
echo "QDRANT_MEM_LIMIT=2G" >> .env    # adjust to SKU

docker compose -f docker-compose.qdrant.yml up -d
docker compose -f docker-compose.qdrant.yml ps   # qdrant should be healthy
curl -s http://127.0.0.1:6333/healthz             # → ok
```

Qdrant is now listening on `127.0.0.1:6333` only — not reachable from
the public IP. That's intentional; the daemon talks to it over
loopback.

## 2. Configure the daemon

Edit `~/.zeroclaw/config.toml`:

```toml
[memory]
backend = "qdrant"
embedding_provider = "openai"     # or "ollama"
embedding_model = "text-embedding-3-small"

[memory.qdrant]
url = "http://127.0.0.1:6333"
collection = "zeroclaw_memories"
api_key = "<paste the same value as QDRANT_API_KEY>"
```

Restart the daemon and confirm:

```
sudo systemctl restart zeroclaw         # if installed as systemd service
# OR
pkill zeroclaw && /opt/zeroclaw/target/release/zeroclaw daemon &
```

Check the daemon log for:
```
INFO zeroclaw_memory: 📦 Qdrant memory backend configured (url: http://127.0.0.1:6333, ...)
```

If you see `failed to connect to qdrant` or `401 Unauthorized`, the
api_key on the daemon and the QDRANT_API_KEY env var don't match.

## 3. Multi-tenant isolation (already wired)

ZeroClaw scopes every vector write/read by an `ACTIVE_TENANT`
task-local. When the dashboard sets `X-Octopus-Tenant: ancestroai` on
an inbound request, the gateway scopes that into the agent loop, and
the Qdrant filter becomes:

```json
{
  "min_should": {
    "min_count": 1,
    "conditions": [
      {"key": "tenant_id", "match": {"value": "ancestroai"}},
      {"is_empty": {"key": "tenant_id"}}
    ]
  }
}
```

Meaning: every recall returns chunks belonging to the active tenant
**or** unscoped chunks (the shared corpus). One Qdrant collection
serves all tenants; no per-tenant collection sprawl. Cross-tenant
data leak requires the daemon to drop the filter — that's a code
path issue, not an ops one.

## 4. Backups

Snapshots are the only durable backup path. Schedule them:

```bash
# /etc/cron.d/qdrant-snapshot — daily at 03:00 UTC
0 3 * * * root curl -sX POST http://127.0.0.1:6333/snapshots \
  -H "api-key: $(grep QDRANT_API_KEY /opt/zeroclaw/.env | cut -d= -f2)" \
  -o /dev/null
```

Then ship `/var/lib/docker/volumes/zeroclaw_qdrant-snapshots/_data/`
off the box (rsync to another VPS / S3 / Hetzner Storage Box).

## 5. (Optional) Remote access via reverse proxy + TLS

Only do this if you need Qdrant reachable from outside the VPS — e.g.
the daemon runs on a different box, or you operate Qdrant from your
laptop's CLI.

```nginx
# /etc/nginx/sites-available/qdrant
server {
  listen 443 ssl http2;
  server_name qdrant.your-domain.com;
  ssl_certificate     /etc/letsencrypt/live/qdrant.your-domain.com/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/qdrant.your-domain.com/privkey.pem;

  location / {
    proxy_pass http://127.0.0.1:6333;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $remote_addr;
  }
}
```

`sudo certbot --nginx -d qdrant.your-domain.com`. Open 443 in ufw;
keep 6333 closed.

## 6. Health monitoring

The compose healthcheck pings `/healthz` every 30s. To surface to the
gateway dashboard, the daemon already polls the memory backend on
startup and reports under `/health` (`memory_backend` field). For
external monitoring (uptime kuma, betterstack), point the probe at
`https://qdrant.your-domain.com/healthz` with header
`api-key: <QDRANT_API_KEY>`.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Daemon logs `qdrant ... 401 Unauthorized` | api_key mismatch | Match `[memory.qdrant].api_key` to `QDRANT_API_KEY` env |
| `memory_recall` returns 400 `unknown field minimum_should_match` | Qdrant ≥ 1.10 vs old client | Already fixed in commit `17064ad8`; rebuild daemon |
| `memory_recall` returns 200 but empty for the active tenant | Vectors never had a `tenant_id` payload | Re-seed under tenant scope; old data is unscoped and only surfaces without a tenant filter |
| Container OOM-kills on first big seed | `QDRANT_MEM_LIMIT` too low | Bump in `.env` and `docker compose -f docker-compose.qdrant.yml up -d` |
