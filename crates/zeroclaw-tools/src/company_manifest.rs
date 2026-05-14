//! Single source of truth for the company the orchestrator is
//! building. Multi-tenant aware:
//!
//!   - When `ACTIVE_TENANT` is scoped (the dashboard sets it from the
//!     selected company), the manifest lives at
//!     `<workspace>/companies/<tenant>/{MANIFEST.md,identity.toml}` so
//!     each company has its own brief and advisor agents anchored to a
//!     specific tenant don't pull a stale neighbour's identity.
//!   - Without a tenant scope, falls back to the legacy
//!     `<workspace>/company/{MANIFEST.md,identity.toml}` location.
//!
//! On the first `read` against a tenant-scoped path with no
//! `identity.toml` yet, the tool **auto-seeds** the identity from
//! `<workspace>/tenants.json` (name, mission, category, stage,
//! description). Advisors get a working brief without an explicit
//! `init` step.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;

const MAX_NARRATIVE_BYTES: usize = 256 * 1024;
const MAX_FIELD_BYTES: usize = 8 * 1024;

/// Read / initialise / update the company manifest.
pub struct CompanyManifestTool {
    security: Arc<SecurityPolicy>,
}

impl CompanyManifestTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }

    /// Slug of the currently scoped tenant (if any). Read once per
    /// call so the rest of the action sees a consistent value even if
    /// scopes change mid-flight.
    fn active_tenant(&self) -> Option<String> {
        zeroclaw_memory::qdrant::ACTIVE_TENANT
            .try_with(|t| t.clone())
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn company_dir(&self) -> PathBuf {
        match self.active_tenant() {
            Some(t) => self
                .security
                .resolve_tool_path(&format!("companies/{t}")),
            None => self.security.resolve_tool_path("company"),
        }
    }
    fn manifest_path(&self) -> PathBuf {
        self.company_dir().join("MANIFEST.md")
    }
    fn identity_path(&self) -> PathBuf {
        self.company_dir().join("identity.toml")
    }

    /// Look the active tenant up in `<workspace>/tenants.json` and
    /// return a JSON object suitable for seeding `identity.toml`.
    /// Returns `None` when no tenant is active, the registry is
    /// missing, or the tenant isn't listed.
    async fn lookup_tenant_meta(&self) -> Option<Value> {
        let tenant = self.active_tenant()?;
        let registry_path = self.security.resolve_tool_path("tenants.json");
        let raw = tokio::fs::read_to_string(&registry_path).await.ok()?;
        let parsed: Value = serde_json::from_str(&raw).ok()?;
        let entries = parsed.get("tenants").and_then(|v| v.as_array())?;
        entries
            .iter()
            .find(|e| {
                e.get("id")
                    .and_then(|v| v.as_str())
                    .map(|id| id.eq_ignore_ascii_case(&tenant))
                    .unwrap_or(false)
            })
            .cloned()
    }
}

#[async_trait]
impl Tool for CompanyManifestTool {
    fn name(&self) -> &str {
        "company_manifest"
    }

