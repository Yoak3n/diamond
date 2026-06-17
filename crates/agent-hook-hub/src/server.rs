//! WS server — handles WebSocket connections from agents and viewers.
//!
//! Protocol:
//! - **Agent** connects to `/hook` → sends JSON events → Hub stores + broadcasts.
//! - **Viewer** connects to `/view` → receives all events in real-time.
//! - **Replay** viewer sends `{"cmd":"replay","after_seq":N}` to get missed events.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tracing::{debug, info, warn};

use crate::api::normalize_event_name;
use crate::logger::EventLogger;
use crate::session::{ClientRole, SessionManager};
use crate::store::EventStore;

// ─── App State ──────────────────────────────────────────────────────────────

/// Shared application state accessible from all handlers.
#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub store: Arc<EventStore>,
    pub event_logger: Option<Arc<EventLogger>>,
}

// ─── WebSocket Handlers ─────────────────────────────────────────────────────

/// Agent endpoint: `/hook`
///
/// Agents connect here to emit events. Events are stored and broadcast to viewers.
pub async fn hook_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_agent(socket, addr, state))
}

/// Viewer endpoint: `/view`
///
/// Viewers connect here to receive real-time events. They can also request replay.
pub async fn view_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_viewer(socket, addr, state))
}

// ─── Agent Handler ──────────────────────────────────────────────────────────

async fn handle_agent(mut socket: WebSocket, addr: std::net::SocketAddr, state: AppState) {
    let remote = addr.to_string();

    // First message should identify the agent: {"framework":"hermes","session_id":"xxx"}
    let (session_id, framework) = match socket.next().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<AgentHello>(&text) {
                Ok(hello) => {
                    let fw = hello.framework.clone();
                    let role = ClientRole::Agent {
                        framework: fw.clone(),
                        session_id: hello.session_id.clone(),
                    };
                    (state.sessions.register(role, remote.clone()).await, fw)
                }
                Err(_) => {
                    let role = ClientRole::Agent {
                        framework: "unknown".into(),
                        session_id: "unknown".into(),
                    };
                    (state.sessions.register(role, remote.clone()).await, "unknown".into())
                }
            }
        }
        _ => {
            let role = ClientRole::Agent {
                framework: "unknown".into(),
                session_id: "unknown".into(),
            };
            (state.sessions.register(role, remote.clone()).await, "unknown".into())
        }
    };

    // Send ack
    let ack = serde_json::json!({
        "type": "hub:connected",
        "session_id": session_id.0,
        "seq": state.store.current_seq(),
    });
    let _ = socket
        .send(Message::Text(ack.to_string().into()))
        .await;

    // Process incoming events
    let (mut _sink, mut source) = socket.split();

    while let Some(msg) = source.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let text_str = text.to_string();
                debug!(session = %session_id, len = text_str.len(), "Received event");

                // Normalize the event name for frameworks using non-standard names
                let normalized = normalize_ws_event(&text_str, &framework);

                // Log to file if enabled
                if let Some(ref logger) = state.event_logger {
                    logger.log(&normalized);
                }

                let seq = state.store.append(normalized.clone()).await;
                state.sessions.broadcast(&normalized);
                state.sessions.mark_event_sent(session_id).await;

                let ack = serde_json::json!({
                    "type": "hub:ack",
                    "seq": seq,
                });
                let _ = _sink.send(Message::Text(ack.to_string().into())).await;
            }
            Ok(Message::Close(_)) => {
                info!(session = %session_id, "Agent closed connection");
                break;
            }
            Ok(_) => {} // Binary, Ping, Pong, Frame
            Err(e) => {
                warn!(session = %session_id, error = %e, "Agent WS error");
                break;
            }
        }
    }

    state.sessions.unregister(session_id).await;
}

// ─── Viewer Handler ─────────────────────────────────────────────────────────

