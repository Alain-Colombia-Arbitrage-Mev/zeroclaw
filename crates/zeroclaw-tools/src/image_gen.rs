use anyhow::Context;
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_config::policy::ToolOperation;

/// Image generation tool backed by [WaveSpeed.ai](https://wavespeed.ai).
///
/// WaveSpeed hosts the current crop of state-of-the-art image models
/// (Nano Banana Pro / Gemini-3-Pro-Image, Seedream, Flux, Hidream, SDXL,
/// etc.) behind a single REST surface with optional async polling.
///
/// API contract used here:
/// - `POST https://api.wavespeed.ai/api/v3/{model}` with a JSON body
///   `{prompt, aspect_ratio, num_images}` and `Authorization: Bearer <key>`.
/// - Response carries `data.status` and either `data.outputs` (sync) or a
///   `data.urls.get` to poll until `status == "completed"` (async).
/// - Reads the API key from `WAVESPEED_API_KEY` by default (overridable
///   from the operator config via `image_gen.api_key_env`).
///
/// Downloads the resulting image and saves it to
/// `{workspace}/images/{filename}.png`.
pub struct ImageGenTool {
    security: Arc<SecurityPolicy>,
    workspace_dir: PathBuf,
    default_model: String,
    api_key_env: String,
}

/// Maximum wall-clock time we wait for an async generation task before
/// giving up. Nano Banana Pro typically completes in ~30 s; complex
/// models like Hidream-i1-full can stretch to 90–180 s on cold workers.
const POLL_MAX_WAIT: Duration = Duration::from_secs(300);

/// Pause between polls while the task is still `processing`.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Cap on the response body for the initial POST and each poll. WaveSpeed
/// responses are tiny (metadata + a few URLs) — anything larger means
/// something is off (HTML error page, redirect loop) and we bail.
const RESPONSE_BODY_CAP: usize = 256 * 1024;

