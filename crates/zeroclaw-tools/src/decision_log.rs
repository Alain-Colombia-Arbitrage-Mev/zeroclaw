//! ADR-style business decision register.
//!
//! Strategic calls (pivot, hire, fundraise, vendor selection, market
//! exit) get a durable record under
//! `business/decisions/<YYYY-MM-DD>-<slug>.md` with TOML frontmatter
//! capturing decider, alternatives considered, reversibility, and a
//! status field that supports the `proposed → accepted → superseded`
//! lifecycle. Every advisor agent is expected to log non-trivial
//! decisions here so the audit trail survives session boundaries.
//!
//! Multi-tenant: paths live under
//! `companies/<tenant>/business/decisions/` when scoped.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_memory::qdrant::ACTIVE_TENANT;

const ALLOWED_STATUS: &[&str] = &["proposed", "accepted", "rejected", "superseded"];
const MAX_BODY_BYTES: usize = 128 * 1024;
const MAX_TITLE_LEN: usize = 200;
const LIST_DEFAULT_LIMIT: usize = 50;

pub struct DecisionLogTool {
    security: Arc<SecurityPolicy>,
}

impl DecisionLogTool {
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

    fn decisions_dir(&self) -> PathBuf {
        self.business_dir().join("decisions")
    }
}

fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut prev_sep = true;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            for low in c.to_lowercase() {
                slug.push(low);
            }
            prev_sep = false;
        } else if !prev_sep {
            slug.push('-');
            prev_sep = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    let capped: String = trimmed.chars().take(60).collect();
    if capped.is_empty() {
        "decision".to_string()
    } else {
        capped
    }
}

#[async_trait]
impl Tool for DecisionLogTool {
    fn name(&self) -> &str {
        "decision_log"
    }

    fn description(&self) -> &str {
        "Durable register for strategic business decisions (pivot, hire, fundraise, vendor \
         selection, market exit). 'record' creates a new entry with frontmatter (id, date, \
         status, decider, alternatives, reversibility) + markdown body. 'read' fetches one by \
         id. 'list' enumerates with optional status filter. 'supersede' marks an old decision \
         as superseded by a new id, preserving the audit trail. Use this whenever the orchestrator \
         or an advisor commits to a non-trivial direction."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["record", "read", "list", "supersede"],
                    "description": "Operation to perform."
                },
                "title": {
                    "type": "string",
                    "description": "For record: one-line summary of the decision (drives the filename slug)."
                },
                "status": {
                    "type": "string",
                    "enum": ALLOWED_STATUS,
                    "description": "For record: lifecycle status. Default 'accepted'."
                },
                "decider": {
                    "type": "string",
                    "description": "For record: who/what called it (agent name, role, or human handle)."
                },
                "context": {
                    "type": "string",
                    "description": "For record: why this came up (situation that forced the decision)."
                },
                "decision": {
                    "type": "string",
                    "description": "For record: the chosen direction in 1-3 sentences."
                },
                "alternatives": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "For record: 1-5 alternatives considered with why they lost."
                },
                "consequences": {
                    "type": "string",
                    "description": "For record: short-term & long-term implications, what to monitor."
                },
                "reversibility": {
                    "type": "string",
                    "enum": ["one_way", "two_way", "expensive"],
                    "description": "For record: cost of reversing. Default 'two_way'."
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "For record: free-form tags for later filtering."
                },
                "id": {
                    "type": "string",
                    "description": "For read/supersede: the decision id (filename without .md)."
                },
                "supersedes_id": {
                    "type": "string",
                    "description": "For supersede: the id of the decision being replaced."
                },
                "status_filter": {
                    "type": "string",
                    "enum": ALLOWED_STATUS,
                    "description": "For list: only return decisions in this status."
                },
                "limit": {
                    "type": "integer",
                    "description": "For list: max ids to return. Default 50."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action'"))?;
        match action {
            "record" => self.record(&args).await,
            "read" => self.read(&args).await,
            "list" => self.list(&args).await,
            "supersede" => self.supersede(&args).await,
            other => Ok(err(format!("Unknown action: {other}"))),
        }
    }
}

