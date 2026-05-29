//! Public access-request flow — lets unauthenticated visitors submit a
//! request for dashboard access without needing a pair code out-of-band,
//! and lets already-paired operators approve or deny those requests from
//! the `/pairing` admin console.
//!
//! Approval surfaces a one-time pair code that the operator forwards to
//! the requester (manual delivery in this iteration — email integration
//! is deliberately out of scope).
//!
//! Storage is a sibling SQLite database (`<workspace>/access_requests.db`)
//! mirroring the `DeviceRegistry` pattern. The unauth POST endpoint reuses
//! the existing `AuthRateLimiter` to throttle abuse by source IP.
//!
//! Routes (wired in `lib.rs`):
//! - `POST   /api/access-request`            unauth, rate-limited per IP
//! - `GET    /api/access-requests`           auth-required (bearer)
//! - `POST   /api/access-requests/{id}/approve`  auth-required
//! - `POST   /api/access-requests/{id}/deny`     auth-required

use super::AppState;
use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Json},
};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Caps on user-supplied fields. Generous enough for legitimate use, tight
/// enough to keep the DB row size predictable and prevent obvious abuse.
const MAX_FIELD_LEN: usize = 500;

/// One queued access request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRequest {
    pub id: String,
    pub name: String,
    pub email: String,
    pub use_case: String,
    pub requested_at: DateTime<Utc>,
    /// `"pending"` | `"approved"` | `"denied"`.
    pub status: String,
    pub ip_address: Option<String>,
}

/// SQLite-backed registry of access requests. Mirrors `DeviceRegistry` shape.
#[derive(Debug)]
pub struct AccessRequestRegistry {
    cache: Mutex<HashMap<String, AccessRequest>>,
    db_path: PathBuf,
}

impl AccessRequestRegistry {
    pub fn new(workspace_dir: &Path) -> Self {
        let db_path = workspace_dir.join("access_requests.db");
        let conn = Connection::open(&db_path).expect("Failed to open access-request database");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS access_requests (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL,
                use_case TEXT NOT NULL,
                requested_at TEXT NOT NULL,
                status TEXT NOT NULL,
                ip_address TEXT
            )",
        )
        .expect("Failed to create access_requests table");

        let mut cache = HashMap::new();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, email, use_case, requested_at, status, ip_address \
                 FROM access_requests",
            )
            .expect("Failed to prepare access_requests select");
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let email: String = row.get(2)?;
                let use_case: String = row.get(3)?;
                let requested_at_str: String = row.get(4)?;
                let status: String = row.get(5)?;
                let ip_address: Option<String> = row.get(6)?;
                let requested_at = DateTime::parse_from_rfc3339(&requested_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                Ok((
                    id.clone(),
                    AccessRequest {
                        id,
                        name,
                        email,
                        use_case,
                        requested_at,
                        status,
                        ip_address,
                    },
                ))
            })
            .expect("Failed to query access_requests");
        for (id, req) in rows.flatten() {
            cache.insert(id, req);
        }

        Self {
            cache: Mutex::new(cache),
            db_path,
        }
    }

    fn open_db(&self) -> Connection {
        Connection::open(&self.db_path).expect("Failed to open access-request database")
    }

    pub fn create(&self, req: AccessRequest) {
        let conn = self.open_db();
        conn.execute(
            "INSERT OR REPLACE INTO access_requests \
             (id, name, email, use_case, requested_at, status, ip_address) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                req.id,
                req.name,
                req.email,
                req.use_case,
                req.requested_at.to_rfc3339(),
                req.status,
                req.ip_address,
            ],
        )
        .expect("Failed to insert access_request");
        self.cache.lock().insert(req.id.clone(), req);
    }

    pub fn list_pending(&self) -> Vec<AccessRequest> {
        let cache = self.cache.lock();
        let mut out: Vec<AccessRequest> = cache
            .values()
            .filter(|r| r.status == "pending")
            .cloned()
            .collect();
        // Newest first — admin sees fresh requests at the top.
        out.sort_by(|a, b| b.requested_at.cmp(&a.requested_at));
        out
    }

    pub fn remove(&self, id: &str) -> bool {
        let conn = self.open_db();
        let deleted = conn
            .execute(
                "DELETE FROM access_requests WHERE id = ?1",
                rusqlite::params![id],
            )
            .unwrap_or(0);
        if deleted > 0 {
            self.cache.lock().remove(id);
            true
        } else {
            false
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn extract_bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
}

fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), (StatusCode, &'static str)> {
    if state.pairing.require_pairing() {
        let token = extract_bearer(headers).unwrap_or("");
        if !state.pairing.is_authenticated(token) {
            return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
        }
    }
    Ok(())
}

/// Best-effort source-IP extraction for rate-limit keying. Respects
/// `trust_forwarded_headers` so a misconfigured deployment can't be
/// spoofed via a forged `X-Forwarded-For`.
fn client_rate_key(state: &AppState, headers: &HeaderMap) -> String {
    if state.trust_forwarded_headers
        && let Some(fwd) = headers
            .get("X-Forwarded-For")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next().map(str::trim))
        && !fwd.is_empty()
    {
        return format!("access-request:{fwd}");
    }
    "access-request:default".to_string()
}

