//! REST API — query events, clients, and hub status.
//!
//! Endpoints:
//! - `GET /api/status` — hub status (uptime, connected clients, stored events)
//! - `GET /api/clients` — list connected clients
//! - `GET /api/events?limit=N&after_seq=N` — query stored events
//! - `GET /api/events/latest?n=N` — get latest N events
//! - `POST /api/emit` — receive an event via HTTP (fallback for WS)

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::server::AppState;
use crate::store::StoredEvent;

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct StatusResponse {
    pub running: bool,
    pub connected_clients: usize,
    pub stored_events: usize,
    pub current_seq: u64,
}

#[derive(Serialize)]
pub struct ClientListResponse {
    pub clients: Vec<crate::session::ClientInfo>,
}

#[derive(Serialize)]
pub struct EventListResponse {
    pub events: Vec<StoredEvent>,
}

// ─── Query Params ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EventsQuery {
    pub limit: Option<usize>,
    pub after_seq: Option<u64>,
}

#[derive(Deserialize)]
pub struct LatestQuery {
    pub n: Option<usize>,
}

// ─── Emit Request ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EmitRequest {
    pub event: String,
    pub framework: Option<String>,
    pub timestamp: Option<String>,
    pub data: Option<serde_json::Value>,
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// `GET /api/status`
pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        running: true,
        connected_clients: state.sessions.client_count().await,
        stored_events: state.store.len().await,
        current_seq: state.store.current_seq(),
    })
}

/// `GET /api/clients`
pub async fn clients(State(state): State<AppState>) -> Json<ClientListResponse> {
    Json(ClientListResponse {
        clients: state.sessions.list_clients().await,
    })
}

