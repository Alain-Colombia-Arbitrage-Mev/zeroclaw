use super::bm25::Bm25Scorer;
use super::embeddings::EmbeddingProvider;
use super::traits::{Memory, MemoryCategory, MemoryEntry};
use super::vector;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;
use uuid::Uuid;
use zeroclaw_config::schema::SearchMode;

/// Qdrant vector database memory backend.
///
/// Uses Qdrant's REST API for vector storage and semantic search.
/// Requires an embedding provider for converting text to vectors.
///
/// When `search_mode = "hybrid"` (default), the recall path over-fetches
/// dense candidates from Qdrant, scores them with an in-memory BM25
/// pass, and fuses both with `keyword_weight` / `vector_weight`.
pub struct QdrantMemory {
    client: reqwest::Client,
    base_url: String,
    collection: String,
    api_key: Option<String>,
    embedder: Arc<dyn EmbeddingProvider>,
    /// Tracks whether collection has been initialized (lazy init for sync factory).
    initialized: OnceCell<()>,
    /// Search strategy: dense-only / bm25-only / hybrid blend.
    search_mode: SearchMode,
    /// Weight applied to the normalized cosine-similarity score.
    vector_weight: f32,
    /// Weight applied to the normalized BM25 keyword score.
    keyword_weight: f32,
    /// How many dense candidates to over-fetch before BM25 re-ranking.
    /// Defaults to 3× the recall limit, capped at 50.
    over_fetch_factor: usize,
}

impl QdrantMemory {
    /// Create a new Qdrant memory backend.
    ///
    /// # Arguments
    /// * `url` - Qdrant server URL (e.g., `"http://localhost:6333"`)
    /// * `collection` - Collection name for storing memories
    /// * `api_key` - Optional API key for Qdrant Cloud
    /// * `embedder` - Embedding provider for vector conversion
    pub async fn new(
        url: &str,
        collection: &str,
        api_key: Option<String>,
        embedder: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self> {
        let mem = Self::new_lazy(url, collection, api_key, embedder);

        // Ensure collection exists with correct schema
        mem.ensure_collection().await?;
        mem.initialized.set(()).ok();

        Ok(mem)
    }

    /// Create a Qdrant memory backend with lazy initialization.
    ///
    /// Collection will be created on first operation. Use this when calling
    /// from a synchronous context (e.g., the memory factory).
    pub fn new_lazy(
        url: &str,
        collection: &str,
        api_key: Option<String>,
        embedder: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self::new_lazy_with_search(
            url,
            collection,
            api_key,
            embedder,
            SearchMode::default(),
            0.7,
            0.3,
        )
    }

    /// Construct with explicit hybrid-search settings. Used by the
    /// memory factory when `[memory] search_mode` is configured.
    pub fn new_lazy_with_search(
        url: &str,
        collection: &str,
        api_key: Option<String>,
        embedder: Arc<dyn EmbeddingProvider>,
        search_mode: SearchMode,
        vector_weight: f32,
        keyword_weight: f32,
    ) -> Self {
        let base_url = url.trim_end_matches('/').to_string();
        let client = zeroclaw_config::schema::build_runtime_proxy_client("memory.qdrant");

        Self {
            client,
            base_url,
            collection: collection.to_string(),
            api_key,
            embedder,
            initialized: OnceCell::new(),
            search_mode,
            vector_weight,
            keyword_weight,
            over_fetch_factor: 3,
        }
    }

    /// Ensure the collection is initialized (called lazily on first operation).
    async fn ensure_initialized(&self) -> Result<()> {
        self.initialized
            .get_or_try_init(|| async {
                self.ensure_collection().await?;
                Ok::<(), anyhow::Error>(())
            })
            .await?;
        Ok(())
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);

        if let Some(ref key) = self.api_key {
            req = req.header("api-key", key);
        }

        req.header("Content-Type", "application/json")
    }

