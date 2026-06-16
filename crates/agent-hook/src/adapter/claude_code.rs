//! Claude Code adapter — hooks into Claude Code's hook system.
//!
//! Claude Code fires hooks at lifecycle events:
//! - `SessionStart` — session begins/resumes
//! - `PreToolUse` — before tool call executes
//! - `PostToolUse` — after tool call succeeds
//! - `Stop` — Claude finishes responding
//!
//! Hooks receive JSON on stdin with fields like:
//! - `session_id`, `cwd`, `hook_event_name`
//! - `permission_mode`, `effort`
//!
//! This adapter converts those events to the unified protocol.

use std::sync::Arc;

use tracing::info;

use super::{Adapter, AdapterError};
use crate::event::{AgentEvent, EventData, EventType};
use crate::client::HubClient;

/// Adapter for Claude Code framework.
pub struct ClaudeCodeAdapter {
    hub: Arc<HubClient>,
    session_id: String,
    attached: bool,
}

impl ClaudeCodeAdapter {
    /// Create a new adapter.
    pub fn new(hub: Arc<HubClient>, session_id: impl Into<String>) -> Self {
        Self {
            hub,
            session_id: session_id.into(),
            attached: false,
        }
    }

    /// Emit an event through the Hub.
    fn emit(&self, event: AgentEvent) {
        self.hub.emit(event);
    }

    /// Handle a `SessionStart` hook event.
    ///
    /// Claude Code sends JSON like:
    /// ```json
    /// {
    ///   "session_id": "abc123",
    ///   "hook_event_name": "SessionStart",
    ///   "cwd": "/path/to/project"
    /// }
    /// ```
    pub fn on_session_start(&self, data: &serde_json::Value) {
        let session_id = data.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.session_id);

        let mut event_data = EventData::from([
            ("session_id", serde_json::Value::String(session_id.into())),
        ]);

        // Extract additional context
        if let Some(cwd) = data.get("cwd").and_then(|v| v.as_str()) {
            event_data.insert("cwd", serde_json::Value::String(cwd.into()));
        }

        self.emit(AgentEvent::new(
            EventType::SessionStart,
            "claude-code",
            &self.session_id,
            event_data,
        ));
    }

    /// Handle a `PreToolUse` hook event.
    ///
    /// Claude Code sends JSON like:
    /// ```json
    /// {
    ///   "session_id": "abc123",
    ///   "hook_event_name": "PreToolUse",
    ///   "tool_name": "Bash",
    ///   "tool_input": {"command": "ls -la"}
    /// }
    /// ```
    pub fn on_pre_tool_use(&self, data: &serde_json::Value) {
        let tool_name = data.get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let tool_input = data.get("tool_input")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        self.emit(AgentEvent::new(
            EventType::ToolStart,
            "claude-code",
            &self.session_id,
            EventData::from([
                ("tool_name", serde_json::Value::String(tool_name.into())),
                ("args", tool_input),
            ]),
        ));
    }

    /// Handle a `PostToolUse` hook event.
    ///
    /// Claude Code sends JSON like:
    /// ```json
    /// {
    ///   "session_id": "abc123",
    ///   "hook_event_name": "PostToolUse",
    ///   "tool_name": "Bash",
    ///   "tool_result": "file1.txt\nfile2.txt"
    /// }
    /// ```
    pub fn on_post_tool_use(&self, data: &serde_json::Value) {
        let tool_name = data.get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let tool_result = data.get("tool_result")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        self.emit(AgentEvent::new(
            EventType::ToolComplete,
            "claude-code",
            &self.session_id,
            EventData::from([
                ("tool_name", serde_json::Value::String(tool_name.into())),
                ("result", tool_result),
            ]),
        ));
    }

    /// Handle a `Stop` hook event.
    ///
    /// Claude Code sends JSON like:
    /// ```json
    /// {
    ///   "session_id": "abc123",
    ///   "hook_event_name": "Stop",
    ///   "stop_reason": "end_turn"
    /// }
    /// ```
    pub fn on_stop(&self, data: &serde_json::Value) {
        let mut event_data = EventData::empty();

        if let Some(reason) = data.get("stop_reason").and_then(|v| v.as_str()) {
            event_data.insert("reason", serde_json::Value::String(reason.into()));
        }

        self.emit(AgentEvent::new(
            EventType::AgentEnd,
            "claude-code",
            &self.session_id,
            event_data,
        ));
    }

    /// Handle any Claude Code hook event by dispatching on `hook_event_name`.
    pub fn on_hook_event(&self, data: &serde_json::Value) {
        let event_name = data.get("hook_event_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match event_name {
            "SessionStart" => self.on_session_start(data),
            "PreToolUse" => self.on_pre_tool_use(data),
            "PostToolUse" => self.on_post_tool_use(data),
            "Stop" | "StopFailure" => self.on_stop(data),
            _ => {
                // Pass through as custom event
                self.emit(AgentEvent::new(
                    EventType::Custom(format!("claude-code:{}", event_name)),
                    "claude-code",
                    &self.session_id,
                    EventData::Map(data.as_object().cloned().unwrap_or_default()),
                ));
            }
        }
    }
}

impl Adapter for ClaudeCodeAdapter {
    fn framework(&self) -> &str {
        "claude-code"
    }

    fn attach(&mut self, _agent: *mut std::ffi::c_void) -> Result<(), AdapterError> {
        self.attached = true;
        info!("Claude Code adapter attached");
        Ok(())
    }

    fn detach(&mut self) -> Result<(), AdapterError> {
        self.attached = false;
        info!("Claude Code adapter detached");
        Ok(())
    }

    fn is_attached(&self) -> bool {
        self.attached
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{with_url, with_framework};

    fn make_adapter() -> ClaudeCodeAdapter {
        let hub = Arc::new(HubClient::new([
            with_url("ws://127.0.0.1:19210"),
            with_framework("claude-code"),
        ]));
        ClaudeCodeAdapter::new(hub, "test-session")
    }

    #[tokio::test]
    async fn test_on_session_start() {
        let adapter = make_adapter();
        let data = serde_json::json!({
            "session_id": "test-123",
            "hook_event_name": "SessionStart",
            "cwd": "/tmp/test"
        });
        adapter.on_session_start(&data);
        // In real usage, this would emit to the hub
    }

    #[tokio::test]
    async fn test_on_pre_tool_use() {
        let adapter = make_adapter();
        let data = serde_json::json!({
            "session_id": "test-123",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"}
        });
        adapter.on_pre_tool_use(&data);
    }

    #[tokio::test]
    async fn test_on_stop() {
        let adapter = make_adapter();
        let data = serde_json::json!({
            "session_id": "test-123",
            "hook_event_name": "Stop",
            "stop_reason": "end_turn"
        });
        adapter.on_stop(&data);
    }

    #[tokio::test]
    async fn test_dispatch() {
        let adapter = make_adapter();
        let data = serde_json::json!({
            "session_id": "test-123",
            "hook_event_name": "PreToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "/tmp/test.txt", "content": "hello"}
        });
        adapter.on_hook_event(&data);
    }
}
