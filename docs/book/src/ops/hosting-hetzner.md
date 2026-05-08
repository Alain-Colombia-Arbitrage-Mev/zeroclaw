# Self-hosting on Hetzner (Docker Compose)

This guide walks through deploying the Octopus Labs runtime
(`zeroclaw` daemon + gateway + Qdrant + FalkorDB) on a Hetzner Cloud
VPS behind a Caddy reverse proxy with automatic TLS.

The same procedure works on any Linux VPS (DigitalOcean, OVH, Linode,
your own metal). We pick Hetzner because the price/performance is
unbeatable for hobby and small-team self-hosting (~€5–€20/month for
the tiers we recommend).

## Prerequisites

| Tier | CPU / RAM | Disk | Use case |
|---|---|---|---|
| **CX22** (€4.51/mo) | 2 vCPU / 4 GB | 40 GB | Single-user, Qdrant only |
| **CX32** (€6.85/mo) | 2 vCPU / 8 GB | 80 GB | Recommended — Qdrant + FalkorDB |
| **CCX13** (€13.49/mo) | 2 dedicated vCPU / 8 GB | 80 GB | Production / multi-user |

You also need:

- A domain (e.g. `octopus.example.com`) with an A record pointing to the VPS IPv4.
- An SSH key uploaded to your Hetzner account.
- An OpenRouter / OpenAI / Anthropic API key.

## 1. Provision the server

```bash
# From your local machine
hcloud server create \
  --name octopus-prod \
  --type cx32 \
  --image ubuntu-24.04 \
  --location nbg1 \
  --ssh-key your-key
```