/// Permissive but non-empty email shape check. We are not validating
/// deliverability — that's a separate concern (and not in scope for the
/// MVP). We only reject obvious garbage so the admin list stays useful.
fn looks_like_email(s: &str) -> bool {
    let at_count = s.chars().filter(|c| *c == '@').count();
    if at_count != 1 {
        return false;
    }
    let (local, domain) = s.split_once('@').unwrap_or(("", ""));
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn clamp(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() > MAX_FIELD_LEN {
        trimmed.chars().take(MAX_FIELD_LEN).collect()
    } else {
        trimmed.to_string()
    }
}

// ── Handlers ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RequestAccessBody {
    pub name: Option<String>,
    pub email: Option<String>,
    pub use_case: Option<String>,
}

/// POST /api/access-request — unauth. Accept a public access request,
/// store it for the operator to review. Rate-limited per source IP.
pub async fn handle_request_access(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RequestAccessBody>,
) -> impl IntoResponse {
    // Rate limit first — never let an unauth POST hammer the SQLite write.
    let rate_key = client_rate_key(&state, &headers);
    if let Err(err) = state.auth_limiter.check_rate_limit(&rate_key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            format!("Rate limited. Retry after {}s.", err.retry_after_secs),
        )
            .into_response();
    }

    let name = clamp(&body.name.unwrap_or_default());
    let email = clamp(&body.email.unwrap_or_default());
    let use_case = clamp(&body.use_case.unwrap_or_default());

    if name.is_empty() || email.is_empty() || use_case.is_empty() {
        // Always record the attempt so rate limit kicks in for empty-spam too.
        state.auth_limiter.record_attempt(&rate_key);
        return (
            StatusCode::BAD_REQUEST,
            "name, email, and use_case are required",
        )
            .into_response();
    }

    if !looks_like_email(&email) {
        state.auth_limiter.record_attempt(&rate_key);
        return (StatusCode::BAD_REQUEST, "email is not a valid address").into_response();
    }

    let Some(ref registry) = state.access_request_registry else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Access-request registry not available",
        )
            .into_response();
    };

    let ip_address = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty());

    let req = AccessRequest {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        email,
        use_case,
        requested_at: Utc::now(),
        status: "pending".to_string(),
        ip_address,
    };
    let req_id = req.id.clone();
    let created_at = req.requested_at;
    registry.create(req);
    state.auth_limiter.record_attempt(&rate_key);

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "request_id": req_id,
            "created_at": created_at,
            "message": "Request received. The operator will contact you when approved."
        })),
    )
        .into_response()
}

/// GET /api/access-requests — auth-required. List pending requests.
pub async fn handle_list_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }

    let requests = state
        .access_request_registry
        .as_ref()
        .map(|r| r.list_pending())
        .unwrap_or_default();

    let count = requests.len();
    Json(serde_json::json!({
        "requests": requests,
        "count": count,
    }))
    .into_response()
}

/// POST /api/access-requests/{id}/approve — auth-required. Generate a
/// fresh pair code, mark the request approved (then remove it — there
/// is no audit table by design for MVP), and return the code so the
/// admin can forward it.
pub async fn handle_approve_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }

    let Some(ref registry) = state.access_request_registry else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Access-request registry not available",
        )
            .into_response();
    };

    // Snapshot the request so we can include its email in the response,
    // useful for the admin UI's "PAIR CODE FOR <email>" surface.
    let snapshot = {
        let cache = registry.cache.lock();
        cache.get(&id).cloned()
    };
    let Some(req) = snapshot else {
        return (StatusCode::NOT_FOUND, "Access request not found").into_response();
    };
    if req.status != "pending" {
        return (StatusCode::CONFLICT, "Request is not pending").into_response();
    }

    // Generate a fresh pair code via the existing pairing infrastructure.
    let Some(code) = state.pairing.generate_new_pairing_code() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Pairing is disabled or not available",
        )
            .into_response();
    };

    // Hard-delete the request on approval — for MVP we don't keep an
    // approval audit log. The operator now owns the trust transfer.
    registry.remove(&id);

    Json(serde_json::json!({
        "request_id": id,
        "email": req.email,
        "name": req.name,
        "pair_code": code,
    }))
    .into_response()
}

/// POST /api/access-requests/{id}/deny — auth-required. Hard-delete.
pub async fn handle_deny_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&state, &headers) {
        return e.into_response();
    }

    let Some(ref registry) = state.access_request_registry else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Access-request registry not available",
        )
            .into_response();
    };

    if registry.remove(&id) {
        Json(serde_json::json!({
            "request_id": id,
            "message": "Request denied",
        }))
        .into_response()
    } else {
        (StatusCode::NOT_FOUND, "Access request not found").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_shape_rejects_obvious_garbage() {
        assert!(!looks_like_email(""));
        assert!(!looks_like_email("no-at-sign"));
        assert!(!looks_like_email("two@@signs.com"));
        assert!(!looks_like_email("@no-local.com"));
        assert!(!looks_like_email("no-domain@"));
        assert!(!looks_like_email("no-dot@nodomain"));
        assert!(!looks_like_email(".leading@dot.com"));
    }

    #[test]
    fn email_shape_accepts_normal_addresses() {
        assert!(looks_like_email("a@b.co"));
        assert!(looks_like_email("user.name+tag@subdomain.example.com"));
    }

    #[test]
    fn clamp_trims_and_caps() {
        assert_eq!(clamp("  hi  "), "hi");
        let huge: String = "x".repeat(MAX_FIELD_LEN + 50);
        assert_eq!(clamp(&huge).chars().count(), MAX_FIELD_LEN);
    }
}
