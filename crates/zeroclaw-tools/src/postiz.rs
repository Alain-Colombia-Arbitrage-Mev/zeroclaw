//! Postiz integration — REST client for the self-hosted or cloud Postiz API.
//!
//! [Postiz](https://github.com/gitroomhq/postiz-app) is an open-source social
//! media scheduler. This tool talks to its public REST API:
//!
//! - `POST   {base}/public/v1/posts`         — schedule or publish a post
//! - `POST   {base}/public/v1/upload`        — upload media (multipart)
//! - `GET    {base}/public/v1/integrations`  — list connected social accounts
//!
//! Authentication uses the `Authorization` header carrying either an API key
//! or an OAuth token (Postiz accepts the raw value, no `Bearer` prefix).
//!
//! Security:
//! - API key is a `#[secret]` config field (OS keyring)
//! - `SecurityPolicy.record_action()` consumes the hourly action budget
//! - Read-only autonomy blocks every operation (Postiz is a publishing tool)

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::{SecurityPolicy, ToolOperation};
use zeroclaw_config::schema::PostizConfig;

const DEFAULT_BASE_URL: &str = "https://api.postiz.com";

pub struct PostizTool {
    security: Arc<SecurityPolicy>,
    config: PostizConfig,
    client: Client,
}

impl PostizTool {
    pub fn new(security: Arc<SecurityPolicy>, config: PostizConfig) -> anyhow::Result<Self> {
        let timeout = Duration::from_secs(config.timeout_secs);
        let builder = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none());
        let builder =
            zeroclaw_config::schema::apply_runtime_proxy_to_builder(builder, "tool.postiz");
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

    fn api_key(&self) -> Result<&str, String> {
        match self.config.api_key.as_deref() {
            Some(k) if !k.is_empty() => Ok(k),
            _ => Err("postiz.api_key is not configured".into()),
        }
    }

