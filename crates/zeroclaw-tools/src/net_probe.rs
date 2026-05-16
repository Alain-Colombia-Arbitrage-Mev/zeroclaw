//! Network probe tool — DNS lookups and TCP connectivity checks.
//!
//! Read-style network diagnostics with the same SSRF safeguards as
//! `http_request`: allowlist-only hosts, private/loopback addresses blocked
//! by default (opt-in via `allow_private_hosts`), per-call timeout.
//!
//! Scope is intentionally narrow: no raw sockets, no UDP, no ICMP. TLS cert
//! inspection is out of scope for v1 to avoid pulling in `tokio-rustls` /
//! `x509-parser` as direct dependencies.

use async_trait::async_trait;
use serde_json::json;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpStream, lookup_host};
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_config::schema::NetProbeConfig;

/// Maximum port number permitted.
const MAX_PORT: u16 = u16::MAX;

/// Minimum port number permitted (reject port 0).
const MIN_PORT: u16 = 1;

/// Network diagnostics tool: dns_lookup and tcp_connect.
pub struct NetProbeTool {
    security: Arc<SecurityPolicy>,
    config: NetProbeConfig,
    allowed_hosts: Vec<String>,
}

impl NetProbeTool {
    pub fn new(security: Arc<SecurityPolicy>, config: NetProbeConfig) -> Self {
        let allowed_hosts = normalize_allowed_hosts(config.allowed_hosts.clone());
        Self {
            security,
            config,
            allowed_hosts,
        }
    }

    fn validate_host(&self, host: &str) -> Result<String, String> {
        let h = host.trim();
        if h.is_empty() {
            return Err("host cannot be empty".into());
        }
        if h.chars().any(char::is_whitespace) {
            return Err("host cannot contain whitespace".into());
        }
        if h.contains('@') || h.contains('/') || h.contains('?') {
            return Err("host must be a bare hostname or IP, not a URL".into());
        }

        if self.allowed_hosts.is_empty() {
            return Err(
                "net_probe is enabled but no allowed_hosts are configured. Add [net_probe].allowed_hosts in config.toml"
                    .into(),
            );
        }

        let lower = h.to_lowercase();
        if !self.config.allow_private_hosts && is_private_or_local_host(&lower) {
            return Err(format!("blocked local/private host: {h}"));
        }

        if !host_matches_allowlist(&lower, &self.allowed_hosts) {
            return Err(format!("host '{h}' is not in net_probe.allowed_hosts"));
        }

        Ok(lower)
    }

    fn validate_port(port: u64) -> Result<u16, String> {
        if !(MIN_PORT as u64..=MAX_PORT as u64).contains(&port) {
            return Err(format!(
                "port must be in [{MIN_PORT}, {MAX_PORT}], got {port}"
            ));
        }
        Ok(port as u16)
    }

    /// Verifies resolved IPs honor the private-host policy.
    fn resolved_addrs_allowed(&self, addrs: &[IpAddr]) -> Result<(), String> {
        if self.config.allow_private_hosts {
            return Ok(());
        }
        for ip in addrs {
            if is_private_ip(*ip) {
                return Err(format!(
                    "resolved address {ip} is non-global and allow_private_hosts is false"
                ));
            }
        }
        Ok(())
    }

    async fn op_dns_lookup(&self, host: &str) -> ToolResult {
        let validated_host = match self.validate_host(host) {
            Ok(h) => h,
            Err(e) => return failure(e),
        };

        let target = format!("{validated_host}:0");
        let timeout = Duration::from_millis(self.config.timeout_ms);

        match tokio::time::timeout(timeout, lookup_host(target.as_str())).await {
            Ok(Ok(iter)) => {
                let mut ips: Vec<IpAddr> = iter.map(|sa| sa.ip()).collect();
                ips.sort_unstable();
                ips.dedup();

                if let Err(e) = self.resolved_addrs_allowed(&ips) {
                    return failure(e);
                }

                let output = json!({
                    "host": validated_host,
                    "addresses": ips.iter().map(|ip| ip.to_string()).collect::<Vec<_>>(),
                    "count": ips.len(),
                });
                ToolResult {
                    success: !ips.is_empty(),
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: if ips.is_empty() {
                        Some("no addresses resolved".into())
                    } else {
                        None
                    },
                }
            }
            Ok(Err(e)) => failure(format!("DNS lookup failed: {e}")),
            Err(_) => failure(format!(
                "DNS lookup timed out after {}ms",
                self.config.timeout_ms
            )),
        }
    }