    async fn ensure_collection(&self) -> Result<()> {
        let dims = self.embedder.dimensions();
        if dims == 0 {
            // Noop embedder — skip vector collection setup
            tracing::warn!(
                "Qdrant memory using noop embedder (0 dimensions); vector search disabled"
            );
            return Ok(());
        }

        // Check if collection exists
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/collections/{}", self.collection),
            )
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                // Collection exists
                return Ok(());
            }
            Ok(r) if r.status().as_u16() == 404 => {
                // Collection doesn't exist, create it
            }
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                anyhow::bail!("Qdrant collection check failed ({status}): {text}");
            }
            Err(e) => {
                anyhow::bail!("Qdrant connection failed: {e}");
            }
        }

        // Create collection with vector config
        let create_body = serde_json::json!({
            "vectors": {
                "size": dims,
                "distance": "Cosine"
            }
        });

        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{}", self.collection),
            )
            .json(&create_body)
            .send()
            .await
            .context("failed to create Qdrant collection")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant collection creation failed ({status}): {text}");
        }

        tracing::info!(
            "Created Qdrant collection '{}' with {} dimensions",
            self.collection,
            dims
        );

        Ok(())
    }

    fn category_to_str(category: &MemoryCategory) -> String {
        match category {
            MemoryCategory::Core => "core".to_string(),
            MemoryCategory::Daily => "daily".to_string(),
            MemoryCategory::Conversation => "conversation".to_string(),
            MemoryCategory::Custom(name) => name.clone(),
        }
    }

    fn parse_category(value: &str) -> MemoryCategory {
        match value {
            "core" => MemoryCategory::Core,
            "daily" => MemoryCategory::Daily,
            "conversation" => MemoryCategory::Conversation,
            other => MemoryCategory::Custom(other.to_string()),
        }
    }
}

/// Qdrant point payload structure.
///
/// `tenant_id` lets a single Qdrant collection host many isolated
/// businesses without duplicating the universal corpus. New writes
/// are stamped with the active tenant (read from
/// [`active_tenant`]); recall filters by `tenant_id == active OR
/// tenant_id IS NULL` so the global corpus stays visible to every
/// tenant. Existing chunks (pre-multi-tenant) have no `tenant_id`
/// and are therefore treated as global — no migration required.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryPayload {
    key: String,
    content: String,
    category: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant_id: Option<String>,
}

tokio::task_local! {
    /// Active tenant for the current async task.
    ///
    /// Set at the gateway WS connect (from `X-Octopus-Tenant`); the
    /// memory backend reads this when stamping new memories and
    /// filtering recall. `None` (or task-local unset) means the
    /// caller is in the global / single-tenant scope.
    pub static ACTIVE_TENANT: Option<String>;
}

/// Read the active tenant from task-local storage. Returns `None`
/// when called outside an `ACTIVE_TENANT.scope(..)` block.
fn current_tenant() -> Option<String> {
    ACTIVE_TENANT
        .try_with(|t| t.clone())
        .ok()
        .flatten()
}

/// Qdrant search result
#[derive(Debug, Deserialize)]
struct QdrantSearchResult {
    result: Vec<QdrantScoredPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantScoredPoint {
    id: serde_json::Value,
    score: f64,
    payload: Option<MemoryPayload>,
}

/// Qdrant scroll result
#[derive(Debug, Deserialize)]
struct QdrantScrollResult {
    result: QdrantScrollPoints,
}

#[derive(Debug, Deserialize)]
struct QdrantScrollPoints {
    points: Vec<QdrantPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantPoint {
    id: serde_json::Value,
    payload: Option<MemoryPayload>,
}

#[async_trait]
impl Memory for QdrantMemory {
    fn name(&self) -> &str {
        "qdrant"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> Result<()> {
        self.ensure_initialized().await?;

        // Generate embedding for the content
        let combined_text = format!("{}\n{}", key, content);
        let embedding = self.embedder.embed_one(&combined_text).await?;

        if embedding.is_empty() {
            anyhow::bail!("Qdrant requires non-zero dimensional embeddings");
        }

        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().to_rfc3339();

        let payload = MemoryPayload {
            key: key.to_string(),
            content: content.to_string(),
            category: Self::category_to_str(&category),
            timestamp,
            session_id: session_id.map(str::to_string),
            tenant_id: current_tenant(),
        };

        // Delete any existing point with the same key first
        let _ = self.forget(key).await;

        // Upsert point
        let upsert_body = serde_json::json!({
            "points": [{
                "id": id,
                "vector": embedding,
                "payload": payload
            }]
        });

        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{}/points", self.collection),
            )
            .query(&[("wait", "true")])
            .json(&upsert_body)
            .send()
            .await
            .context("failed to upsert point to Qdrant")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant upsert failed ({status}): {text}");
        }

