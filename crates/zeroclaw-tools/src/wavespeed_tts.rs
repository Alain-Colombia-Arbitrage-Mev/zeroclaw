//! WaveSpeed TTS — text-to-speech generation companion to `image_gen`.
//!
//! [WaveSpeed.ai](https://wavespeed.ai) hosts a catalog of state-of-the-art
//! TTS models behind the same REST surface as their image catalog:
//!
//! - `POST https://api.wavespeed.ai/api/v3/{model}` with `{text, voice, language, ...}`
//! - poll `GET data.urls.get` until `status == "completed"`
//! - download the resulting audio URL to a workspace-local path
//!
//! Models intended for this tool (operator picks via `allowed_models`):
//!
//! - `wavespeed-ai/qwen3-tts/text-to-speech` — natural voices, ~97 ms TTFA
//! - `minimax/speech-2.6-hd` — premium quality, serverless
//! - `wavespeed-ai/omnivoice/text-to-speech` — 600+ languages, zero-shot cloning
//! - `wavespeed-ai/vibevoice` — long-form podcast / multi-speaker
//!
//! Shares the `WAVESPEED_API_KEY` env var convention with `image_gen` so
//! operators don't manage two keys for the same vendor.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::WaveSpeedTtsConfig;

const API_BASE: &str = "https://api.wavespeed.ai/api/v3";
const MAX_INITIAL_RESPONSE_BYTES: usize = 256 * 1024;
const POLL_INTERVAL_MS: u64 = 1_500;

pub struct WaveSpeedTtsTool {
    security: Arc<SecurityPolicy>,
    config: WaveSpeedTtsConfig,
    client: Client,
}

