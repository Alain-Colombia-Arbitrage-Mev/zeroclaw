//! Constrained deliverable writer for advisor / strategy sub-agents.
//!
//! Unlike `file_write`, this tool refuses to write anywhere except
//! under `<workspace>/deliverables/<agent>/<YYYY-MM-DD>-<slug>/`. The
//! filename is also sanitised. Existing files in the same slug folder
//! are never overwritten — the tool appends a `-N` suffix instead, so
//! the audit trail of what each agent produced stays intact.
//!
//! It is meant to be granted to read-mostly advisor presets
//! (phd_business, ngo_architect, ceo_advisor, cfo_advisor, …) which
//! should not have free `file_write` access but DO need to materialise
//! their analyses as markdown / json / yaml / svg artefacts the
//! orchestrator can later collate.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use zeroclaw_api::tool::{Tool, ToolResult};
use zeroclaw_config::policy::SecurityPolicy;
use zeroclaw_memory::qdrant::ACTIVE_AGENT;

/// Maximum bytes per single deliverable file (1 MB).
const MAX_DELIVERABLE_BYTES: usize = 1024 * 1024;

/// Allowed file extensions. Keep this list small — advisor agents
/// produce documents, not executables. Anything code-like belongs in
/// `file_write` granted to engineering presets.
const ALLOWED_EXTENSIONS: &[&str] = &[
    "md", "markdown", "txt", "json", "yaml", "yml", "csv", "svg", "html",
];

/// Constrained deliverable writer. See module docs.
pub struct DeliverableWriteTool {
    security: Arc<SecurityPolicy>,
}

impl DeliverableWriteTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

/// Sanitise an arbitrary string into a filesystem-safe slug. Keeps
/// ascii alphanumerics, lowercases, collapses runs of separators to a
/// single hyphen, trims hyphens, and caps length.
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
        "deliverable".to_string()
    } else {
        capped
    }
}

/// Reject any path component that could escape the deliverables tree.
fn has_traversal(p: &Path) -> bool {
    p.components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
}

#[async_trait]
impl Tool for DeliverableWriteTool {
    fn name(&self) -> &str {
        "deliverable_write"
    }

