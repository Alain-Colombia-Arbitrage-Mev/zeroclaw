//! `/api/files/*` — read-only browser for the workspace artifacts
//! that sub-agents produce. Specifically:
//!
//! - `GET /api/files/deliverables` — tree under
//!   `<workspace>/deliverables/<agent>/<date>-<slug>/<file>`, used by
//!   the dashboard's WORKBENCH tab to list documents per agent + run.
//! - `GET /api/files/deliverables/raw?path=...` — read one file's
//!   content (text). Path is validated against the deliverables root
//!   to block traversal.
//!
//! Authentication uses the same bearer-token guard as the rest of
//! the `/api/*` surface. Read-only by design — there is no PUT or
//! DELETE here. Cleanup happens by deleting the folder out-of-band.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::AppState;

/// Max bytes we'll return inline for a file read. Bigger files
/// surface a `truncated: true` flag.
const MAX_INLINE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct DeliverableFile {
    pub name: String,
    /// Relative path from the deliverables root, used as the stable
    /// id when reading the file back via `/raw?path=...`.
    pub rel_path: String,
    pub size_bytes: u64,
    pub mtime_iso: String,
    /// Best-guess MIME from extension. Browser uses this to pick the
    /// renderer (text / image / pdf).
    pub mime: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliverableRun {
    /// Folder name under the agent dir, e.g. "2026-05-11-ancestro-unit".
    pub slug: String,
    /// Parsed YYYY-MM-DD prefix when the folder follows convention,
    /// otherwise the slug verbatim.
    pub date: String,
    pub files: Vec<DeliverableFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliverableAgent {
    pub agent: String,
    pub run_count: usize,
    pub file_count: usize,
    pub runs: Vec<DeliverableRun>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliverableTreeResponse {
    pub root: String,
    pub agents: Vec<DeliverableAgent>,
    pub total_agents: usize,
    pub total_runs: usize,
    pub total_files: usize,
}

fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    if !state.pairing.require_pairing() {
        return Ok(());
    }
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if !state.pairing.is_authenticated(token) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }
    Ok(())
}

/// Resolve the deliverables root for the requested tenant.
/// When `tenant_id` is set we look under `companies/<tenant>/deliverables/`
/// (the multi-tenant layout written by `deliverable_write` whenever a
/// tenant scope is active). When unset we use the legacy single-tenant
/// `deliverables/` root so older artifacts remain reachable.
fn deliverables_root(state: &AppState, tenant_id: Option<&str>) -> PathBuf {
    let ws = state.config.lock().workspace_dir.clone();
    match tenant_id {
        Some(t) if !t.is_empty() => ws
            .join("companies")
            .join(t)
            .join("deliverables"),
        _ => ws.join("deliverables"),
    }
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown" | "txt") => "text/markdown",
        Some("json") => "application/json",
        Some("yaml" | "yml") => "text/yaml",
        Some("toml") => "text/toml",
        Some("csv") => "text/csv",
        Some("html" | "htm") => "text/html",
        Some("xml") => "text/xml",
        Some("rs" | "py" | "ts" | "js" | "tsx" | "jsx" | "go" | "rb" | "java" | "kt") => {
            "text/plain"
        }
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn parse_date_prefix(slug: &str) -> String {
    // Convention: <YYYY-MM-DD>-<rest>
    if slug.len() >= 10 && slug.as_bytes().get(4) == Some(&b'-')
        && slug.as_bytes().get(7) == Some(&b'-')
    {
        slug[..10].to_string()
    } else {
        slug.to_string()
    }
}

/// Walk a deliverables root, returning each agent's runs + files as
/// metadata. `agent_suffix` is appended to every agent name (used to
/// tag the legacy unscoped root with " · UNSCOPED" when listing
/// inside a tenant view). `rel_prefix` is prepended to every file's
/// rel_path so the read endpoint can route to the right root.
fn walk_root(root: &Path, agent_suffix: Option<&str>, rel_prefix: &str) -> Vec<DeliverableAgent> {
    let mut agents: Vec<DeliverableAgent> = Vec::new();
    if !root.exists() {
        return agents;
    }
    let agent_dirs = match std::fs::read_dir(root) {
        Ok(rd) => rd,
        Err(_) => return agents,
    };
    for entry in agent_dirs.flatten() {
        let agent_path = entry.path();
        if !agent_path.is_dir() {
            continue;
        }
        let raw_name = match agent_path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let display_name = match agent_suffix {
            Some(suffix) => format!("{raw_name}{suffix}"),
            None => raw_name.clone(),
        };
        let mut runs: Vec<DeliverableRun> = Vec::new();
        let Ok(run_dirs) = std::fs::read_dir(&agent_path) else {
            continue;
        };
        for run_entry in run_dirs.flatten() {
            let run_path = run_entry.path();
            let slug = match run_path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if run_path.is_dir() {
                let mut files = Vec::new();
                if let Ok(file_iter) = std::fs::read_dir(&run_path) {
                    for f in file_iter.flatten() {
                        let fp = f.path();
                        if !fp.is_file() {
                            continue;
                        }
                        let Ok(meta) = f.metadata() else {
                            continue;
                        };
                        let name = fp
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        let rel_path = format!("{rel_prefix}{raw_name}/{slug}/{name}");
                        let mtime_iso = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| {
                                chrono::DateTime::<chrono::Utc>::from_timestamp(
                                    d.as_secs() as i64,
                                    d.subsec_nanos(),
                                )
                                .map(|dt| dt.to_rfc3339())
                                .unwrap_or_default()
                            })
                            .unwrap_or_default();
                        files.push(DeliverableFile {
                            name,
                            rel_path,
                            size_bytes: meta.len(),
                            mtime_iso,
                            mime: mime_for(&fp).to_string(),
                        });
                    }
                }
                if !files.is_empty() {
                    files.sort_by(|a, b| b.mtime_iso.cmp(&a.mtime_iso));
                    runs.push(DeliverableRun {
                        slug: slug.clone(),
                        date: parse_date_prefix(&slug),
                        files,
                    });
                }
            } else if run_path.is_file() {
                // Stray loose file under <agent>/ (e.g. INDEX.md).
                let Ok(meta) = run_entry.metadata() else {
                    continue;
                };
                let mtime_iso = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| {
                        chrono::DateTime::<chrono::Utc>::from_timestamp(
                            d.as_secs() as i64,
                            d.subsec_nanos(),
                        )
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                    })
                    .unwrap_or_default();
                runs.push(DeliverableRun {
                    slug: format!("(loose) {slug}"),
                    date: "—".to_string(),
                    files: vec![DeliverableFile {
                        name: slug.clone(),
                        rel_path: format!("{rel_prefix}{raw_name}/{slug}"),
                        size_bytes: meta.len(),
                        mtime_iso,
                        mime: mime_for(&run_path).to_string(),
                    }],
                });
            }
        }
        if !runs.is_empty() {
            runs.sort_by(|a, b| b.date.cmp(&a.date));
            let file_count: usize = runs.iter().map(|r| r.files.len()).sum();
            agents.push(DeliverableAgent {
                agent: display_name,
                run_count: runs.len(),
                file_count,
                runs,
            });
        }
    }
    agents
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Optional tenant slug. When present, list only that company's
    /// artifacts under `companies/<tenant>/deliverables/`. When absent
    /// or empty, fall back to the legacy `deliverables/` root.
    #[serde(default)]
    pub tenant: Option<String>,
    /// When `true` on the wipe endpoint, also remove the legacy
    /// pre-multi-tenant `deliverables/` root. The dashboard's
    /// LAUNCH FROM ZERO opts in so "from zero" includes the stale
    /// UNSCOPED runs the operator sees in the workbench.
    #[serde(default)]
    pub include_legacy: Option<bool>,
}