        Ok(())
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<Vec<MemoryEntry>> {
        if query.trim().is_empty() {
            let mut entries = self.list(None, session_id).await?;
            if let Some(s) = since {
                entries.retain(|e| e.timestamp.as_str() >= s);
            }
            if let Some(u) = until {
                entries.retain(|e| e.timestamp.as_str() <= u);
            }
            entries.truncate(limit);
            return Ok(entries);
        }

        self.ensure_initialized().await?;

        // Generate embedding for the query
        let embedding = self.embedder.embed_one(query).await?;

        if embedding.is_empty() {
            // Fallback to listing if embeddings aren't available
            return self.list(None, session_id).await;
        }

        // Build filter combining session_id (when provided) and the
        // tenant scope. Tenant logic:
        //   - active tenant set: match `tenant_id == active` OR
        //     payload missing `tenant_id` (global corpus is visible
        //     to every tenant).
        //   - no active tenant: no tenant filter — caller sees
        //     everything (single-tenant / dev mode).
        let mut must_clauses: Vec<serde_json::Value> = Vec::new();
        if let Some(sid) = session_id {
            must_clauses.push(serde_json::json!({
                "key": "session_id",
                "match": { "value": sid }
            }));
        }
        let tenant = current_tenant();
        let filter: Option<serde_json::Value> = match (must_clauses.is_empty(), tenant.as_deref()) {
            (true, None) => None,
            (false, None) => Some(serde_json::json!({"must": must_clauses})),
            (true, Some(t)) => Some(serde_json::json!({
                "should": [
                    {"key": "tenant_id", "match": {"value": t}},
                    {"is_empty": {"key": "tenant_id"}}
                ],
                "minimum_should_match": 1
            })),
            (false, Some(t)) => Some(serde_json::json!({
                "must": must_clauses,
                "should": [
                    {"key": "tenant_id", "match": {"value": t}},
                    {"is_empty": {"key": "tenant_id"}}
                ],
                "minimum_should_match": 1
            })),
        };

        // Over-fetch dense candidates so BM25 has a richer batch to
        // re-rank when hybrid search is enabled. Pure-embedding mode
        // skips the over-fetch and returns the top `limit` directly.
        let dense_limit = match self.search_mode {
            SearchMode::Embedding => limit,
            SearchMode::Hybrid | SearchMode::Bm25 => {
                (limit * self.over_fetch_factor).min(50)
            }
        };

        let mut search_body = serde_json::json!({
            "vector": embedding,
            "limit": dense_limit,
            "with_payload": true
        });

        if let Some(f) = filter {
            search_body["filter"] = f;
        }

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/search", self.collection),
            )
            .json(&search_body)
            .send()
            .await
            .context("failed to search Qdrant")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant search failed ({status}): {text}");
        }

        let result: QdrantSearchResult = resp.json().await?;

        let mut entries: Vec<MemoryEntry> = result
            .result
            .into_iter()
            .filter_map(|point| {
                let payload = point.payload?;
                let id = match &point.id {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => return None,
                };

                Some(MemoryEntry {
                    id,
                    key: payload.key,
                    content: payload.content,
                    category: Self::parse_category(&payload.category),
                    timestamp: payload.timestamp,
                    session_id: payload.session_id,
                    score: Some(point.score),
                    namespace: "default".into(),
                    importance: None,
                    superseded_by: None,
                })
            })
            .collect();

