# Hosting ZeroClaw on the public web

This is the operational reference for running ZeroClaw as a long-lived
public-facing service — typically on a single VPS, behind a TLS reverse
proxy, with a custom domain and remote-pairable admin panel.

ZeroClaw is one Rust binary (`zeroclaw`) that ships an HTTP gateway, a
WebSocket chat surface, an SSE event stream, a static web dashboard
(`web/dist`), and an MCP / channel supervisor. Hosting it is the same
shape as hosting any single-binary Go / Rust service.

> **Default bind is `127.0.0.1:42617`** (loopback only). The daemon
> never serves the public internet directly. A reverse proxy in front
> of it terminates TLS, sets the right forwarded headers, and acts as
> the only public surface. Do not publish port 42617 to the world.

---

## 1. What you need

A single-VPS deployment (recommended) requires:

| Resource | Minimum | Notes |
|---|---|---|
| OS | Linux x86_64 (Debian 12 / Ubuntu 22.04 / 24.04) | Windows + macOS work for dev; Linux is the supported host |
| RAM | 1 GB | 2 GB if using FalkorDB / Qdrant locally |
| CPU | 1 vCPU | More helps when channel supervisor is running webhooks |
| Disk | 5 GB | Plus whatever your memory backend grows to |
| Public IP | Required | IPv4 minimum; IPv6 recommended |
| DNS | A or AAAA record pointing at the VPS | Subdomain like `octopus.example.com` is typical |
| Firewall | inbound 22 (SSH), 80 + 443 (HTTPS) | Block everything else inbound, especially 42617 |

Hetzner Cloud, Hetzner Robot, Linode, DigitalOcean, OVH and Vultr all
work without modification. Any provider that gives you a Debian-class
VM is fine. AWS Lightsail and Oracle Free Tier are also viable.

---

## 2. Quick path: VPS + Caddy + systemd (recommended)