impl DecisionLogTool {
    async fn record(&self, args: &Value) -> anyhow::Result<ToolResult> {
        if !self.security.can_act() {
            return Ok(err("Action blocked: autonomy is read-only".into()));
        }
        if self.security.is_rate_limited() {
            return Ok(err(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'title' for record"))?;
        if title.is_empty() || title.len() > MAX_TITLE_LEN {
            return Ok(err(format!(
                "title must be 1..={MAX_TITLE_LEN} chars"
            )));
        }
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("accepted");
        if !ALLOWED_STATUS.iter().any(|s| *s == status) {
            return Ok(err(format!(
                "status must be one of: {}",
                ALLOWED_STATUS.join(", ")
            )));
        }
        let decider = args.get("decider").and_then(|v| v.as_str()).unwrap_or("");
        let context = args.get("context").and_then(|v| v.as_str()).unwrap_or("");
        let decision_body = args.get("decision").and_then(|v| v.as_str()).unwrap_or("");
        let consequences = args
            .get("consequences")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let reversibility = args
            .get("reversibility")
            .and_then(|v| v.as_str())
            .unwrap_or("two_way");
        let alternatives: Vec<String> = args
            .get("alternatives")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let slug = slugify(title);
        let id = format!("{date}-{slug}");
        let path = self.decisions_dir().join(format!("{id}.md"));

        if path.exists() {
            return Ok(err(format!(
                "Decision id '{id}' already exists. Change the title slightly or use 'supersede'."
            )));
        }
        tokio::fs::create_dir_all(self.decisions_dir()).await?;

        let alt_lines = if alternatives.is_empty() {
            "  (none recorded)".to_string()
        } else {
            alternatives
                .iter()
                .enumerate()
                .map(|(i, a)| format!("  {}. {a}", i + 1))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let body = format!(
            "+++\n\
             id = \"{id}\"\n\
             title = \"{}\"\n\
             date = \"{}\"\n\
             status = \"{status}\"\n\
             decider = \"{}\"\n\
             reversibility = \"{reversibility}\"\n\
             tags = {tags_toml}\n\
             +++\n\
             \n\
             # {}\n\
             \n\
             **Status:** {status}  \n\
             **Decider:** {decider}  \n\
             **Reversibility:** {reversibility}\n\
             \n\
             ## Context\n\n{context}\n\
             \n\
             ## Decision\n\n{decision_body}\n\
             \n\
             ## Alternatives considered\n\n{alt_lines}\n\
             \n\
             ## Consequences\n\n{consequences}\n",
            escape_toml_str(title),
            now.to_rfc3339(),
            escape_toml_str(decider),
            title,
            tags_toml = toml_str_array(&tags),
        );

        if body.len() > MAX_BODY_BYTES {
            return Ok(err(format!(
                "decision exceeds {MAX_BODY_BYTES} byte cap (got {})",
                body.len()
            )));
        }

        tokio::fs::write(&path, &body).await?;
        let _ = self.security.record_action();

        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "recorded",
                "id": id,
                "title": title,
                "decision_status": status,
                "path": path.display().to_string(),
            })
            .to_string(),
            error: None,
        })
    }

    async fn read(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' for read"))?;
        let path = self.decisions_dir().join(format!("{id}.md"));
        if !path.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("No decision with id '{id}'")),
            });
        }
        let content = tokio::fs::read_to_string(&path).await?;
        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "ok",
                "id": id,
                "path": path.display().to_string(),
                "markdown": content,
            })
            .to_string(),
            error: None,
        })
    }

    async fn list(&self, args: &Value) -> anyhow::Result<ToolResult> {
        let dir = self.decisions_dir();
        let status_filter = args.get("status_filter").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(LIST_DEFAULT_LIMIT)
            .min(500);
        if !dir.exists() {
            return Ok(ToolResult {
                success: true,
                output: json!({"status": "ok", "count": 0, "ids": []}).to_string(),
                error: None,
            });
        }
        let mut entries = tokio::fs::read_dir(&dir).await?;
        let mut results: Vec<(String, String)> = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
            let status = parse_status_from_frontmatter(&content).unwrap_or_else(|| "unknown".into());
            if let Some(f) = status_filter
                && status != f
            {
                continue;
            }
            results.push((id.to_string(), status));
        }
        results.sort_by(|a, b| b.0.cmp(&a.0)); // newest id first (date-prefixed)
        let total = results.len();
        results.truncate(limit);
        let summarised: Vec<Value> = results
            .into_iter()
            .map(|(id, status)| json!({"id": id, "status": status}))
            .collect();
        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "ok",
                "count": total,
                "returned": summarised.len(),
                "decisions": summarised,
            })
            .to_string(),
            error: None,
        })
    }

    async fn supersede(&self, args: &Value) -> anyhow::Result<ToolResult> {
        if !self.security.can_act() {
            return Ok(err("Action blocked: autonomy is read-only".into()));
        }
        let old_id = args
            .get("supersedes_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'supersedes_id'"))?;
        let new_id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' (the new decision)"))?;
        let path = self.decisions_dir().join(format!("{old_id}.md"));
        if !path.exists() {
            return Ok(err(format!("No decision with id '{old_id}'")));
        }
        let content = tokio::fs::read_to_string(&path).await?;
        let rewritten = rewrite_frontmatter_status(&content, "superseded", Some(new_id));
        tokio::fs::write(&path, &rewritten).await?;
        let _ = self.security.record_action();
        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "superseded",
                "old_id": old_id,
                "new_id": new_id,
                "path": path.display().to_string(),
            })
            .to_string(),
            error: None,
        })
    }
}

fn escape_toml_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn toml_str_array(items: &[String]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|s| format!("\"{}\"", escape_toml_str(s)))
        .collect();
    format!("[{}]", parts.join(", "))
}

fn parse_status_from_frontmatter(content: &str) -> Option<String> {
    let rest = content.strip_prefix("+++\n")?;
    let (frontmatter, _) = rest.split_once("\n+++")?;
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("status") {
            let value = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=');
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn rewrite_frontmatter_status(content: &str, new_status: &str, superseded_by: Option<&str>) -> String {
    let Some(rest) = content.strip_prefix("+++\n") else {
        return content.to_string();
    };
    let Some((frontmatter, body)) = rest.split_once("\n+++") else {
        return content.to_string();
    };
    let mut updated_lines: Vec<String> = Vec::new();
    let mut saw_status = false;
    let mut saw_superseded_by = false;
    for line in frontmatter.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("status") {
            updated_lines.push(format!("status = \"{new_status}\""));
            saw_status = true;
            continue;
        }
        if trimmed.starts_with("superseded_by") {
            saw_superseded_by = true;
            if let Some(by) = superseded_by {
                updated_lines.push(format!("superseded_by = \"{by}\""));
                continue;
            }
        }
        updated_lines.push(line.to_string());
    }
    if !saw_status {
        updated_lines.push(format!("status = \"{new_status}\""));
    }
    if let Some(by) = superseded_by
        && !saw_superseded_by
    {
        updated_lines.push(format!("superseded_by = \"{by}\""));
    }
    format!("+++\n{}\n+++{body}", updated_lines.join("\n"))
}

fn err(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}
