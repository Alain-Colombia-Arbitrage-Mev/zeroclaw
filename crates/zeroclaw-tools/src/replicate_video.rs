//! Replicate video generation tool — POST+poll against the Replicate API.
//!
//! [Replicate](https://replicate.com) hosts hundreds of generative video
//! models (Veo, Kling, Luma Dream Machine, Pika, Stable Video Diffusion,
//! Hunyuan, etc.) behind a single REST surface:
//!
//! - `POST https://api.replicate.com/v1/predictions` with `{version, input}`
//! - poll `GET urls.get` until `status` is `succeeded` / `failed` / `canceled`
//! - download the first MP4 URL in `output` to a workspace-local path
//!
//! Security:
//! - hard allowlist of model identifiers (`owner/model` or with `:version`)
//! - API token is a `#[secret]` config field (OS keyring)
//! - output written under `workspace_dir`; arbitrary paths rejected
//! - `SecurityPolicy.record_action()` consumes the hourly action budget

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::ReplicateVideoConfig;

const API_BASE: &str = "https://api.replicate.com/v1";
const MAX_INITIAL_RESPONSE_BYTES: usize = 512 * 1024;
const POLL_INTERVAL_MS: u64 = 2_000;

pub struct ReplicateVideoTool {
    security: Arc<SecurityPolicy>,
    config: ReplicateVideoConfig,
    client: Client,
}