impl WaveSpeedTtsTool {
    pub fn new(security: Arc<SecurityPolicy>, config: WaveSpeedTtsConfig) -> anyhow::Result<Self> {
        let builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5));
        let builder =
            zeroclaw_config::schema::apply_runtime_proxy_to_builder(builder, "tool.wavespeed_tts");
        let client = builder.build()?;
        Ok(Self {
            security,
            config,
            client,
        })
    }

    fn api_key(&self) -> Result<String, String> {
        if let Some(k) = self.config.api_key.as_deref()
            && !k.is_empty()
        {
            return Ok(k.to_string());
        }
        let env_var = if self.config.api_key_env.is_empty() {
            "WAVESPEED_API_KEY"
        } else {
            self.config.api_key_env.as_str()
        };
        std::env::var(env_var)
            .map(|v| v.trim().to_string())
            .ok()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                format!(
                    "Missing WaveSpeed API key: set the {env_var} environment variable or wavespeed_tts.api_key in config"
                )
            })
    }

    fn validate_model(&self, model: &str) -> Result<String, String> {
        let m = model.trim();
        if m.is_empty() {
            return Err("'model' cannot be empty".into());
        }
        if m.chars().any(char::is_whitespace) {
            return Err("'model' cannot contain whitespace".into());
        }
        if !m.contains('/') {
            return Err(format!(
                "'model' must be a WaveSpeed model path (e.g. 'wavespeed-ai/qwen3-tts/text-to-speech'), got '{m}'"
            ));
        }
        if self.config.allowed_models.iter().any(|p| p == "*") {
            return Ok(m.to_string());
        }
        if self.config.allowed_models.is_empty() {
            return Err(
                "wavespeed_tts is enabled but no allowed_models are configured. Add at least one entry to [wavespeed_tts].allowed_models in config.toml".into(),
            );
        }
        if self
            .config
            .allowed_models
            .iter()
            .any(|allowed| allowed == m)
        {
            return Ok(m.to_string());
        }
        Err(format!(
            "model '{m}' is not in wavespeed_tts.allowed_models"
        ))
    }

    fn validate_text(&self, text: &str) -> Result<String, String> {
        let t = text.trim();
        if t.is_empty() {
            return Err("'text' cannot be empty".into());
        }
        let char_count = t.chars().count();
        if char_count > self.config.max_input_chars {
            return Err(format!(
                "'text' length {char_count} exceeds max_input_chars {}",
                self.config.max_input_chars
            ));
        }
        Ok(t.to_string())
    }

    fn resolve_output_path(&self, requested: Option<&str>) -> Result<PathBuf, String> {
        let workspace = self
            .security
            .workspace_dir
            .canonicalize()
            .map_err(|e| format!("workspace_dir not accessible: {e}"))?;
        let target = match requested {
            Some(p) => {
                let base = PathBuf::from(p);
                if base.is_absolute() {
                    base
                } else {
                    workspace.join(base)
                }
            }
            None => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let ext = match self.config.output_format.as_str() {
                    "wav" => "wav",
                    "opus" => "opus",
                    "flac" => "flac",
                    _ => "mp3",
                };
                workspace.join(format!("wavespeed-tts-{ts}.{ext}"))
            }
        };
        let parent = target
            .parent()
            .ok_or_else(|| "output path has no parent directory".to_string())?
            .to_path_buf();
        let parent_canon = if parent.exists() {
            parent
                .canonicalize()
                .map_err(|e| format!("output parent dir not accessible: {e}"))?
        } else {
            return Err(format!(
                "output parent dir '{}' does not exist",
                parent.display()
            ));
        };
        if !parent_canon.starts_with(&workspace) {
            return Err(format!(
                "output path '{}' is outside workspace_dir",
                target.display()
            ));
        }
        Ok(target)
    }

    async fn submit(&self, key: &str, model: &str, body: &Value) -> Result<Value, String> {
        let url = format!("{API_BASE}/{model}");
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .json(body)
            .timeout(Duration::from_secs(self.config.request_timeout_secs))
            .send()
            .await
            .map_err(|e| format!("WaveSpeed TTS submit failed: {e}"))?;
        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("WaveSpeed TTS response body read failed: {e}"))?;
        if bytes.len() > MAX_INITIAL_RESPONSE_BYTES {
            return Err(format!(
                "WaveSpeed TTS response exceeded {MAX_INITIAL_RESPONSE_BYTES} bytes"
            ));
        }
        let text = String::from_utf8_lossy(&bytes).to_string();
        if !status.is_success() {
            return Err(format!("WaveSpeed TTS API error ({status}): {text}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("WaveSpeed TTS JSON parse failed: {e}"))
    }

    async fn poll(&self, key: &str, poll_url: &str) -> Result<Value, String> {
        let deadline =
            std::time::Instant::now() + Duration::from_secs(self.config.total_timeout_secs);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "WaveSpeed TTS task did not complete within {}s. Poll URL: {poll_url}",
                    self.config.total_timeout_secs
                ));
            }
            let resp = self
                .client
                .get(poll_url)
                .header("Authorization", format!("Bearer {key}"))
                .timeout(Duration::from_secs(self.config.request_timeout_secs))
                .send()
                .await
                .map_err(|e| format!("WaveSpeed TTS poll failed: {e}"))?;
            let status = resp.status();
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| format!("WaveSpeed TTS poll body read failed: {e}"))?;
            if bytes.len() > MAX_INITIAL_RESPONSE_BYTES {
                return Err(format!(
                    "WaveSpeed TTS poll response exceeded {MAX_INITIAL_RESPONSE_BYTES} bytes"
                ));
            }
            let text = String::from_utf8_lossy(&bytes).to_string();
            if !status.is_success() {
                return Err(format!("WaveSpeed TTS poll error ({status}): {text}"));
            }
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| format!("WaveSpeed TTS poll JSON parse failed: {e}"))?;
            let phase = value
                .pointer("/data/status")
                .and_then(|s| s.as_str())
                .or_else(|| value.get("status").and_then(|s| s.as_str()))
                .unwrap_or("unknown");
            match phase {
                "completed" | "succeeded" => return Ok(value),
                "failed" | "canceled" | "error" => {
                    let err_msg = value
                        .pointer("/data/error")
                        .and_then(|e| e.as_str())
                        .or_else(|| value.get("error").and_then(|e| e.as_str()))
                        .unwrap_or("unknown error");
                    return Err(format!("WaveSpeed TTS task {phase}: {err_msg}"));
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
                }
            }
        }
    }

    /// Find the first audio URL in a `completed` task's `outputs` field.
    /// WaveSpeed shape: `{data: {outputs: ["https://..."], ...}}`. Some
    /// models return `output` (singular) string; handle both.
    fn extract_audio_url(value: &Value) -> Option<String> {
        if let Some(outputs) = value.pointer("/data/outputs").and_then(|v| v.as_array())
            && let Some(first) = outputs.first().and_then(|v| v.as_str())
        {
            return Some(first.to_string());
        }
        if let Some(s) = value.pointer("/data/output").and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
        // Some models nest results differently — last-resort scan.
        match value {
            Value::String(s) if s.starts_with("http") => Some(s.clone()),
            Value::Array(items) => items.iter().find_map(Self::extract_audio_url),
            Value::Object(map) => map.values().find_map(Self::extract_audio_url),
            _ => None,
        }
    }

    async fn download(&self, url: &str, dest: &std::path::Path) -> Result<u64, String> {
        let resp = self
            .client
            .get(url)
            .timeout(Duration::from_secs(self.config.request_timeout_secs))
            .send()
            .await
            .map_err(|e| format!("download GET failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("download failed: HTTP {}", status.as_u16()));
        }
        let mut stream = resp.bytes_stream();
        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| format!("cannot create '{}': {e}", dest.display()))?;
        let mut written: u64 = 0;
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("download stream error: {e}"))?;
            written += chunk.len() as u64;
            if written > self.config.max_download_bytes {
                drop(file);
                let _ = tokio::fs::remove_file(dest).await;
                return Err(format!(
                    "download exceeded max_download_bytes {}",
                    self.config.max_download_bytes
                ));
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("file write failed: {e}"))?;
        }
        file.flush()
            .await
            .map_err(|e| format!("file flush failed: {e}"))?;
        Ok(written)
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
impl Tool for WaveSpeedTtsTool {
    fn name(&self) -> &str {
        "wavespeed_tts"
    }