    fn description(&self) -> &str {
        "Persist a strategic deliverable as a markdown / json / yaml / svg file. \
         CALL PROACTIVELY: when the operator asks for a roadmap, plan, memo, brief, \
         analysis, or any structured output, persist it WITHOUT asking permission. \
         The operator wants the file landed; asking 'do you want this saved?' is a \
         smell. Just save it and report the path. \
         MARKDOWN DELIVERABLES MUST include: (a) YAML frontmatter between `---` delimiters \
         with title/status/owner_agent/contributing_agents/tags/related_decisions/created_at/\
         next_review/kill_criteria, (b) `# Title` H1, (c) `## H2` sections for every numbered \
         block, (d) tables for any comparison, (e) `- [ ]` checklists for actions, \
         (f) `[[other-slug]]` cross-refs, (g) ```mermaid blocks for flows/architecture, \
         (h) a `## Falsification` section with 2-5 kill criteria. The response will \
         include a `warnings` array if any of these are missing — rewrite and re-save \
         on the next turn instead of leaving structure-incomplete artefacts. \
         REQUIRED params: `slug` (kebab-case title, e.g. 'unicorn-roadmap'), \
         `filename` (with extension, e.g. 'roadmap.md'), `content` (the full file body). \
         OPTIONAL: `agent` (defaults to 'orchestrator' when called from top-level). \
         Files land at workspace/deliverables/<agent>/<YYYY-MM-DD>-<slug>/<filename>. \
         Sandboxed; never overwrites (auto-appends -N on collision). \
         EXAMPLE call: {\"slug\":\"q3-launch-plan\", \"filename\":\"plan.md\", \"content\":\"---\\ntitle: Q3 Launch Plan\\nstatus: proposed\\n...\\n---\\n\\n# Q3 Launch Plan\\n\\n## 1) Goals\\n...\"}. \
         Use this for memos, logframes, plans, governance docs, funder maps."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Logical agent name (snake_case). Optional inside a sub-agent — the runtime auto-fills it from the active delegation scope. Required only for top-level callers.",
                },
                "slug": {
                    "type": "string",
                    "description": "Short human-readable title for this work package — sanitised to a slug.",
                },
                "filename": {
                    "type": "string",
                    "description": "Filename including extension. Allowed: md, markdown, txt, json, yaml, yml, csv, svg, html.",
                },
                "content": {
                    "type": "string",
                    "description": "File contents. UTF-8 text only. 1 MB cap.",
                }
            },
            "required": ["slug", "filename", "content"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        // `agent` is auto-filled from the ACTIVE_AGENT task-local that
        // delegate.execute_agentic scopes around every sub-agent's
        // tool loop. Sub-agents almost never need to pass it.
        //
        // The top-level orchestrator (and the CLI) run OUTSIDE that
        // scope. When they call `deliverable_write` without an explicit
        // `agent`, default to "orchestrator" instead of erroring — the
        // alternative is a hard error the LLM retries identically until
        // the loop-detector circuit-breaks (`runtime-trace.jsonl` turn
        // `87fefcff` saw exactly this). Erroring once is fine; erroring
        // five times in a row aborts a useful run.
        let agent_owned: String;
        let agent: &str = match args.get("agent").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s,
            _ => match ACTIVE_AGENT.try_with(|a| a.clone()).ok().flatten() {
                Some(name) if !name.trim().is_empty() => {
                    agent_owned = name;
                    agent_owned.as_str()
                }
                _ => "orchestrator",
            },
        };
        let slug_raw = args
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing 'slug' parameter (a kebab-case title for the work package, \
                     e.g. 'unicorn-roadmap-v1'). Required fields: slug, filename, content. \
                     Example: {{\"slug\":\"q3-launch-plan\",\"filename\":\"plan.md\",\
                     \"content\":\"# ...\"}}"
                )
            })?;
        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing 'filename' parameter (must include extension, e.g. 'plan.md'). \
                     Allowed extensions: md, markdown, txt, json, yaml, yml, csv, svg, html."
                )
            })?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing 'content' parameter (the full file body as a UTF-8 string, \
                     up to 1 MB). Pass the entire deliverable in one call — chunking is \
                     unnecessary."
                )
            })?;

        if content.len() > MAX_DELIVERABLE_BYTES {
            return Ok(err(format!(
                "Content exceeds {} byte cap (got {})",
                MAX_DELIVERABLE_BYTES,
                content.len()
            )));
        }

        if !self.security.can_act() {
            return Ok(err("Action blocked: autonomy is read-only".into()));
        }
        if self.security.is_rate_limited() {
            return Ok(err(
                "Rate limit exceeded: too many actions in the last hour".into(),
            ));
        }

        let agent_slug = slugify(agent);
        let work_slug = slugify(slug_raw);
        if agent_slug.is_empty() || work_slug.is_empty() {
            return Ok(err("'agent' and 'slug' must contain alphanumeric characters".into()));
        }

        let fname_path = Path::new(filename);
        if has_traversal(fname_path) || fname_path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
            return Ok(err(format!(
                "filename must be a bare name, no path components (got: {filename})"
            )));
        }
        let ext = fname_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !ALLOWED_EXTENSIONS.iter().any(|e| *e == ext) {
            return Ok(err(format!(
                "extension '{ext}' not allowed. Allowed: {}",
                ALLOWED_EXTENSIONS.join(", ")
            )));
        }
        let stem = fname_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(slugify)
            .unwrap_or_default();
        if stem.is_empty() {
            return Ok(err("filename stem is empty after sanitisation".into()));
        }
        let safe_filename = format!("{stem}.{ext}");

        let date = Utc::now().format("%Y-%m-%d").to_string();
        // Multi-tenant isolation: when a tenant is active (scope set
        // by the gateway from the X-Octopus-Tenant header), file
        // lives under companies/<tenant>/deliverables/<agent>/<date>-<slug>/
        // so each company has its own folder. Falls back to the
        // legacy single-tenant root when no tenant scope is set.
        let tenant_slug = zeroclaw_memory::qdrant::ACTIVE_TENANT
            .try_with(|t| t.clone())
            .ok()
            .flatten()
            .map(|s| slugify(&s))
            .filter(|s| !s.is_empty());
        let dated_slug = format!("{date}-{work_slug}");
        let relative: PathBuf = match tenant_slug.as_deref() {
            Some(tenant) => ["companies", tenant, "deliverables", &agent_slug, &dated_slug]
                .iter()
                .collect(),
            None => ["deliverables", &agent_slug, &dated_slug].iter().collect(),
        };

        let full_dir = self
            .security
            .resolve_tool_path(relative.to_string_lossy().as_ref());
        tokio::fs::create_dir_all(&full_dir).await?;
        let resolved_dir = match tokio::fs::canonicalize(&full_dir).await {
            Ok(p) => p,
            Err(e) => return Ok(err(format!("Failed to resolve deliverable dir: {e}"))),
        };
        if !self.security.is_resolved_path_allowed(&resolved_dir) {
            return Ok(err(self.security.resolved_path_violation_message(&resolved_dir)));
        }

        // Collision-safe filename: append -2, -3, ... when the basename exists.
        let mut target = resolved_dir.join(&safe_filename);
        let mut n: u32 = 2;
        while tokio::fs::try_exists(&target).await.unwrap_or(false) {
            target = resolved_dir.join(format!("{stem}-{n}.{ext}"));
            n += 1;
            if n > 100 {
                return Ok(err(
                    "too many name collisions in this slug folder (>100); pick a new slug".into(),
                ));
            }
        }

        if !self.security.record_action() {
            return Ok(err(
                "Rate limit exceeded: action budget exhausted".into(),
            ));
        }

        tokio::fs::write(&target, content).await?;

        // Append a one-line entry to the per-agent INDEX.md so the
        // orchestrator (and operator) can browse what each agent has
        // shipped without crawling the tree. Under multi-tenant the
        // index lives next to the company's deliverables so deleting
        // a company purges its index too.
        let index_rel = match tenant_slug.as_deref() {
            Some(tenant) => format!("companies/{tenant}/deliverables/{agent_slug}/INDEX.md"),
            None => format!("deliverables/{agent_slug}/INDEX.md"),
        };
        let index_path = self.security.resolve_tool_path(&index_rel);
        if let Some(parent) = index_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let relative_target = target
            .strip_prefix(&self.security.workspace_dir)
            .unwrap_or(&target);
        let line = format!(
            "- {} · `{}` · {}\n",
            Utc::now().to_rfc3339(),
            relative_target.display(),
            work_slug,
        );
        let _ = append(&index_path, &line).await;

        let warnings = structure_warnings(&ext, content);
        let mut payload = json!({
            "path": target.display().to_string(),
            "relative": relative_target.display().to_string(),
            "bytes": content.len(),
            "agent": agent_slug,
            "slug": work_slug,
        });
        if !warnings.is_empty() {
            payload["warnings"] = json!(warnings);
            payload["fix_hint"] = json!(
                "Re-save with the missing structure on your next turn. The file is \
                 already on disk; calling deliverable_write again with the corrected \
                 content overwrites by adding -2/-3 etc., so include the version in \
                 the slug (e.g. 'q3-launch-plan-v2') for a clean iteration trail."
            );
        }

        Ok(ToolResult {
            success: true,
            output: payload.to_string(),
            error: None,
        })
    }
}

