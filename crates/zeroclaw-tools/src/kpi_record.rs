//! Append-only KPI log for the orchestrator and finance / analytics
//! agents. One JSONL file per domain
//! (`business/kpis/<domain>.jsonl`), so a quarterly snapshot or a
//! daily product metric is preserved as a time series. Querying reads
//! the tail with optional filters; recording appends one JSON line
//! per call.
//!
//! Multi-tenant: paths live under
//! `companies/<tenant>/business/kpis/` when a tenant is scoped.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_memory::qdrant::ACTIVE_TENANT;

const ALLOWED_DOMAINS: &[&str] = &[
    "financial",
    "product",
    "operations",
    "sales",
    "marketing",
    "people",
    "risk",
    "compliance",
];

const MAX_NOTE_LEN: usize = 2_000;
const QUERY_DEFAULT_LIMIT: usize = 50;
const QUERY_MAX_LIMIT: usize = 1_000;

pub struct KpiRecordTool {
    security: Arc<SecurityPolicy>,
}

impl KpiRecordTool {
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

    fn domain_path(&self, domain: &str) -> PathBuf {
        self.business_dir()
            .join("kpis")
            .join(format!("{domain}.jsonl"))
    }

    fn validate_domain(d: &str) -> Result<(), String> {
        if ALLOWED_DOMAINS.iter().any(|allowed| *allowed == d) {
            Ok(())
        } else {
            Err(format!(
                "Unknown domain '{d}'. Allowed: {}",
                ALLOWED_DOMAINS.join(", ")
            ))
        }
    }
}

#[async_trait]
impl Tool for KpiRecordTool {
    fn name(&self) -> &str {
        "kpi_record"
    }

    fn description(&self) -> &str {
        "Append-only KPI log. Use 'record' to append a metric reading (timestamp auto-stamped); \
         'query' to read recent entries from a domain with optional metric/since/limit filters. \
         Domains: financial (MRR, burn, runway), product (DAU, retention, NPS), operations \
         (incidents, ticket volume), sales (pipeline, conversion), marketing (CAC, MQL, payback), \
         people (headcount, attrition), risk, compliance. Prefer this over free file_write for \
         any number you'd want to chart later."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["record", "query"],
                    "description": "Operation to perform."
                },
                "domain": {
                    "type": "string",
                    "enum": ALLOWED_DOMAINS,
                    "description": "KPI domain (file). Required."
                },
                "metric": {
                    "type": "string",
                    "description": "For record: metric name (snake_case). For query: optional filter."
                },
                "value": {
                    "description": "For record: numeric value. Accepts integer, float, or numeric string."
                },
                "unit": {
                    "type": "string",
                    "description": "For record: unit hint (USD, %, count, ms, …). Optional."
                },
                "dimensions": {
                    "type": "object",
                    "description": "For record: free-form labels (segment, region, plan, …). Stored as-is for downstream pivoting."
                },
                "note": {
                    "type": "string",
                    "description": "For record: short context (≤2000 chars). Optional."
                },
                "since": {
                    "type": "string",
                    "description": "For query: ISO-8601 timestamp; only entries at or after this point are returned."
                },
                "limit": {
                    "type": "integer",
                    "description": "For query: max entries to return (newest first). Default 50, max 1000."
                }
            },
            "required": ["action", "domain"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action'"))?;
        let domain = args
            .get("domain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'domain'"))?;
        if let Err(e) = Self::validate_domain(domain) {
            return Ok(err(e));
        }
        match action {
            "record" => self.record(domain, &args).await,
            "query" => self.query(domain, &args).await,
            other => Ok(err(format!("Unknown action: {other}"))),
        }
    }
}

impl KpiRecordTool {
    async fn record(&self, domain: &str, args: &Value) -> anyhow::Result<ToolResult> {
        if !self.security.can_act() {
            return Ok(err("Action blocked: autonomy is read-only".into()));
        }
        if self.security.is_rate_limited() {
            return Ok(err(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }
        let metric = args
            .get("metric")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'metric' for record"))?;
        if metric.is_empty() || metric.len() > 80 {
            return Ok(err("metric must be 1..=80 chars".into()));
        }
        let value = args
            .get("value")
            .ok_or_else(|| anyhow::anyhow!("Missing 'value' for record"))?;
        let numeric: f64 = match value {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) => s
                .trim()
                .parse::<f64>()
                .map_err(|_| anyhow::anyhow!("'value' must be numeric (got string {s:?})"))?,
            _ => return Ok(err("'value' must be a number or numeric string".into())),
        };
        if !numeric.is_finite() {
            return Ok(err("'value' must be finite".into()));
        }
        let unit = args
            .get("unit")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let dimensions = args.get("dimensions").cloned().unwrap_or(json!({}));
        let note_opt = args.get("note").and_then(|v| v.as_str());
        if let Some(note) = note_opt
            && note.len() > MAX_NOTE_LEN
        {
            return Ok(err(format!(
                "note exceeds {MAX_NOTE_LEN} chars (got {})",
                note.len()
            )));
        }

        let entry = json!({
            "ts": Utc::now().to_rfc3339(),
            "metric": metric,
            "value": numeric,
            "unit": unit,
            "dimensions": dimensions,
            "note": note_opt,
        });
        let line = format!("{}\n", serde_json::to_string(&entry)?);

        let path = self.domain_path(domain);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;
        let _ = self.security.record_action();

        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "recorded",
                "domain": domain,
                "metric": metric,
                "value": numeric,
                "path": path.display().to_string(),
            })
            .to_string(),
            error: None,
        })
    }

    async fn query(&self, domain: &str, args: &Value) -> anyhow::Result<ToolResult> {
        let path = self.domain_path(domain);
        if !path.exists() {
            return Ok(ToolResult {
                success: true,
                output: json!({
                    "status": "ok",
                    "domain": domain,
                    "count": 0,
                    "entries": [],
                })
                .to_string(),
                error: None,
            });
        }
        let metric_filter = args.get("metric").and_then(|v| v.as_str());
        let since_filter = args.get("since").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(QUERY_DEFAULT_LIMIT)
            .min(QUERY_MAX_LIMIT);

        let raw = tokio::fs::read_to_string(&path).await?;
        let mut matched: Vec<Value> = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(entry) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };
            if let Some(m) = metric_filter
                && entry.get("metric").and_then(|v| v.as_str()) != Some(m)
            {
                continue;
            }
            if let Some(s) = since_filter
                && let Some(ts) = entry.get("ts").and_then(|v| v.as_str())
                && ts < s
            {
                continue;
            }
            matched.push(entry);
        }
        let total = matched.len();
        // Newest first.
        let returned: Vec<Value> = matched.into_iter().rev().take(limit).collect();

        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "ok",
                "domain": domain,
                "count": total,
                "returned": returned.len(),
                "entries": returned,
            })
            .to_string(),
            error: None,
        })
    }
}

fn err(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}
