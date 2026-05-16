//! Email tool — SMTP outbound (lettre) and IMAP inbound (async-imap).
//!
//! Operations:
//! - `send` — SMTP envelope; optional attachments validated against `workspace_dir`.
//! - `list_mailboxes` — IMAP LIST.
//! - `list_messages` — IMAP fetch metadata of recent messages.
//! - `fetch_message` — IMAP fetch envelope + plain-text body of one UID.
//!
//! Security:
//! - from-address allowlist (anti-spoofing)
//! - to-address allowlist (anti-spam) with `*@domain` wildcards
//! - hard cap on recipient count per call
//! - attachments must reside within `workspace_dir` after canonicalization
//! - SMTP/IMAP credentials sourced from `#[secret]` config fields
//! - `SecurityPolicy.record_action()` consumes the hourly action budget

use async_imap::Client as ImapClient;
use async_trait::async_trait;
use lettre::AsyncSmtpTransport;
use lettre::AsyncTransport;
use lettre::Tokio1Executor;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use mail_parser::MessageParser;
use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls_pki_types::ServerName;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::{EmailConfig, EmailTlsMode};

pub struct EmailTool {
    security: Arc<SecurityPolicy>,
    config: EmailConfig,
}

impl EmailTool {
    pub fn new(security: Arc<SecurityPolicy>, config: EmailConfig) -> Self {
        Self { security, config }
    }

    fn validate_from(&self, addr: &str) -> Result<Mailbox, String> {
        let mb: Mailbox = addr
            .parse()
            .map_err(|e| format!("invalid from address '{addr}': {e}"))?;
        if self
            .config
            .from_allowlist
            .iter()
            .any(|a| a.eq_ignore_ascii_case(mb.email.as_ref()))
        {
            Ok(mb)
        } else {
            Err(format!(
                "from address '{addr}' is not in email.from_allowlist"
            ))
        }
    }

    fn validate_to(&self, addr: &str) -> Result<Mailbox, String> {
        let mb: Mailbox = addr
            .parse()
            .map_err(|e| format!("invalid recipient '{addr}': {e}"))?;
        let email_str = mb.email.to_string().to_lowercase();
        let allowed = self.config.to_allowlist.iter().any(|pattern| {
            let p = pattern.to_lowercase();
            if let Some(domain) = p.strip_prefix("*@") {
                email_str.split('@').nth(1).is_some_and(|d| d == domain)
            } else {
                p == email_str
            }
        });
        if allowed {
            Ok(mb)
        } else {
            Err(format!("recipient '{addr}' is not in email.to_allowlist"))
        }
    }

