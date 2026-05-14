//! Generic business-entity store for the orchestrator.
//!
//! Persists structured records (customers, deals, vendors, employees,
//! partnerships, competitors) under
//! `<workspace>/companies/<tenant>/business/entities/<type>/<id>.toml`
//! when a tenant is scoped, or `<workspace>/business/entities/...`
//! otherwise. Merge semantics replace whole top-level fields; advisors
//! that need finer granularity should `read`, mutate locally, then
//! `upsert` the full record back.
//!
//! Type names are constrained to a small allowlist so the LLM can't
//! invent arbitrary directories and the FalkorDB / Postgres relational
//! views stay schema-aware.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_memory::qdrant::ACTIVE_TENANT;

const ALLOWED_TYPES: &[&str] = &[
    "customers",
    "deals",
    "vendors",
    "employees",
    "partnerships",
    "competitors",
    "investors",
    "regulators",
];

const MAX_RECORD_BYTES: usize = 64 * 1024;
const MAX_ID_LEN: usize = 80;
const LIST_DEFAULT_LIMIT: usize = 50;

/// Generic CRUD over `business/entities/<type>/<id>.toml`.
pub struct EntityUpsertTool {
    security: Arc<SecurityPolicy>,
}

impl EntityUpsertTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    fn active_tenant(&self) -> Option<String> {
        ACTIVE_TENANT
            .try_with(|t| t.clone())
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn business_dir(&self) -> PathBuf {
        match self.active_tenant() {
            Some(t) => self
                .security
                .resolve_tool_path(&format!("companies/{t}/business")),
            None => self.security.resolve_tool_path("business"),
        }
    }

    fn type_dir(&self, entity_type: &str) -> PathBuf {
        self.business_dir().join("entities").join(entity_type)
    }

    fn record_path(&self, entity_type: &str, id: &str) -> PathBuf {
        self.type_dir(entity_type).join(format!("{id}.toml"))
    }

    fn validate_type(t: &str) -> Result<(), String> {
        if ALLOWED_TYPES.iter().any(|allowed| *allowed == t) {
            Ok(())
        } else {
            Err(format!(
                "Unknown entity type '{t}'. Allowed: {}",
                ALLOWED_TYPES.join(", ")
            ))
        }
    }

    fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty() || id.len() > MAX_ID_LEN {
            return Err(format!(
                "id must be 1..={MAX_ID_LEN} chars, got {}",
                id.len()
            ));
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(
                "id must be ascii alphanumeric with '-' or '_' only (no slashes, dots, spaces)"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[async_trait]
impl Tool for EntityUpsertTool {
    fn name(&self) -> &str {
        "entity_upsert"
    }

    fn description(&self) -> &str {
        "Read, write, list, or delete structured business records (customers, deals, vendors, \
         employees, partnerships, competitors, investors, regulators). Files live under \
         business/entities/<type>/<id>.toml and are multi-tenant scoped via the active \
         delegation. Use 'upsert' to create or merge fields; 'read' returns the current record; \
         'list' enumerates ids of a type; 'delete' removes a record. Prefer this over \
         free file_write for any persistent customer / deal / vendor state."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "upsert", "list", "delete"],
                    "description": "Operation to perform."
                },
                "type": {
                    "type": "string",
                    "description": "Entity type. Allowed: customers, deals, vendors, employees, partnerships, competitors, investors, regulators.",
                    "enum": ALLOWED_TYPES
                },
                "id": {
                    "type": "string",
                    "description": "Entity id (slug). Required for read/upsert/delete. Ignored for list."
                },
                "fields": {
                    "type": "object",
                    "description": "For upsert: top-level fields to merge into the record. Existing fields not in this object are preserved unless `replace=true`."
                },
                "replace": {
                    "type": "boolean",
                    "description": "For upsert: when true, overwrite the entire record instead of merging. Default false."
                },
                "limit": {
                    "type": "integer",
                    "description": "For list: max ids to return. Default 50."
                }
            },
            "required": ["action", "type"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action'"))?;
        let entity_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'type'"))?;

        if let Err(e) = Self::validate_type(entity_type) {
            return Ok(err(e));
        }

        match action {
            "read" => self.read(entity_type, &args).await,
            "upsert" => self.upsert(entity_type, &args).await,
            "list" => self.list(entity_type, &args).await,
            "delete" => self.delete(entity_type, &args).await,
            other => Ok(err(format!("Unknown action: {other}"))),
        }
    }
}

impl EntityUpsertTool {
    async fn read(&self, entity_type: &str, args: &Value) -> anyhow::Result<ToolResult> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' for read"))?;
        if let Err(e) = Self::validate_id(id) {
            return Ok(err(e));
        }