/// DELETE /api/files/deliverables?tenant=<id>[&include_legacy=true] —
/// wipe every deliverable for one tenant. Used by the dashboard's
/// LAUNCH FROM ZERO flow so "from zero" actually means from zero: the
/// old runs are gone before new agents fire. The tenant-scoped folder
/// is always wiped (and tenant scope is required, no global wipes).
/// The legacy unscoped `deliverables/` root is only wiped when the
/// caller passes `include_legacy=true` — opt-in because that root is
/// shared across sessions and historic runs.
pub async fn handle_files_deliverables_wipe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    if let Err((code, msg)) = require_auth(&state, &headers) {
        return (code, Json(serde_json::json!({"error": msg}))).into_response();
    }
    let tenant = q.tenant.as_deref().filter(|s| !s.is_empty()).or_else(|| {
        headers
            .get("x-octopus-tenant")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    });
    // Hard rule: must scope to a tenant. Without it, this would wipe
    // the global unscoped deliverables folder which is shared across
    // sessions and historic runs — not what "LAUNCH FROM ZERO" means.
    let Some(tenant_id) = tenant else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "tenant scope required (set X-Octopus-Tenant header or ?tenant=...)",
            })),
        )
            .into_response();
    };

    // Slugify defensively — must contain only [a-z0-9_-] so we never
    // walk a path the operator didn't intend.
    if !tenant_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "tenant id contains illegal characters"})),
        )
            .into_response();
    }

    let root = state
        .config
        .lock()
        .workspace_dir
        .join("companies")
        .join(tenant_id)
        .join("deliverables");

    // Count what's there before we nuke it so the response is honest.
    let mut file_count: usize = 0;
    if root.exists() {
        // Quick recursive count without external dep
        let mut stack: Vec<PathBuf> = vec![root.clone()];
        while let Some(p) = stack.pop() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                for e in rd.flatten() {
                    let ep = e.path();
                    if ep.is_dir() {
                        stack.push(ep);
                    } else {
                        file_count += 1;
                    }
                }
            }
        }
    }

    let removed = if root.exists() {
        match std::fs::remove_dir_all(&root) {
            Ok(_) => true,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("failed to remove {}: {e}", root.display()),
                    })),
                )
                    .into_response();
            }
        }
    } else {
        false
    };

    // Optional legacy purge — only when the caller opts in. Counts
    // files there too, and recovers from a missing path silently
    // (very common: most tenants never had any legacy artifacts).
    let include_legacy = q.include_legacy.unwrap_or(false);
    let mut legacy_files: usize = 0;
    let mut legacy_removed = false;
    let mut legacy_root_display: Option<String> = None;
    if include_legacy {
        let legacy_root = state.config.lock().workspace_dir.join("deliverables");
        legacy_root_display = Some(legacy_root.display().to_string());
        if legacy_root.exists() {
            let mut stack: Vec<PathBuf> = vec![legacy_root.clone()];
            while let Some(p) = stack.pop() {
                if let Ok(rd) = std::fs::read_dir(&p) {
                    for e in rd.flatten() {
                        let ep = e.path();
                        if ep.is_dir() {
                            stack.push(ep);
                        } else {
                            legacy_files += 1;
                        }
                    }
                }
            }
            match std::fs::remove_dir_all(&legacy_root) {
                Ok(_) => legacy_removed = true,
                Err(e) => {
                    tracing::warn!(
                        "files.deliverables.wipe: legacy purge failed at {}: {e}",
                        legacy_root.display()
                    );
                }
            }
        }
    }

    tracing::info!(
        tenant = %tenant_id,
        removed = removed,
        files = file_count,
        legacy_removed = legacy_removed,
        legacy_files = legacy_files,
        "files.deliverables.wipe: tenant deliverables cleared"
    );

    Json(serde_json::json!({
        "tenant": tenant_id,
        "removed": removed,
        "files_deleted": file_count + legacy_files,
        "root": root.display().to_string(),
        "legacy_root": legacy_root_display,
        "legacy_removed": legacy_removed,
        "legacy_files_deleted": legacy_files,
    }))
    .into_response()
}

