# Email

The `email` tool gives the agent outbound (SMTP via [lettre](https://github.com/lettre/lettre)) and inbound (IMAP via [async-imap](https://github.com/async-email/async-imap)) email capabilities, with strict address allowlists and workspace-scoped attachments.

Operations:

- `send` — SMTP envelope; optional attachments from inside `workspace_dir`.
- `list_mailboxes` — IMAP LIST.
- `list_messages` — IMAP fetch metadata (UID, From, Subject, size) of recent messages.
- `fetch_message` — IMAP fetch envelope + plain-text body of one UID.

## Configuration

```toml
[email]
enabled = true

# Outbound (SMTP)
smtp_host = "smtp.example.com"
smtp_port = 587
smtp_username = "agent@example.com"
smtp_password = "..."                  # stored via OS keyring
smtp_tls = "starttls"                  # "starttls" | "implicittls" | "none"
from_address = "agent@example.com"     # default From when omitted
from_allowlist = ["agent@example.com"] # anti-spoofing
to_allowlist = ["*@example.com",       # *@domain wildcard
                "stakeholder@partner.com"]
max_recipients = 10
allow_attachments = true
max_attachment_bytes = 5_242_880

# Inbound (IMAP)
imap_host = "imap.example.com"
imap_port = 993
imap_username = "agent@example.com"
imap_password = "..."                  # stored via OS keyring

# Common
timeout_secs = 30
max_body_bytes = 65_536
```

Leaving `smtp_host` empty disables `send`. Leaving `imap_host` empty disables the read operations. Both can run independently.

## Invocation

### Send (text, no attachments)

```jsonc
{
  "operation": "send",
  "to": ["alice@example.com", "bob@example.com"],
  "subject": "Daily summary",
  "body": "Pipeline OK. 42 records processed."
}
```

### Send with attachments

```jsonc
{
  "operation": "send",
  "to": ["alice@example.com"],
  "subject": "Report attached",
  "body": "See attached.",
  "attachments": ["reports/2026-05/summary.pdf"]
}
```

Attachment paths are resolved relative to the workspace and must canonicalize inside `workspace_dir` — anything outside is rejected. Size is checked against `max_attachment_bytes`.

### List mailboxes

```jsonc
{ "operation": "list_mailboxes" }
```

### List recent messages

```jsonc
{ "operation": "list_messages", "mailbox": "INBOX", "limit": 20 }
```

Returns the last `limit` UIDs (capped at 200) with envelope metadata. Each item: `{uid, size, subject, from}`.

### Fetch one message

```jsonc
{ "operation": "fetch_message", "mailbox": "INBOX", "uid": 12345 }
```

Returns `{uid, mailbox, subject, from, body_text}` with the plain-text body truncated to `max_body_bytes`.

## Security

- **From allowlist**: every `send` validates the From address (default or override) against `from_allowlist`. Prevents the agent from forging sender identities.
- **To allowlist**: every recipient is matched against `to_allowlist`, accepting `*@domain` wildcards. Prevents spam blast-out.
- **Recipient cap**: `max_recipients` is a hard ceiling per call.
- **Attachments**: each attachment path is canonicalized; if it escapes `workspace_dir`, the call fails. Size is bounded by `max_attachment_bytes`. Set `allow_attachments = false` to forbid attachments entirely.
- **Credentials**: both SMTP and IMAP passwords are `#[secret]` config fields, stored in the OS keyring and never logged.
- **TLS**: IMAP always uses TLS (rustls + OS root certs via `rustls-native-certs`). SMTP TLS mode is configurable; `none` should only be used against local test relays.
- **Budget**: every operation consumes one slot in `SecurityPolicy.record_action()`. Read-only autonomy blocks all email operations.

## Operational notes

- For Gmail with 2FA, use an App Password as `smtp_password` / `imap_password`.
- Office 365 / Outlook: SMTP `smtp.office365.com:587` STARTTLS; IMAP `outlook.office365.com:993` TLS.
- Some IMAP servers reject `RFC822` body fetches for very large messages; in that case the operation returns the parser's best-effort text. Raise `max_body_bytes` only when needed — large captures inflate the LLM context.