    fn description(&self) -> &str {
        "Single source of truth for the company the orchestrator is building. \
         Action 'read' returns the current MANIFEST.md + identity.toml. \
         Action 'init' creates them with seed fields if missing. \
         Action 'set_field' updates a single identity.toml key. \
         Action 'append_narrative' appends a dated section to MANIFEST.md. \
         Every advisor agent MUST 'read' this before producing strategy output."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "init", "set_field", "append_narrative"],
                    "description": "Which manifest operation to perform."
                },
                "name": { "type": "string", "description": "For init: company name." },
                "mission": { "type": "string", "description": "For init: one-sentence mission." },
                "stage": { "type": "string", "description": "For init: stage (idea/mvp/pmf/scale/growth)." },
                "geography": { "type": "string", "description": "For init: primary geography." },
                "government_plan": {
                    "type": "string",
                    "enum": ["yes", "no", "undecided"],
                    "description": "For init: will this company pitch governments / sovereign / royal-court buyers? Default 'undecided' forces advisors to ask the operator."
                },
                "target_tiers": {
                    "type": "string",
                    "description": "For init: comma-separated buyer tiers (consumer, smb, enterprise, government, sovereign). Drives sales cycle, compliance, pricing assumptions."
                },
                "key": { "type": "string", "description": "For set_field: dotted key in identity.toml." },
                "value": { "type": "string", "description": "For set_field: new value (quoted automatically)." },
                "heading": { "type": "string", "description": "For append_narrative: section heading." },
                "body": { "type": "string", "description": "For append_narrative: section body (markdown)." }
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
            "read" => self.read().await,
            "init" => self.init(&args).await,
            "set_field" => self.set_field(&args).await,
            "append_narrative" => self.append_narrative(&args).await,
            other => Ok(err(format!("Unknown action: {other}"))),
        }
    }
}

impl CompanyManifestTool {
    async fn read(&self) -> anyhow::Result<ToolResult> {
        let manifest = tokio::fs::read_to_string(self.manifest_path())
            .await
            .ok()
            .unwrap_or_default();
        let identity = tokio::fs::read_to_string(self.identity_path())
            .await
            .ok()
            .unwrap_or_default();

        // Auto-bootstrap: when a tenant is scoped but the manifest is
        // empty, seed both identity.toml and MANIFEST.md from the entry
        // in tenants.json so the advisor agent reading us as step zero
        // gets a real brief instead of "uninitialised". Without this,
        // every multi-tenant flow needs an explicit `init` round-trip
        // before any advisor can produce coherent output.
        if manifest.is_empty() && identity.is_empty() {
            if let Some(meta) = self.lookup_tenant_meta().await
                && let Some(tenant) = self.active_tenant()
                && self.gate().await
            {
                if let Err(e) = self.seed_from_tenant(&tenant, &meta).await {
                    tracing::warn!(
                        tenant = %tenant,
                        "company_manifest: tenant auto-seed failed: {e}",
                    );
                } else {
                    // Re-read the freshly seeded files.
                    let manifest_seeded = tokio::fs::read_to_string(self.manifest_path())
                        .await
                        .ok()
                        .unwrap_or_default();
                    let identity_seeded = tokio::fs::read_to_string(self.identity_path())
                        .await
                        .ok()
                        .unwrap_or_default();
                    return Ok(ToolResult {
                        success: true,
                        output: json!({
                            "status": "auto_seeded_from_tenant",
                            "tenant": tenant,
                            "manifest_path": self.manifest_path().display().to_string(),
                            "identity_path": self.identity_path().display().to_string(),
                            "manifest_md": manifest_seeded,
                            "identity_toml": identity_seeded,
                        })
                        .to_string(),
                        error: None,
                    });
                }
            }

            return Ok(ToolResult {
                success: true,
                output: json!({
                    "status": "uninitialised",
                    "hint": "Call action='init' with at least name + mission to seed the manifest."
                })
                .to_string(),
                error: None,
            });
        }
        Ok(ToolResult {
            success: true,
            output: json!({
                "status": "ok",
                "manifest_path": self.manifest_path().display().to_string(),
                "identity_path": self.identity_path().display().to_string(),
                "manifest_md": manifest,
                "identity_toml": identity,
            })
            .to_string(),
            error: None,
        })
    }

