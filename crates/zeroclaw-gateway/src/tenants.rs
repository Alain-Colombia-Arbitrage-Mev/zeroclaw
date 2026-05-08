//! Multi-tenant company registry.
//!
//! A "tenant" represents one business the operator is building or
//! running through the Octopus Labs orchestrator — its category drives
//! which sub-agents are most relevant, its stage tracks progress from
//! ideation to scale, and its memory namespace (eventually) keeps its
//! conversations isolated from sibling tenants.
//!
//! Storage is a single JSON file at `workspace/tenants.json`. Locking
//! is in-process; concurrent writes from two daemon instances are not
//! supported (they should not share a workspace).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Business category. Drives the recommended sub-agent subset and
/// default KPI scaffolding for new tenants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantCategory {
    Saas,
    Marketplace,
    Fintech,
    Ecommerce,
    Agency,
    Hardware,
    Media,
    Ai,
    Other,
}

impl TenantCategory {
    /// Sub-agents most relevant to operating this category. The
    /// orchestrator UI uses this to surface them first; the bench
    /// is otherwise the full 38.
    pub fn recommended_agents(self) -> &'static [&'static str] {
        match self {
            TenantCategory::Saas => &[
                "product_manager", "growth_hacker", "pricing_strategist",
                "sdr_outbound", "account_executive", "customer_success",
                "cto_advisor", "data_analyst", "finance_controller",
                "ceo_advisor", "marketing", "coder", "tester",
            ],
            TenantCategory::Marketplace => &[
                "business_developer", "product_manager", "growth_hacker",
                "customer_researcher", "customer_success",
                "data_analyst", "risk_analyst", "legal_compliance",
                "finance_controller", "ceo_advisor", "marketing",
            ],
            TenantCategory::Fintech => &[
                "risk_analyst", "legal_compliance", "security",
                "finance_controller", "cfo_advisor", "data_analyst",
                "product_manager", "account_executive", "customer_success",
                "ceo_advisor", "architect", "db_designer",
            ],
            TenantCategory::Ecommerce => &[
                "marketing", "growth_hacker", "content_creator",
                "pricing_strategist", "customer_success", "data_analyst",
                "business_developer", "finance_controller", "ceo_advisor",
                "designer", "scriptwriter",
            ],
            TenantCategory::Agency => &[
                "business_developer", "account_executive", "customer_success",
                "content_creator", "marketing", "scriptwriter",
                "finance_controller", "legal_compliance", "ceo_advisor",
                "designer", "product_manager",
            ],
            TenantCategory::Hardware => &[
                "architect", "server_architect", "designer",
                "risk_analyst", "legal_compliance", "security",
                "business_developer", "pricing_strategist",
                "finance_controller", "cfo_advisor", "ceo_advisor",
                "market_researcher",
            ],
            TenantCategory::Media => &[
                "content_creator", "scriptwriter", "marketing",
                "growth_hacker", "data_analyst", "designer",
                "business_developer", "ceo_advisor", "pricing_strategist",
                "customer_researcher",
            ],
            TenantCategory::Ai => &[
                "cto_advisor", "architect", "ml_engineer", "data_analyst",
                "product_manager", "growth_hacker", "pricing_strategist",
                "risk_analyst", "legal_compliance", "ceo_advisor",
                "coder", "tester", "security",
            ],
            TenantCategory::Other => &[
                "idea_generator", "idea_validator", "customer_researcher",
                "competitor_analyst", "red_teamer", "pivot_strategist",
                "product_manager", "ceo_advisor",
            ],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TenantCategory::Saas => "SaaS",
            TenantCategory::Marketplace => "Marketplace",
            TenantCategory::Fintech => "Fintech",
            TenantCategory::Ecommerce => "E-commerce",
            TenantCategory::Agency => "Agency",
            TenantCategory::Hardware => "Hardware",
            TenantCategory::Media => "Media",
            TenantCategory::Ai => "AI",
            TenantCategory::Other => "Other",
        }
    }
}

/// Where the business is on the journey from seed idea to scale.
/// Used by the dashboard to render a progress bar and by the
/// orchestrator to bias delegation (a `Validation` stage tenant
/// gets idea_validator + customer_researcher first; a `Scale` stage
/// tenant gets cfo_advisor + risk_analyst first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantStage {
    Ideation,
    Validation,
    Mvp,
    Launch,
    Growth,
    Scale,
}

impl TenantStage {
    pub fn ordinal(self) -> u8 {
        match self {
            TenantStage::Ideation => 0,
            TenantStage::Validation => 1,
            TenantStage::Mvp => 2,
            TenantStage::Launch => 3,
            TenantStage::Growth => 4,
            TenantStage::Scale => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TenantStage::Ideation => "Ideation",
            TenantStage::Validation => "Validation",
            TenantStage::Mvp => "MVP",
            TenantStage::Launch => "Launch",
            TenantStage::Growth => "Growth",
            TenantStage::Scale => "Scale",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub category: TenantCategory,
    #[serde(default = "default_stage")]
    pub stage: TenantStage,
    /// Optional one-line pitch / positioning statement.
    #[serde(default)]
    pub mission: String,
    /// Subset of bench agents the operator wants surfaced first for
    /// this tenant. When empty, the category's recommended set is
    /// used.
    #[serde(default)]
    pub agents: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_stage() -> TenantStage {
    TenantStage::Ideation
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TenantsFile {
    #[serde(default)]
    tenants: Vec<Tenant>,
}

/// In-memory tenant registry backed by a single JSON file.
#[derive(Debug, Clone)]
pub struct TenantRegistry {
    path: PathBuf,
    inner: Arc<RwLock<Vec<Tenant>>>,
}

impl TenantRegistry {
    /// Load the registry from `<workspace>/tenants.json`. Missing or
    /// unreadable file is treated as an empty registry — this lets
    /// fresh installs start with no tenants and create the first one
    /// from the dashboard.
    pub fn load(workspace_dir: &std::path::Path) -> Self {
        let path = workspace_dir.join("tenants.json");
        let tenants = match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str::<TenantsFile>(&raw)
                .map(|f| f.tenants)
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to parse tenants.json — starting with empty registry"
                    );
                    Vec::new()
                }),
            Err(_) => Vec::new(),
        };
        Self {
            path,
            inner: Arc::new(RwLock::new(tenants)),
        }
    }

