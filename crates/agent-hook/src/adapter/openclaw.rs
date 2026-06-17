//! OpenClaw adapter — maps OpenClaw plugin hooks to the unified event protocol.
//!
//! OpenClaw is a TypeScript/Node.js framework with a rich plugin hook system.
//! The actual hooking is done by a TypeScript plugin (`openclaw_plugin.ts`)
//! that runs inside the OpenClaw Gateway and forwards events to the WS Hub.
//!
//! This Rust module provides:
//! - Event type mapping (OpenClaw hook names → unified [`EventType`])
//! - Plugin manifest generation
//! - Protocol documentation
//!
//! ## OpenClaw Hook System
//!
//! OpenClaw plugins register hooks via `api.on(name, handler, opts?)`:
//!
//! ```javascript
//! // Inside an OpenClaw plugin
//! export async function register(api) {
//!     api.on("before_agent_run", async (event) => {
//!         // Forward to WS Hub
//!         hub.emit("agent:start", { runId: event.runId });
//!     });
//! }
//! ```
//!
//! ## Coverage
//!
//! | OpenClaw Hook | Unified Event | Direction |
//! |---------------|---------------|-----------|
//! | `session_start` | `session:start` | S→C |
//! | `session_end` | `session:end` | S→C |
//! | `before_agent_run` | `agent:start` | S→C |
//! | `agent_end` | `agent:end` | S→C |
//! | `before_tool_call` | `tool:start` | S→C |
//! | `after_tool_call` | `tool:complete` | S→C |
//! | `message_received` | `message:user` | S→C |
//! | `message_sending` | `message:complete` | S→C |
//! | `llm_input` | `agent:step` | S→C |
//! | `llm_output` | `message:delta` | S→C |
//! | `subagent_spawned` | `subagent:start` | S→C |
//! | `subagent_ended` | `subagent:complete` | S→C |
//! | `gateway_start` | `gateway:start` | S→C |
//! | `gateway_stop` | `gateway:shutdown` | S→C |

use std::collections::HashMap;

use crate::event::EventType;
use super::{Adapter, AdapterError};

/// OpenClaw adapter — bridges OpenClaw plugin hooks to the unified event protocol.
///
/// Since OpenClaw is TypeScript/Node.js, the actual hooking is done by a
/// TypeScript plugin. This Rust struct provides the configuration and
/// plugin generation capabilities.
pub struct OpenClawAdapter {
    /// Hub URL to connect to.
    hub_url: String,

    /// Whether the plugin source has been generated.
    generated: bool,
}

impl OpenClawAdapter {
    /// Create a new OpenClaw adapter.
    pub fn new(hub_url: impl Into<String>) -> Self {
        Self {
            hub_url: hub_url.into(),
            generated: false,
        }
    }

    /// Generate the TypeScript plugin source code.
    pub fn generate_plugin(&mut self) -> String {
        let source = generate_plugin_source(&self.hub_url);
        self.generated = true;
        source
    }

    /// Get the recommended plugin install path.
    pub fn plugin_install_path() -> Option<std::path::PathBuf> {
        let home = dirs_next::home_dir()?;
        Some(home.join(".openclaw/plugins/hub-bridge"))
    }
}

impl Adapter for OpenClawAdapter {
    fn framework(&self) -> &str {
        "openclaw"
    }

    fn attach(&mut self, _agent: *mut std::ffi::c_void) -> Result<(), AdapterError> {
        // OpenClaw plugins run in the Node.js process, not via FFI.
        // Use generate_plugin() to create the TypeScript plugin instead.
        Err(AdapterError::Internal(
            "OpenClaw uses TypeScript plugins; use generate_plugin() instead".into(),
        ))
    }

    fn detach(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn is_attached(&self) -> bool {
        false
    }
}

/// OpenClaw hook name → unified event type mapping.
///
/// This mapping is used by the TypeScript plugin to normalize events.
pub fn hook_event_mapping() -> HashMap<&'static str, EventType> {
    let mut m = HashMap::new();

    // Gateway lifecycle
    m.insert("gateway_start", EventType::GatewayStart);
    m.insert("gateway_stop", EventType::GatewayShutdown);

    // Session lifecycle
    m.insert("session_start", EventType::SessionStart);
    m.insert("session_end", EventType::SessionEnd);

    // Agent turn
    m.insert("before_agent_run", EventType::AgentStart);
    m.insert("agent_end", EventType::AgentEnd);
    m.insert("before_agent_reply", EventType::AgentEnd);
    m.insert("before_agent_finalize", EventType::AgentEnd);

    // Model calls
    m.insert("llm_input", EventType::AgentStep);
    m.insert("llm_output", EventType::MessageDelta);
    m.insert("model_call_started", EventType::AgentStep);
    m.insert("model_call_ended", EventType::AgentEnd);