    fn validate_attachment_path(&self, raw: &str) -> Result<(PathBuf, String), String> {
        let p = PathBuf::from(raw);
        let canon = p
            .canonicalize()
            .map_err(|e| format!("attachment '{raw}' is not accessible: {e}"))?;
        let workspace = self
            .security
            .workspace_dir
            .canonicalize()
            .map_err(|e| format!("workspace_dir not accessible: {e}"))?;
        if !canon.starts_with(&workspace) {
            return Err(format!("attachment '{raw}' is outside workspace_dir"));
        }
        let size = std::fs::metadata(&canon)
            .map(|m| m.len())
            .map_err(|e| format!("cannot stat attachment '{raw}': {e}"))?;
        if size > self.config.max_attachment_bytes {
            return Err(format!(
                "attachment '{raw}' size {size} exceeds max_attachment_bytes {}",
                self.config.max_attachment_bytes
            ));
        }
        let filename = canon
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("attachment '{raw}' has no readable filename"))?
            .to_string();
        Ok((canon, filename))
    }

    async fn op_send(&self, args: &serde_json::Value) -> ToolResult {
        if !self.config.smtp_enabled() {
            return failure("SMTP is not configured (set email.smtp_host)".into());
        }
        let from_str = match args.get("from").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => self.config.from_address.clone(),
        };
        if from_str.is_empty() {
            return failure("missing 'from' and email.from_address is empty".into());
        }
        let from = match self.validate_from(&from_str) {
            Ok(m) => m,
            Err(e) => return failure(e),
        };

        let to_arr = match args.get("to").and_then(|v| v.as_array()) {
            Some(a) if !a.is_empty() => a.clone(),
            _ => return failure("'to' is required and must be a non-empty array".into()),
        };
        if to_arr.len() > self.config.max_recipients {
            return failure(format!(
                "{} recipients exceeds max_recipients {}",
                to_arr.len(),
                self.config.max_recipients
            ));
        }

        let subject = args
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = args
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut builder = Message::builder().from(from.clone()).subject(subject);
        for v in &to_arr {
            let addr = match v.as_str() {
                Some(s) => s,
                None => return failure("'to' items must be strings".into()),
            };
            let mb = match self.validate_to(addr) {
                Ok(m) => m,
                Err(e) => return failure(e),
            };
            builder = builder.to(mb);
        }

        let attachments_paths: Vec<(PathBuf, String)> =
            match args.get("attachments").and_then(|v| v.as_array()) {
                None => vec![],
                Some(arr) => {
                    if !self.config.allow_attachments {
                        return failure(
                            "attachments are disabled (email.allow_attachments = false)".into(),
                        );
                    }
                    let mut out = Vec::with_capacity(arr.len());
                    for item in arr {
                        let p = match item.as_str() {
                            Some(s) => s,
                            None => return failure("attachment items must be strings".into()),
                        };
                        match self.validate_attachment_path(p) {
                            Ok(x) => out.push(x),
                            Err(e) => return failure(e),
                        }
                    }
                    out
                }
            };

        let message = if attachments_paths.is_empty() {
            builder
                .header(ContentType::TEXT_PLAIN)
                .body(body)
                .map_err(|e| failure_string(format!("failed to build message: {e}")))
        } else {
            let mut multi = MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(body),
            );
            for (path, filename) in &attachments_paths {
                let bytes = match std::fs::read(path) {
                    Ok(b) => b,
                    Err(e) => {
                        return failure(format!(
                            "failed to read attachment '{}': {e}",
                            path.display()
                        ));
                    }
                };
                let content_type = ContentType::parse("application/octet-stream")
                    .unwrap_or(ContentType::TEXT_PLAIN);
                multi =
                    multi.singlepart(Attachment::new(filename.clone()).body(bytes, content_type));
            }
            builder
                .multipart(multi)
                .map_err(|e| failure_string(format!("failed to build multipart message: {e}")))
        };
        let message = match message {
            Ok(m) => m,
            Err(f) => return f,
        };

        let transport = match self.build_smtp_transport() {
            Ok(t) => t,
            Err(e) => return failure(e),
        };
        match transport.send(message).await {
            Ok(resp) => {
                let output = json!({
                    "sent": true,
                    "from": from.email.to_string(),
                    "to_count": to_arr.len(),
                    "smtp_code": resp.code().to_string(),
                });
                ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                    error: None,
                }
            }
            Err(e) => failure(format!("SMTP send failed: {e}")),
        }
    }

    fn build_smtp_transport(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
        let host = &self.config.smtp_host;
        let port = self.config.smtp_port;
        let builder = match self.config.smtp_tls {
            EmailTlsMode::ImplicitTls => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
                .map_err(|e| format!("SMTP TLS relay setup failed: {e}"))?,
            EmailTlsMode::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|e| format!("SMTP STARTTLS relay setup failed: {e}"))?,
            EmailTlsMode::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
        };
        let mut builder = builder
            .port(port)
            .timeout(Some(Duration::from_secs(self.config.timeout_secs)));
        if let (Some(user), Some(pass)) = (
            self.config.smtp_username.as_deref(),
            self.config.smtp_password.as_deref(),
        ) && !user.is_empty()
            && !pass.is_empty()
        {
            builder = builder.credentials(Credentials::new(user.into(), pass.into()));
        }
        Ok(builder.build())
    }

    async fn imap_session(
        &self,
    ) -> Result<async_imap::Session<tokio_rustls::client::TlsStream<TcpStream>>, String> {
        let host = self.config.imap_host.clone();
        let port = self.config.imap_port;
        let username = self
            .config
            .imap_username
            .clone()
            .ok_or_else(|| "email.imap_username is required".to_string())?;
        let password = self
            .config
            .imap_password
            .clone()
            .ok_or_else(|| "email.imap_password is required".to_string())?;

        let tcp = timeout(
            Duration::from_secs(self.config.timeout_secs),
            TcpStream::connect((host.as_str(), port)),
        )
        .await
        .map_err(|_| "IMAP connect timed out".to_string())?
        .map_err(|e| format!("IMAP TCP connect failed: {e}"))?;

        let mut root_store = RootCertStore::empty();
        let native = rustls_native_certs::load_native_certs();
        if !native.errors.is_empty() {
            tracing::debug!(
                "rustls-native-certs returned {} non-fatal errors while loading the OS store",
                native.errors.len()
            );
        }
        for cert in native.certs {
            let _ = root_store.add(cert);
        }
        if root_store.is_empty() {
            return Err(
                "no root certificates loaded from OS store — IMAP TLS cannot proceed".into(),
            );
        }
        let tls_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(tls_config));
        let dns_name = ServerName::try_from(host.clone())
            .map_err(|e| format!("invalid IMAP host '{host}': {e}"))?;
        let tls = connector
            .connect(dns_name, tcp)
            .await
            .map_err(|e| format!("IMAP TLS handshake failed: {e}"))?;

        let client = ImapClient::new(tls);
        let session = client
            .login(&username, &password)
            .await
            .map_err(|(e, _)| format!("IMAP login failed: {e}"))?;
        Ok(session)
    }

    async fn op_list_mailboxes(&self) -> ToolResult {
        if !self.config.imap_enabled() {
            return failure("IMAP is not configured (set email.imap_host)".into());
        }
        let mut session = match self.imap_session().await {
            Ok(s) => s,
            Err(e) => return failure(e),
        };

        let collected: Result<Vec<String>, String> = {
            use futures_util::StreamExt;
            match session.list(Some(""), Some("*")).await {
                Ok(stream) => {
                    let mut s = std::pin::pin!(stream);
                    let mut out: Vec<String> = Vec::new();
                    let mut err: Option<String> = None;
                    while let Some(entry) = s.next().await {
                        match entry {
                            Ok(name) => out.push(name.name().to_string()),
                            Err(e) => {
                                err = Some(format!("IMAP LIST item failed: {e}"));
                                break;
                            }
                        }
                    }
                    match err {
                        Some(e) => Err(e),
                        None => Ok(out),
                    }
                }
                Err(e) => Err(format!("IMAP LIST failed: {e}")),
            }
        };

        let _ = session.logout().await;
        let mailboxes = match collected {
            Ok(v) => v,
            Err(e) => return failure(e),
        };
        let output = json!({ "mailboxes": mailboxes });
        ToolResult {
            success: true,
            output: serde_json::to_string_pretty(&output).unwrap_or_default(),
            error: None,
        }
    }

    async fn op_list_messages(&self, args: &serde_json::Value) -> ToolResult {
        if !self.config.imap_enabled() {
            return failure("IMAP is not configured (set email.imap_host)".into());
        }
        let mailbox = args
            .get("mailbox")
            .and_then(|v| v.as_str())
            .unwrap_or("INBOX")
            .to_string();
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20)
            .clamp(1, 200);

        let mut session = match self.imap_session().await {
            Ok(s) => s,
            Err(e) => return failure(e),
        };
        let total: u64 = match session.select(&mailbox).await {
            Ok(m) => m.exists as u64,
            Err(e) => {
                let _ = session.logout().await;
                return failure(format!("IMAP SELECT '{mailbox}' failed: {e}"));
            }
        };
        if total == 0 {
            let _ = session.logout().await;
            let output = json!({
                "mailbox": mailbox,
                "exists": 0,
                "returned": 0,
                "messages": [],
            });
            return ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&output).unwrap_or_default(),
                error: None,
            };
        }
        let start = total.saturating_sub(limit - 1).max(1);
        let range = format!("{start}:{total}");

        let collected: Result<Vec<serde_json::Value>, String> = {
            use futures_util::StreamExt;
            match session
                .fetch(&range, "(UID ENVELOPE FLAGS RFC822.SIZE)")
                .await
            {
                Ok(stream) => {
                    let mut s = std::pin::pin!(stream);
                    let mut items = Vec::new();
                    while let Some(entry) = s.next().await {
                        if let Ok(msg) = entry {
                            let uid = msg.uid;
                            let size = msg.size;
                            let subject = msg
                                .envelope()
                                .and_then(|e| e.subject.as_ref())
                                .map(|s| String::from_utf8_lossy(s).to_string());
                            let from = msg.envelope().and_then(|e| {
                                e.from.as_ref().and_then(|v| v.first()).map(|addr| {
                                    let mailbox = addr
                                        .mailbox
                                        .as_ref()
                                        .map(|b| String::from_utf8_lossy(b).to_string())
                                        .unwrap_or_default();
                                    let host = addr
                                        .host
                                        .as_ref()
                                        .map(|b| String::from_utf8_lossy(b).to_string())
                                        .unwrap_or_default();
                                    if mailbox.is_empty() && host.is_empty() {
                                        String::new()
                                    } else {
                                        format!("{mailbox}@{host}")
                                    }
                                })
                            });
                            items.push(json!({
                                "uid": uid,
                                "size": size,
                                "subject": subject,
                                "from": from,
                            }));
                        }
                    }
                    Ok(items)
                }
                Err(e) => Err(format!("IMAP FETCH failed: {e}")),
            }
        };

        let _ = session.logout().await;
        let items = match collected {
            Ok(v) => v,
            Err(e) => return failure(e),
        };
        let output = json!({
            "mailbox": mailbox,
            "exists": total,
            "returned": items.len(),
            "messages": items,
        });
        ToolResult {
            success: true,
            output: serde_json::to_string_pretty(&output).unwrap_or_default(),
            error: None,
        }
    }

    async fn op_fetch_message(&self, args: &serde_json::Value) -> ToolResult {
        if !self.config.imap_enabled() {
            return failure("IMAP is not configured (set email.imap_host)".into());
        }
        let mailbox = args
            .get("mailbox")
            .and_then(|v| v.as_str())
            .unwrap_or("INBOX")
            .to_string();
        let uid = match args.get("uid").and_then(|v| v.as_u64()) {
            Some(u) => u,
            None => return failure("'uid' (u32) is required for fetch_message".into()),
        };

        let mut session = match self.imap_session().await {
            Ok(s) => s,
            Err(e) => return failure(e),
        };
        if let Err(e) = session.select(&mailbox).await {
            let _ = session.logout().await;
            return failure(format!("IMAP SELECT '{mailbox}' failed: {e}"));
        }

        let raw_result: Result<Option<Vec<u8>>, String> = {
            use futures_util::StreamExt;
            match session.uid_fetch(uid.to_string(), "RFC822").await {
                Ok(stream) => {
                    let mut s = std::pin::pin!(stream);
                    let mut raw: Option<Vec<u8>> = None;
                    while let Some(entry) = s.next().await {
                        if let Ok(msg) = entry
                            && let Some(body) = msg.body()
                        {
                            raw = Some(body.to_vec());
                            break;
                        }
                    }
                    Ok(raw)
                }
                Err(e) => Err(format!("IMAP UID FETCH failed: {e}")),
            }
        };

        let _ = session.logout().await;

        let raw_body = match raw_result {
            Ok(v) => v,
            Err(e) => return failure(e),
        };

        let raw = match raw_body {
            Some(b) => b,
            None => return failure(format!("UID {uid} not found in '{mailbox}'")),
        };
        let _ = uid;

        let parsed = MessageParser::default().parse(&raw[..]);
        let (subject, from, body_text) = match parsed {
            Some(p) => {
                let subj = p.subject().map(|s| s.to_string());
                let from = p
                    .from()
                    .and_then(|addrs| addrs.first())
                    .and_then(|addr| addr.address())
                    .map(|a| a.to_string());
                let text = p.body_text(0).map(|c| c.to_string()).unwrap_or_default();
                (subj, from, text)
            }
            None => (None, None, String::from_utf8_lossy(&raw).to_string()),
        };

        let truncated = truncate_to_bytes(body_text, self.config.max_body_bytes);
        let output = json!({
            "uid": uid,
            "mailbox": mailbox,
            "subject": subject,
            "from": from,
            "body_text": truncated,
        });
        ToolResult {
            success: true,
            output: serde_json::to_string_pretty(&output).unwrap_or_default(),
            error: None,
        }
    }
}