    /// Materialise `identity.toml` + `MANIFEST.md` for the active
    /// tenant using fields from its `tenants.json` entry. Writes the
    /// minimal viable manifest — advisors can append narrative later
    /// via `action='append_narrative'`.
    async fn seed_from_tenant(&self, tenant: &str, meta: &Value) -> anyhow::Result<()> {
        let name = meta
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(tenant);
        let mission = meta
            .get("mission")
            .and_then(|v| v.as_str())
            .unwrap_or("(set me)");
        let stage = meta
            .get("stage")
            .and_then(|v| v.as_str())
            .unwrap_or("idea");
        let category = meta
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let description = meta
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        tokio::fs::create_dir_all(self.company_dir()).await?;

        let toml = format!(
            "# Company identity — auto-seeded from tenants.json.\n\
             # Maintained via the `company_manifest` tool. Hand-edits allowed.\n\
             \n\
             name = {name}\n\
             mission = {mission}\n\
             stage = {stage}\n\
             category = {category}\n\
             tenant_id = {tenant_id}\n\
             created_at = {created}\n\
             \n\
             [product]\n\
             surface = {description}\n\
             target_customer = \"\"\n\
             \n\
             [capital]\n\
             stack = \"\"\n\
             runway_months = 0\n\
             \n\
             [market]\n\
             government_plan = \"undecided\"\n\
             target_tiers = []\n\
             sovereign_buyers = []\n\
             procurement_vehicles = []\n\
             \n\
             [impact]\n\
             sdg_targets = []\n\
             beneficiary_population = \"\"\n",
            name = toml_string(name),
            mission = toml_string(mission),
            stage = toml_string(stage),
            category = toml_string(category),
            tenant_id = toml_string(tenant),
            description = toml_string(description),
            created = toml_string(&Utc::now().to_rfc3339()),
        );
        tokio::fs::write(self.identity_path(), toml).await?;

        let md = format!(
            "# {name} — Company Manifest\n\
             \n\
             > Seeded {date} from `tenants.json` (id=`{tenant}`).\n\
             \n\
             ## Mission\n\n{mission}\n\n\
             ## Stage\n\n{stage}\n\n\
             ## Category\n\n{category}\n\n\
             ## Description\n\n{description}\n\n\
             ## Theory of change\n\n(To be authored by `phd_business` / `ceo_advisor`.)\n\n\
             ## Strategic bets\n\n(Append via `action='append_narrative'`.)\n",
            name = name,
            date = Utc::now().to_rfc3339(),
            tenant = tenant,
            mission = mission,
            stage = stage,
            category = if category.is_empty() {
                "(unset)"
            } else {
                category
            },
            description = if description.is_empty() {
                "(none yet)"
            } else {
                description
            },
        );
        tokio::fs::write(self.manifest_path(), md).await?;
        Ok(())
    }

