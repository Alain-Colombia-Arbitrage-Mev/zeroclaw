//! Server-Sent Events (SSE) stream for real-time event delivery.
//!
//! Wraps the broadcast channel in AppState to deliver events to web dashboard clients.

use super::AppState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::{Arc, Mutex, OnceLock};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

/// LIFO of delegate sub-agent names that are currently in flight.
///
/// We push on `ToolCallStart { tool: "delegate", … }` (after parsing the
/// `agent` arg) and pop on the matching `ToolCall { tool: "delegate", … }`
/// completion event. The result is a best-effort attribution of the
/// completion event to the right sub-agent — best-effort because nested
/// delegations can interleave, but the agent loop is currently single-
/// threaded per session so the LIFO is correct in the common case.
fn inflight_targets() -> &'static Mutex<VecDeque<String>> {
    static INFLIGHT: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(VecDeque::with_capacity(8)))
}

fn push_inflight_delegate_target(name: String) {
    if let Ok(mut q) = inflight_targets().lock() {
        // Cap to avoid pathological growth from a leaked target.
        if q.len() >= 64 {
            q.pop_front();
        }
        q.push_back(name);
    }
}

fn consume_inflight_delegate_target() -> Option<String> {
    inflight_targets().lock().ok().and_then(|mut q| q.pop_back())
}

/// Thread-safe ring buffer that retains recent events for history replay.
pub struct EventBuffer {
    inner: Mutex<VecDeque<serde_json::Value>>,
    capacity: usize,
}

impl EventBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push an event into the buffer, evicting the oldest if at capacity.
    pub fn push(&self, event: serde_json::Value) {
        let mut buf = self.inner.lock().unwrap();
        if buf.len() == self.capacity {
            buf.pop_front();
        }
        buf.push_back(event);
    }

    /// Return a snapshot of all buffered events (oldest first).
    pub fn snapshot(&self) -> Vec<serde_json::Value> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }
}

/// GET /api/events — SSE event stream
pub async fn handle_sse_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Auth check
    if state.pairing.require_pairing() {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|auth| auth.strip_prefix("Bearer "))
            .unwrap_or("");

        if !state.pairing.is_authenticated(token) {
            return (
                StatusCode::UNAUTHORIZED,
                "Unauthorized — provide Authorization: Bearer <token>",
            )
                .into_response();
        }
    }

    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(
        |result: Result<
            serde_json::Value,
            tokio_stream::wrappers::errors::BroadcastStreamRecvError,
        >| {
            match result {
                Ok(value) => Some(Ok::<_, Infallible>(
                    Event::default().data(value.to_string()),
                )),
                Err(_) => None, // Skip lagged messages
            }
        },
    );

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// GET /api/events/history — return buffered recent events as JSON.
pub async fn handle_events_history(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = super::api::require_auth(&state, &headers) {
        return e.into_response();
    }
    let events = state.event_buffer.snapshot();
    Json(serde_json::json!({ "events": events })).into_response()
}

/// Broadcast observer that forwards events to the SSE broadcast channel.
pub struct BroadcastObserver {
    inner: Box<dyn zeroclaw_runtime::observability::Observer>,
    tx: tokio::sync::broadcast::Sender<serde_json::Value>,
    buffer: Arc<EventBuffer>,
}

impl BroadcastObserver {
    pub fn new(
        inner: Box<dyn zeroclaw_runtime::observability::Observer>,
        tx: tokio::sync::broadcast::Sender<serde_json::Value>,
        buffer: Arc<EventBuffer>,
    ) -> Self {
        Self { inner, tx, buffer }
    }

    pub fn inner(&self) -> &dyn zeroclaw_runtime::observability::Observer {
        self.inner.as_ref()
    }
}

impl zeroclaw_runtime::observability::Observer for BroadcastObserver {
    fn record_event(&self, event: &zeroclaw_runtime::observability::ObserverEvent) {
        // Forward to inner observer
        self.inner.record_event(event);

        // Broadcast to SSE subscribers
        let json = match event {
            zeroclaw_runtime::observability::ObserverEvent::LlmRequest {
                provider, model, ..
            } => serde_json::json!({
                "type": "llm_request",
                "provider": provider,
                "model": model,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
            zeroclaw_runtime::observability::ObserverEvent::ToolCall {
                tool,
                duration,
                success,
            } => {
                let mut obj = serde_json::json!({
                    "type": "tool_call",
                    "tool": tool,
                    "duration_ms": duration.as_millis(),
                    "success": success,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                // Surface the most recently dispatched delegate target so the
                // dashboard can attribute completion events to the right
                // sub-agent. The end event itself doesn't carry args; we rely
                // on the LIFO of in-flight starts maintained alongside.
                if tool == "delegate"
                    && let Some(target) = consume_inflight_delegate_target()
                {
                    obj["target_agent"] = serde_json::Value::String(target);
                }
                obj
            }
            zeroclaw_runtime::observability::ObserverEvent::ToolCallStart { tool, arguments } => {
                let mut obj = serde_json::json!({
                    "type": "tool_call_start",
                    "tool": tool,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                if tool == "delegate"
                    && let Some(args) = arguments.as_ref()
                    && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(args)
                {
                    if let Some(agent) = parsed.get("agent").and_then(|v| v.as_str()) {
                        obj["target_agent"] = serde_json::Value::String(agent.to_string());
                        push_inflight_delegate_target(agent.to_string());
                    } else if let Some(parallel) =
                        parsed.get("parallel").and_then(|v| v.as_array())
                    {
                        let names: Vec<String> = parallel
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if !names.is_empty() {
                            // For parallel dispatch, expose all targets and
                            // queue them in order so the matching ToolCall
                            // ends consume in LIFO.
                            obj["target_agents"] = serde_json::Value::Array(
                                names
                                    .iter()
                                    .map(|s| serde_json::Value::String(s.clone()))
                                    .collect(),
                            );
                            for n in names {
                                push_inflight_delegate_target(n);
                            }
                        }
                    }
                }
                obj
            }
            zeroclaw_runtime::observability::ObserverEvent::Error { component, message } => {
                serde_json::json!({
                    "type": "error",
                    "component": component,
                    "message": message,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
            }
            zeroclaw_runtime::observability::ObserverEvent::AgentStart { provider, model } => {
                serde_json::json!({
                    "type": "agent_start",
                    "provider": provider,
                    "model": model,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
            }
            zeroclaw_runtime::observability::ObserverEvent::AgentEnd {
                provider,
                model,
                duration,
                tokens_used,
                cost_usd,
            } => serde_json::json!({
                "type": "agent_end",
                "provider": provider,
                "model": model,
                "duration_ms": duration.as_millis(),
                "tokens_used": tokens_used,
                "cost_usd": cost_usd,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
            _ => return, // Skip events we don't broadcast
        };

        self.buffer.push(json.clone());
        let _ = self.tx.send(json);
    }

    fn record_metric(&self, metric: &zeroclaw_runtime::observability::traits::ObserverMetric) {
        self.inner.record_metric(metric);
    }

    fn flush(&self) {
        self.inner.flush();
    }

    fn name(&self) -> &str {
        "broadcast"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