/// Soft structure-quality checks for markdown deliverables. Returns
/// the list of issues; never blocks the write. The agent sees the
/// warnings in the tool result and learns to fix them next turn.
fn structure_warnings(ext: &str, content: &str) -> Vec<String> {
    if ext != "md" && ext != "markdown" {
        return Vec::new();
    }
    let mut warns: Vec<String> = Vec::new();
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---\n") && !trimmed.starts_with("---\r\n") {
        warns.push(
            "Missing YAML frontmatter. Start the file with `---` ... `---` containing \
             title, status, owner_agent, contributing_agents, tags, related_decisions, \
             created_at, next_review, kill_criteria."
                .to_string(),
        );
    }

    let h1_count = content.lines().filter(|l| l.starts_with("# ")).count();
    let h2_count = content.lines().filter(|l| l.starts_with("## ")).count();
    let line_count = content.lines().count();

    if h1_count == 0 {
        warns.push("Missing `# Title` H1 heading.".to_string());
    } else if h1_count > 1 {
        warns.push(format!(
            "Found {h1_count} H1 headings — only one `# Title` per document; promote \
             the rest to `## H2`."
        ));
    }

    if line_count > 30 && h2_count == 0 {
        warns.push(
            "Long document with no `## H2` sections. Break the body into numbered \
             `## 1) Section` blocks; flat numbered paragraphs under H1 are unreadable."
                .to_string(),
        );
    }

    let lower = content.to_lowercase();
    if line_count > 40
        && !lower.contains("## falsification")
        && !lower.contains("## kill criteria")
    {
        warns.push(
            "Missing `## Falsification` section. Add 2-5 specific signals that, if \
             observed, end the plan or trigger replanning. Without it, the deliverable \
             reads as opinion, not as a testable plan."
                .to_string(),
        );
    }

    warns
}

fn err(msg: String) -> ToolResult {
    ToolResult {
        success: false,
        output: String::new(),
        error: Some(msg),
    }
}

async fn append(path: &Path, line: &str) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    f.write_all(line.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_collapses_separators() {
        assert_eq!(slugify("NGO  Architect — Plan #1"), "ngo-architect-plan-1");
        assert_eq!(slugify("   "), "deliverable");
        assert_eq!(slugify("__"), "deliverable");
    }

    #[test]
    fn slugify_strips_trailing_separators() {
        assert_eq!(slugify("hello---"), "hello");
        assert_eq!(slugify("---hello---"), "hello");
    }

    #[test]
    fn slugify_caps_length() {
        let long = "a".repeat(200);
        assert_eq!(slugify(&long).len(), 60);
    }

    #[test]
    fn has_traversal_catches_parent_and_root() {
        assert!(has_traversal(Path::new("../escape.md")));
        assert!(has_traversal(Path::new("/etc/passwd")));
        assert!(!has_traversal(Path::new("safe.md")));
        assert!(!has_traversal(Path::new("subdir/safe.md")));
    }
}