This is the lowest-friction deployment for one operator running one
ZeroClaw instance. Caddy handles TLS automatically (Let's Encrypt),
and systemd keeps the binary alive.

### 2.1 Provision

```bash
# As root on a freshly-provisioned Debian 12 box:
adduser --system --group --home /var/lib/zeroclaw --shell /bin/bash zeroclaw
mkdir -p /opt/zeroclaw /var/lib/zeroclaw
chown zeroclaw:zeroclaw /var/lib/zeroclaw
```

### 2.2 Install the binary

You can either build from source on the box, or scp a binary built
elsewhere.

**Build from source (recommended for first install):**

```bash
apt-get update && apt-get install -y build-essential pkg-config libssl-dev curl git
sudo -u zeroclaw bash <<'EOF'
cd /var/lib/zeroclaw
git clone https://github.com/<your-fork>/zeroclaw.git src
cd src
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
cargo build --release
EOF
install -o root -g root -m 0755 \
  /var/lib/zeroclaw/src/target/release/zeroclaw /opt/zeroclaw/zeroclaw
```

**Or copy a binary built locally:**

```bash
scp target/release/zeroclaw root@your.vps:/opt/zeroclaw/zeroclaw
ssh root@your.vps "chown root:root /opt/zeroclaw/zeroclaw && chmod 0755 /opt/zeroclaw/zeroclaw"
```

The web dashboard is served from `web/dist`. If you want it bundled
with the binary, copy that folder too:

```bash
scp -r web/dist root@your.vps:/opt/zeroclaw/web-dist
```

### 2.3 First-time onboarding

Run the onboarding wizard once as the service user so the workspace
and config are created with the right ownership:

```bash
sudo -u zeroclaw HOME=/var/lib/zeroclaw /opt/zeroclaw/zeroclaw onboard
```

This creates `/var/lib/zeroclaw/.zeroclaw/config.toml` and the work-
space at `/var/lib/zeroclaw/.zeroclaw/workspace/`. Set provider, model,
and at least one memory backend at this step.

### 2.4 systemd service

Write `/etc/systemd/system/zeroclaw.service`:

```ini
[Unit]
Description=ZeroClaw agent runtime
After=network-online.target
Wants=network-online.target

[Service]
User=zeroclaw
Group=zeroclaw
WorkingDirectory=/var/lib/zeroclaw
Environment=HOME=/var/lib/zeroclaw
Environment=ZEROCLAW_WEB_DIST=/opt/zeroclaw/web-dist
ExecStart=/opt/zeroclaw/zeroclaw daemon
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/zeroclaw
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictNamespaces=true
RestrictRealtime=true
LockPersonality=true

[Install]
WantedBy=multi-user.target
```

Then:

```bash
systemctl daemon-reload
systemctl enable --now zeroclaw
systemctl status zeroclaw
journalctl -u zeroclaw -f          # live logs
```

The daemon now binds `127.0.0.1:42617` and is invisible from the
public internet.

### 2.5 Caddy reverse proxy + automatic HTTPS

Install Caddy (Debian/Ubuntu):

```bash
apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
  | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
  | tee /etc/apt/sources.list.d/caddy-stable.list
apt-get update && apt-get install -y caddy
```

Replace `/etc/caddy/Caddyfile` with:

```caddyfile
octopus.example.com {
    encode zstd gzip

    # WebSocket chat — needs upgrade headers
    @ws {
        path /ws/*
        header Connection *Upgrade*
        header Upgrade websocket
    }
    reverse_proxy @ws 127.0.0.1:42617

    # SSE event stream — disable buffering
    @sse {
        path /api/events
    }
    reverse_proxy @sse 127.0.0.1:42617 {
        flush_interval -1
        transport http {
            response_header_timeout 24h
        }
    }

    # Block the localhost-only admin endpoints from the public side.
    # The daemon already enforces localhost on these, but a 404 at
    # the proxy is cheaper and clearer.
    @adminLocal {
        path /admin/paircode*
        path /pair/code
    }
    respond @adminLocal 404

    # Everything else
    reverse_proxy 127.0.0.1:42617 {
        header_up X-Real-IP {remote_host}
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }

    # Security headers
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Content-Type-Options nosniff
        X-Frame-Options DENY
        Referrer-Policy strict-origin-when-cross-origin
        Permissions-Policy "accelerometer=(), camera=(), geolocation=(), microphone=()"
        # Don't leak Caddy version
        -Server
    }
}
```

Reload:

```bash
systemctl reload caddy
```

Caddy provisions a Let's Encrypt cert for `octopus.example.com`
automatically on first request. Visit `https://octopus.example.com/`
to see the login screen.

### 2.6 Trust forwarded headers (rate limit + IP correctness)

Once Caddy is in front, the daemon needs to know the real client IP
isn't `127.0.0.1`. Open `/var/lib/zeroclaw/.zeroclaw/config.toml`:

```toml
[gateway]
host = "127.0.0.1"
port = 42617
trust_forwarded_headers = true   # parse X-Forwarded-For from Caddy
require_pairing = true           # bearer-token required for /api/*
```

Restart:

```bash
systemctl restart zeroclaw
```

### 2.7 Firewall

Use `nftables` (Debian default) or `ufw`. Allow only SSH + HTTPS;
explicitly drop 42617 from the outside.

```bash
# ufw
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw enable
```

The daemon already binds loopback, but the firewall is a second
layer in case someone toggles `host` to `0.0.0.0`.

### 2.8 First pair

After everything is up, get a code over SSH and pair from your laptop:

```bash
ssh root@your.vps "sudo -u zeroclaw HOME=/var/lib/zeroclaw \
  /opt/zeroclaw/zeroclaw gateway get-paircode --new"
```

Open `https://octopus.example.com/`, enter the code, click `Pair`.

From then on you can issue and revoke additional pair codes from the
admin panel at `/pairing` — no SSH required.

---

## 3. Alternative: Docker Compose

Useful if you already orchestrate workloads with Docker, or want
isolated upgrades.

`Dockerfile`:

```dockerfile
FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/*
RUN groupadd --system --gid 1000 zeroclaw \
 && useradd  --system --uid 1000 --gid zeroclaw --home /var/lib/zeroclaw zeroclaw
RUN install -d -o zeroclaw -g zeroclaw /var/lib/zeroclaw /opt/zeroclaw
COPY --from=build /src/target/release/zeroclaw /opt/zeroclaw/zeroclaw
COPY web/dist /opt/zeroclaw/web-dist
USER zeroclaw
WORKDIR /var/lib/zeroclaw
ENV HOME=/var/lib/zeroclaw \
    ZEROCLAW_WEB_DIST=/opt/zeroclaw/web-dist
EXPOSE 42617
CMD ["/opt/zeroclaw/zeroclaw", "daemon"]
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:42617/health || exit 1
```

`docker-compose.yml`:

```yaml
services:
  zeroclaw:
    build: .
    restart: unless-stopped
    volumes:
      - zc-data:/var/lib/zeroclaw
    networks: [edge]
    # No ports: published — the proxy reaches it inside the network

  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
      - caddy-config:/config
    networks: [edge]
    depends_on:
      zeroclaw:
        condition: service_healthy

volumes:
  zc-data:
  caddy-data:
  caddy-config:

networks:
  edge:
```

Same `Caddyfile` as section 2.5, but `reverse_proxy zeroclaw:42617`
instead of `127.0.0.1:42617`.

First-time onboarding inside the container:

```bash
docker compose up -d
docker compose exec zeroclaw /opt/zeroclaw/zeroclaw onboard
docker compose restart zeroclaw
docker compose exec zeroclaw /opt/zeroclaw/zeroclaw gateway get-paircode --new
```

---

## 4. Cloud-platform shortcuts

These work but trade flexibility for convenience:

- **Fly.io** — `fly launch` against the Dockerfile in section 3. Set
  `internal_port = 42617`, give it a `[mounts]` for `/var/lib/zeroclaw`,
  scale to 1 machine. Built-in TLS + custom domain.
- **Railway** — same Dockerfile; set `PORT=42617`, mount a volume at
  `/var/lib/zeroclaw`. Built-in TLS.
- **Render** — `Web Service` + `Disk` mount, Docker runtime, port
  42617. Built-in TLS + custom domain.
- **AWS Lightsail container service** — Dockerfile, public endpoint
  on `:42617`, persistent storage via mounted EFS or Lightsail block.

All four give you HTTPS automatically. None of them give you the
loopback-only safety of section 2 — you must keep `require_pairing =
true` and rotate pair codes from the admin panel.

---

## 5. Configuration knobs that matter for hosting

`~/.zeroclaw/config.toml` on the host (or `/var/lib/zeroclaw/.zeroclaw/config.toml`
under systemd):

```toml
[gateway]
host = "127.0.0.1"            # never expose 42617 publicly
port = 42617
require_pairing = true        # bearer token required for /api/*
trust_forwarded_headers = true # if behind a reverse proxy
path_prefix = ""              # set to "/octopus" if hosted on a sub-path
webhook_secret = ""           # HMAC for inbound webhooks (optional)

[gateway.rate_limits]
webhook_per_min = 60
pair_per_min = 10
api_per_min = 240

[storage]
# JSON file at workspace/tenants.json + memory backend per [memory]

[memory]
backend = "sqlite"            # or "qdrant" / "falkordb" if you run them
# qdrant_url = "http://qdrant:6333"
# qdrant_collection = "zeroclaw_memories"

[providers]
fallback = "default"

[providers.models.openrouter]
api_key = "sk-or-v1-..."      # do NOT commit; keep file mode 0600
```

Lock the config down:

```bash
chmod 0600 /var/lib/zeroclaw/.zeroclaw/config.toml
chown zeroclaw:zeroclaw /var/lib/zeroclaw/.zeroclaw/config.toml
```

API keys can also be supplied via env so they never touch disk:

```ini
# In zeroclaw.service [Service] block:
Environment=OPENROUTER_API_KEY=sk-or-v1-...
Environment=ANTHROPIC_API_KEY=sk-ant-...
```

(Use `EnvironmentFile=/etc/zeroclaw/secrets.env` with `0600` perms if
you have many.)

---

## 6. Backups

The blast radius of losing the workspace is: tenant registry,
memory store, paired devices, cron jobs, knowledge graph snapshots.
Back it up.

```bash
# Daily, retain 14 days
cat >/etc/cron.daily/zeroclaw-backup <<'EOF'
#!/bin/sh
set -e
DEST=/var/backups/zeroclaw
mkdir -p "$DEST"
tar -czf "$DEST/zeroclaw-$(date +\%F).tar.gz" \
  -C /var/lib/zeroclaw .zeroclaw
find "$DEST" -name 'zeroclaw-*.tar.gz' -mtime +14 -delete
EOF
chmod +x /etc/cron.daily/zeroclaw-backup
```

Push the backup tarballs offsite (rclone → S3 / Backblaze / Storj /
Hetzner Storage Box) — local-only backups die with the disk.

Excludes worth setting if your memory backend is large:
`--exclude='.zeroclaw/workspace/cache'`,
`--exclude='.zeroclaw/workspace/transcripts'`.

---

## 7. Updates

```bash
# Build new binary on the host (or scp from elsewhere)
cd /var/lib/zeroclaw/src && sudo -u zeroclaw git pull && sudo -u zeroclaw cargo build --release

# Hot-swap with minimal downtime
systemctl stop zeroclaw
install -o root -g root -m 0755 \
  /var/lib/zeroclaw/src/target/release/zeroclaw /opt/zeroclaw/zeroclaw
systemctl start zeroclaw
systemctl status zeroclaw
```

For a no-downtime update during business hours, build into a
`zeroclaw.next`, point a symlink, and `systemctl restart`. Or run two
versions on different ports and flip the Caddy upstream.

The web-dashboard bundle (`web/dist`) is static — copy the new folder
to `/opt/zeroclaw/web-dist` and reload Caddy. No daemon restart
needed for dashboard-only changes.

---

## 8. Monitoring

Three signals worth watching:

1. **HTTP health** — `curl -fsS https://octopus.example.com/health`
   should return JSON with `status:"ok"` and every component's
   `status:"ok"`. Wire to UptimeRobot / Better Stack / Healthchecks.io.
2. **Prometheus metrics** — `GET /metrics` exposes per-component
   counters. Scrape from a colocated Prometheus or push via OTel.
3. **journalctl** — `journalctl -u zeroclaw -f --since '5 min ago'`
   for live tail. Ship to Loki / Vector if you operate more than
   one instance.

A minimal alert set worth wiring:
- HTTP 5xx rate over 1% for 5 min
- `/health` returns non-`ok` for any component for 2 min
- Disk usage > 85% on `/var/lib/zeroclaw`
- systemd unit not active

---

## 9. Security checklist

- [ ] `host = "127.0.0.1"` in config — daemon never binds public
- [ ] `require_pairing = true` — every `/api/*` call needs bearer
- [ ] Caddy / nginx terminates TLS; HSTS header set
- [ ] Firewall denies all but 22 / 80 / 443
- [ ] Config file is `0600` and owned by service user
- [ ] API keys in env, not in config (or config is `0600`)
- [ ] `/admin/*` and `/pair/code` blocked at the proxy (section 2.5)
- [ ] Pair codes rotated regularly from `/pairing` admin panel
- [ ] Long-unused devices revoked in `/pairing`
- [ ] Backup runs daily and is verified once a month
- [ ] OS updates applied weekly (`unattended-upgrades` on Debian)
- [ ] SSH: key auth only, no root login, fail2ban or
      `--match-set` rate limit
- [ ] Cloudflare or similar WAF in front for DDoS absorption (optional
      but recommended for any public-facing instance)

---

## 10. Multi-tenant shape

The current `Tenant` registry inside ZeroClaw is multi-business under
one operator — it does **not** isolate operators from each other. If
you want N independent customers each with their own instance:

- Run one ZeroClaw container per customer, distinct workspace volume
- Front each with a per-tenant subdomain (`acme.octopus.example.com`,
  `globex.octopus.example.com`)
- Use Caddy's `on_demand_tls` for cert provisioning at scale
- Centralise auth at an OAuth proxy (oauth2-proxy, Pomerium) above
  Caddy if you need SSO per tenant

Cross-instance shared memory is not supported today — keep customer
data physically separated.

---

## 11. Troubleshooting

**`502 Bad Gateway` from Caddy**
Daemon not running or wrong port. `systemctl status zeroclaw`,
`curl 127.0.0.1:42617/health` from the host.

**Pair code generated but `/pair` returns 401**
Either the code expired (default 5 min TTL), or `X-Pairing-Code`
header was sent through with leading/trailing whitespace. Generate a
fresh one from the admin panel.

**Bearer token works locally but not from a remote browser**
Check `trust_forwarded_headers = true` is set, the proxy is sending
`X-Forwarded-For`, and the rate limiter isn't blocking the proxy IP.

**SSE / WebSocket disconnects every ~60s**
Caddy default response timeout. Add `flush_interval -1` and
`response_header_timeout 24h` to the SSE block (already in section
2.5's Caddyfile).

**Web dashboard 404s**
Set `ZEROCLAW_WEB_DIST=/opt/zeroclaw/web-dist` in the systemd unit
or run from the source tree where `web/dist/` exists.

**Memory backend: connection refused (Qdrant / FalkorDB)**
The daemon caches the failure for a few seconds. If you started those
services after the daemon, restart `zeroclaw` once they're healthy.

**Channel webhooks rejected with `signature mismatch`**
The provider's secret must match `[channels_config.<name>].webhook_secret`
in the config. Common cause after migration is the `\n` literal vs
actual newline in the secret.

---

## See also

- `docs/book/src/setup/` — per-channel setup (Telegram, Discord, etc.)
- `docs/book/src/operating/security.md` — pairing flow internals
- `crates/zeroclaw-gateway/src/api_pairing.rs` — admin pair endpoints
- `web/src/pages/Pairing.tsx` — admin panel implementation