    pub fn list(&self) -> Vec<Tenant> {
        self.inner.read().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn get(&self, id: &str) -> Option<Tenant> {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.iter().find(|t| t.id == id).cloned())
    }

    pub fn create(&self, mut tenant: Tenant) -> Result<Tenant, String> {
        if tenant.id.trim().is_empty() {
            tenant.id = Self::slugify(&tenant.name);
        }
        if tenant.id.is_empty() {
            return Err("tenant id is empty after slugify".into());
        }
        let now = chrono::Utc::now();
        tenant.created_at = now;
        tenant.updated_at = now;
        {
            let mut guard = self.inner.write().map_err(|e| e.to_string())?;
            if guard.iter().any(|t| t.id == tenant.id) {
                return Err(format!("tenant '{}' already exists", tenant.id));
            }
            guard.push(tenant.clone());
        }
        self.persist()?;
        Ok(tenant)
    }

    pub fn update(&self, id: &str, patch: TenantPatch) -> Result<Tenant, String> {
        let updated;
        {
            let mut guard = self.inner.write().map_err(|e| e.to_string())?;
            let entry = guard
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| format!("tenant '{id}' not found"))?;
            if let Some(name) = patch.name {
                entry.name = name;
            }
            if let Some(category) = patch.category {
                entry.category = category;
            }
            if let Some(stage) = patch.stage {
                entry.stage = stage;
            }
            if let Some(mission) = patch.mission {
                entry.mission = mission;
            }
            if let Some(agents) = patch.agents {
                entry.agents = agents;
            }
            entry.updated_at = chrono::Utc::now();
            updated = entry.clone();
        }
        self.persist()?;
        Ok(updated)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        {
            let mut guard = self.inner.write().map_err(|e| e.to_string())?;
            let len_before = guard.len();
            guard.retain(|t| t.id != id);
            if guard.len() == len_before {
                return Err(format!("tenant '{id}' not found"));
            }
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let tenants = self.inner.read().map_err(|e| e.to_string())?.clone();
        let file = TenantsFile { tenants };
        let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Atomic-ish write: write to temp, rename. Avoids leaving the
        // file half-written if the process dies mid-save.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    fn slugify(name: &str) -> String {
        name.chars()
            .filter_map(|c| {
                if c.is_ascii_alphanumeric() {
                    Some(c.to_ascii_lowercase())
                } else if c.is_whitespace() || c == '-' || c == '_' {
                    Some('_')
                } else {
                    None
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TenantPatch {
    pub name: Option<String>,
    pub category: Option<TenantCategory>,
    pub stage: Option<TenantStage>,
    pub mission: Option<String>,
    pub agents: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(TenantRegistry::slugify("Acme SaaS Co"), "acme_saas_co");
        assert_eq!(TenantRegistry::slugify("  Hello-World!"), "hello-world");
        assert_eq!(TenantRegistry::slugify("123 Foo"), "123_foo");
    }

    #[test]
    fn category_recommends_agents() {
        for cat in [
            TenantCategory::Saas,
            TenantCategory::Fintech,
            TenantCategory::Marketplace,
            TenantCategory::Other,
        ] {
            assert!(!cat.recommended_agents().is_empty());
        }
    }

    #[test]
    fn stage_ordinal_monotonic() {
        let stages = [
            TenantStage::Ideation,
            TenantStage::Validation,
            TenantStage::Mvp,
            TenantStage::Launch,
            TenantStage::Growth,
            TenantStage::Scale,
        ];
        for w in stages.windows(2) {
            assert!(w[0].ordinal() < w[1].ordinal());
        }
    }

    #[test]
    fn registry_create_get_update_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = TenantRegistry::load(tmp.path());
        assert!(reg.list().is_empty());

        let created = reg
            .create(Tenant {
                id: String::new(),
                name: "Acme SaaS".into(),
                category: TenantCategory::Saas,
                stage: TenantStage::Validation,
                mission: "Provisioning DBs".into(),
                agents: vec![],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .expect("create");
        assert_eq!(created.id, "acme_saas");
        assert_eq!(reg.list().len(), 1);

        let got = reg.get("acme_saas").expect("get");
        assert_eq!(got.name, "Acme SaaS");

        let updated = reg
            .update(
                "acme_saas",
                TenantPatch {
                    stage: Some(TenantStage::Mvp),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.stage, TenantStage::Mvp);

        reg.delete("acme_saas").expect("delete");
        assert!(reg.list().is_empty());
    }

    #[test]
    fn duplicate_create_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = TenantRegistry::load(tmp.path());
        let make = || Tenant {
            id: "dup".into(),
            name: "dup".into(),
            category: TenantCategory::Other,
            stage: TenantStage::Ideation,
            mission: String::new(),
            agents: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        reg.create(make()).expect("first");
        assert!(reg.create(make()).is_err());
    }
}