    // Tools
    m.insert("before_tool_call", EventType::ToolStart);
    m.insert("after_tool_call", EventType::ToolComplete);
    m.insert("tool_error", EventType::ToolError);
    m.insert("after_tool_error", EventType::ToolError);

    // Messages
    m.insert("message_received", EventType::MessageUser);
    m.insert("message_sending", EventType::MessageComplete);
    m.insert("message_sent", EventType::MessageComplete);
    m.insert("message_delta", EventType::MessageDelta);
    m.insert("llm_streaming", EventType::MessageDelta);

    // Subagents
    m.insert("subagent_spawned", EventType::SubagentStart);
    m.insert("subagent_ended", EventType::SubagentComplete);
    m.insert("subagent_tool", EventType::SubagentTool);
    m.insert("subagent_thinking", EventType::SubagentThinking);

    // Approval
    m.insert("approval_request", EventType::ApprovalRequest);

    // Prompt building
    m.insert("before_prompt_build", EventType::Custom("prompt:build".into()));
    m.insert("before_model_resolve", EventType::Custom("model:resolve".into()));

    // Compaction
    m.insert("before_compaction", EventType::SystemCompression);
    m.insert("after_compaction", EventType::SystemCompression);

    m
}

/// Generate the TypeScript plugin source code.
///
/// This produces a complete OpenClaw plugin that bridges all hooks to the WS Hub.
pub fn generate_plugin_source(hub_url: &str) -> String {
    let mapping = hook_event_mapping();

    let _hook_registrations: String = mapping
        .iter()
        .map(|(hook, event)| {
            let event_str = event.as_str();
            format!(
                r#"    api.on("{hook}", async (event) => {{
        hub.emit("{event_str}", serializeEvent(event));
    }}, {{ priority: 0 }});"#,
                hook = hook,
                event_str = event_str,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        r#"/**
 * OpenClaw → WS Hub Bridge Plugin
 *
 * Generated by agent-hook crate.
 * Bridges OpenClaw plugin hooks to a central WebSocket Hub.
 *
 * Install:
 *   1. Copy this file to ~/.openclaw/plugins/hub-bridge/index.ts
 *   2. Add to openclaw.json: {{ "plugins": {{ "enabled": ["hub-bridge"] }} }}
 *   3. Set HUB_URL env var or edit the config below
 */

const HUB_URL = process.env.AGENT_HOOK_HUB_URL || "{hub_url}";
const FRAMEWORK = "openclaw";
const RECONNECT_MS = 3000;
const MAX_BUFFER = 1000;

// ── WebSocket Hub Client ──

let ws = null;
let buffer = [];
let connected = false;
let reconnectTimer = null;

function connectHub() {{
    if (ws && (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN)) return;

    try {{
        const WebSocket = require("ws");
        ws = new WebSocket(HUB_URL);

        ws.on("open", () => {{
            connected = true;
            console.log(`[hub-bridge] Connected to ${{HUB_URL}}`);
            // Flush buffered events
            while (buffer.length > 0 && connected) {{
                const msg = buffer.shift();
                ws.send(msg);
            }}
        }});

        ws.on("message", (data) => {{
            // Handle Hub commands (ping, etc.)
            try {{
                const msg = JSON.parse(data.toString());
                if (msg.event === "hub:ping") {{
                    ws.send(JSON.stringify({{ event: "hub:pong", framework: FRAMEWORK }}));
                }}
            }} catch (e) {{}}
        }});

        ws.on("close", () => {{
            connected = false;
            scheduleReconnect();
        }});

        ws.on("error", (err) => {{
            connected = false;
            console.error("[hub-bridge] WS error:", err.message);
        }});
    }} catch (e) {{
        console.error("[hub-bridge] Connect failed:", e.message);
        scheduleReconnect();
    }}
}}

function scheduleReconnect() {{
    if (reconnectTimer) return;
    reconnectTimer = setTimeout(() => {{
        reconnectTimer = null;
        connectHub();
    }}, RECONNECT_MS);
}}

function emit(eventType, data) {{
    // 构建符合统一协议的消息格式
    const msg = {{
        event: eventType,
        framework: FRAMEWORK,
        session_id: data?.session_id || "default",
        timestamp: new Date().toISOString(),
    }};

    // 合并数据字段到顶层（排除 session_id，已提取到顶层）
    if (data) {{
        const {{ session_id, ...rest }} = data;
        Object.assign(msg, rest);
    }}

    const jsonStr = JSON.stringify(msg);

    if (connected && ws && ws.readyState === WebSocket.OPEN) {{
        ws.send(jsonStr);
    }} else {{
        buffer.push(jsonStr);
        if (buffer.length > MAX_BUFFER) buffer.shift();
        connectHub();
    }}
}}

// ── Event Serialization ──

function serializeEvent(event) {{
    if (!event) return {{}};
    const result = {{}};

    // 会话 ID
    if (event.sessionKey) result.session_id = event.sessionKey;

    // 工具相关字段（统一命名）
    if (event.toolName) result.tool_name = event.toolName;
    if (event.params) result.tool_input = typeof event.params === 'object' ? event.params : JSON.stringify(event.params).slice(0, 500);
    if (event.toolResult) result.tool_response = typeof event.toolResult === 'object' ? event.toolResult : String(event.toolResult).slice(0, 500);
    if (event.durationMs) result.duration_ms = event.durationMs;

    // 消息内容
    if (event.text) result.text = String(event.text).slice(0, 500);
    if (event.content) result.content = String(event.content).slice(0, 500);

    // 错误信息
    if (event.error) result.error = String(event.error).slice(0, 500);

    // 模型信息
    if (event.provider) result.provider = event.provider;
    if (event.model) result.model = event.model;

    // 其他
    if (event.outcome) result.outcome = event.outcome;
    if (event.runId) result.run_id = event.runId;

    return result;
}}

// ── Plugin Registration ──

export async function register(api) {{
    console.log("[hub-bridge] Registering hooks → " + HUB_URL);
    connectHub();

    // Gateway lifecycle
    api.on("gateway_start", async () => emit("gateway:start", {{}}));
    api.on("gateway_stop", async () => emit("gateway:shutdown", {{}}));

    // Session lifecycle
    api.on("session_start", async (e) => emit("session:start", serializeEvent(e)));
    api.on("session_end", async (e) => emit("session:end", serializeEvent(e)));

    // Agent turn
    api.on("before_agent_run", async (e) => emit("agent:start", serializeEvent(e)));
    api.on("agent_end", async (e) => emit("agent:end", serializeEvent(e)));

    // Model calls
    api.on("llm_input", async (e) => emit("agent:step", serializeEvent(e)));
    api.on("llm_output", async (e) => emit("message:delta", serializeEvent(e)));
    api.on("model_call_started", async (e) => emit("agent:step", serializeEvent(e)));
    api.on("model_call_ended", async (e) => emit("agent:end", serializeEvent(e)));

    // Tools
    api.on("before_tool_call", async (e) => emit("tool:start", serializeEvent(e)));
    api.on("after_tool_call", async (e) => emit("tool:complete", serializeEvent(e)));
    api.on("tool_error", async (e) => emit("tool:error", serializeEvent(e)));
    api.on("after_tool_error", async (e) => emit("tool:error", serializeEvent(e)));

    // Messages
    api.on("message_received", async (e) => emit("message:user", serializeEvent(e)));
    api.on("message_sending", async (e) => emit("message:complete", serializeEvent(e)));
    api.on("message_sent", async (e) => emit("message:complete", serializeEvent(e)));
    api.on("message_delta", async (e) => emit("message:delta", serializeEvent(e)));
    api.on("llm_streaming", async (e) => emit("message:delta", serializeEvent(e)));

    // Subagents
    api.on("subagent_spawned", async (e) => emit("subagent:start", serializeEvent(e)));
    api.on("subagent_ended", async (e) => emit("subagent:complete", serializeEvent(e)));
    api.on("subagent_tool", async (e) => emit("subagent:tool", serializeEvent(e)));
    api.on("subagent_thinking", async (e) => emit("subagent:thinking", serializeEvent(e)));

    // Approval
    api.on("approval_request", async (e) => emit("approval:request", serializeEvent(e)));

    // Compaction
    api.on("before_compaction", async (e) => emit("system:compression", serializeEvent(e)));
    api.on("after_compaction", async (e) => emit("system:compression", serializeEvent(e)));

    console.log("[hub-bridge] 26 hooks registered");
}}
"#
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_covers_key_hooks() {
        let m = hook_event_mapping();
        assert!(m.contains_key("gateway_start"));
        assert!(m.contains_key("session_start"));
        assert!(m.contains_key("before_agent_run"));
        assert!(m.contains_key("agent_end"));
        assert!(m.contains_key("before_tool_call"));
        assert!(m.contains_key("after_tool_call"));
        assert!(m.contains_key("message_received"));
        assert!(m.contains_key("message_sending"));
        assert!(m.contains_key("subagent_spawned"));
        assert!(m.contains_key("subagent_ended"));
        assert!(m.contains_key("llm_input"));
        assert!(m.contains_key("llm_output"));
    }

    #[test]
    fn generate_plugin_produces_valid_source() {
        let source = generate_plugin_source("ws://127.0.0.1:9210/hook");
        assert!(source.contains("export async function register"));
        assert!(source.contains("api.on("));
        assert!(source.contains("ws://127.0.0.1:9210/hook"));
        assert!(source.contains("hub-bridge"));
    }

    #[test]
    fn all_mapped_events_are_valid() {
        let m = hook_event_mapping();
        for (hook, event) in &m {
            // All event types should have a valid as_str()
            let s = event.as_str();
            assert!(!s.is_empty(), "Empty event string for hook: {}", hook);
            assert!(s.contains(':'), "Missing colon in event: {}", s);
        }
    }
}