fn truncate_to_bytes(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut b = max.min(s.len());
    while b > 0 && !s.is_char_boundary(b) {
        b -= 1;
    }
    s.truncate(b);
    s.push_str("\n... [body truncated]");
    s
}

fn failure(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}

fn failure_string(msg: String) -> ToolResult {
    failure(msg)
}

#[async_trait]
impl Tool for EmailTool {
    fn name(&self) -> &str {
        "email"
    }

    fn description(&self) -> &str {
        "Send and read email via SMTP/IMAP. Operations: 'send' (SMTP outbound with optional \
         attachments from workspace_dir), 'list_mailboxes' (IMAP), 'list_messages' (IMAP recent \
         metadata), 'fetch_message' (IMAP body of one UID). Address allowlists enforced for both \
         sender and recipients."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["send", "list_mailboxes", "list_messages", "fetch_message"]
                },
                "from": { "type": "string", "description": "From address (send only). Defaults to email.from_address." },
                "to": { "type": "array", "items": {"type": "string"}, "description": "Recipient list (send)." },
                "subject": { "type": "string" },
                "body": { "type": "string" },
                "attachments": { "type": "array", "items": {"type": "string"}, "description": "File paths inside workspace_dir." },
                "mailbox": { "type": "string", "description": "IMAP mailbox name (default 'INBOX')." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                "uid": { "type": "integer", "description": "Message UID (fetch_message)." }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(failure(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }
        if let Err(e) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "email")
        {
            return Ok(failure(e));
        }

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'operation' parameter"))?;

        if !self.security.record_action() {
            return Ok(failure(
                "Rate limit exceeded: action budget exhausted".into(),
            ));
        }

        let result = match operation {
            "send" => self.op_send(&args).await,
            "list_mailboxes" => self.op_list_mailboxes().await,
            "list_messages" => self.op_list_messages(&args).await,
            "fetch_message" => self.op_fetch_message(&args).await,
            other => failure(format!(
                "Unknown operation '{other}'. Supported: send, list_mailboxes, list_messages, fetch_message"
            )),
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;

    fn cfg() -> EmailConfig {
        EmailConfig {
            enabled: true,
            smtp_host: "smtp.example.com".into(),
            smtp_port: 587,
            smtp_username: Some("u".into()),
            smtp_password: Some("p".into()),
            smtp_tls: EmailTlsMode::Starttls,
            from_address: "agent@example.com".into(),
            from_allowlist: vec!["agent@example.com".into()],
            to_allowlist: vec!["*@example.com".into(), "specific@other.com".into()],
            max_recipients: 5,
            allow_attachments: true,
            max_attachment_bytes: 1_048_576,
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            imap_username: Some("u".into()),
            imap_password: Some("p".into()),
            timeout_secs: 10,
            max_body_bytes: 65_536,
        }
    }

    fn tool() -> EmailTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        EmailTool::new(security, cfg())
    }

    #[test]
    fn name_is_email() {
        assert_eq!(tool().name(), "email");
    }

    #[test]
    fn schema_requires_operation() {
        let s = tool().parameters_schema();
        let req = s["required"].as_array().unwrap();
        assert!(req.contains(&json!("operation")));
    }

    #[test]
    fn validate_from_allows_listed() {
        assert!(tool().validate_from("agent@example.com").is_ok());
    }

    #[test]
    fn validate_from_rejects_unlisted() {
        let err = tool().validate_from("evil@example.com").unwrap_err();
        assert!(err.contains("from_allowlist"));
    }

    #[test]
    fn validate_from_rejects_malformed() {
        assert!(tool().validate_from("not-an-email").is_err());
    }

    #[test]
    fn validate_to_wildcard_domain() {
        assert!(tool().validate_to("anyone@example.com").is_ok());
    }

    #[test]
    fn validate_to_specific_address() {
        assert!(tool().validate_to("specific@other.com").is_ok());
    }

    #[test]
    fn validate_to_rejects_other_domain() {
        let err = tool().validate_to("someone@evil.com").unwrap_err();
        assert!(err.contains("to_allowlist"));
    }

    #[test]
    fn validate_to_is_case_insensitive_for_domain() {
        assert!(tool().validate_to("USER@Example.COM").is_ok());
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let t = EmailTool::new(security, cfg());
        let r = t
            .execute(json!({"operation": "list_mailboxes"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("read-only"));
    }

    #[tokio::test]
    async fn execute_blocks_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            max_actions_per_hour: 0,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let t = EmailTool::new(security, cfg());
        let r = t
            .execute(json!({"operation": "list_mailboxes"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_unknown_operation_non_fatal() {
        let r = tool()
            .execute(json!({"operation": "delete_all"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Unknown operation"));
    }

    #[tokio::test]
    async fn execute_missing_operation_errors() {
        let r = tool().execute(json!({})).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn send_requires_to_array() {
        let r = tool()
            .execute(json!({"operation": "send", "subject": "x", "body": "y"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("'to' is required"));
    }

    #[tokio::test]
    async fn send_rejects_too_many_recipients() {
        let mut recipients = Vec::new();
        for i in 0..10 {
            recipients.push(format!("user{i}@example.com"));
        }
        let r = tool()
            .execute(json!({"operation": "send", "to": recipients, "subject": "x", "body": "y"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("max_recipients"));
    }

    #[tokio::test]
    async fn send_rejects_disallowed_recipient() {
        let r = tool()
            .execute(json!({
                "operation": "send",
                "to": ["agent@evil.com"],
                "subject": "x", "body": "y"
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("to_allowlist"));
    }

    #[test]
    fn truncate_below_limit_unchanged() {
        let s = truncate_to_bytes("hello".into(), 100);
        assert_eq!(s, "hello");
    }

    #[test]
    fn truncate_above_limit_marked() {
        let s = truncate_to_bytes("hello world this is longer".into(), 5);
        assert!(s.contains("[body truncated]"));
        assert!(s.starts_with("hello"));
    }
}