    fn description(&self) -> &str {
        "Generate speech audio from text via WaveSpeed.ai. Submits a TTS task against an \
         allowlisted model (Qwen3-TTS, Minimax Speech 2.6 HD, OmniVoice, VibeVoice), polls \
         until completion, and downloads the audio file to a workspace path. Voice and \
         language defaults come from config; per-call overrides via `voice`/`language` args."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to synthesize (capped by max_input_chars in config)"
                },
                "model": {
                    "type": "string",
                    "description": "WaveSpeed model path. Must be in allowed_models. Examples: 'wavespeed-ai/qwen3-tts/text-to-speech', 'minimax/speech-2.6-hd', 'wavespeed-ai/omnivoice/text-to-speech'."
                },
                "voice": {
                    "type": "string",
                    "description": "Voice ID for the chosen model. If omitted, uses config.default_voice. Refer to each model's docs on wavespeed.ai for available voices."
                },
                "language": {
                    "type": "string",
                    "description": "Language code or 'auto'. If omitted, uses config.default_language."
                },
                "output_path": {
                    "type": "string",
                    "description": "Optional output path inside workspace_dir. If omitted, a timestamped file in output_format is generated."
                },
                "extra": {
                    "type": "object",
                    "description": "Optional extra fields passed through to the WaveSpeed submit body (e.g. style_instruction for Qwen3-TTS, speakers config for VibeVoice). Refer to each model's API page."
                }
            },
            "required": ["text", "model"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if self.security.is_rate_limited() {
            return Ok(failure(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }
        if let Err(e) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "wavespeed_tts")
        {
            return Ok(failure(e));
        }

        let text_arg = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;
        let model_arg = args
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;

        let text = match self.validate_text(text_arg) {
            Ok(t) => t,
            Err(e) => return Ok(failure(e)),
        };
        let model = match self.validate_model(model_arg) {
            Ok(m) => m,
            Err(e) => return Ok(failure(e)),
        };
        let voice = args
            .get("voice")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| self.config.default_voice.clone());
        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| self.config.default_language.clone());

        let output_path =
            match self.resolve_output_path(args.get("output_path").and_then(|v| v.as_str())) {
                Ok(p) => p,
                Err(e) => return Ok(failure(e)),
            };
        let key = match self.api_key() {
            Ok(k) => k,
            Err(e) => return Ok(failure(e)),
        };

        if !self.security.record_action() {
            return Ok(failure(
                "Rate limit exceeded: action budget exhausted".into(),
            ));
        }

        // Build the submit body. Common fields across WaveSpeed TTS models:
        // text, voice, language. Optional `extra` is merged on top so callers
        // can pass model-specific knobs (style_instruction, speakers, etc.)
        // without us encoding every model's quirks.
        let mut body = json!({
            "text": text,
            "voice": voice,
            "language": language,
        });
        if let Some(extra) = args.get("extra")
            && let Some(extra_obj) = extra.as_object()
            && let Some(body_obj) = body.as_object_mut()
        {
            for (k, v) in extra_obj {
                body_obj.insert(k.clone(), v.clone());
            }
        }

        let submitted = match self.submit(&key, &model, &body).await {
            Ok(v) => v,
            Err(e) => return Ok(failure(e)),
        };

        let phase = submitted
            .pointer("/data/status")
            .and_then(|s| s.as_str())
            .or_else(|| submitted.get("status").and_then(|s| s.as_str()))
            .unwrap_or("");
        let final_value = if matches!(phase, "completed" | "succeeded") {
            submitted
        } else {
            let poll_url = submitted
                .pointer("/data/urls/get")
                .and_then(|u| u.as_str())
                .map(String::from)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "WaveSpeed TTS async task has no urls.get to poll. Response: {submitted}"
                    )
                });
            let poll_url = match poll_url {
                Ok(u) => u,
                Err(e) => return Ok(failure(e.to_string())),
            };
            match self.poll(&key, &poll_url).await {
                Ok(v) => v,
                Err(e) => return Ok(failure(e)),
            }
        };

        let audio_url = match Self::extract_audio_url(&final_value) {
            Some(u) => u,
            None => {
                return Ok(failure(format!(
                    "WaveSpeed TTS completed but no audio URL in response. Raw: {final_value}"
                )));
            }
        };

        let bytes_written = match self.download(&audio_url, &output_path).await {
            Ok(b) => b,
            Err(e) => return Ok(failure(e)),
        };

        let out = json!({
            "model": model,
            "voice": voice,
            "language": language,
            "audio_url": audio_url,
            "output_path": output_path.display().to_string(),
            "bytes_written": bytes_written,
            "text_chars": text.chars().count(),
        });
        Ok(ToolResult {
            success: true,
            output: serde_json::to_string_pretty(&out).unwrap_or_default(),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;

    fn cfg() -> WaveSpeedTtsConfig {
        WaveSpeedTtsConfig {
            enabled: true,
            api_key: Some("test-key".into()),
            api_key_env: String::new(),
            allowed_models: vec![
                "wavespeed-ai/qwen3-tts/text-to-speech".into(),
                "minimax/speech-2.6-hd".into(),
            ],
            default_voice: "Vivian".into(),
            default_language: "auto".into(),
            output_format: "mp3".into(),
            max_input_chars: 2000,
            request_timeout_secs: 60,
            total_timeout_secs: 300,
            max_download_bytes: 50_000_000,
        }
    }

    fn tool() -> WaveSpeedTtsTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        WaveSpeedTtsTool::new(security, cfg()).expect("client builds")
    }

    #[test]
    fn name_is_wavespeed_tts() {
        assert_eq!(tool().name(), "wavespeed_tts");
    }

    #[test]
    fn schema_requires_text_and_model() {
        let s = tool().parameters_schema();
        let req = s["required"].as_array().unwrap();
        assert!(req.contains(&json!("text")));
        assert!(req.contains(&json!("model")));
    }

    #[test]
    fn validate_model_accepts_listed() {
        assert!(
            tool()
                .validate_model("wavespeed-ai/qwen3-tts/text-to-speech")
                .is_ok()
        );
        assert!(tool().validate_model("minimax/speech-2.6-hd").is_ok());
    }

    #[test]
    fn validate_model_rejects_unlisted() {
        let err = tool().validate_model("evil/spam").unwrap_err();
        assert!(err.contains("allowed_models"));
    }

    #[test]
    fn validate_model_rejects_bad_format() {
        assert!(tool().validate_model("noslash").is_err());
        assert!(tool().validate_model("").is_err());
        assert!(tool().validate_model("has space").is_err());
    }

    #[test]
    fn validate_model_wildcard_allows_anything() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.allowed_models = vec!["*".into()];
        let t = WaveSpeedTtsTool::new(security, c).unwrap();
        assert!(t.validate_model("any/model/here").is_ok());
    }

    #[test]
    fn validate_model_empty_allowlist_errors_clearly() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.allowed_models = vec![];
        let t = WaveSpeedTtsTool::new(security, c).unwrap();
        let err = t.validate_model("a/b").unwrap_err();
        assert!(err.contains("no allowed_models"));
    }

    #[test]
    fn validate_text_rejects_empty() {
        assert!(tool().validate_text("").is_err());
        assert!(tool().validate_text("   ").is_err());
    }

    #[test]
    fn validate_text_rejects_over_max_chars() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.max_input_chars = 10;
        let t = WaveSpeedTtsTool::new(security, c).unwrap();
        let err = t
            .validate_text("this is far more than ten characters")
            .unwrap_err();
        assert!(err.contains("max_input_chars"));
    }

    #[test]
    fn validate_text_accepts_at_limit() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.max_input_chars = 10;
        let t = WaveSpeedTtsTool::new(security, c).unwrap();
        assert!(t.validate_text("1234567890").is_ok());
    }

    #[test]
    fn extract_audio_url_from_outputs_array() {
        let v =
            json!({"data": {"outputs": ["https://cdn.wavespeed/x.mp3"], "status": "completed"}});
        assert_eq!(
            WaveSpeedTtsTool::extract_audio_url(&v).unwrap(),
            "https://cdn.wavespeed/x.mp3"
        );
    }

    #[test]
    fn extract_audio_url_from_output_singular() {
        let v = json!({"data": {"output": "https://cdn.wavespeed/y.mp3", "status": "completed"}});
        assert_eq!(
            WaveSpeedTtsTool::extract_audio_url(&v).unwrap(),
            "https://cdn.wavespeed/y.mp3"
        );
    }

    #[test]
    fn extract_audio_url_missing_returns_none() {
        let v = json!({"data": {"status": "completed"}});
        assert!(WaveSpeedTtsTool::extract_audio_url(&v).is_none());
    }

    #[test]
    fn api_key_uses_config_first() {
        // Config takes priority over env var when set
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.api_key = Some("from-config".into());
        let t = WaveSpeedTtsTool::new(security, c).unwrap();
        assert_eq!(t.api_key().unwrap(), "from-config");
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let t = WaveSpeedTtsTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({
                "text": "hello",
                "model": "wavespeed-ai/qwen3-tts/text-to-speech"
            }))
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
        let t = WaveSpeedTtsTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({
                "text": "hello",
                "model": "wavespeed-ai/qwen3-tts/text-to-speech"
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_missing_text_errors() {
        let r = tool()
            .execute(json!({"model": "wavespeed-ai/qwen3-tts/text-to-speech"}))
            .await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_missing_model_errors() {
        let r = tool().execute(json!({"text": "hello"})).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_rejects_disallowed_model_non_fatal() {
        let r = tool()
            .execute(json!({
                "text": "hello",
                "model": "evil/model"
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("allowed_models"));
    }

    #[tokio::test]
    async fn execute_rejects_too_long_text_non_fatal() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.max_input_chars = 5;
        let t = WaveSpeedTtsTool::new(security, c).unwrap();
        let r = t
            .execute(json!({
                "text": "way too long for the limit",
                "model": "wavespeed-ai/qwen3-tts/text-to-speech"
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("max_input_chars"));
    }
}