        let path = self.record_path(entity_type, id);
        if !path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("No {entity_type} record with id '{id}'")),
            });
        }
        let content = tokio::fs::read_to_string(&path).await?;
        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "ok",
                "type": entity_type,
                "id": id,
                "path": path.display().to_string(),
                "toml": content,
            })
            .to_string(),
            error: None,
        })
    }

    async fn upsert(&self, entity_type: &str, args: &Value) -> anyhow::Result<ToolResult> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' for upsert"))?;
        if let Err(e) = Self::validate_id(id) {
            return Ok(err(e));
        }
        let fields = args
            .get("fields")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("Missing 'fields' object for upsert"))?;
        let replace = args
            .get("replace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !self.security.can_act() {
            return Ok(err("Action blocked: autonomy is read-only".into()));
        }
        if self.security.is_rate_limited() {
            return Ok(err(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }

        let path = self.record_path(entity_type, id);
        tokio::fs::create_dir_all(self.type_dir(entity_type)).await?;

        let now = Utc::now().to_rfc3339();
        let merged_toml: String = if replace || !path.exists() {
            render_toml(entity_type, id, fields, &now, /* created */ true)?
        } else {
            let existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            let existing_value: toml::Value =
                toml::from_str(&existing).unwrap_or_else(|_| toml::Value::Table(Default::default()));
            let mut existing_table = match existing_value {
                toml::Value::Table(t) => t,
                _ => Default::default(),
            };
            for (k, v) in fields {
                existing_table.insert(k.clone(), json_to_toml(v));
            }
            existing_table.insert(
                "updated_at".to_string(),
                toml::Value::String(now.clone()),
            );
            let body = toml::to_string_pretty(&toml::Value::Table(existing_table))
                .unwrap_or_default();
            format!(
                "# Auto-managed by entity_upsert. Hand-edits allowed.\n# type = \"{entity_type}\", id = \"{id}\"\n\n{body}"
            )
        };

        if merged_toml.len() > MAX_RECORD_BYTES {
            return Ok(err(format!(
                "Record exceeds {MAX_RECORD_BYTES} byte cap (got {})",
                merged_toml.len()
            )));
        }

        tokio::fs::write(&path, &merged_toml).await?;
        let _ = self.security.record_action();

        Ok(ToolResult {
            success: true,
            output: json!({
                "status": if replace { "replaced" } else { "upserted" },
                "type": entity_type,
                "id": id,
                "path": path.display().to_string(),
                "bytes": merged_toml.len(),
            })
            .to_string(),
            error: None,
        })
    }

    async fn list(&self, entity_type: &str, args: &Value) -> anyhow::Result<ToolResult> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(LIST_DEFAULT_LIMIT)
            .min(500);
        let dir = self.type_dir(entity_type);
        if !dir.exists() {
            return Ok(ToolResult {
                success: true,
                output: json!({
                    "status": "ok",
                    "type": entity_type,
                    "count": 0,
                    "ids": [],
                })
                .to_string(),
                error: None,
            });
        }
        let mut entries = tokio::fs::read_dir(&dir).await?;
        let mut ids: Vec<String> = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                ids.push(stem.to_string());
            }
        }
        ids.sort();
        let total = ids.len();
        ids.truncate(limit);

        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "ok",
                "type": entity_type,
                "count": total,
                "returned": ids.len(),
                "ids": ids,
            })
            .to_string(),
            error: None,
        })
    }

    async fn delete(&self, entity_type: &str, args: &Value) -> anyhow::Result<ToolResult> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' for delete"))?;
        if let Err(e) = Self::validate_id(id) {
            return Ok(err(e));
        }
        if !self.security.can_act() {
            return Ok(err("Action blocked: autonomy is read-only".into()));
        }
        let path = self.record_path(entity_type, id);
        if !path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("No {entity_type} record with id '{id}'")),
            });
        }
        tokio::fs::remove_file(&path).await?;
        let _ = self.security.record_action();
        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "deleted",
                "type": entity_type,
                "id": id,
            })
            .to_string(),
            error: None,
        })
    }
}

fn render_toml(
    entity_type: &str,
    id: &str,
    fields: &serde_json::Map<String, Value>,
    now: &str,
    created: bool,
) -> anyhow::Result<String> {
    let mut table = toml::map::Map::new();
    for (k, v) in fields {
        table.insert(k.clone(), json_to_toml(v));
    }
    if created {
        table
            .entry("created_at".to_string())
            .or_insert_with(|| toml::Value::String(now.to_string()));
    }
    table.insert(
        "updated_at".to_string(),
        toml::Value::String(now.to_string()),
    );
    let body = toml::to_string_pretty(&toml::Value::Table(table)).unwrap_or_default();
    Ok(format!(
        "# Auto-managed by entity_upsert. Hand-edits allowed.\n# type = \"{entity_type}\", id = \"{id}\"\n\n{body}"
    ))
}

fn json_to_toml(v: &Value) -> toml::Value {
    match v {
        Value::Null => toml::Value::String(String::new()),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(arr) => toml::Value::Array(arr.iter().map(json_to_toml).collect()),
        Value::Object(obj) => {
            let mut m = toml::map::Map::new();
            for (k, v) in obj {
                m.insert(k.clone(), json_to_toml(v));
            }
            toml::Value::Table(m)
        }
    }
}

fn err(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}
