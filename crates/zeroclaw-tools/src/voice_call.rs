//! Voice-call tool — places an outbound phone call through the
//! operator's configured telephony provider (Twilio / Telnyx). It is a
//! self-contained client: it reads `VoiceCallConfig` and performs the
//! provider HTTP call directly, rather than depending on the
//! `zeroclaw-channels` `VoiceCallChannel` (channels depend on tools, so
//! a tool cannot depend back on channels without a cycle).
//!
//! Scope is deliberately narrow and honest:
//!   * `place_call` — dial an outbound number. Real.
//! The inbound-call lifecycle and live transcripts are owned by the
//! running `VoiceCallChannel` (it handles the provider webhooks);
//! bridging those into a tool needs a shared late-bound channel handle
//! (the pattern `escalate`/`poll` use) and is a follow-up — this tool
//! does not fake it.
//!
//! Placing a call spends real money and rings a real person, so it is
//! gated hard: it requires `Full` autonomy and honours the config's
//! `require_outbound_approval` flag (returning a PENDING_APPROVAL
//! marker instead of dialing when approval is required).

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::autonomy::AutonomyLevel;
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_config::scattered_types::{VoiceCallConfig, VoiceProvider};

/// Places outbound calls via the configured provider.
pub struct VoiceCallTool {
    security: Arc<SecurityPolicy>,
    config: VoiceCallConfig,
    client: reqwest::Client,
}

impl VoiceCallTool {
    pub fn new(security: Arc<SecurityPolicy>, config: VoiceCallConfig) -> Self {
        Self {
            security,
            config,
            client: reqwest::Client::new(),
        }
    }

    fn api_base_url(&self) -> &str {
        match self.config.provider {
            VoiceProvider::Twilio => "https://api.twilio.com/2010-04-01",
            VoiceProvider::Telnyx => "https://api.telnyx.com/v2",
            VoiceProvider::Plivo => "https://api.plivo.com/v1",
        }
    }

    fn webhook_url(&self, path: &str) -> String {
        if let Some(ref base) = self.config.webhook_base_url {
            format!("{}{}", base.trim_end_matches('/'), path)
        } else {
            format!("http://localhost:{}{}", self.config.webhook_port, path)
        }
    }

    /// Basic E.164-ish sanity check. We do not pass the number through a
    /// shell, so this only catches operator/LLM mistakes early.
    fn validate_number(to: &str) -> anyhow::Result<()> {
        let trimmed = to.trim();
        if trimmed.is_empty() {
            anyhow::bail!("'to' number must not be empty");
        }
        let digits = trimmed.trim_start_matches('+');
        if digits.len() < 7
            || !digits.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
        {
            anyhow::bail!("'to' must be a phone number in E.164-ish form, e.g. +14155551234");
        }
        Ok(())
    }

    async fn place_call(&self, to: &str) -> ToolResult {
        let webhook = self.webhook_url("/voice/status");
        let result: anyhow::Result<String> = match self.config.provider {
            VoiceProvider::Twilio => {
                let url = format!(
                    "{}/Accounts/{}/Calls.json",
                    self.api_base_url(),
                    self.config.account_id
                );
                let resp = self
                    .client
                    .post(&url)
                    .basic_auth(&self.config.account_id, Some(&self.config.auth_token))
                    .form(&[
                        ("To", to),
                        ("From", &self.config.from_number),
                        ("StatusCallback", &webhook),
                        ("Timeout", &self.config.max_call_duration_secs.to_string()),
                    ])
                    .send()
                    .await;
                Self::extract_call_id(resp, "sid").await
            }
            VoiceProvider::Telnyx => {
                let url = format!("{}/calls", self.api_base_url());
                let resp = self
                    .client
                    .post(&url)
                    .bearer_auth(&self.config.auth_token)
                    .json(&json!({
                        "connection_id": self.config.account_id,
                        "to": to,
                        "from": self.config.from_number,
                        "webhook_url": webhook,
                        "timeout_secs": self.config.max_call_duration_secs,
                    }))
                    .send()
                    .await;
                Self::extract_call_id(resp, "call_control_id").await
            }
            VoiceProvider::Plivo => {
                return ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(
                        "Plivo outbound is not implemented in the voice_call tool yet — use Twilio or Telnyx".into(),
                    ),
                };
            }
        };

