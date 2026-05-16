# Network probe

The `net_probe` tool gives the agent narrow, read-style network diagnostics:

- `dns_lookup` — resolve a hostname to A/AAAA addresses via the OS resolver.
- `tcp_connect` — open a TCP connection and report RTT, local and peer address.

No raw sockets, no UDP, no ICMP. TLS certificate inspection is out of scope for v1.

## Configuration

```toml
[net_probe]
enabled = true
allowed_hosts = ["example.com", "api.example.com"]   # or ["*"] for any public host
allow_private_hosts = false                          # off by default — blocks loopback, RFC1918, link-local, ULA, etc.
timeout_ms = 5000
```

`allowed_hosts` is a hard allowlist matched against the bare hostname (subdomains accepted). `allow_private_hosts = false` (the default) refuses both literal private addresses and hostnames that resolve to non-global IPs — protection against DNS rebinding.

## Invocation

```jsonc
{ "operation": "dns_lookup", "host": "example.com" }
```

```jsonc
{ "operation": "tcp_connect", "host": "example.com", "port": 443 }
```

`dns_lookup` returns:

```json
{
  "host": "example.com",
  "addresses": ["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"],
  "count": 2
}
```

`tcp_connect` returns:

```json
{
  "host": "example.com",
  "port": 443,
  "connected": true,
  "rtt_ms": 38,
  "local_addr": "192.0.2.10:53412",
  "peer_addr": "93.184.216.34:443"
}
```

## Security

- All operations require `autonomy != read-only` and consume the hourly action budget via `SecurityPolicy.record_action()`.
- Resolved addresses are re-checked after DNS — if any resolves to a non-global IP and `allow_private_hosts = false`, the call fails (prevents DNS rebinding).
- Port `0` and ports above `65535` are rejected.