/// `GET /api/events?limit=N&after_seq=N`
pub async fn events(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> Json<EventListResponse> {
    let events = if let Some(after_seq) = query.after_seq {
        let mut ev = state.store.since(after_seq).await;
        if let Some(limit) = query.limit {
            ev.truncate(limit);
        }
        ev
    } else {
        let limit = query.limit.unwrap_or(100);
        state.store.latest(limit).await
    };

    Json(EventListResponse { events })
}

/// `GET /api/events/latest?n=N`
pub async fn events_latest(
    State(state): State<AppState>,
    Query(query): Query<LatestQuery>,
) -> Json<EventListResponse> {
    let n = query.n.unwrap_or(50);
    Json(EventListResponse {
        events: state.store.latest(n).await,
    })
}

/// `POST /api/emit` — receive an event via HTTP.
///
/// This is the fallback endpoint for frameworks that can't maintain
/// a persistent WebSocket connection (e.g., Python hooks).
pub async fn emit(
    State(state): State<AppState>,
    Json(payload): Json<EmitRequest>,
) -> StatusCode {
    let framework = payload.framework.unwrap_or_else(|| "unknown".into());
    let normalized = normalize_event_name(&payload.event, &framework);
    let data = normalize_event_data(payload.data.unwrap_or_default(), &framework, &normalized);

    let event_json = serde_json::json!({
        "event": normalized,
        "framework": framework,
        "timestamp": payload.timestamp.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        "data": data,
    });

    let json_str = event_json.to_string();

    let seq = state.store.append(json_str.clone()).await;
    state.sessions.broadcast(&json_str);

    tracing::debug!(seq, event = %normalized, "Event received via HTTP");
    StatusCode::OK
}

// ─── Event Name Normalization ────────────────────────────────────────────────

/// Canonical unified event names. Events already in this set pass through.
const UNIFIED_EVENTS: &[&str] = &[
    "gateway:start", "gateway:shutdown",
    "session:start", "session:end", "session:reset",
    "agent:start", "agent:step", "agent:end", "agent:error",
    "message:user", "message:start", "message:delta", "message:complete",
    "message:interim", "message:stream_end",
    "thinking:delta", "reasoning:available",
    "tool:start", "tool:progress", "tool:complete", "tool:error",
    "tool:generating", "tool:guardrail_halt",
    "approval:request", "approval:result",
    "system:status", "system:warning", "system:error", "system:compression",
    "subagent:start", "subagent:progress", "subagent:complete",
    "subagent:tool", "subagent:thinking",
    "chain:start", "chain:end",
    "memory:saved", "memory:loaded",
    "skill:loaded", "skill:saved",
    "cron:job_start", "cron:job_end",
    "background:started", "background:finished",
];

/// Normalize framework-specific event names to the unified wire format.
///
/// Checks in order:
/// 1. Framework-specific mappings (handles non-standard names like `assistant:response`)
/// 2. Already-canonical unified names (pass through)
/// 3. Fallback to `custom:<lowercased>`
pub fn normalize_event_name(event: &str, framework: &str) -> String {
    // 1. Framework-specific mappings
    let mapped = match framework {
        "claude-code" => match event {
            "SessionStart" => Some("session:start"),
            "SessionEnd" => Some("session:end"),
            "PreToolUse" => Some("tool:start"),
            "PostToolUse" => Some("tool:complete"),
            "Stop" => Some("agent:end"),
            "StopFailure" => Some("agent:error"),
            "PreCompact" => Some("system:compression"),
            "SubagentStop" => Some("subagent:complete"),
            "UserPromptSubmit" => Some("message:user"),
            "PermissionRequest" => Some("approval:request"),
            _ => None,
        },
        "codex" => match event {
            "SessionStart" => Some("session:start"),
            "SessionEnd" => Some("session:end"),
            "PreToolUse" => Some("tool:start"),
            "PostToolUse" => Some("tool:complete"),
            "Stop" => Some("agent:end"),
            _ => None,
        },
        "hermes" => match event {
            "assistant:response" => Some("message:complete"),
            "assistant:interim" => Some("message:interim"),
            "thinking:output" => Some("thinking:delta"),
            "reasoning:output" => Some("reasoning:available"),
            _ => None,
        },
        _ => None,
    };

    if let Some(m) = mapped {
        return m.to_string();
    }

    // 2. Already in unified format
    if UNIFIED_EVENTS.contains(&event) {
        return event.to_string();
    }

    // 3. Generic adapter prefixes: `hook:tool_start` → `tool:start`, etc.
    //    Strips `hook:`, `callback:`, or `handle:` prefix, converts underscores
    //    to colons, then checks if the result is a unified event.
    if let Some(suffix) = event.strip_prefix("hook:")
        .or_else(|| event.strip_prefix("callback:"))
        .or_else(|| event.strip_prefix("handle:"))
    {
        let candidate = suffix.replace('_', ":");
        if UNIFIED_EVENTS.contains(&candidate.as_str()) {
            return candidate;
        }
    }

    // 4. Fallback
    format!("custom:{}", event.to_lowercase())
}

// ─── Event Data Normalization ────────────────────────────────────────────────

/// Unified field names for tool events.
const TOOL_NAME_FIELDS: &[&str] = &["tool_name", "name"];
const TOOL_INPUT_FIELDS: &[&str] = &["tool_input", "args", "input", "parameters", "arguments"];
const TOOL_RESULT_FIELDS: &[&str] = &["tool_response", "tool_result", "result", "output"];
const TOOL_ID_FIELDS: &[&str] = &["tool_use_id", "tool_call_id", "call_id", "id", "run_id"];

/// Normalize event data fields to a unified schema.
///
/// Ensures consistent field names across frameworks:
/// - `tool_name` — name of the tool (unified)
/// - `tool_input` — input/arguments to the tool (unified)
/// - `tool_response` — output/result from the tool (unified)
/// - `tool_call_id` — unique identifier for the tool call (unified)
/// - `duration_ms` — execution duration (unified)
/// - `session_id` — session identifier (unified)
pub fn normalize_event_data(
    data: serde_json::Value,
    _framework: &str,
    event: &str,
) -> serde_json::Value {
    // Only normalize tool events
    if !event.starts_with("tool:") {
        return data;
    }

    let mut obj = match data {
        serde_json::Value::Object(map) => map,
        _ => return data,
    };

    // Normalize tool_name: find first existing field and rename to tool_name
    normalize_field_group(&mut obj, TOOL_NAME_FIELDS, "tool_name");

    // Normalize tool_input: find first existing field and rename to tool_input
    normalize_field_group(&mut obj, TOOL_INPUT_FIELDS, "tool_input");

    // Normalize tool_response: find first existing field and rename to tool_response
    normalize_field_group(&mut obj, TOOL_RESULT_FIELDS, "tool_response");

    // Normalize tool_call_id: find first existing field and rename to tool_call_id
    normalize_field_group(&mut obj, TOOL_ID_FIELDS, "tool_call_id");

    // Remove framework-specific fields that are not part of the unified schema
    let framework_specific = [
        "hook_event_name", "cwd", "effort", "permission_mode",
        "transcript_path", "callback",
    ];
    for key in framework_specific {
        obj.remove(key);
    }

    serde_json::Value::Object(obj)
}

/// Find the first existing field in `candidates` and rename it to `target`.
/// If `target` already exists, just remove the candidates.
fn normalize_field_group(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    candidates: &[&str],
    target: &str,
) {
    // Find the first candidate that exists
    let source = candidates.iter().find(|&&f| obj.contains_key(f)).copied();

    if let Some(source) = source {
        if source != target {
            // Move value from source to target
            if let Some(value) = obj.remove(source) {
                obj.insert(target.to_string(), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_normalization() {
        assert_eq!(normalize_event_name("SessionStart", "claude-code"), "session:start");
        assert_eq!(normalize_event_name("PreToolUse", "claude-code"), "tool:start");
        assert_eq!(normalize_event_name("PostToolUse", "claude-code"), "tool:complete");
        assert_eq!(normalize_event_name("Stop", "claude-code"), "agent:end");
        assert_eq!(normalize_event_name("StopFailure", "claude-code"), "agent:error");
    }

    #[test]
    fn test_hermes_normalization() {
        assert_eq!(normalize_event_name("assistant:response", "hermes"), "message:complete");
        assert_eq!(normalize_event_name("assistant:interim", "hermes"), "message:interim");
        assert_eq!(normalize_event_name("thinking:output", "hermes"), "thinking:delta");
    }

    #[test]
    fn test_generic_adapter_hook_prefix() {
        // Generic adapter: `on_tool_start` → `hook:tool_start` → `tool:start`
        assert_eq!(normalize_event_name("hook:tool_start", "generic"), "tool:start");
        assert_eq!(normalize_event_name("hook:tool_complete", "generic"), "tool:complete");
        assert_eq!(normalize_event_name("hook:tool_error", "generic"), "tool:error");
        assert_eq!(normalize_event_name("hook:agent_step", "generic"), "agent:step");
        assert_eq!(normalize_event_name("hook:agent_start", "generic"), "agent:start");
        assert_eq!(normalize_event_name("hook:agent_end", "generic"), "agent:end");
        assert_eq!(normalize_event_name("hook:session_start", "generic"), "session:start");
        assert_eq!(normalize_event_name("hook:session_end", "generic"), "session:end");
        assert_eq!(normalize_event_name("hook:message_delta", "generic"), "message:delta");
        assert_eq!(normalize_event_name("hook:message_complete", "generic"), "message:complete");
    }

    #[test]
    fn test_generic_adapter_callback_prefix() {
        // Generic adapter: `tool_start_callback` → `callback:tool_start` → `tool:start`
        assert_eq!(normalize_event_name("callback:tool_start", "generic"), "tool:start");
        assert_eq!(normalize_event_name("callback:tool_complete", "generic"), "tool:complete");
    }

    #[test]
    fn test_generic_adapter_handle_prefix() {
        // Generic adapter: `handle_tool_start` → `handle:tool_start` → `tool:start`
        assert_eq!(normalize_event_name("handle:tool_start", "generic"), "tool:start");
        assert_eq!(normalize_event_name("handle:tool_complete", "generic"), "tool:complete");
    }

    #[test]
    fn test_unified_events_pass_through() {
        assert_eq!(normalize_event_name("tool:start", "any"), "tool:start");
        assert_eq!(normalize_event_name("tool:complete", "any"), "tool:complete");
        assert_eq!(normalize_event_name("session:start", "any"), "session:start");
        assert_eq!(normalize_event_name("agent:end", "any"), "agent:end");
    }

    #[test]
    fn test_unknown_events_fallback() {
        assert_eq!(normalize_event_name("SomeUnknownEvent", "unknown"), "custom:someunknownevent");
        assert_eq!(normalize_event_name("hook:unknown_event", "generic"), "custom:hook:unknown_event");
    }

    #[test]
    fn test_normalize_claude_code_tool_data() {
        let data = serde_json::json!({
            "tool_name": "Grep",
            "tool_input": {"pattern": "test"},
            "tool_response": {"content": "result"},
            "tool_use_id": "call_123",
            "duration_ms": 100,
            "hook_event_name": "PostToolUse",
            "cwd": "/some/path"
        });
        let result = normalize_event_data(data, "claude-code", "tool:complete");
        assert_eq!(result["tool_name"], "Grep");
        assert_eq!(result["tool_input"]["pattern"], "test");
        // tool_response stays as tool_response (already unified)
        assert_eq!(result["tool_response"]["content"], "result");
        // tool_use_id should be normalized to tool_call_id
        assert_eq!(result["tool_call_id"], "call_123");
        assert!(result.get("tool_use_id").is_none());
        assert_eq!(result["duration_ms"], 100);
        // Framework-specific fields should be removed
        assert!(result.get("hook_event_name").is_none());
        assert!(result.get("cwd").is_none());
    }

    #[test]
    fn test_normalize_hermes_tool_data() {
        let data = serde_json::json!({
            "tool_name": "session_search",
            "args": {"limit": "5", "sort": "newest"},
            "result": {"count": 5, "success": true},
            "tool_call_id": "call_456",
            "duration_ms": 3,
            "session_id": "test-session",
            "task_id": "test-task"
        });
        let result = normalize_event_data(data, "hermes", "tool:complete");
        assert_eq!(result["tool_name"], "session_search");
        // args should be renamed to tool_input
        assert_eq!(result["tool_input"]["limit"], "5");
        assert_eq!(result["tool_input"]["sort"], "newest");
        assert!(result.get("args").is_none());
        // result should be renamed to tool_response
        assert_eq!(result["tool_response"]["count"], 5);
        assert_eq!(result["tool_response"]["success"], true);
        assert!(result.get("result").is_none());
        // tool_call_id stays as tool_call_id
        assert_eq!(result["tool_call_id"], "call_456");
        // session_id stays
        assert_eq!(result["session_id"], "test-session");
        // task_id is kept as-is (framework-specific but not removed)
        assert_eq!(result["task_id"], "test-task");
    }

    #[test]
    fn test_normalize_non_tool_event_passthrough() {
        let data = serde_json::json!({
            "text": "hello",
            "custom_field": "value"
        });
        let result = normalize_event_data(data.clone(), "hermes", "message:delta");
        // Non-tool events should pass through unchanged
        assert_eq!(result, data);
    }
}