        // ── Hybrid re-ranking with BM25 ─────────────────────────────
        // Run BM25 over the over-fetched batch in-memory and merge
        // with the dense scores using `vector::hybrid_merge`. For
        // pure-embedding mode the dense ranking is already final.
        if !matches!(self.search_mode, SearchMode::Embedding) && entries.len() > 1 {
            // Build (id, content) pairs for BM25.
            let docs: Vec<(&str, &str)> = entries
                .iter()
                .map(|e| (e.id.as_str(), e.content.as_str()))
                .collect();
            let scorer = Bm25Scorer::default();
            let bm25_scores: Vec<(String, f32)> = scorer.rank(query, &docs);

            // Dense scores for the SAME entries, in id order.
            let dense_scores: Vec<(String, f32)> = entries
                .iter()
                .map(|e| (e.id.clone(), e.score.unwrap_or(0.0) as f32))
                .collect();

            let merged = match self.search_mode {
                SearchMode::Bm25 => vector::hybrid_merge(
                    &[],
                    &bm25_scores,
                    self.vector_weight,
                    self.keyword_weight,
                    limit,
                ),
                _ => vector::hybrid_merge(
                    &dense_scores,
                    &bm25_scores,
                    self.vector_weight,
                    self.keyword_weight,
                    limit,
                ),
            };

            // Reorder + truncate `entries` to match merged ranking.
            let order: std::collections::HashMap<String, usize> = merged
                .iter()
                .enumerate()
                .map(|(i, s)| (s.id.clone(), i))
                .collect();
            let merged_score: std::collections::HashMap<String, f32> =
                merged.iter().map(|s| (s.id.clone(), s.final_score)).collect();
            entries.retain(|e| order.contains_key(&e.id));
            entries.sort_by_key(|e| order.get(&e.id).copied().unwrap_or(usize::MAX));
            for e in entries.iter_mut() {
                if let Some(s) = merged_score.get(&e.id) {
                    e.score = Some(*s as f64);
                }
            }
        }

        // Filter by time range if specified
        if let Some(s) = since {
            entries.retain(|e| e.timestamp.as_str() >= s);
        }
        if let Some(u) = until {
            entries.retain(|e| e.timestamp.as_str() <= u);
        }

        // Final cap at `limit` (over-fetch may still leave us with extras
        // when zero-overlap BM25 drops some candidates entirely).
        entries.truncate(limit);

        Ok(entries)
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        self.ensure_initialized().await?;

        // Scroll with filter for exact key match
        let scroll_body = serde_json::json!({
            "filter": {
                "must": [{
                    "key": "key",
                    "match": { "value": key }
                }]
            },
            "limit": 1,
            "with_payload": true
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/scroll", self.collection),
            )
            .json(&scroll_body)
            .send()
            .await
            .context("failed to scroll Qdrant")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant scroll failed ({status}): {text}");
        }

        let result: QdrantScrollResult = resp.json().await?;

        let entry = result.result.points.into_iter().next().and_then(|point| {
            let payload = point.payload?;
            let id = match &point.id {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => return None,
            };

            Some(MemoryEntry {
                id,
                key: payload.key,
                content: payload.content,
                category: Self::parse_category(&payload.category),
                timestamp: payload.timestamp,
                session_id: payload.session_id,
                score: None,
                namespace: "default".into(),
                importance: None,
                superseded_by: None,
            })
        });

        Ok(entry)
    }