impl ImageGenTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        workspace_dir: PathBuf,
        default_model: String,
        api_key_env: String,
    ) -> Self {
        Self {
            security,
            workspace_dir,
            default_model,
            api_key_env,
        }
    }

    /// Build a reusable HTTP client. Generous timeout because a single
    /// POST to a busy model can sit open for ~60 s before WaveSpeed
    /// flips it to async mode and returns a task id.
    fn http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_default()
    }

    /// Read an API key from the environment.
    fn read_api_key(env_var: &str) -> Result<String, String> {
        std::env::var(env_var)
            .map(|v| v.trim().to_string())
            .ok()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                format!(
                    "Missing API key: set the {env_var} environment variable \
                     (get one at https://wavespeed.ai/dashboard/api-keys)"
                )
            })
    }

    /// Map the public size enum (`square_hd`, `landscape_4_3`, ...) to
    /// `(width, height)` pixels. WaveSpeed's documented `submit-task`
    /// common params are `width` / `height` (integers); model-specific
    /// `aspect_ratio` strings are not portable across the catalog. We
    /// keep the enum stable so the LLM-facing schema doesn't change
    /// when the backend swaps.
    fn dimensions_for(size: &str) -> (u32, u32) {
        match size {
            "landscape_4_3" => (1024, 768),
            "portrait_4_3" => (768, 1024),
            "landscape_16_9" => (1024, 576),
            "portrait_16_9" => (576, 1024),
            _ => (1024, 1024), // square_hd + fallback
        }
    }

    /// Core generation logic: call WaveSpeed, poll if async, download,
    /// save to disk.
    async fn generate(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let prompt = match args.get("prompt").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p.trim().to_string(),
            _ => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required parameter: 'prompt'".into()),
                });
            }
        };

        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("generated_image");

        // Sanitize filename — strip path components to prevent traversal.
        let safe_name = PathBuf::from(filename).file_name().map_or_else(
            || "generated_image".to_string(),
            |n| n.to_string_lossy().to_string(),
        );

        let size = args
            .get("size")
            .and_then(|v| v.as_str())
            .unwrap_or("square_hd");

        const VALID_SIZES: &[&str] = &[
            "square_hd",
            "landscape_4_3",
            "portrait_4_3",
            "landscape_16_9",
            "portrait_16_9",
        ];
        if !VALID_SIZES.contains(&size) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Invalid size '{size}'. Valid values: {}",
                    VALID_SIZES.join(", ")
                )),
            });
        }
        let (width, height) = Self::dimensions_for(size);

        let model = args
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&self.default_model);

        // WaveSpeed model paths look like `google/nano-banana-pro`,
        // `bytedance/seedream-4`, `wavespeed-ai/hidream-i1-full`. Reject
        // anything that could redirect the request (path traversal,
        // query, fragment, leading slash, backslash).
        if model.contains("..")
            || model.contains('?')
            || model.contains('#')
            || model.contains('\\')
            || model.starts_with('/')
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Invalid model identifier '{model}'. \
                     Must be a WaveSpeed model path (e.g. 'google/nano-banana-pro')."
                )),
            });
        }

        let api_key = match Self::read_api_key(&self.api_key_env) {
            Ok(k) => k,
            Err(msg) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(msg),
                });
            }
        };

        // ── Submit the generation request ───────────────────────────
        let client = Self::http_client();
        let submit_url = format!("https://api.wavespeed.ai/api/v3/{model}");
        // Documented common params on WaveSpeed `submit-task`: prompt,
        // width, height, seed. Model-specific fields like `aspect_ratio`,
        // `num_images`, `enable_safety_checker`, `negative_prompt` vary
        // by model — sending only the documented common set keeps this
        // working across the catalog (Nano Banana Pro, Flux, Seedream,
        // Hidream, SDXL). Caller can override via the `model` arg if
        // they want a backend that requires extra fields.
        let body = json!({
            "prompt": prompt,
            "width": width,
            "height": height,
        });

        let resp = client
            .post(&submit_url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("WaveSpeed submit request failed")?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("WaveSpeed API error ({status}): {text}")),
            });
        }
        if text.len() > RESPONSE_BODY_CAP {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "WaveSpeed submit response exceeded {} bytes — likely an \
                     HTML error page; aborting",
                    RESPONSE_BODY_CAP
                )),
            });
        }
        let initial: serde_json::Value =
            serde_json::from_str(&text).context("Failed to parse WaveSpeed response as JSON")?;

        // Resolve to a final completed payload — sync responses include
        // `outputs` immediately; async responses require polling.
        let final_payload = match Self::extract_outputs(&initial) {
            Some(_) => initial,
            None => {
                let poll_url = initial
                    .pointer("/data/urls/get")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "WaveSpeed async task has no urls.get to poll. Response: {text}"
                        )
                    })?
                    .to_string();

                match self.poll_until_complete(&client, &poll_url, &api_key).await? {
                    Ok(payload) => payload,
                    Err(msg) => {
                        return Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(msg),
                        });
                    }
                }
            }
        };

        let image_url = Self::extract_outputs(&final_payload).ok_or_else(|| {
            anyhow::anyhow!("WaveSpeed completed without an image URL in `data.outputs`")
        })?;

        // ── Download the image ──────────────────────────────────────
        let img_resp = client
            .get(&image_url)
            .send()
            .await
            .context("Failed to download generated image")?;

        if !img_resp.status().is_success() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Failed to download image from {image_url} ({})",
                    img_resp.status()
                )),
            });
        }

        let bytes = img_resp
            .bytes()
            .await
            .context("Failed to read image bytes")?;

        let images_dir = self.workspace_dir.join("images");
        tokio::fs::create_dir_all(&images_dir)
            .await
            .context("Failed to create images directory")?;
        let output_path = images_dir.join(format!("{safe_name}.png"));
        tokio::fs::write(&output_path, &bytes)
            .await
            .context("Failed to write image file")?;

        let size_kb = bytes.len() / 1024;

        Ok(ToolResult {
            success: true,
            output: format!(
                "Image generated successfully.\n\
                 File: {}\n\
                 Size: {} KB\n\
                 Model: {}\n\
                 Prompt: {}",
                output_path.display(),
                size_kb,
                model,
                prompt,
            ),
            error: None,
        })
    }

    /// Look for the first output URL in a WaveSpeed payload. WaveSpeed
    /// uses `data.outputs[0]` for completed tasks; some legacy models
    /// nest it under `data.images[0].url`.
    fn extract_outputs(payload: &serde_json::Value) -> Option<String> {
        if let Some(arr) = payload.pointer("/data/outputs").and_then(|v| v.as_array())
            && let Some(first) = arr.first().and_then(|v| v.as_str())
            && !first.is_empty()
        {
            return Some(first.to_string());
        }
        if let Some(url) = payload
            .pointer("/data/images/0/url")
            .and_then(|v| v.as_str())
        {
            return Some(url.to_string());
        }
        None
    }

    /// Poll WaveSpeed's `urls.get` endpoint until the task either
    /// completes (`status == "completed"`), fails (`status == "failed"`),
    /// or the [`POLL_MAX_WAIT`] budget is exhausted.
    ///
    /// Returns `Ok(Ok(payload))` on success, `Ok(Err(msg))` for explicit
    /// API failures the caller should surface to the agent, and the
    /// outer `Err` only for unrecoverable transport problems.
    async fn poll_until_complete(
        &self,
        client: &reqwest::Client,
        poll_url: &str,
        api_key: &str,
    ) -> anyhow::Result<Result<serde_json::Value, String>> {
        let started = std::time::Instant::now();
        loop {
            if started.elapsed() > POLL_MAX_WAIT {
                return Ok(Err(format!(
                    "WaveSpeed task did not complete within {}s. Poll URL: {poll_url}",
                    POLL_MAX_WAIT.as_secs()
                )));
            }
            tokio::time::sleep(POLL_INTERVAL).await;

            let resp = client
                .get(poll_url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
                .context("WaveSpeed poll request failed")?;

            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Ok(Err(format!("WaveSpeed poll error ({status}): {text}")));
            }
            if text.len() > RESPONSE_BODY_CAP {
                return Ok(Err(format!(
                    "WaveSpeed poll response exceeded {} bytes",
                    RESPONSE_BODY_CAP
                )));
            }
            let payload: serde_json::Value = serde_json::from_str(&text)
                .context("Failed to parse WaveSpeed poll response as JSON")?;

            let task_status = payload
                .pointer("/data/status")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match task_status {
                "completed" => return Ok(Ok(payload)),
                "failed" | "error" => {
                    let err_msg = payload
                        .pointer("/data/error")
                        .and_then(|v| v.as_str())
                        .or_else(|| payload.pointer("/message").and_then(|v| v.as_str()))
                        .unwrap_or("(no error message returned)");
                    return Ok(Err(format!(
                        "WaveSpeed task failed: {err_msg}. Raw: {text}"
                    )));
                }
                // "processing" / "queued" / "" → keep polling
                _ => continue,
            }
        }
    }
}