    async fn op_tcp_connect(&self, host: &str, port: u16) -> ToolResult {
        let validated_host = match self.validate_host(host) {
            Ok(h) => h,
            Err(e) => return failure(e),
        };

        let target = format!("{validated_host}:{port}");
        let timeout = Duration::from_millis(self.config.timeout_ms);

        // Resolve first so we can apply private-IP policy before connecting.
        let resolved: Vec<IpAddr> =
            match tokio::time::timeout(timeout, lookup_host(target.as_str())).await {
                Ok(Ok(iter)) => iter.map(|sa| sa.ip()).collect(),
                Ok(Err(e)) => return failure(format!("DNS lookup failed: {e}")),
                Err(_) => {
                    return failure(format!(
                        "DNS lookup timed out after {}ms",
                        self.config.timeout_ms
                    ));
                }
            };

        if resolved.is_empty() {
            return failure("no addresses resolved".into());
        }

        if let Err(e) = self.resolved_addrs_allowed(&resolved) {
            return failure(e);
        }

        let start = std::time::Instant::now();
        match tokio::time::timeout(timeout, TcpStream::connect(target.as_str())).await {
            Ok(Ok(stream)) => {
                let elapsed_ms = start.elapsed().as_millis();
                let local = stream
                    .local_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown".into());
                let peer = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown".into());
                drop(stream);
                let output = json!({
                    "host": validated_host,
                    "port": port,
                    "connected": true,
                    "rtt_ms": elapsed_ms,
                    "local_addr": local,
                    "peer_addr": peer,
                });
                ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: None,
                }
            }
            Ok(Err(e)) => failure(format!("TCP connect failed: {e}")),
            Err(_) => failure(format!(
                "TCP connect timed out after {}ms",
                self.config.timeout_ms
            )),
        }
    }
}

fn failure(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}

#[async_trait]
impl Tool for NetProbeTool {
    fn name(&self) -> &str {
        "net_probe"
    }

    fn description(&self) -> &str {
        "Network diagnostics. Operations: 'dns_lookup' resolves a host to A/AAAA addresses; \
         'tcp_connect' attempts a TCP handshake and reports RTT. Allowlist-only hosts; \
         private/loopback addresses blocked by default."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["dns_lookup", "tcp_connect"],
                    "description": "Diagnostic operation to run"
                },
                "host": {
                    "type": "string",
                    "description": "Hostname or IP address (must be in allowed_hosts)"
                },
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535,
                    "description": "TCP port (required for tcp_connect, ignored for dns_lookup)"
                }
            },
            "required": ["operation", "host"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if !self.security.can_act() {
            return Ok(failure("Action blocked: autonomy is read-only".into()));
        }
        if !self.security.record_action() {
            return Ok(failure("Action blocked: rate limit exceeded".into()));
        }

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'operation' parameter"))?;

        let host = args
            .get("host")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'host' parameter"))?;

        let result = match operation {
            "dns_lookup" => self.op_dns_lookup(host).await,
            "tcp_connect" => {
                let port_val = args.get("port").and_then(|v| v.as_u64()).ok_or_else(|| {
                    anyhow::anyhow!("'port' is required for operation 'tcp_connect'")
                })?;
                let port = match Self::validate_port(port_val) {
                    Ok(p) => p,
                    Err(e) => return Ok(failure(e)),
                };
                self.op_tcp_connect(host, port).await
            }
            other => failure(format!(
                "Unknown operation '{other}'. Supported: dns_lookup, tcp_connect"
            )),
        };

        Ok(result)
    }
}

// ── Helpers (locally owned; intentionally parallel to http_request's SSRF
//    defenses so net_probe's policy can evolve independently) ─────────────