    async fn init(&self, args: &Value) -> anyhow::Result<ToolResult> {
        if !self.gate().await {
            return Ok(err("Action blocked or rate-limited".into()));
        }
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("Untitled");
        let mission = args
            .get("mission")
            .and_then(|v| v.as_str())
            .unwrap_or("(set me)");
        let stage = args.get("stage").and_then(|v| v.as_str()).unwrap_or("idea");
        let geography = args
            .get("geography")
            .and_then(|v| v.as_str())
            .unwrap_or("(set me)");
        let government_plan = args
            .get("government_plan")
            .and_then(|v| v.as_str())
            .unwrap_or("undecided");
        let target_tiers_raw = args
            .get("target_tiers")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        for f in [name, mission, stage, geography, government_plan, target_tiers_raw] {
            if f.len() > MAX_FIELD_BYTES {
                return Ok(err(format!("Field exceeds {MAX_FIELD_BYTES} bytes")));
            }
        }
        if !matches!(government_plan, "yes" | "no" | "undecided") {
            return Ok(err(
                "government_plan must be one of: yes, no, undecided".into(),
            ));
        }
        let target_tiers: Vec<String> = target_tiers_raw
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let tiers_toml = if target_tiers.is_empty() {
            "[]".to_string()
        } else {
            let quoted: Vec<String> = target_tiers.iter().map(|s| toml_string(s)).collect();
            format!("[{}]", quoted.join(", "))
        };

        tokio::fs::create_dir_all(self.company_dir()).await?;

        if !tokio::fs::try_exists(self.identity_path()).await.unwrap_or(false) {
            let toml = format!(
                "# Company identity — structured source of truth.\n\
                 # Maintained via the `company_manifest` tool. Hand-edits allowed.\n\
                 \n\
                 name = {name}\n\
                 mission = {mission}\n\
                 stage = {stage}\n\
                 geography = {geography}\n\
                 created_at = {created}\n\
                 \n\
                 [product]\n\
                 surface = \"\"\n\
                 target_customer = \"\"\n\
                 \n\
                 [capital]\n\
                 stack = \"\"\n\
                 runway_months = 0\n\
                 \n\
                 [market]\n\
                 # Gating decision: will this company pitch governments / sovereign /\n\
                 # royal-court / multilateral buyers? Until this is 'yes' or 'no', every\n\
                 # advisor agent must surface it to the operator before producing\n\
                 # plans, because the answer reshapes sales cycle, compliance posture,\n\
                 # pricing tiers, partnership strategy, and reference architecture.\n\
                 government_plan = {gov}\n\
                 # Buyer tiers: consumer / smb / enterprise / government / sovereign.\n\
                 target_tiers = {tiers}\n\
                 # Named sovereign / royal / state-buyer prospects (e.g. \"UAE Royal\n\
                 # Court\", \"PIF\", \"Mubadala\", \"GCF\", \"BMZ\", \"USAID\").\n\
                 sovereign_buyers = []\n\
                 # Procurement vehicles in play (GSA, UNGM, EU TED, OECC, framework\n\
                 # agreements, direct award, royal decree, etc.).\n\
                 procurement_vehicles = []\n\
                 \n\
                 [impact]\n\
                 sdg_targets = []\n\
                 beneficiary_population = \"\"\n",
                name = toml_string(name),
                mission = toml_string(mission),
                stage = toml_string(stage),
                geography = toml_string(geography),
                created = toml_string(&Utc::now().to_rfc3339()),
                gov = toml_string(government_plan),
                tiers = tiers_toml,
            );
            tokio::fs::write(self.identity_path(), toml).await?;
        }

        if !tokio::fs::try_exists(self.manifest_path()).await.unwrap_or(false) {
            let md = format!(
                "# {name} — Company Manifest\n\
                 \n\
                 > Seeded {date}. Structured fields live in `identity.toml`.\n\
                 \n\
                 ## Mission\n\
                 \n\
                 {mission}\n\
                 \n\
                 ## Stage\n\
                 \n\
                 {stage}\n\
                 \n\
                 ## Geography\n\
                 \n\
                 {geography}\n\
                 \n\
                 ## Government / sovereign plan\n\
                 \n\
                 `government_plan = {gov_plan}` · `target_tiers = {tiers_md}`\n\
                 \n\
                 Until `government_plan` is `yes` or `no`, every advisor agent must \
                 surface this question to the operator before producing strategy. \
                 The answer reshapes sales cycle, compliance posture, pricing tiers, \
                 partnerships and security architecture.\n\
                 \n\
                 ## Theory of change\n\
                 \n\
                 (To be authored by `phd_business` / `ceo_advisor`.)\n\
                 \n\
                 ## Strategic bets\n\
                 \n\
                 (To be filled — append via `action='append_narrative'`.)\n\
                 \n\
                 ## Impact arm\n\
                 \n\
                 (Authored by `ngo_architect` once the parent thesis is set.)\n",
                name = name,
                date = Utc::now().to_rfc3339(),
                mission = mission,
                stage = stage,
                geography = geography,
                gov_plan = government_plan,
                tiers_md = if target_tiers.is_empty() {
                    "(none chosen yet)".to_string()
                } else {
                    target_tiers.join(", ")
                },
            );
            tokio::fs::write(self.manifest_path(), md).await?;
        }

        self.read().await
    }

