# Hosting ZeroClaw on Hetzner Cloud

End-to-end walkthrough for deploying a public ZeroClaw instance on
Hetzner Cloud. Total wall-clock time: ~30 minutes including DNS
propagation. Total monthly cost: **€4.51 + €0.60 IPv4 + €0.60
backups = €5.71 / month** for the lightest viable setup.

This is the concrete companion to the generic [`hosting.md`](./hosting.md);
read that first for security checklist + Caddyfile + systemd unit.
This file fills in the Hetzner-specific bits.

---

## 1. Why Hetzner

- **Cheapest credible EU host.** A `cx22` (2 vCPU AMD, 4 GB, 40 GB
  NVMe, 20 TB traffic) is €4.51/month. ARM `cax11` (2 vCPU Ampere,
  4 GB, 40 GB) is €3.92/month and runs ZeroClaw fine.
- **Falkenstein / Nuremberg / Helsinki / Ashburn / Hillsboro / Singapore.**
  Pick whichever is closest to your operator + your channel users.
- **One control plane** for VPS + DNS + load balancer + S3-compatible
  object storage (Storage Box for backups).
- **Mature `hcloud` CLI**, project-scoped API tokens, snapshot pricing
  by GB-hour rather than flat fee.

If you need US presence specifically, `cax11` Hillsboro or `cx22`
Ashburn give you the same price as EU.

---

## 2. Pre-flight

You'll need:

- A **Hetzner Cloud account** ([https://console.hetzner.cloud](https://console.hetzner.cloud))
- A **payment method** (credit card or SEPA)
- A **domain** with DNS you can edit (any registrar; or transfer it
  to Hetzner DNS Console for one less moving part)
- An **SSH key** on your laptop (`ssh-keygen -t ed25519` if you don't
  have one — never use a passworded password, always a key)

Optional but worth installing:

```bash
# macOS
brew install hcloud

# Linux
curl -L https://github.com/hetznercloud/cli/releases/latest/download/hcloud-linux-amd64.tar.gz | tar xz
sudo mv hcloud /usr/local/bin/

# Windows (pwsh)
winget install Hetzner.Cloud.CLI
```

The hcloud CLI is faster than the web console for everything below.

---

## 3. Create a project + API token

In the web console:

1. Top-left → **Projects** → **+ New project** → name it `zeroclaw`.
2. Inside the project, **Security** → **API Tokens** → **Generate API
   Token** → permissions **Read & Write**, label `cli`. Copy the
   token immediately — you won't see it again.

Configure the CLI:

```bash
hcloud context create zeroclaw
# Paste the token when prompted
hcloud context active   # verify
```

Upload your SSH key:

```bash
hcloud ssh-key create --name laptop --public-key-from-file ~/.ssh/id_ed25519.pub
```

---

## 4. Create the firewall

Lock down to SSH + HTTP + HTTPS *before* the server exists, so it's
applied on first boot.

```bash
hcloud firewall create --name zeroclaw-edge

hcloud firewall add-rule zeroclaw-edge \
  --direction in --protocol tcp --port 22 \
  --source-ips 0.0.0.0/0 --source-ips ::/0 --description ssh

hcloud firewall add-rule zeroclaw-edge \
  --direction in --protocol tcp --port 80 \
  --source-ips 0.0.0.0/0 --source-ips ::/0 --description http

hcloud firewall add-rule zeroclaw-edge \
  --direction in --protocol tcp --port 443 \
  --source-ips 0.0.0.0/0 --source-ips ::/0 --description https
```

For better SSH hygiene, restrict 22 to your IP only:

```bash
MY_IP=$(curl -s https://ipv4.icanhazip.com)/32
hcloud firewall replace-rules zeroclaw-edge --rules-from-file - <<EOF
[
  { "direction": "in", "protocol": "tcp", "port": "22",
    "source_ips": ["${MY_IP}"], "description": "ssh-from-laptop" },
  { "direction": "in", "protocol": "tcp", "port": "80",
    "source_ips": ["0.0.0.0/0", "::/0"], "description": "http" },
  { "direction": "in", "protocol": "tcp", "port": "443",
    "source_ips": ["0.0.0.0/0", "::/0"], "description": "https" }
]
EOF
```

---

## 5. Provision the server

Pick a **type** (compute) and **location** (region).

| Type | vCPU | RAM | Disk | Price | Best for |
|---|---|---|---|---|---|
| `cax11` | 2 ARM | 4 GB | 40 GB | €3.92/mo | Lightest viable; Rust binary runs fine on ARM |
| `cx22`  | 2 x86 | 4 GB | 40 GB | €4.51/mo | Same price ballpark, x86 if you bring binaries from CI |
| `cax21` | 4 ARM | 8 GB | 80 GB | €7.44/mo | Add Qdrant / FalkorDB locally |
| `cx32`  | 4 x86 | 8 GB | 80 GB | €8.92/mo | x86 + headroom |

Locations: `fsn1` (Falkenstein DE), `nbg1` (Nuremberg DE), `hel1`
(Helsinki FI), `ash` (Ashburn US-East), `hil` (Hillsboro US-West),
`sin` (Singapore).

```bash
SSH_KEY_NAME=laptop
hcloud server create \
  --name octopus-1 \
  --type cax11 \
  --image debian-12 \
  --location fsn1 \
  --ssh-key "$SSH_KEY_NAME" \
  --firewall zeroclaw-edge \
  --enable-backup
```

`--enable-backup` adds 20% to the server price (€0.78/mo on `cax11`)
and gives you 7 daily snapshots managed by Hetzner. Worth every
cent — kill the box, recreate from snapshot in 60 seconds.

Get the IP:

```bash
hcloud server ip octopus-1               # IPv4
hcloud server describe octopus-1 -o json | jq -r '.public_net.ipv6.ip'
```

---

## 6. DNS

Two options. Pick one.

### 6a. Hetzner DNS Console (free, single pane of glass)

```bash
# Web console: https://dns.hetzner.com → Add zone → enter your domain
# Then either point your registrar's nameservers to Hetzner's
# (helsinki1/falkenstein1/nuremberg1.ns.hetzner.com), or just keep
# DNS at your existing registrar and use 6b.
```

Once the zone is in Hetzner DNS, add records via web or `hetznerdns-cli`:

```
A    octopus    →  <IPv4 from §5>     TTL 300
AAAA octopus    →  <IPv6 from §5>     TTL 300
```

### 6b. Existing registrar (Cloudflare / Namecheap / etc.)

Just add the A + AAAA records pointing at the IP from §5. TTL 300
is fine. Cloudflare proxy (orange cloud) can be on or off; on adds
DDoS absorption but breaks WebSocket without "Network → WebSockets:
on" which is the default these days.

Verify:

```bash
dig +short octopus.example.com A
dig +short octopus.example.com AAAA
```

---

## 7. Initial OS hardening

```bash
ssh root@<server-ip>          # first connect; accept fingerprint

# Update + upgrade
apt-get update && apt-get -y upgrade

# Unattended security updates so kernel CVEs land without ops attention
apt-get -y install unattended-upgrades
dpkg-reconfigure -plow unattended-upgrades

# Create the service user (matches docs/hosting.md)
adduser --system --group --home /var/lib/zeroclaw --shell /bin/bash zeroclaw
mkdir -p /opt/zeroclaw
chown zeroclaw:zeroclaw /var/lib/zeroclaw

# Disable root SSH login + password auth
sed -i 's/^#*PermitRootLogin.*/PermitRootLogin no/' /etc/ssh/sshd_config
sed -i 's/^#*PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config
# Add a non-root admin user before doing this (or use the deploy user)
adduser deploy
usermod -aG sudo deploy
mkdir -p /home/deploy/.ssh
cp /root/.ssh/authorized_keys /home/deploy/.ssh/
chown -R deploy:deploy /home/deploy/.ssh
chmod 700 /home/deploy/.ssh && chmod 600 /home/deploy/.ssh/authorized_keys
systemctl reload ssh
# Test from a new terminal: ssh deploy@<ip>  → must work before closing root!

# fail2ban for SSH
apt-get -y install fail2ban
```

---

## 8. Install ZeroClaw

Follow [`docs/hosting.md` §2.2 – §2.6](./hosting.md#22-install-the-binary)
verbatim from here. The summary:

```bash
# As deploy or root, install build deps + Rust if compiling on the box
apt-get -y install build-essential pkg-config libssl-dev curl git

# Build
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

# Bundle the dashboard
cp -r /var/lib/zeroclaw/src/web/dist /opt/zeroclaw/web-dist

# Onboard
sudo -u zeroclaw HOME=/var/lib/zeroclaw /opt/zeroclaw/zeroclaw onboard
```

Lock the API key file:

```bash
chmod 0600 /var/lib/zeroclaw/.zeroclaw/config.toml
```

Then create the systemd unit + Caddyfile from [`docs/hosting.md`
§2.4 – §2.5](./hosting.md#24-systemd-service). Replace
`octopus.example.com` in the Caddyfile with your real subdomain.

Start everything:

```bash
systemctl daemon-reload
systemctl enable --now zeroclaw caddy
systemctl status zeroclaw caddy
```

Visit `https://<your-domain>/` — Caddy provisions a Let's Encrypt
cert on first request (10–30 seconds), then the pairing dialog
appears.

---

## 9. First pair

The web pairing dialog wants a 6-digit code that the daemon prints
in the systemd journal. Get it:

```bash
ssh root@<ip> "sudo -u zeroclaw HOME=/var/lib/zeroclaw \
  /opt/zeroclaw/zeroclaw gateway get-paircode --new"
```

Or read the journal:

```bash
ssh root@<ip> "journalctl -u zeroclaw --since '5 min ago' | grep -i pair"
```

Type the code into the dialog, click `Pair`. From this moment on,
new devices are paired from `/pairing` in the dashboard — no SSH
required. The `Pair Control` panel shows a QR; scanning it with a
phone camera opens the dashboard with the code pre-filled, and one
tap completes the handshake.

---

## 10. Backups (Hetzner-native)

Snapshots are already happening daily (you used `--enable-backup`
in §5). To restore:

```bash
hcloud server backup list octopus-1            # list available backups
# In the web console: Server → Backups → "Rebuild from this image"
# Takes ~60s for a 40 GB volume.
```

For application-level backups (workspace + tenants.json + memory
backend), use the Hetzner Storage Box as cheap cold storage:

```bash
# Order a Storage Box from console.hetzner.com → Storage Boxes
# 1 TB = €3.81/mo. Get the SSH endpoint (e.g. u123456@u123456.your-storagebox.de)

# On the daemon host:
apt-get -y install rclone
sudo -u zeroclaw rclone config             # setup as 'sftp' to the storagebox
```

Then schedule the backup script from
[`docs/hosting.md` §6](./hosting.md#6-backups) and follow it with:

```bash
rclone copy /var/backups/zeroclaw storagebox:zeroclaw/$(hostname)/
rclone delete --min-age 30d storagebox:zeroclaw/$(hostname)/
```

---

## 11. Costs at-a-glance

| Item | Monthly |
|---|---|
| `cx22` server (2 vCPU, 4 GB) | €4.51 |
| Backups (auto, daily, 7 days) | €0.90 |
| IPv4 (Hetzner now charges for these) | €0.60 |
| Hetzner DNS | €0.00 |
| **Subtotal — minimum production** | **€6.01** |
| Storage Box 1 TB (offsite backups) | +€3.81 |
| `cax21` upgrade for Qdrant locally | +€2.93 (over cax11) |
| Hetzner Load Balancer (HA upgrade) | +€5.39 |

The `cax11` ARM box at €3.92 + €0.60 IPv4 + €0.78 backups = **€5.30/mo**
runs the daemon, Caddy, and a SQLite memory backend with no sweat.
Add the `cax21` only when you need to host Qdrant or FalkorDB on the
same machine.

---

## 12. Update workflow

```bash
# Pull + rebuild on the box
ssh deploy@<ip>
sudo -u zeroclaw bash -c '
  cd /var/lib/zeroclaw/src && git pull
  cargo build --release
'
sudo systemctl stop zeroclaw
sudo install -o root -g root -m 0755 \
  /var/lib/zeroclaw/src/target/release/zeroclaw /opt/zeroclaw/zeroclaw
sudo cp -r /var/lib/zeroclaw/src/web/dist /opt/zeroclaw/web-dist
sudo systemctl start zeroclaw
```

Or, if you want zero compile time on the box, build a release in CI,
push the binary to a private S3 / GitHub Release, and `curl` it down
on the box — `cax11` ARM builds take ~3 minutes; `cx22` x86 takes
~2 minutes. Either is fine.

---

## 13. Tear down

```bash
hcloud server delete octopus-1
hcloud firewall delete zeroclaw-edge
# Snapshots auto-delete with the server unless you saved them as
# images. List with: hcloud image list --type backup
```

---

## See also

- [`docs/hosting.md`](./hosting.md) — generic deploy reference (Caddyfile,
  systemd unit, security checklist, backup strategy, multi-tenant)
- [`docs/desktop-plan.md`](./desktop-plan.md) — roadmap for the desktop
  build (Tauri-wrapped daemon + dashboard) once the hosted version
  is stable
- Hetzner Cloud docs — [https://docs.hetzner.com/cloud/](https://docs.hetzner.com/cloud/)
- hcloud CLI — `hcloud --help` (every command has a `-h`)