/// GET /api/files/deliverables?tenant=<id> — full tree for one tenant
/// (or legacy root if no tenant set). Lightweight metadata only.
pub async fn handle_files_deliverables_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    if let Err((code, msg)) = require_auth(&state, &headers) {
        return (code, Json(serde_json::json!({"error": msg}))).into_response();
    }
    // Prefer query param, fall back to X-Octopus-Tenant header so the
    // dashboard's fetch wrapper "just works" without extra plumbing.
    let tenant = q.tenant.as_deref().filter(|s| !s.is_empty()).or_else(|| {
        headers
            .get("x-octopus-tenant")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    });
    let root = deliverables_root(&state, tenant);
    // Also include the legacy single-tenant root so artifacts written
    // before multi-tenant isolation was wired up remain visible in the
    // workbench. Tagged with an `(unscoped)` agent suffix so the
    // operator can tell them apart. Only added when a tenant filter
    // is active (otherwise the unscoped root IS the primary root).
    let legacy_root = if tenant.is_some() {
        let legacy = state.config.lock().workspace_dir.join("deliverables");
        if legacy.exists() {
            Some(legacy)
        } else {
            None
        }
    } else {
        None
    };
    // Build the tree from the primary root. If the operator is tenant-
    // scoped AND a legacy unscoped root exists, append those agents
    // with an "(unscoped)" suffix so the operator can still see
    // pre-multi-tenant artifacts without losing tenant isolation.
    let mut agents: Vec<DeliverableAgent> = walk_root(&root, None, "");
    if let Some(ref legacy) = legacy_root {
        let mut legacy_agents = walk_root(legacy, Some(" · UNSCOPED"), "__legacy__/");
        agents.append(&mut legacy_agents);
    }

    agents.sort_by(|a, b| {
        // Most recent run first, then alpha
        let a_latest = a.runs.first().map(|r| r.date.as_str()).unwrap_or("");
        let b_latest = b.runs.first().map(|r| r.date.as_str()).unwrap_or("");
        b_latest.cmp(a_latest).then_with(|| a.agent.cmp(&b.agent))
    });

    let total_agents = agents.len();
    let total_runs: usize = agents.iter().map(|a| a.run_count).sum();
    let total_files: usize = agents.iter().map(|a| a.file_count).sum();

    Json(DeliverableTreeResponse {
        root: root.display().to_string(),
        agents,
        total_agents,
        total_runs,
        total_files,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct ReadQuery {
    pub path: String,
    /// Same as `tenant` on the list endpoint — selects which root to
    /// resolve `path` against.
    #[serde(default)]
    pub tenant: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadResponse {
    pub path: String,
    pub mime: String,
    pub size_bytes: u64,
    pub truncated: bool,
    /// UTF-8 content if the file is text-like; for binary it's a
    /// base64 string. The mime field tells the client which.
    pub content: String,
    pub encoding: &'static str,
}

/// GET /api/files/deliverables/raw?path=<rel> — read one file.
/// Path is canonicalized and verified to stay under deliverables/.
pub async fn handle_files_deliverables_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ReadQuery>,
) -> impl IntoResponse {
    if let Err((code, msg)) = require_auth(&state, &headers) {
        return (code, Json(serde_json::json!({"error": msg}))).into_response();
    }
    let tenant = q.tenant.as_deref().filter(|s| !s.is_empty()).or_else(|| {
        headers
            .get("x-octopus-tenant")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    });
    // Path prefix `__legacy__/` means the file lives under the
    // pre-multi-tenant `deliverables/` root rather than the
    // tenant-scoped one. The list endpoint emits this prefix for
    // legacy artifacts when tenant scope is active.
    let (root, path_in_root) = if let Some(stripped) = q.path.strip_prefix("__legacy__/") {
        (
            state.config.lock().workspace_dir.join("deliverables"),
            stripped.to_string(),
        )
    } else {
        (deliverables_root(&state, tenant), q.path.clone())
    };
    let q_path = path_in_root;
    let root_canon = match root.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "deliverables root missing"})),
            )
                .into_response();
        }
    };

    // Reject obvious traversal inputs before touching the FS.
    if q_path.contains("..")
        || q_path.starts_with('/')
        || q_path.starts_with('\\')
        || q_path.contains(':')
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid path"})),
        )
            .into_response();
    }

    let candidate = root_canon.join(&q_path);
    let canon = match candidate.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "file not found"})),
            )
                .into_response();
        }
    };
    if !canon.starts_with(&root_canon) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "path escapes deliverables root"})),
        )
            .into_response();
    }
    let meta = match std::fs::metadata(&canon) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };
    let mime = mime_for(&canon).to_string();
    let total = meta.len();
    let truncated = total > MAX_INLINE_BYTES;
    let to_read = if truncated { MAX_INLINE_BYTES } else { total } as usize;

    let bytes = match std::fs::read(&canon) {
        Ok(b) => b.into_iter().take(to_read).collect::<Vec<u8>>(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };
    let is_text = mime.starts_with("text/")
        || mime == "application/json"
        || mime == "image/svg+xml";
    let (content, encoding) = if is_text {
        match String::from_utf8(bytes.clone()) {
            Ok(s) => (s, "utf-8"),
            Err(_) => {
                use base64::Engine;
                (
                    base64::engine::general_purpose::STANDARD.encode(&bytes),
                    "base64",
                )
            }
        }
    } else {
        use base64::Engine;
        (
            base64::engine::general_purpose::STANDARD.encode(&bytes),
            "base64",
        )
    };

    Json(ReadResponse {
        path: q.path,
        mime,
        size_bytes: total,
        truncated,
        content,
        encoding,
    })
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search term. Treated as a literal string (regex special chars
    /// are escaped) unless `regex=true` is set.
    pub q: String,
    /// Optional tenant scope. Same fallback chain as the read+list
    /// endpoints (query > x-octopus-tenant header > none).
    pub tenant: Option<String>,
    /// Optional agent filter (matches the `<agent>` directory name).
    pub agent: Option<String>,
    /// Optional file extension filter ("md", "json", "yaml"). Without
    /// it, every text-like file under deliverables is searched.
    pub ext: Option<String>,
    /// Treat `q` as a regex (ripgrep flavour). Defaults to literal.
    #[serde(default)]
    pub regex: bool,
    /// Case-insensitive search. Defaults to true (most users expect it).
    #[serde(default = "default_true")]
    pub case_insensitive: bool,
    /// Cap on returned matches. Defaults to 200, clamped to 1000.
    pub limit: Option<usize>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    /// Relative path under the deliverables root (the same id format
    /// as the list endpoint emits, with the same `__legacy__/` prefix
    /// for cross-tenant pre-multitenant artefacts).
    pub path: String,
    pub agent: String,
    pub line_number: u64,
    /// The matching line (trimmed to ~400 chars to keep responses sane).
    pub line: String,
    /// Byte offset of the first match within the line — lets the
    /// dashboard highlight the precise term.
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub matched_files: usize,
    pub total_matches: usize,
    pub matches: Vec<SearchMatch>,
    /// True when ripgrep returned more than `limit` and we truncated.
    pub truncated: bool,
    /// True when ripgrep is missing on PATH; the response is empty in
    /// that case so the UI can render an actionable error.
    pub ripgrep_missing: bool,
}

pub async fn handle_files_deliverables_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    if let Err((code, msg)) = require_auth(&state, &headers) {
        return (code, Json(serde_json::json!({"error": msg}))).into_response();
    }

    let trimmed = q.q.trim();
    if trimmed.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "q parameter is required"})),
        )
            .into_response();
    }
    if trimmed.len() > 200 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "q too long (max 200 chars)"})),
        )
            .into_response();
    }

    let tenant = q.tenant.as_deref().filter(|s| !s.is_empty()).or_else(|| {
        headers
            .get("x-octopus-tenant")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    });
    let root = deliverables_root(&state, tenant);
    if !root.exists() {
        return Json(SearchResponse {
            query: trimmed.to_string(),
            matched_files: 0,
            total_matches: 0,
            matches: Vec::new(),
            truncated: false,
            ripgrep_missing: false,
        })
        .into_response();
    }

    let limit = q.limit.unwrap_or(200).min(1000);
    let pattern = if q.regex {
        trimmed.to_string()
    } else {
        regex::escape(trimmed)
    };

    // Build the matcher (ripgrep's regex engine as a library, identical
    // semantics — Unicode-aware, smart-case capable).
    let matcher = match grep_regex::RegexMatcherBuilder::new()
        .case_insensitive(q.case_insensitive)
        .build(&pattern)
    {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("invalid pattern: {e}")})),
            )
                .into_response();
        }
    };

    // Walker with: text-only files, gitignore-aware, max-filesize 2 MB.
    let root_for_walk = root.clone();
    let ext_filter = q.ext.clone();
    let agent_filter = q.agent.clone();
    let matcher_for_thread = matcher.clone();

    // Run the (sync) walk + match on a blocking task so we don't block
    // the tokio reactor.
    let result = tokio::task::spawn_blocking(move || -> Result<(Vec<SearchMatch>, bool), String> {
        let mut walker = ignore::WalkBuilder::new(&root_for_walk);
        walker.max_filesize(Some(2 * 1024 * 1024)).hidden(false);
        if let Some(ext) = ext_filter.as_deref().filter(|s| !s.is_empty()) {
            if ext.chars().all(|c| c.is_ascii_alphanumeric()) {
                let mut overrides = ignore::overrides::OverrideBuilder::new(&root_for_walk);
                overrides
                    .add(&format!("*.{ext}"))
                    .map_err(|e| e.to_string())?;
                walker.overrides(overrides.build().map_err(|e| e.to_string())?);
            }
        }

        // Note: do NOT canonicalize the prefix on Windows — canonicalize()
        // returns UNC paths (\\?\C:\...) while WalkBuilder emits plain
        // C:\... paths, and strip_prefix would fail silently.
        let root_prefix = root_for_walk.clone();
        let mut matches: Vec<SearchMatch> = Vec::new();

        for entry in walker.build() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.file_type().is_none_or(|ft| !ft.is_file()) {
                continue;
            }
            let abs = entry.path();
            let rel = match abs.strip_prefix(&root_prefix) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            let agent_name = rel.split('/').next().unwrap_or("").to_string();
            if let Some(a) = agent_filter.as_deref().filter(|s| !s.is_empty()) {
                if agent_name != *a {
                    continue;
                }
            }

            let mut searcher = grep_searcher::SearcherBuilder::new()
                .line_number(true)
                .build();
            let collected = matches_for_file(&matcher_for_thread, &mut searcher, abs, &rel, &agent_name);
            for m in collected {
                matches.push(m);
                if matches.len() >= limit {
                    return Ok((matches, true));
                }
            }
        }

        Ok((matches, false))
    })
    .await;

    let (matches, truncated) = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("search task panic: {e}")})),
            )
                .into_response();
        }
    };

    let files_seen: std::collections::HashSet<&str> =
        matches.iter().map(|m| m.path.as_str()).collect();
    let matched_files = files_seen.len();
    let total_matches = matches.len();

    Json(SearchResponse {
        query: trimmed.to_string(),
        matched_files,
        total_matches,
        matches,
        truncated,
        ripgrep_missing: false,
    })
    .into_response()
}