    async fn list(
        &self,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>,
    ) -> Result<Vec<MemoryEntry>> {
        self.ensure_initialized().await?;

        // Build filter conditions
        let mut must_conditions = Vec::new();

        if let Some(cat) = category {
            must_conditions.push(serde_json::json!({
                "key": "category",
                "match": { "value": Self::category_to_str(cat) }
            }));
        }

        if let Some(sid) = session_id {
            must_conditions.push(serde_json::json!({
                "key": "session_id",
                "match": { "value": sid }
            }));
        }

        let mut scroll_body = serde_json::json!({
            "limit": 1000,
            "with_payload": true
        });

        if !must_conditions.is_empty() {
            scroll_body["filter"] = serde_json::json!({ "must": must_conditions });
        }

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/scroll", self.collection),
            )
            .json(&scroll_body)
            .send()
            .await
            .context("failed to scroll Qdrant")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant scroll failed ({status}): {text}");
        }

        let result: QdrantScrollResult = resp.json().await?;

        let entries = result
            .result
            .points
            .into_iter()
            .filter_map(|point| {
                let payload = point.payload?;
                let id = match &point.id {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => return None,
                };

                Some(MemoryEntry {
                    id,
                    key: payload.key,
                    content: payload.content,
                    category: Self::parse_category(&payload.category),
                    timestamp: payload.timestamp,
                    session_id: payload.session_id,
                    score: None,
                    namespace: "default".into(),
                    importance: None,
                    superseded_by: None,
                })
            })
            .collect();

        Ok(entries)
    }

    async fn forget(&self, key: &str) -> Result<bool> {
        self.ensure_initialized().await?;

        // Delete points matching the key
        let delete_body = serde_json::json!({
            "filter": {
                "must": [{
                    "key": "key",
                    "match": { "value": key }
                }]
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/delete", self.collection),
            )
            .query(&[("wait", "true")])
            .json(&delete_body)
            .send()
            .await
            .context("failed to delete from Qdrant")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant delete failed ({status}): {text}");
        }

        // Qdrant doesn't return deleted count easily, assume success
        Ok(true)
    }

    async fn count(&self) -> Result<usize> {
        self.ensure_initialized().await?;

        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/collections/{}", self.collection),
            )
            .send()
            .await
            .context("failed to get Qdrant collection info")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Qdrant collection info failed ({status}): {text}");
        }

        let json: serde_json::Value = resp.json().await?;

        let count = json
            .get("result")
            .and_then(|r| r.get("points_count"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);

        let count =
            usize::try_from(count).context("Qdrant returned a points count that exceeds usize")?;
        Ok(count)
    }

    async fn health_check(&self) -> bool {
        let resp = self.request(reqwest::Method::GET, "/").send().await;

        matches!(resp, Ok(r) if r.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_to_str_maps_known_categories() {
        assert_eq!(QdrantMemory::category_to_str(&MemoryCategory::Core), "core");
        assert_eq!(
            QdrantMemory::category_to_str(&MemoryCategory::Daily),
            "daily"
        );
        assert_eq!(
            QdrantMemory::category_to_str(&MemoryCategory::Conversation),
            "conversation"
        );
        assert_eq!(
            QdrantMemory::category_to_str(&MemoryCategory::Custom("notes".into())),
            "notes"
        );
    }

    #[test]
    fn parse_category_maps_known_and_custom_values() {
        assert_eq!(QdrantMemory::parse_category("core"), MemoryCategory::Core);
        assert_eq!(QdrantMemory::parse_category("daily"), MemoryCategory::Daily);
        assert_eq!(
            QdrantMemory::parse_category("conversation"),
            MemoryCategory::Conversation
        );
        assert_eq!(
            QdrantMemory::parse_category("custom_notes"),
            MemoryCategory::Custom("custom_notes".into())
        );
    }

    #[test]
    fn memory_payload_serializes_correctly() {
        let payload = MemoryPayload {
            key: "test_key".into(),
            content: "test content".into(),
            category: "core".into(),
            timestamp: "2026-02-20T00:00:00Z".into(),
            session_id: Some("session-1".into()),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("test_key"));
        assert!(json.contains("test content"));
        assert!(json.contains("session-1"));
    }

    #[test]
    fn memory_payload_skips_none_session_id() {
        let payload = MemoryPayload {
            key: "test_key".into(),
            content: "test content".into(),
            category: "core".into(),
            timestamp: "2026-02-20T00:00:00Z".into(),
            session_id: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("session_id"));
    }
}
