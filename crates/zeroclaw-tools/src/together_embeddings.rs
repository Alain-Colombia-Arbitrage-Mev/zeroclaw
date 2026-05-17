//! Together embeddings — direct REST tool against the Together.ai embeddings API.
//!
//! [Together](https://api.together.xyz) hosts open-weight embedding models
//! (BGE, M2-BERT, UAE) behind an OpenAI-compatible REST surface:
//!
//! - `POST https://api.together.xyz/v1/embeddings` with `{model, input}`
//! - response is OpenAI-shaped: `{data: [{embedding: [f32...], index, ...}], ...}`
//!
//! Scope (v1): agentic use case only. The agent calls this tool when it
//! needs an embedding for clustering, classification, or ad-hoc semantic
//! search of in-context data. Using Together as the automatic embedding
//! provider for the memory subsystem is a separate, larger change (touches
//! `embeddings_provider` dispatch in `zeroclaw-memory`).

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::TogetherEmbeddingsConfig;

const DEFAULT_BASE_URL: &str = "https://api.together.xyz";
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub struct TogetherEmbeddingsTool {
    security: Arc<SecurityPolicy>,
    config: TogetherEmbeddingsConfig,
    client: Client,
}

impl TogetherEmbeddingsTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        config: TogetherEmbeddingsConfig,
    ) -> anyhow::Result<Self> {
        let timeout = Duration::from_secs(config.timeout_secs);
        let builder = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none());
        let builder = zeroclaw_config::schema::apply_runtime_proxy_to_builder(
            builder,
            "tool.together_embeddings",
        );
        let client = builder.build()?;
        Ok(Self {
            security,
            config,
            client,
        })
    }

    fn base_url(&self) -> &str {
        if self.config.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            self.config.base_url.trim_end_matches('/')
        }
    }

    fn api_key(&self) -> Result<String, String> {
        if let Some(k) = self.config.api_key.as_deref()
            && !k.is_empty()
        {
            return Ok(k.to_string());
        }
        let env_var = if self.config.api_key_env.is_empty() {
            "TOGETHER_API_KEY"
        } else {
            self.config.api_key_env.as_str()
        };
        std::env::var(env_var)
            .map(|v| v.trim().to_string())
            .ok()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                format!(
                    "Missing Together API key: set the {env_var} env var or together_embeddings.api_key in config"
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
        if self.config.allowed_models.iter().any(|p| p == "*") {
            return Ok(m.to_string());
        }
        if self.config.allowed_models.is_empty() {
            return Err(
                "together_embeddings is enabled but no allowed_models are configured. Add at least one entry to [together_embeddings].allowed_models in config.toml".into(),
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
            "model '{m}' is not in together_embeddings.allowed_models"
        ))
    }

    /// Validate input is either a non-empty string or a non-empty array of
    /// non-empty strings, capped at `max_batch_size`. Total character count
    /// across all inputs is capped at `max_total_chars` (Together bills per
    /// input token, so this prevents accidental large bills).
    fn validate_input(&self, input: &Value) -> Result<Value, String> {
        let total_chars: usize;
        let normalized: Value;
        match input {
            Value::String(s) => {
                if s.trim().is_empty() {
                    return Err("'input' string cannot be empty".into());
                }
                total_chars = s.chars().count();
                normalized = input.clone();
            }
            Value::Array(items) => {
                if items.is_empty() {
                    return Err("'input' array cannot be empty".into());
                }
                if items.len() > self.config.max_batch_size {
                    return Err(format!(
                        "input batch size {} exceeds max_batch_size {}",
                        items.len(),
                        self.config.max_batch_size
                    ));
                }
                let mut chars: usize = 0;
                for item in items {
                    let s = item
                        .as_str()
                        .ok_or_else(|| "'input' array items must be strings".to_string())?;
                    if s.trim().is_empty() {
                        return Err("'input' array items cannot be empty".into());
                    }
                    chars += s.chars().count();
                }
                total_chars = chars;
                normalized = input.clone();
            }
            _ => {
                return Err("'input' must be a string or array of strings".into());
            }
        }
        if total_chars > self.config.max_total_chars {
            return Err(format!(
                "total input chars {total_chars} exceeds max_total_chars {}",
                self.config.max_total_chars
            ));
        }
        Ok(normalized)
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
impl Tool for TogetherEmbeddingsTool {
    fn name(&self) -> &str {
        "together_embeddings"
    }

    fn description(&self) -> &str {
        "Generate text embeddings via the Together.ai API (https://api.together.xyz/v1/embeddings). \
         Supports BGE, M2-BERT, UAE and other open-weight embedding models. OpenAI-compatible response \
         shape. Use for clustering, classification, or ad-hoc semantic similarity. NOT a memory provider — \
         the agent stores the vectors itself if needed."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "model": {
                    "type": "string",
                    "description": "Together model identifier. Examples: 'togethercomputer/m2-bert-80M-8k-retrieval', 'BAAI/bge-large-en-v1.5'. Must be in allowed_models."
                },
                "input": {
                    "oneOf": [
                        {"type": "string"},
                        {"type": "array", "items": {"type": "string"}}
                    ],
                    "description": "Single text string or array of strings (batch). Subject to max_batch_size and max_total_chars."
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
            .enforce_tool_operation(ToolOperation::Act, "together_embeddings")
        {
            return Ok(failure(e));
        }

        let model_arg = args
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'model' parameter"))?;
        let input_arg = args
            .get("input")
            .ok_or_else(|| anyhow::anyhow!("Missing 'input' parameter"))?;

        let model = match self.validate_model(model_arg) {
            Ok(m) => m,
            Err(e) => return Ok(failure(e)),
        };
        let input = match self.validate_input(input_arg) {
            Ok(i) => i,
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

        let url = format!("{}/v1/embeddings", self.base_url());
        let body = json!({ "model": model, "input": input });

        let resp = match self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return Ok(failure(format!("Together embeddings POST failed: {e}"))),
        };

        let status = resp.status();
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                return Ok(failure(format!(
                    "Together embeddings response body read failed: {e}"
                )));
            }
        };
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Ok(failure(format!(
                "Together embeddings response exceeded {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let text = String::from_utf8_lossy(&bytes).to_string();
        if !status.is_success() {
            return Ok(failure(format!(
                "Together embeddings API error ({status}): {text}"
            )));
        }
        Ok(ToolResult {
            success: true,
            output: text,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;

    fn cfg() -> TogetherEmbeddingsConfig {
        TogetherEmbeddingsConfig {
            enabled: true,
            base_url: "https://api.together.xyz".into(),
            api_key: Some("test-key".into()),
            api_key_env: String::new(),
            allowed_models: vec![
                "togethercomputer/m2-bert-80M-8k-retrieval".into(),
                "BAAI/bge-large-en-v1.5".into(),
            ],
            max_batch_size: 32,
            max_total_chars: 50_000,
            timeout_secs: 30,
        }
    }

    fn tool() -> TogetherEmbeddingsTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            ..SecurityPolicy::default()
        });
        TogetherEmbeddingsTool::new(security, cfg()).expect("client builds")
    }

    #[test]
    fn name_is_together_embeddings() {
        assert_eq!(tool().name(), "together_embeddings");
    }

    #[test]
    fn schema_requires_model_and_input() {
        let s = tool().parameters_schema();
        let req = s["required"].as_array().unwrap();
        assert!(req.contains(&json!("model")));
        assert!(req.contains(&json!("input")));
    }

    #[test]
    fn base_url_defaults_when_empty() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.base_url = String::new();
        let t = TogetherEmbeddingsTool::new(security, c).unwrap();
        assert_eq!(t.base_url(), DEFAULT_BASE_URL);
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.base_url = "https://my-together.local/".into();
        let t = TogetherEmbeddingsTool::new(security, c).unwrap();
        assert_eq!(t.base_url(), "https://my-together.local");
    }

    #[test]
    fn validate_model_accepts_listed() {
        assert!(
            tool()
                .validate_model("togethercomputer/m2-bert-80M-8k-retrieval")
                .is_ok()
        );
        assert!(tool().validate_model("BAAI/bge-large-en-v1.5").is_ok());
    }

    #[test]
    fn validate_model_rejects_unlisted() {
        let err = tool().validate_model("evil/spam").unwrap_err();
        assert!(err.contains("allowed_models"));
    }

    #[test]
    fn validate_model_rejects_bad_format() {
        assert!(tool().validate_model("").is_err());
        assert!(tool().validate_model("has space").is_err());
    }

    #[test]
    fn validate_model_wildcard_allows_anything() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.allowed_models = vec!["*".into()];
        let t = TogetherEmbeddingsTool::new(security, c).unwrap();
        assert!(t.validate_model("anyone/anything").is_ok());
    }

    #[test]
    fn validate_model_empty_allowlist_errors_clearly() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.allowed_models = vec![];
        let t = TogetherEmbeddingsTool::new(security, c).unwrap();
        let err = t.validate_model("any/thing").unwrap_err();
        assert!(err.contains("no allowed_models"));
    }

    #[test]
    fn validate_input_string_ok() {
        assert!(tool().validate_input(&json!("hello world")).is_ok());
    }

    #[test]
    fn validate_input_array_ok() {
        assert!(
            tool()
                .validate_input(&json!(["text 1", "text 2", "text 3"]))
                .is_ok()
        );
    }

    #[test]
    fn validate_input_empty_string_rejected() {
        assert!(tool().validate_input(&json!("")).is_err());
        assert!(tool().validate_input(&json!("   ")).is_err());
    }

    #[test]
    fn validate_input_empty_array_rejected() {
        let err = tool().validate_input(&json!([])).unwrap_err();
        assert!(err.contains("cannot be empty"));
    }

    #[test]
    fn validate_input_array_with_empty_item_rejected() {
        assert!(tool().validate_input(&json!(["valid", ""])).is_err());
    }

    #[test]
    fn validate_input_array_with_non_string_rejected() {
        assert!(tool().validate_input(&json!(["valid", 42])).is_err());
    }

    #[test]
    fn validate_input_non_string_non_array_rejected() {
        assert!(tool().validate_input(&json!({"a": 1})).is_err());
        assert!(tool().validate_input(&json!(null)).is_err());
        assert!(tool().validate_input(&json!(42)).is_err());
    }

    #[test]
    fn validate_input_oversized_batch_rejected() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.max_batch_size = 2;
        let t = TogetherEmbeddingsTool::new(security, c).unwrap();
        let err = t.validate_input(&json!(["a", "b", "c"])).unwrap_err();
        assert!(err.contains("max_batch_size"));
    }

    #[test]
    fn validate_input_total_chars_capped() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.max_total_chars = 10;
        let t = TogetherEmbeddingsTool::new(security, c).unwrap();
        let err = t
            .validate_input(&json!(
                "this string is definitely more than ten characters long"
            ))
            .unwrap_err();
        assert!(err.contains("max_total_chars"));
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            ..SecurityPolicy::default()
        });
        let t = TogetherEmbeddingsTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({
                "model": "BAAI/bge-large-en-v1.5",
                "input": "hello"
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
            ..SecurityPolicy::default()
        });
        let t = TogetherEmbeddingsTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({
                "model": "BAAI/bge-large-en-v1.5",
                "input": "hello"
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_missing_model_errors() {
        let r = tool().execute(json!({"input": "hello"})).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_missing_input_errors() {
        let r = tool()
            .execute(json!({"model": "BAAI/bge-large-en-v1.5"}))
            .await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn execute_rejects_disallowed_model_non_fatal() {
        let r = tool()
            .execute(json!({
                "model": "evil/model",
                "input": "hello"
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("allowed_models"));
    }
}