#[async_trait]
impl Tool for ImageGenTool {
    fn name(&self) -> &str {
        "image_gen"
    }

    fn description(&self) -> &str {
        "Generate an image from a text prompt using WaveSpeed.ai \
         (Nano Banana Pro, Flux, Seedream, Hidream, SDXL, etc.). Saves \
         the result to the workspace `images/` directory and returns the \
         file path. Requires WAVESPEED_API_KEY in the environment."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "required": ["prompt"],
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Text prompt describing the image to generate."
                },
                "filename": {
                    "type": "string",
                    "description": "Output filename without extension (default: 'generated_image'). Saved as PNG in workspace/images/."
                },
                "size": {
                    "type": "string",
                    "enum": ["square_hd", "landscape_4_3", "portrait_4_3", "landscape_16_9", "portrait_16_9"],
                    "description": "Image aspect ratio preset (default: 'square_hd'). Mapped to WaveSpeed aspect_ratio internally."
                },
                "model": {
                    "type": "string",
                    "description": "WaveSpeed model path (default: 'google/nano-banana-pro'). Examples: 'google/nano-banana-pro', 'bytedance/seedream-4', 'wavespeed-ai/flux-schnell'."
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if let Err(error) = self
            .security
            .enforce_tool_operation(ToolOperation::Act, "image_gen")
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(error),
            });
        }

        self.generate(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;
    use zeroclaw_config::policy::SecurityPolicy;

    fn test_security() -> Arc<SecurityPolicy> {
        Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Full,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        })
    }

    fn test_tool() -> ImageGenTool {
        ImageGenTool::new(
            test_security(),
            std::env::temp_dir(),
            "google/nano-banana-pro-text-to-image".into(),
            "WAVESPEED_API_KEY".into(),
        )
    }

    #[test]
    fn tool_name() {
        let tool = test_tool();
        assert_eq!(tool.name(), "image_gen");
    }

    #[test]
    fn tool_description_mentions_wavespeed() {
        let tool = test_tool();
        assert!(tool.description().contains("WaveSpeed"));
    }

    #[test]
    fn tool_schema_has_required_prompt() {
        let tool = test_tool();
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["prompt"]));
        assert!(schema["properties"]["prompt"].is_object());
    }

    #[test]
    fn tool_schema_has_optional_params() {
        let tool = test_tool();
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["filename"].is_object());
        assert!(schema["properties"]["size"].is_object());
        assert!(schema["properties"]["model"].is_object());
    }

    #[test]
    fn dimensions_mapping_covers_enum() {
        assert_eq!(ImageGenTool::dimensions_for("square_hd"), (1024, 1024));
        assert_eq!(ImageGenTool::dimensions_for("landscape_4_3"), (1024, 768));
        assert_eq!(ImageGenTool::dimensions_for("portrait_4_3"), (768, 1024));
        assert_eq!(
            ImageGenTool::dimensions_for("landscape_16_9"),
            (1024, 576)
        );
        assert_eq!(
            ImageGenTool::dimensions_for("portrait_16_9"),
            (576, 1024)
        );
        // Unknown values fall back to square 1024×1024.
        assert_eq!(ImageGenTool::dimensions_for("garbage"), (1024, 1024));
    }

    #[test]
    fn extract_outputs_handles_outputs_array() {
        let payload = json!({
            "data": { "outputs": ["https://media.wavespeed.ai/x.png"] }
        });
        assert_eq!(
            ImageGenTool::extract_outputs(&payload).as_deref(),
            Some("https://media.wavespeed.ai/x.png")
        );
    }

    #[test]
    fn extract_outputs_falls_back_to_legacy_images_field() {
        let payload = json!({
            "data": { "images": [ {"url": "https://legacy/y.png"} ] }
        });
        assert_eq!(
            ImageGenTool::extract_outputs(&payload).as_deref(),
            Some("https://legacy/y.png")
        );
    }

    #[test]
    fn extract_outputs_returns_none_when_empty() {
        assert!(ImageGenTool::extract_outputs(&json!({"data": {}})).is_none());
        assert!(ImageGenTool::extract_outputs(&json!({"data": {"outputs": []}})).is_none());
    }

    #[test]
    fn tool_spec_roundtrip() {
        let tool = test_tool();
        let spec = tool.spec();
        assert_eq!(spec.name, "image_gen");
        assert!(spec.parameters.is_object());
    }

    #[tokio::test]
    async fn missing_prompt_returns_error() {
        let tool = test_tool();
        let result = tool.execute(json!({})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap().contains("prompt"));
    }

    #[tokio::test]
    async fn empty_prompt_returns_error() {
        let tool = test_tool();
        let result = tool.execute(json!({"prompt": "   "})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap().contains("prompt"));
    }

    #[tokio::test]
    async fn missing_api_key_returns_error() {
        // Reference a dedicated env var that we can guarantee is unset.
        unsafe { std::env::remove_var("ZC_TEST_WAVESPEED_KEY_MISSING") };

        let tool = ImageGenTool::new(
            test_security(),
            std::env::temp_dir(),
            "google/nano-banana-pro-text-to-image".into(),
            "ZC_TEST_WAVESPEED_KEY_MISSING".into(),
        );
        let result = tool
            .execute(json!({"prompt": "a sleeping cat"}))
            .await
            .unwrap();
        assert!(!result.success);
        let err = result.error.unwrap_or_default();
        assert!(
            err.contains("ZC_TEST_WAVESPEED_KEY_MISSING"),
            "expected env var name in error: {err}"
        );
        assert!(
            err.to_lowercase().contains("wavespeed") || err.to_lowercase().contains("api key"),
            "expected api-key-style error: {err}"
        );
    }

    #[tokio::test]
    async fn invalid_size_returns_error() {
        unsafe { std::env::set_var("WAVESPEED_API_KEY", "dummy") };
        let tool = test_tool();
        let result = tool
            .execute(json!({"prompt": "x", "size": "tiny"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap_or_default().contains("Invalid size"));
        unsafe { std::env::remove_var("WAVESPEED_API_KEY") };
    }

    #[tokio::test]
    async fn invalid_model_path_returns_error() {
        unsafe { std::env::set_var("WAVESPEED_API_KEY", "dummy") };
        let tool = test_tool();
        for bad in [
            "../etc/passwd",
            "google/nano-banana-pro?evil=1",
            "google/nano#frag",
            "/absolute/path",
            "windows\\path",
        ] {
            let result = tool
                .execute(json!({"prompt": "x", "model": bad}))
                .await
                .unwrap();
            assert!(!result.success, "should reject model '{bad}'");
            assert!(
                result.error.unwrap_or_default().contains("Invalid model"),
                "expected invalid-model error for '{bad}'"
            );
        }
        unsafe { std::env::remove_var("WAVESPEED_API_KEY") };
    }
}