impl ReplicateVideoTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        config: ReplicateVideoConfig,
    ) -> anyhow::Result<Self> {
        let builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5));
        let builder = zeroclaw_config::schema::apply_runtime_proxy_to_builder(
            builder,
            "tool.replicate_video",
        );
        let client = builder.build()?;
        Ok(Self {
            security,
            config,
            client,
        })
    }

    fn api_token(&self) -> Result<String, String> {
        if let Some(t) = self.config.api_token.as_deref()
            && !t.is_empty()
        {
            return Ok(t.to_string());
        }
        if let Ok(t) = std::env::var("REPLICATE_API_TOKEN")
            && !t.is_empty()
        {
            return Ok(t);
        }
        Err(
            "replicate_video.api_token is not configured and REPLICATE_API_TOKEN env var is empty"
                .into(),
        )
    }

    /// Verify the requested model identifier matches the configured allowlist.
    /// The agent may pass `owner/model` or `owner/model:version`; the allowlist
    /// matches against `owner/model` (the `:version` suffix is stripped before
    /// comparison). `["*"]` allows any model.
    fn validate_model(&self, model: &str) -> Result<String, String> {
        let m = model.trim();
        if m.is_empty() {
            return Err("'model' cannot be empty".into());
        }
        if m.chars().any(char::is_whitespace) {
            return Err("'model' cannot contain whitespace".into());
        }
        let bare = m.split_once(':').map(|(a, _)| a).unwrap_or(m);
        if !bare.contains('/') {
            return Err(format!(
                "'model' must be in the form owner/model or owner/model:version, got '{m}'"
            ));
        }
        if self.config.allowed_models.iter().any(|p| p == "*") {
            return Ok(m.to_string());
        }
        if self
            .config
            .allowed_models
            .iter()
            .any(|allowed| allowed == bare)
        {
            return Ok(m.to_string());
        }
        if self.config.allowed_models.is_empty() {
            return Err(
                "replicate_video is enabled but no allowed_models are configured. Add at least one entry to [replicate_video].allowed_models in config.toml".into(),
            );
        }
        Err(format!(
            "model '{bare}' is not in replicate_video.allowed_models"
        ))
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
                workspace.join(format!("replicate-video-{ts}.mp4"))
            }
        };
        // Canonicalize the parent (target may not exist yet); reject if the
        // parent escapes the workspace.
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

    async fn submit(&self, token: &str, model: &str, input: &Value) -> Result<Value, String> {
        // Replicate accepts the model spec in either `version` (owner/model:version
        // or version_id) or, for the newer "model-only" form, a path of
        // `/v1/models/{owner}/{model}/predictions`. We use the latter when the
        // identifier has no `:version` suffix, otherwise the `version` payload.
        let url = if model.contains(':') {
            let body = json!({ "version": model, "input": input });
            return self
                .do_post(token, &format!("{API_BASE}/predictions"), &body)
                .await;
        } else {
            format!("{API_BASE}/models/{model}/predictions")
        };
        let body = json!({ "input": input });
        self.do_post(token, &url, &body).await
    }

    async fn do_post(&self, token: &str, url: &str, body: &Value) -> Result<Value, String> {
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(body)
            .timeout(Duration::from_secs(self.config.request_timeout_secs))
            .send()
            .await
            .map_err(|e| format!("Replicate POST failed: {e}"))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Replicate response body read failed: {e}"))?;
        if bytes.len() > MAX_INITIAL_RESPONSE_BYTES {
            return Err(format!(
                "Replicate response exceeded {MAX_INITIAL_RESPONSE_BYTES} bytes"
            ));
        }
        let text = String::from_utf8_lossy(&bytes).to_string();
        if !status.is_success() {
            return Err(format!("Replicate API error ({status}): {text}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("Replicate JSON parse failed: {e}"))
    }

    async fn poll(&self, token: &str, poll_url: &str) -> Result<Value, String> {
        let deadline =
            std::time::Instant::now() + Duration::from_secs(self.config.total_timeout_secs);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "Replicate prediction did not complete within {}s",
                    self.config.total_timeout_secs
                ));
            }
            let resp = self
                .client
                .get(poll_url)
                .header("Authorization", format!("Bearer {token}"))
                .timeout(Duration::from_secs(self.config.request_timeout_secs))
                .send()
                .await
                .map_err(|e| format!("Replicate poll failed: {e}"))?;
            let status = resp.status();
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| format!("Replicate poll body read failed: {e}"))?;
            if bytes.len() > MAX_INITIAL_RESPONSE_BYTES {
                return Err(format!(
                    "Replicate poll response exceeded {MAX_INITIAL_RESPONSE_BYTES} bytes"
                ));
            }
            let text = String::from_utf8_lossy(&bytes).to_string();
            if !status.is_success() {
                return Err(format!("Replicate poll error ({status}): {text}"));
            }
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| format!("Replicate poll JSON parse failed: {e}"))?;
            let phase = value
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");
            match phase {
                "succeeded" => return Ok(value),
                "failed" | "canceled" => {
                    let err_msg = value
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("unknown error");
                    return Err(format!("Replicate prediction {phase}: {err_msg}"));
                }
                _ => {
                    tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
                }
            }
        }
    }

    /// Find the first video URL in a `succeeded` prediction's `output`.
    /// Output shape varies by model: string, array of strings, or object.
    fn extract_video_url(value: &Value) -> Option<String> {
        let output = value.get("output")?;
        match output {
            Value::String(s) => Some(s.clone()),
            Value::Array(items) => items.iter().find_map(|v| v.as_str().map(|s| s.to_string())),
            Value::Object(map) => map
                .values()
                .find_map(|v| Self::extract_video_url(&json!({ "output": v }))),
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
impl Tool for ReplicateVideoTool {
    fn name(&self) -> &str {
        "replicate_video"
    }

    fn description(&self) -> &str {
        "Generate video via Replicate (https://replicate.com). Submits a prediction \
         against an allowlisted model, polls until completion, and downloads the resulting \
         MP4 to a workspace path. The `input` object is forwarded verbatim — refer to each \
         model's page on replicate.com for its specific input schema."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "model": {
                    "type": "string",
                    "description": "Replicate model identifier: 'owner/model' or 'owner/model:version_id'. Must be in allowed_models."
                },
                "input": {
                    "type": "object",
                    "description": "Model-specific input object (e.g. {prompt, duration, aspect_ratio}). See the model's Replicate page for its schema."
                },
                "output_path": {
                    "type": "string",
                    "description": "Optional output path inside workspace_dir. If omitted, a timestamped .mp4 path is generated."
                }
            },
            "required": ["model", "input"]
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
            .enforce_tool_operation(ToolOperation::Act, "replicate_video")
        {
            return Ok(failure(e));
        }

        let model_arg = args
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
        let input = match args.get("input") {
            Some(i) if i.is_object() => i.clone(),
            _ => return Ok(failure("'input' must be a JSON object".into())),
        };
        let model = match self.validate_model(model_arg) {
            Ok(m) => m,
            Err(e) => return Ok(failure(e)),
        };
        let output_path =
            match self.resolve_output_path(args.get("output_path").and_then(|v| v.as_str())) {
                Ok(p) => p,
                Err(e) => return Ok(failure(e)),
            };
        let token = match self.api_token() {
            Ok(t) => t,
            Err(e) => return Ok(failure(e)),
        };

        if !self.security.record_action() {
            return Ok(failure(
                "Rate limit exceeded: action budget exhausted".into(),
            ));
        }

        let submitted = match self.submit(&token, &model, &input).await {
            Ok(v) => v,
            Err(e) => return Ok(failure(e)),
        };
        let prediction_id = submitted
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let phase = submitted
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Handle synchronous-success (rare) or poll for async completion.
        let final_value = if phase == "succeeded" {
            submitted
        } else {
            let poll_url = submitted
                .get("urls")
                .and_then(|u| u.get("get"))
                .and_then(|g| g.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{API_BASE}/predictions/{prediction_id}"));
            match self.poll(&token, &poll_url).await {
                Ok(v) => v,
                Err(e) => return Ok(failure(e)),
            }
        };

        let video_url = match Self::extract_video_url(&final_value) {
            Some(u) => u,
            None => {
                return Ok(failure(format!(
                    "Replicate succeeded but no video URL in `output`. Prediction id: {prediction_id}"
                )));
            }
        };

        let bytes_written = match self.download(&video_url, &output_path).await {
            Ok(b) => b,
            Err(e) => return Ok(failure(e)),
        };

        let out = json!({
            "prediction_id": prediction_id,
            "model": model,
            "video_url": video_url,
            "output_path": output_path.display().to_string(),
            "bytes_written": bytes_written,
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

    fn cfg() -> ReplicateVideoConfig {
        ReplicateVideoConfig {
            enabled: true,
            api_token: Some("test-token".into()),
            allowed_models: vec!["google/veo-3.1".into(), "kwaivgi/kling-v2.5-master".into()],
            request_timeout_secs: 30,
            total_timeout_secs: 600,
            max_download_bytes: 100_000_000,
        }
    }

    fn tool() -> ReplicateVideoTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        ReplicateVideoTool::new(security, cfg()).expect("client builds")
    }

    #[test]
    fn name_is_replicate_video() {
        assert_eq!(tool().name(), "replicate_video");
    }

    #[test]
    fn schema_requires_model_and_input() {
        let s = tool().parameters_schema();
        let req = s["required"].as_array().unwrap();
        assert!(req.contains(&json!("model")));
        assert!(req.contains(&json!("input")));
    }

    #[test]
    fn validate_model_accepts_listed() {
        assert!(tool().validate_model("google/veo-3.1").is_ok());
    }

    #[test]
    fn validate_model_accepts_listed_with_version() {
        assert!(tool().validate_model("google/veo-3.1:abc123").is_ok());
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
        let t = ReplicateVideoTool::new(security, c).unwrap();
        assert!(t.validate_model("anyone/anything").is_ok());
        assert!(t.validate_model("anyone/anything:abc").is_ok());
    }

    #[test]
    fn validate_model_empty_allowlist_errors_clearly() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.allowed_models = vec![];
        let t = ReplicateVideoTool::new(security, c).unwrap();
        let err = t.validate_model("google/veo-3.1").unwrap_err();
        assert!(err.contains("no allowed_models"));
    }

    #[test]
    fn extract_video_url_string_output() {
        let v = json!({"output": "https://r2.replicate.delivery/foo.mp4"});
        assert_eq!(
            ReplicateVideoTool::extract_video_url(&v).unwrap(),
            "https://r2.replicate.delivery/foo.mp4"
        );
    }

    #[test]
    fn extract_video_url_array_output() {
        let v = json!({"output": ["https://r2.replicate.delivery/a.mp4", "irrelevant"]});
        assert_eq!(
            ReplicateVideoTool::extract_video_url(&v).unwrap(),
            "https://r2.replicate.delivery/a.mp4"
        );
    }

    #[test]
    fn extract_video_url_nested_object() {
        let v = json!({"output": {"video": "https://r2.replicate.delivery/b.mp4"}});
        assert_eq!(
            ReplicateVideoTool::extract_video_url(&v).unwrap(),
            "https://r2.replicate.delivery/b.mp4"
        );
    }

    #[test]
    fn extract_video_url_missing_returns_none() {
        let v = json!({"output": null});
        assert!(ReplicateVideoTool::extract_video_url(&v).is_none());
        let v2 = json!({});
        assert!(ReplicateVideoTool::extract_video_url(&v2).is_none());
    }

    // env-var-driven cases bundled into one test to avoid races with parallel
    // test runners — REPLICATE_API_TOKEN is process-global.
    #[test]
    fn api_token_env_fallback_and_missing() {
        let security = Arc::new(SecurityPolicy::default());

        // Case 1: config has a token — env doesn't matter
        let mut c = cfg();
        c.api_token = Some("from-config".into());
        let t = ReplicateVideoTool::new(security.clone(), c).unwrap();
        assert_eq!(t.api_token().unwrap(), "from-config");

        // Case 2: no config token, env var set → falls back to env
        unsafe { std::env::set_var("REPLICATE_API_TOKEN", "from-env") };
        let mut c = cfg();
        c.api_token = None;
        let t = ReplicateVideoTool::new(security.clone(), c).unwrap();
        assert_eq!(t.api_token().unwrap(), "from-env");

        // Case 3: no config token and no env → explicit error
        unsafe { std::env::remove_var("REPLICATE_API_TOKEN") };
        let mut c = cfg();
        c.api_token = None;
        let t = ReplicateVideoTool::new(security, c).unwrap();
        let err = t.api_token().unwrap_err();
        assert!(err.contains("api_token"));
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let t = ReplicateVideoTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({"model": "google/veo-3.1", "input": {"prompt": "test"}}))
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
        let t = ReplicateVideoTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({"model": "google/veo-3.1", "input": {"prompt": "test"}}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_missing_model_errors() {
        let r = tool().execute(json!({"input": {}})).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_non_object_input_non_fatal() {
        let r = tool()
            .execute(json!({"model": "google/veo-3.1", "input": "string"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("must be a JSON object"));
    }

    #[tokio::test]
    async fn execute_rejects_disallowed_model_non_fatal() {
        let r = tool()
            .execute(json!({"model": "evil/spam", "input": {"prompt": "x"}}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("allowed_models"));
    }
}