    /// Inspect a post body for `integrations` and confirm each entry's
    /// `provider`/`providerIdentifier` falls inside the configured allowlist.
    /// Postiz POST /posts body has shape `{ posts: [{ integration: {id, ...}, ... }], ... }`
    /// or `{ posts: [{ integrationId: "..." }] }`. We don't enforce a schema; we just
    /// look for any field with the provider name and check it.
    fn validate_providers_in_body(&self, body: &serde_json::Value) -> Result<(), String> {
        if self.config.allowed_providers.iter().any(|p| p == "*") {
            return Ok(());
        }
        let allowed: Vec<String> = self
            .config
            .allowed_providers
            .iter()
            .map(|p| p.to_lowercase())
            .collect();
        let mut bad: Option<String> = None;
        fn walk(value: &serde_json::Value, allowed: &[String], bad: &mut Option<String>) {
            if bad.is_some() {
                return;
            }
            match value {
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        let k_lc = k.to_lowercase();
                        if (k_lc == "provider"
                            || k_lc == "providername"
                            || k_lc == "provideridentifier")
                            && let Some(s) = v.as_str()
                        {
                            let s_lc = s.to_lowercase();
                            if !allowed.iter().any(|a| a == &s_lc) {
                                *bad = Some(s.to_string());
                                return;
                            }
                        }
                        walk(v, allowed, bad);
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        walk(item, allowed, bad);
                        if bad.is_some() {
                            return;
                        }
                    }
                }
                _ => {}
            }
        }
        walk(body, &allowed, &mut bad);
        match bad {
            Some(p) => Err(format!("provider '{p}' is not in postiz.allowed_providers")),
            None => Ok(()),
        }
    }

    async fn op_create_post(&self, args: &serde_json::Value) -> ToolResult {
        let key = match self.api_key() {
            Ok(k) => k,
            Err(e) => return failure(e),
        };
        let body = match args.get("body") {
            Some(b) if b.is_object() => b,
            _ => {
                return failure("'body' must be a JSON object (Postiz POST /posts payload)".into());
            }
        };
        if let Err(e) = self.validate_providers_in_body(body) {
            return failure(e);
        }
        let url = format!("{}/public/v1/posts", self.base_url());
        let req = self
            .client
            .post(&url)
            .header("Authorization", key)
            .header("Content-Type", "application/json")
            .json(body);
        match req.send().await {
            Ok(resp) => translate_response(resp).await,
            Err(e) => failure(format!("Postiz POST /posts failed: {e}")),
        }
    }

    async fn op_list_integrations(&self) -> ToolResult {
        let key = match self.api_key() {
            Ok(k) => k,
            Err(e) => return failure(e),
        };
        let url = format!("{}/public/v1/integrations", self.base_url());
        let req = self.client.get(&url).header("Authorization", key);
        match req.send().await {
            Ok(resp) => translate_response(resp).await,
            Err(e) => failure(format!("Postiz GET /integrations failed: {e}")),
        }
    }

    /// Upload a file from `workspace_dir` to Postiz as multipart.
    async fn op_upload_media(&self, args: &serde_json::Value) -> ToolResult {
        let key = match self.api_key() {
            Ok(k) => k,
            Err(e) => return failure(e),
        };
        let path_raw = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return failure("'path' (string) is required".into()),
        };
        let path = std::path::PathBuf::from(path_raw);
        let canon = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => return failure(format!("path '{path_raw}' not accessible: {e}")),
        };
        let workspace = match self.security.workspace_dir.canonicalize() {
            Ok(w) => w,
            Err(e) => return failure(format!("workspace_dir not accessible: {e}")),
        };
        if !canon.starts_with(&workspace) {
            return failure(format!("path '{path_raw}' is outside workspace_dir"));
        }
        let bytes = match tokio::fs::read(&canon).await {
            Ok(b) => b,
            Err(e) => return failure(format!("cannot read '{path_raw}': {e}")),
        };
        if bytes.len() as u64 > self.config.max_upload_bytes {
            return failure(format!(
                "file size {} exceeds max_upload_bytes {}",
                bytes.len(),
                self.config.max_upload_bytes
            ));
        }
        let filename = canon
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload.bin")
            .to_string();
        let part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
        let form = reqwest::multipart::Form::new().part("file", part);
        let url = format!("{}/public/v1/upload", self.base_url());
        let req = self
            .client
            .post(&url)
            .header("Authorization", key)
            .multipart(form);
        match req.send().await {
            Ok(resp) => translate_response(resp).await,
            Err(e) => failure(format!("Postiz POST /upload failed: {e}")),
        }
    }
}