(or use the [Hetzner Cloud console](https://console.hetzner.cloud/)
and pick the same image/region.)

## 2. Harden + install Docker

SSH in and run:

```bash
ssh root@$SERVER_IP

# Create a non-root user
adduser --disabled-password --gecos "" octopus
usermod -aG sudo octopus
mkdir -p /home/octopus/.ssh
cp ~/.ssh/authorized_keys /home/octopus/.ssh/
chown -R octopus:octopus /home/octopus/.ssh
chmod 600 /home/octopus/.ssh/authorized_keys

# Disable password + root SSH
sed -i 's/^#\?PermitRootLogin .*/PermitRootLogin no/' /etc/ssh/sshd_config
sed -i 's/^#\?PasswordAuthentication .*/PasswordAuthentication no/' /etc/ssh/sshd_config
systemctl restart ssh

# Firewall — only 22/80/443 reach the public internet
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable

# Docker Engine + compose plugin
curl -fsSL https://get.docker.com | sh
usermod -aG docker octopus
```

Reconnect as `octopus`:

```bash
ssh octopus@$SERVER_IP
```

## 3. Lay out the deployment

```bash
sudo mkdir -p /opt/octopus
sudo chown octopus:octopus /opt/octopus
cd /opt/octopus
mkdir -p config workspace data/qdrant data/falkordb data/caddy/data data/caddy/config
```

Create `/opt/octopus/.env`:

```bash
cat > .env <<'EOF'
# LLM provider key — required
API_KEY=sk-or-v1-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
PROVIDER=openrouter

# Octopus identity
OCTOPUS_DOMAIN=octopus.example.com
OCTOPUS_PAIRING_REQUIRED=true

# Caddy email for Let's Encrypt
CADDY_EMAIL=ops@example.com

# Internal ports
OCTOPUS_GATEWAY_PORT=42617
QDRANT_HTTP_PORT=6333
FALKORDB_PORT=6379
EOF
chmod 600 .env
```

## 4. `docker-compose.yml`

```yaml
services:
  octopus:
    image: ghcr.io/zeroclaw-labs/zeroclaw:debian
    container_name: octopus
    restart: unless-stopped
    depends_on:
      qdrant: { condition: service_healthy }
      falkordb: { condition: service_healthy }
    environment:
      - API_KEY=${API_KEY}
      - PROVIDER=${PROVIDER}
      - ZEROCLAW_ALLOW_PUBLIC_BIND=true
      - ZEROCLAW_GATEWAY_PORT=${OCTOPUS_GATEWAY_PORT}
      - ZEROCLAW_PAIRING_REQUIRED=${OCTOPUS_PAIRING_REQUIRED}
    volumes:
      - ./config:/root/.zeroclaw
      - ./workspace:/root/.zeroclaw/workspace
    networks: [octopus-net]

  qdrant:
    image: qdrant/qdrant:latest
    container_name: octopus-qdrant
    restart: unless-stopped
    volumes:
      - ./data/qdrant:/qdrant/storage
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:6333/healthz"]
      interval: 30s
      timeout: 5s
      retries: 3
    networks: [octopus-net]

  falkordb:
    image: falkordb/falkordb:latest
    container_name: octopus-falkordb
    restart: unless-stopped
    environment:
      FALKORDB_ARGS: "MAX_QUEUED_QUERIES 50"
    volumes:
      - ./data/falkordb:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 30s
      timeout: 5s
      retries: 3
    networks: [octopus-net]

  caddy:
    image: caddy:2-alpine
    container_name: octopus-caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - ./data/caddy/data:/data
      - ./data/caddy/config:/config
    environment:
      - CADDY_EMAIL=${CADDY_EMAIL}
      - OCTOPUS_DOMAIN=${OCTOPUS_DOMAIN}
      - OCTOPUS_GATEWAY_PORT=${OCTOPUS_GATEWAY_PORT}
    networks: [octopus-net]

networks:
  octopus-net:
    driver: bridge
```

## 5. `Caddyfile`

```caddyfile
{
  email {env.CADDY_EMAIL}
}

{env.OCTOPUS_DOMAIN} {
  encode zstd gzip

  @websocket {
    header Connection *Upgrade*
    header Upgrade websocket
  }

  reverse_proxy @websocket octopus:{env.OCTOPUS_GATEWAY_PORT}
  reverse_proxy octopus:{env.OCTOPUS_GATEWAY_PORT}

  header {
    Strict-Transport-Security "max-age=31536000; includeSubDomains"
    X-Content-Type-Options nosniff
    X-Frame-Options DENY
    Referrer-Policy strict-origin-when-cross-origin
    -Server
  }
}
```

## 6. Bootstrap the daemon config

The first run needs a config seed. Two options:

**Option A — copy your local config:**

```bash
# From your laptop
scp ~/.zeroclaw/config.toml octopus@$SERVER_IP:/opt/octopus/config/config.toml
```

**Option B — let the daemon onboard interactively:**

```bash
cd /opt/octopus
docker compose run --rm octopus zeroclaw onboard
```

Either way, edit `config/config.toml` to enable the FalkorDB knowledge
graph and point at the in-network services:

```toml
[memory.qdrant]
url = "http://qdrant:6333"
collection = "octopus_memories"

[knowledge]
enabled = true
backend = "falkordb"
auto_capture = true

[knowledge.falkordb]
url = "redis://falkordb:6379"
graph = "octopus_kg"
```

## 7. Launch

```bash
cd /opt/octopus
docker compose up -d
docker compose logs -f octopus  # watch boot
```

When you see `Gateway listening on http://0.0.0.0:42617` Caddy will
already have a TLS cert via Let's Encrypt. Visit
`https://octopus.example.com/` — you'll hit the pairing screen.

Get the pairing code:

```bash
docker compose exec octopus zeroclaw gateway get-paircode --new
```

Type it into the dashboard, save the bearer token in your browser, and
you're paired.

## 8. Updates

```bash
cd /opt/octopus
docker compose pull
docker compose up -d
```

Rolling restarts keep volumes intact. Bearer tokens persist (they live
in `config/`).

## 9. Backups

The state worth saving:

| Path | Contents |
|---|---|
| `/opt/octopus/config/` | `config.toml`, OTP secrets, paired devices |
| `/opt/octopus/workspace/` | Skills, SOPs, knowledge SQLite (if used) |
| `/opt/octopus/data/qdrant/` | Vector memories |
| `/opt/octopus/data/falkordb/` | Knowledge graph |

Hetzner Storage Box + `restic`:

```bash
restic -r sftp:u123456@u123456.your-storagebox.de:/backups/octopus init
restic -r sftp:... backup /opt/octopus/config /opt/octopus/workspace /opt/octopus/data
```

Schedule daily via cron.

## 10. Observability (optional)

The gateway exposes Prometheus metrics at `/metrics` (bearer-gated).
Scrape it from a separate Grafana stack or use the bundled
[Logs & observability](./observability.md) guide.

## Troubleshooting

| Symptom | Fix |
|---|---|
| 502 from Caddy | `docker compose logs octopus` — usually first-run config error |
| Qdrant `connection refused` | `data/qdrant` permissions; `chown -R 1000:1000 data/qdrant` |
| FalkorDB ping fails | Memory limit too low; remove `mem_limit` or raise it |
| Pairing screen loops | Old bearer in browser localStorage; hard refresh |
| TLS cert errors | DNS must resolve before Caddy boots; check `dig $OCTOPUS_DOMAIN` |