async fn handle_viewer(mut socket: WebSocket, addr: std::net::SocketAddr, state: AppState) {
    let remote = addr.to_string();
    let session_id = state
        .sessions
        .register(ClientRole::Viewer, remote)
        .await;

    // Subscribe to broadcast channel
    let mut rx = state.sessions.subscribe();

    // Send hello with current seq
    let hello = serde_json::json!({
        "type": "hub:hello",
        "session_id": session_id.0,
        "current_seq": state.store.current_seq(),
        "stored_events": state.store.len().await,
    });
    let _ = socket
        .send(Message::Text(hello.to_string().into()))
        .await;

    // Split for concurrent read/write
    let (mut sink, mut source) = socket.split();

    // Task: forward broadcast events to this viewer
    let send_session = session_id;
    let send_sessions = state.sessions.clone();
    let send_task = tokio::spawn(async move {
        while let Ok(event_json) = rx.recv().await {
            if sink
                .send(Message::Text(event_json.into()))
                .await
                .is_err()
            {
                break;
            }
            send_sessions.mark_event_received(send_session).await;
        }
    });

    // Task: handle incoming commands from viewer (e.g. replay request)
    let recv_store = state.store.clone();
    let recv_session = session_id;
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = source.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let text_str = text.to_string();
                    // Handle viewer commands
                    if let Ok(cmd) = serde_json::from_str::<ViewerCommand>(&text_str) {
                        match cmd {
                            ViewerCommand::Replay { after_seq } => {
                                let events = recv_store.since(after_seq).await;
                                info!(
                                    session = %recv_session,
                                    count = events.len(),
                                    after_seq,
                                    "Replaying events"
                                );
                                // Replay responses are sent through a separate mechanism
                                // since we don't have the sink here. We'll handle this
                                // by storing replay events as JSON.
                                // For now, the viewer can use the REST API for replay.
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!(session = %recv_session, "Viewer closed connection");
                    break;
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    state.sessions.unregister(session_id).await;
}

// ─── Protocol Messages ──────────────────────────────────────────────────────

/// First message from an agent identifying itself.
#[derive(Debug, serde::Deserialize)]
struct AgentHello {
    /// Framework name (e.g. "hermes", "langchain", "openclaw").
    framework: String,

    /// Session ID from the framework.
    session_id: String,
}

/// Command from a viewer.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "cmd")]
enum ViewerCommand {
    /// Request events after a given sequence number.
    #[serde(rename = "replay")]
    Replay { after_seq: u64 },
}

/// Normalize an incoming event JSON to the strict protocol format.
///
/// Protocol requires all fields flattened to the top level:
/// ```json
/// {
///   "event": "tool:start",
///   "framework": "claude-code",
///   "session_id": "xxx",
///   "turn_id": "yyy",       // optional
///   "timestamp": "2024-01-01T00:00:00Z",
///   "tool_name": "Bash",    // event-specific fields at top level
///   "tool_input": {...}
/// }
/// ```
fn normalize_ws_event(json_str: &str, framework: &str) -> String {
    let mut json: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return json_str.to_string(),
    };

    let Some(obj) = json.as_object_mut() else {
        return json_str.to_string();
    };

    // 1. Normalize event name
    if let Some(event) = obj.get("event").and_then(|v| v.as_str()) {
        let normalized = normalize_event_name(event, framework);
        obj.insert("event".into(), serde_json::Value::String(normalized));
    }

    // 2. Flatten `data` fields to top level
    //    Incoming may have: { "event": "...", "framework": "...", "data": { "session_id": "...", ... } }
    //    Or already flat:   { "event": "...", "framework": "...", "session_id": "...", ... }
    if let Some(data) = obj.remove("data") {
        if let Some(data_obj) = data.as_object() {
            for (key, value) in data_obj {
                // Don't overwrite existing top-level keys (except session_id/timestamp which we prefer top-level)
                if !obj.contains_key(key) {
                    obj.insert(key.clone(), value.clone());
                }
            }
        }
    }

    // 3. Extract session_id to top level if buried elsewhere (e.g. tool_input, tool_response)
    if !obj.contains_key("session_id") {
        // Try to find session_id in nested objects
        for key in &["tool_input", "tool_response"] {
            if let Some(nested) = obj.get(*key).and_then(|v| v.as_object()) {
                if let Some(sid) = nested.get("session_id") {
                    obj.insert("session_id".into(), sid.clone());
                    break;
                }
            }
        }
    }

    // 4. Ensure framework is present
    if !obj.contains_key("framework") {
        obj.insert("framework".into(), serde_json::Value::String(framework.to_string()));
    }

    // 5. Normalize timestamp to ISO-8601 with Z suffix
    if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
        let normalized_ts = normalize_timestamp(ts);
        obj.insert("timestamp".into(), serde_json::Value::String(normalized_ts));
    } else {
        obj.insert("timestamp".into(), serde_json::Value::String(
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        ));
    }

    // 6. Normalize event-specific data fields (tool field names, etc.)
    //    Must happen after flattening so we operate on the full top-level object.
    if let Some(event) = obj.get("event").and_then(|v| v.as_str()) {
        let event = event.to_string();
        normalize_event_fields(obj, &event);
    }

    json.to_string()
}

/// Normalize timestamp to ISO-8601 with `Z` suffix (not `+00:00`).
fn normalize_timestamp(ts: &str) -> String {
    // Try parsing chrono's RFC3339 output (with nanos and +00:00)
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        return dt.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    }
    // Try parsing with nanosecond precision
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f%:z") {
        return dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    }
    // Fallback: strip trailing nanoseconds and normalize +00:00 to Z
    let s = ts.trim();
    if s.ends_with("+00:00") {
        format!("{}Z", &s[..s.len() - 6])
    } else {
        s.to_string()
    }
}

/// Normalize event-specific fields at the top level.
///
/// Renames framework-specific field names to the unified schema:
/// - `name` → `tool_name`, `args` → `tool_input`, `result` → `tool_response`, etc.
/// - Removes framework-specific metadata fields.
pub fn normalize_event_fields(obj: &mut serde_json::Map<String, serde_json::Value>, event: &str) {
    if !event.starts_with("tool:") {
        return;
    }

    // tool_name aliases
    for alias in &["name"] {
        if let Some(val) = obj.remove(*alias) {
            if !obj.contains_key("tool_name") {
                obj.insert("tool_name".into(), val);
            }
        }
    }

    // tool_input aliases
    for alias in &["args", "input", "parameters", "arguments"] {
        if let Some(val) = obj.remove(*alias) {
            if !obj.contains_key("tool_input") {
                obj.insert("tool_input".into(), val);
            }
        }
    }

    // tool_response aliases
    for alias in &["tool_result", "result", "output"] {
        if let Some(val) = obj.remove(*alias) {
            if !obj.contains_key("tool_response") {
                obj.insert("tool_response".into(), val);
            }
        }
    }

    // tool_call_id aliases
    for alias in &["tool_use_id", "call_id"] {
        if let Some(val) = obj.remove(*alias) {
            if !obj.contains_key("tool_call_id") {
                obj.insert("tool_call_id".into(), val);
            }
        }
    }

    // Remove framework-specific metadata that shouldn't be in the output
    for key in &["hook_event_name", "cwd", "effort", "permission_mode", "transcript_path", "callback"] {
        obj.remove(*key);
    }
}