    async fn set_field(&self, args: &Value) -> anyhow::Result<ToolResult> {
        if !self.gate().await {
            return Ok(err("Action blocked or rate-limited".into()));
        }
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'key'"))?;
        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'value'"))?;
        if value.len() > MAX_FIELD_BYTES {
            return Ok(err(format!("Value exceeds {MAX_FIELD_BYTES} bytes")));
        }
        let current = tokio::fs::read_to_string(self.identity_path())
            .await
            .unwrap_or_default();
        let updated = upsert_toml_line(&current, key, value);
        tokio::fs::write(self.identity_path(), updated).await?;
        Ok(ToolResult {
            success: true,
            output: json!({ "status": "updated", "key": key }).to_string(),
            error: None,
        })
    }

    async fn append_narrative(&self, args: &Value) -> anyhow::Result<ToolResult> {
        if !self.gate().await {
            return Ok(err("Action blocked or rate-limited".into()));
        }
        let heading = args
            .get("heading")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'heading'"))?;
        let body = args
            .get("body")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'body'"))?;
        if body.len() > MAX_NARRATIVE_BYTES {
            return Ok(err(format!(
                "Body exceeds {MAX_NARRATIVE_BYTES} byte cap"
            )));
        }
        tokio::fs::create_dir_all(self.company_dir()).await?;
        let section = format!(
            "\n## {} — {}\n\n{}\n",
            heading,
            Utc::now().format("%Y-%m-%d"),
            body
        );
        use tokio::io::AsyncWriteExt;
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.manifest_path())
            .await?;
        f.write_all(section.as_bytes()).await?;
        Ok(ToolResult {
            success: true,
            output: json!({ "status": "appended", "heading": heading }).to_string(),
            error: None,
        })
    }

    async fn gate(&self) -> bool {
        if !self.security.can_act() {
            return false;
        }
        if self.security.is_rate_limited() {
            return false;
        }
        self.security.record_action()
    }
}

fn err(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}

/// TOML string literal: escapes quotes + backslashes, wraps in `"..."`.
fn toml_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Naive upsert of a top-level `key = "value"` line. If the key exists
/// (matches `^<key>\s*=`), the line is replaced. Otherwise it is
/// appended. Sections (`[foo]`) are preserved unchanged. This is
/// intentionally small — the manifest is meant to be hand-editable
/// markdown-adjacent TOML, not a full data model.
fn upsert_toml_line(current: &str, key: &str, value: &str) -> String {
    let new_line = format!("{key} = {}", toml_string(value));
    let mut out = String::with_capacity(current.len() + new_line.len() + 1);
    let mut replaced = false;
    for line in current.lines() {
        let trimmed = line.trim_start();
        let matches = trimmed.starts_with(key)
            && trimmed[key.len()..]
                .trim_start()
                .starts_with('=');
        if matches && !replaced {
            out.push_str(&new_line);
            out.push('\n');
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&new_line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_string_escapes_quotes_and_backslashes() {
        assert_eq!(toml_string("hello"), "\"hello\"");
        assert_eq!(toml_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(toml_string("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn upsert_replaces_existing_top_level_key() {
        let input = "name = \"old\"\nstage = \"idea\"\n";
        let out = upsert_toml_line(input, "name", "new");
        assert!(out.contains("name = \"new\""));
        assert!(out.contains("stage = \"idea\""));
        assert!(!out.contains("\"old\""));
    }

    #[test]
    fn upsert_appends_missing_key() {
        let input = "name = \"x\"\n";
        let out = upsert_toml_line(input, "stage", "mvp");
        assert!(out.contains("name = \"x\""));
        assert!(out.contains("stage = \"mvp\""));
    }

    #[test]
    fn upsert_preserves_section_headers() {
        let input = "name = \"x\"\n\n[product]\nsurface = \"\"\n";
        let out = upsert_toml_line(input, "name", "y");
        assert!(out.contains("[product]"));
        assert!(out.contains("surface = \"\""));
    }
}