async fn translate_response(resp: reqwest::Response) -> ToolResult {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if status.is_success() {
        ToolResult {
            success: true,
            output: body,
            error: None,
        }
    } else {
        ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("HTTP {}: {body}", status.as_u16())),
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
impl Tool for PostizTool {
    fn name(&self) -> &str {
        "postiz"
    }

    fn description(&self) -> &str {
        "Schedule and publish social posts via Postiz (https://postiz.com). Operations: \
         'create_post' (POST /posts with full body), 'list_integrations' (GET /integrations \
         to discover connected accounts and their IDs), 'upload_media' (POST /upload from a \
         workspace path). The agent constructs the post body; the tool only enforces auth, \
         provider allowlist, and rate limiting."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["create_post", "list_integrations", "upload_media"]
                },
                "body": {
                    "type": "object",
                    "description": "Postiz POST /posts payload. Required when operation=create_post. Reference: docs.postiz.com/public-api"
                },
                "path": {
                    "type": "string",
                    "description": "File path inside workspace_dir. Required when operation=upload_media."
                }
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
            .enforce_tool_operation(ToolOperation::Act, "postiz")
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
            "create_post" => self.op_create_post(&args).await,
            "list_integrations" => self.op_list_integrations().await,
            "upload_media" => self.op_upload_media(&args).await,
            other => failure(format!(
                "Unknown operation '{other}'. Supported: create_post, list_integrations, upload_media"
            )),
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroclaw_config::autonomy::AutonomyLevel;

    fn cfg() -> PostizConfig {
        PostizConfig {
            enabled: true,
            base_url: "https://example.test".into(),
            api_key: Some("test-key".into()),
            allowed_providers: vec!["x".into(), "linkedin".into()],
            timeout_secs: 10,
            max_upload_bytes: 1_048_576,
        }
    }

    fn tool() -> PostizTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        PostizTool::new(security, cfg()).expect("client builds")
    }

    #[test]
    fn name_is_postiz() {
        assert_eq!(tool().name(), "postiz");
    }

    #[test]
    fn schema_requires_operation() {
        let s = tool().parameters_schema();
        assert!(
            s["required"]
                .as_array()
                .unwrap()
                .contains(&json!("operation"))
        );
    }

    #[test]
    fn base_url_defaults_when_empty() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.base_url = String::new();
        let t = PostizTool::new(security, c).unwrap();
        assert_eq!(t.base_url(), DEFAULT_BASE_URL);
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.base_url = "https://my-postiz.local/".into();
        let t = PostizTool::new(security, c).unwrap();
        assert_eq!(t.base_url(), "https://my-postiz.local");
    }

    #[test]
    fn api_key_missing_errors() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.api_key = None;
        let t = PostizTool::new(security, c).unwrap();
        let err = t.api_key().unwrap_err();
        assert!(err.contains("api_key"));
    }

    #[test]
    fn provider_allowlist_wildcard_allows_anything() {
        let security = Arc::new(SecurityPolicy::default());
        let mut c = cfg();
        c.allowed_providers = vec!["*".into()];
        let t = PostizTool::new(security, c).unwrap();
        let body = json!({"posts": [{"integration": {"provider": "tiktok"}}]});
        assert!(t.validate_providers_in_body(&body).is_ok());
    }

    #[test]
    fn provider_allowlist_rejects_unlisted() {
        let t = tool();
        let body = json!({"posts": [{"integration": {"provider": "tiktok", "content": "x"}}]});
        let err = t.validate_providers_in_body(&body).unwrap_err();
        assert!(err.contains("allowed_providers"));
    }

    #[test]
    fn provider_allowlist_accepts_listed() {
        let t = tool();
        let body = json!({"posts": [{"integration": {"provider": "x", "content": "ok"}}]});
        assert!(t.validate_providers_in_body(&body).is_ok());
    }

    #[test]
    fn provider_allowlist_case_insensitive() {
        let t = tool();
        let body = json!({"posts": [{"integration": {"provider": "LinkedIn"}}]});
        assert!(t.validate_providers_in_body(&body).is_ok());
    }

    #[test]
    fn provider_allowlist_walks_nested_arrays() {
        let t = tool();
        let body = json!({
            "posts": [
                {"integration": {"provider": "x"}},
                {"integration": {"provider": "tiktok"}}
            ]
        });
        assert!(t.validate_providers_in_body(&body).is_err());
    }

    #[test]
    fn provider_allowlist_matches_provideridentifier_key() {
        let t = tool();
        let body = json!({"providerIdentifier": "tiktok"});
        assert!(t.validate_providers_in_body(&body).is_err());
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            workspace_dir: std::env::temp_dir(),
            ..SecurityPolicy::default()
        });
        let t = PostizTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({"operation": "list_integrations"}))
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
        let t = PostizTool::new(security, cfg()).unwrap();
        let r = t
            .execute(json!({"operation": "list_integrations"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("Rate limit"));
    }

    #[tokio::test]
    async fn execute_unknown_operation_non_fatal() {
        let r = tool()
            .execute(json!({"operation": "delete_everything"}))
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
    async fn create_post_rejects_non_object_body() {
        let r = tool()
            .execute(json!({"operation": "create_post", "body": "not-an-object"}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("must be a JSON object"));
    }

    #[tokio::test]
    async fn create_post_rejects_disallowed_provider() {
        let r = tool()
            .execute(json!({
                "operation": "create_post",
                "body": {"posts": [{"integration": {"provider": "tiktok"}}]}
            }))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.error.unwrap().contains("allowed_providers"));
    }
}