/// Search one file and collect its line matches.
fn matches_for_file(
    matcher: &grep_regex::RegexMatcher,
    searcher: &mut grep_searcher::Searcher,
    abs: &Path,
    rel: &str,
    agent: &str,
) -> Vec<SearchMatch> {
    use grep_searcher::sinks::UTF8;
    let mut out: Vec<SearchMatch> = Vec::new();
    let _ = searcher.search_path(
        matcher,
        abs,
        UTF8(|lnum, line| {
            // Find the first match column within the line for the UI
            // highlight. We use grep_matcher::Matcher::find but stay
            // within the existing matcher trait via the regex crate to
            // avoid pulling in another dep.
            let (start, end) = find_first_match(line, matcher);
            let truncated: String = line.lines().next().unwrap_or("").chars().take(400).collect();
            out.push(SearchMatch {
                path: rel.to_string(),
                agent: agent.to_string(),
                line_number: lnum,
                line: truncated,
                match_start: start,
                match_end: end,
            });
            Ok(true)
        }),
    );
    out
}

fn find_first_match(line: &str, matcher: &grep_regex::RegexMatcher) -> (usize, usize) {
    use grep_matcher::Matcher;
    match matcher.find(line.as_bytes()) {
        Ok(Some(m)) => (m.start(), m.end()),
        _ => (0, 0),
    }
}