fn normalize_allowed_hosts(hosts: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = hosts
        .into_iter()
        .filter_map(|h| {
            let t = h.trim().to_lowercase();
            if t.is_empty() || t.chars().any(char::is_whitespace) {
                None
            } else {
                Some(t)
            }
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn host_matches_allowlist(host: &str, allowed: &[String]) -> bool {
    if allowed.iter().any(|d| d == "*") {
        return true;
    }
    allowed.iter().any(|d| {
        host == d
            || host
                .strip_suffix(d.as_str())
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

fn is_private_or_local_host(host: &str) -> bool {
    let bare = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);

    if bare == "localhost" || bare.ends_with(".localhost") {
        return true;
    }
    if bare.rsplit('.').next().is_some_and(|tld| tld == "local") {
        return true;
    }

    if let Ok(ip) = bare.parse::<IpAddr>() {
        return is_private_ip(ip);
    }
    false
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || (a == 100 && (64..=127).contains(&b))
                || a >= 240
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 198 && b == 51)
                || (a == 203 && b == 0)
                || (a == 198 && (18..=19).contains(&b))
        }
        IpAddr::V6(v6) => {
            let segs = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (segs[0] & 0xfe00) == 0xfc00
                || (segs[0] & 0xffc0) == 0xfe80
                || (segs[0] == 0x2001 && segs[1] == 0x0db8)
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_private_ip(IpAddr::V4(v4)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;

    fn tool_with(allowed: Vec<&str>, allow_private: bool) -> NetProbeTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            ..SecurityPolicy::default()
        });
        let config = NetProbeConfig {
            enabled: true,
            allowed_hosts: allowed.into_iter().map(String::from).collect(),
            allow_private_hosts: allow_private,
            timeout_ms: 5000,
        };
        NetProbeTool::new(security, config)
    }

    #[test]
    fn name_is_net_probe() {
        let t = tool_with(vec!["example.com"], false);
        assert_eq!(t.name(), "net_probe");
    }

    #[test]
    fn schema_requires_operation_and_host() {
        let t = tool_with(vec!["example.com"], false);
        let s = t.parameters_schema();
        let required = s["required"].as_array().unwrap();
        assert!(required.contains(&json!("operation")));
        assert!(required.contains(&json!("host")));
    }

    #[test]
    fn validate_host_requires_allowlist() {
        let t = tool_with(vec![], false);
        let err = t.validate_host("example.com").unwrap_err();
        assert!(err.contains("allowed_hosts"));
    }

    #[test]
    fn validate_host_accepts_exact_match() {
        let t = tool_with(vec!["example.com"], false);
        assert_eq!(t.validate_host("example.com").unwrap(), "example.com");
    }

    #[test]
    fn validate_host_accepts_subdomain() {
        let t = tool_with(vec!["example.com"], false);
        assert!(t.validate_host("api.example.com").is_ok());
    }

    #[test]
    fn validate_host_rejects_unrelated() {
        let t = tool_with(vec!["example.com"], false);
        assert!(t.validate_host("evil.com").is_err());
    }

    #[test]
    fn validate_host_rejects_url_form() {
        let t = tool_with(vec!["example.com"], false);
        assert!(t.validate_host("https://example.com").is_err());
        assert!(t.validate_host("example.com/path").is_err());
        assert!(t.validate_host("user@example.com").is_err());
    }

    #[test]
    fn validate_host_rejects_whitespace() {
        let t = tool_with(vec!["example.com"], false);
        assert!(t.validate_host("foo bar").is_err());
        assert!(t.validate_host("").is_err());
    }

    #[test]
    fn validate_host_blocks_localhost_by_default() {
        let t = tool_with(vec!["localhost", "*"], false);
        let err = t.validate_host("localhost").unwrap_err();
        assert!(err.contains("local/private"));
    }

    #[test]
    fn validate_host_blocks_private_ipv4_by_default() {
        let t = tool_with(vec!["*"], false);
        let err = t.validate_host("192.168.1.1").unwrap_err();
        assert!(err.contains("local/private"));
    }

    #[test]
    fn allow_private_hosts_permits_localhost() {
        let t = tool_with(vec!["localhost"], true);
        assert!(t.validate_host("localhost").is_ok());
    }

    #[test]
    fn allow_private_hosts_still_requires_allowlist() {
        let t = tool_with(vec!["example.com"], true);
        assert!(t.validate_host("192.168.1.1").is_err());
    }

    #[test]
    fn wildcard_allowlist_matches_public_host() {
        let t = tool_with(vec!["*"], false);
        assert!(t.validate_host("docs.rs").is_ok());
        assert!(t.validate_host("crates.io").is_ok());
    }

    #[test]
    fn validate_port_rejects_zero_and_oversized() {
        assert!(NetProbeTool::validate_port(0).is_err());
        assert!(NetProbeTool::validate_port(70_000).is_err());
    }

    #[test]
    fn validate_port_accepts_in_range() {
        assert_eq!(NetProbeTool::validate_port(80).unwrap(), 80);
        assert_eq!(NetProbeTool::validate_port(65535).unwrap(), 65535);
    }

    #[test]
    fn resolved_addrs_allowed_blocks_private_when_strict() {
        let t = tool_with(vec!["*"], false);
        let private = "10.0.0.1".parse::<IpAddr>().unwrap();
        assert!(t.resolved_addrs_allowed(&[private]).is_err());
    }

    #[test]
    fn resolved_addrs_allowed_permits_public() {
        let t = tool_with(vec!["*"], false);
        let public = "8.8.8.8".parse::<IpAddr>().unwrap();
        assert!(t.resolved_addrs_allowed(&[public]).is_ok());
    }

    #[test]
    fn resolved_addrs_allowed_skips_check_when_private_allowed() {
        let t = tool_with(vec!["*"], true);
        let private = "10.0.0.1".parse::<IpAddr>().unwrap();
        assert!(t.resolved_addrs_allowed(&[private]).is_ok());
    }

    #[test]
    fn is_private_ip_v4_loopback_and_rfc1918() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn is_private_ip_v4_public() {
        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn is_private_ip_v6_loopback_and_link_local() {
        assert!(is_private_ip("::1".parse().unwrap()));
        assert!(is_private_ip("fe80::1".parse().unwrap()));
        assert!(is_private_ip("fc00::1".parse().unwrap()));
    }

    #[test]
    fn normalize_allowed_hosts_dedupes_and_lowercases() {
        let out = normalize_allowed_hosts(vec![
            "Example.com".into(),
            "example.com".into(),
            "  ".into(),
            "Docs.Rs".into(),
        ]);
        assert_eq!(out, vec!["docs.rs".to_string(), "example.com".to_string()]);
    }

    #[tokio::test]
    async fn execute_blocks_readonly_mode() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            ..SecurityPolicy::default()
        });
        let cfg = NetProbeConfig {
            enabled: true,
            allowed_hosts: vec!["example.com".into()],
            allow_private_hosts: false,
            timeout_ms: 1000,
        };
        let t = NetProbeTool::new(security, cfg);
        let res = t
            .execute(json!({"operation": "dns_lookup", "host": "example.com"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("read-only"));
    }

    #[tokio::test]
    async fn execute_blocks_when_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            max_actions_per_hour: 0,
            ..SecurityPolicy::default()
        });
        let cfg = NetProbeConfig {
            enabled: true,
            allowed_hosts: vec!["example.com".into()],
            allow_private_hosts: false,
            timeout_ms: 1000,
        };
        let t = NetProbeTool::new(security, cfg);
        let res = t
            .execute(json!({"operation": "dns_lookup", "host": "example.com"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("rate limit"));
    }

    #[tokio::test]
    async fn execute_rejects_unknown_operation() {
        let t = tool_with(vec!["example.com"], false);
        let res = t
            .execute(json!({"operation": "ping_icmp", "host": "example.com"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("Unknown operation"));
    }

    #[tokio::test]
    async fn execute_missing_operation_param_errors() {
        let t = tool_with(vec!["example.com"], false);
        let res = t.execute(json!({"host": "example.com"})).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn execute_tcp_connect_requires_port() {
        let t = tool_with(vec!["example.com"], false);
        let res = t
            .execute(json!({"operation": "tcp_connect", "host": "example.com"}))
            .await;
        assert!(res.is_err());
    }
}