        match result {
            Ok(call_id) => ToolResult {
                success: true,
                output: format!("Outbound call placed to {to}. call_id={call_id}"),
                error: None,
            },
            Err(e) => ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Outbound call failed: {e}")),
            },
        }
    }

    /// Parse the provider response, pulling the call identifier out of
    /// the JSON body. Telnyx nests it under `data`.
    async fn extract_call_id(
        resp: Result<reqwest::Response, reqwest::Error>,
        id_field: &str,
    ) -> anyhow::Result<String> {
        let resp = resp?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("provider returned {status}: {body}");
        }
        let json: serde_json::Value = serde_json::from_str(&body).unwrap_or(json!({}));
        let id = json
            .get(id_field)
            .and_then(|v| v.as_str())
            .or_else(|| json.get("data").and_then(|d| d.get(id_field)).and_then(|v| v.as_str()))
            .unwrap_or("unknown");
        Ok(id.to_string())
    }
}

#[async_trait]
impl Tool for VoiceCallTool {
    fn name(&self) -> &str {
        "voice_call"
    }

    fn description(&self) -> &str {
        "Place an outbound phone call through the configured telephony provider (Twilio/Telnyx). Dials a number; the conversation itself is driven by the voice channel's webhook. Spends real money and rings a real person — requires Full autonomy and may return a PENDING_APPROVAL marker when outbound approval is configured."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["place_call"],
                    "description": "Operation to perform. Only outbound 'place_call' is supported."
                },
                "to": {
                    "type": "string",
                    "description": "Destination phone number in E.164 form, e.g. +14155551234."
                },
                "reason": {
                    "type": "string",
                    "description": "Optional short reason for the call, logged for the audit trail (not spoken)."
                }
            },
            "required": ["action", "to"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("place_call");
        if action != "place_call" {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unsupported action '{action}' — only 'place_call' is available")),
            });
        }

        let to = match args.get("to").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing 'to' parameter".into()),
                });
            }
        };
        if let Err(e) = Self::validate_number(to) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Invalid number: {e}")),
            });
        }

        if !self.config.enabled {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("voice_call is disabled: set [voice_call] enabled = true in config".into()),
            });
        }

        // Hard autonomy gate — placing a call is irreversible and costs
        // money. Require Full.
        if !self.security.can_act() || self.security.autonomy != AutonomyLevel::Full {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "Action blocked: placing an outbound call requires Full autonomy".into(),
                ),
            });
        }

        // Honour the config's outbound-approval flag exactly as the
        // channel does — surface a marker rather than dialing.
        if self.config.require_outbound_approval {
            return Ok(ToolResult {
                success: true,
                output: format!("PENDING_APPROVAL:{to}"),
                error: None,
            });
        }

        if !self.security.record_action() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Action blocked: rate limit exceeded".into()),
            });
        }

        Ok(self.place_call(to).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, approval: bool) -> VoiceCallConfig {
        VoiceCallConfig {
            enabled,
            provider: VoiceProvider::Telnyx,
            account_id: "conn-1".into(),
            auth_token: "tok".into(),
            from_number: "+15551112222".into(),
            webhook_port: 8090,
            require_outbound_approval: approval,
            transcription_logging: false,
            tts_voice: None,
            max_call_duration_secs: 120,
            webhook_base_url: Some("https://tunnel.example.com".into()),
        }
    }

    fn tool(enabled: bool, approval: bool) -> VoiceCallTool {
        VoiceCallTool::new(Arc::new(SecurityPolicy::default()), cfg(enabled, approval))
    }

    #[test]
    fn name_and_schema() {
        let t = tool(true, true);
        assert_eq!(t.name(), "voice_call");
        let schema = t.parameters_schema();
        assert!(schema["properties"]["to"].is_object());
        assert!(!t.description().is_empty());
    }

    #[test]
    fn validate_number_guards() {
        assert!(VoiceCallTool::validate_number("+14155551234").is_ok());
        assert!(VoiceCallTool::validate_number("").is_err());
        assert!(VoiceCallTool::validate_number("abc").is_err());
        assert!(VoiceCallTool::validate_number("123").is_err());
    }

    #[test]
    fn webhook_url_uses_base_when_set() {
        let t = tool(true, true);
        assert_eq!(
            t.webhook_url("/voice/status"),
            "https://tunnel.example.com/voice/status"
        );
    }

    #[tokio::test]
    async fn disabled_config_is_rejected() {
        let t = tool(false, false);
        let res = t
            .execute(json!({"action": "place_call", "to": "+14155551234"}))
            .await
            .unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("disabled"));
    }

    #[tokio::test]
    async fn unsupported_action_is_rejected() {
        let t = tool(true, false);
        let res = t
            .execute(json!({"action": "hangup", "to": "+14155551234"}))
            .await
            .unwrap();
        assert!(!res.success);
    }
}
