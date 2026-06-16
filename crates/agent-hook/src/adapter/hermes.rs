//! Hermes Agent adapter — hooks into Hermes Agent's callback system.
//!
//! Hermes exposes these callback points on `AIAgent`:
//! - `stream_delta_callback` — called with each streamed token
//! - `tool_start_callback` — called when a tool begins execution
//! - `step_callback` — called after each API iteration
//! - `status_callback` — called on status changes
//! - `interim_assistant_callback` — called with partial assistant text
//!
//! Gateway-level hooks (via `hooks.emit`):
//! - `agent:start`, `agent:end`, `session:start`, `session:end`
//!
//! This adapter injects callbacks that normalize events and forward
//! them to the Hub.

use std::sync::Arc;

use tracing::info;

use super::{Adapter, AdapterError};
use crate::event::{AgentEvent, EventData, EventType};
use crate::client::HubClient;

/// Adapter for Hermes Agent framework.
pub struct HermesAdapter {
    hub: Arc<HubClient>,
    session_id: String,
    attached: bool,
}

impl HermesAdapter {
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

    /// Handle a `stream_delta_callback` invocation.
    ///
    /// The callback receives a single `Option<String>` argument:
    /// - `Some(text)` = streamed token
    /// - `None` = end of stream
    pub fn on_stream_delta(&self, delta: Option<String>) {
        match delta {
            Some(text) => {
                self.emit(AgentEvent::new(
                    EventType::MessageDelta,
                    "hermes",
                    &self.session_id,
                    EventData::from([("text", serde_json::Value::String(text))]),
                ));
            }
            None => {
                self.emit(AgentEvent::new(
                    EventType::MessageStreamEnd,
                    "hermes",
                    &self.session_id,
                    EventData::empty(),
                ));
            }
        }
    }

    /// Handle a `tool_start_callback` invocation.
    pub fn on_tool_start(&self, name: &str) {
        self.emit(AgentEvent::new(
            EventType::ToolStart,
            "hermes",
            &self.session_id,
            EventData::from([("name", serde_json::Value::String(name.into()))]),
        ));
    }

    /// Handle a `step_callback` invocation.
    pub fn on_step(&self, iteration: u32, prev_tools: Vec<String>) {
        let tools: Vec<serde_json::Value> = prev_tools
            .into_iter()
            .map(|t| serde_json::Value::String(t))
            .collect();

        self.emit(AgentEvent::new(
            EventType::AgentStep,
            "hermes",
            &self.session_id,
            EventData::from([
                ("iteration", serde_json::Value::Number(iteration.into())),
                ("prev_tools", serde_json::Value::Array(tools)),
            ]),
        ));
    }

    /// Handle a `status_callback` invocation.
    pub fn on_status(&self, kind: &str, message: &str) {
        let event_type = match kind {
            "lifecycle" => EventType::SystemStatus,
            "warn" => EventType::SystemWarning,
            "error" => EventType::SystemError,
            _ => EventType::SystemStatus,
        };

        self.emit(AgentEvent::new(
            event_type,
            "hermes",
            &self.session_id,
            EventData::from([
                ("kind", serde_json::Value::String(kind.into())),
                ("message", serde_json::Value::String(message.into())),
            ]),
        ));
    }

    /// Handle a `interim_assistant_callback` invocation.
    pub fn on_interim(&self, text: &str) {
        self.emit(AgentEvent::new(
            EventType::MessageInterim,
            "hermes",
            &self.session_id,
            EventData::from([("text", serde_json::Value::String(text.into()))]),
        ));
    }

    /// Handle a gateway hook event.
    pub fn on_gateway_hook(&self, hook_name: &str, data: serde_json::Value) {
        let event_type = EventType::from_str_lossy(hook_name);
        let event_data = match data {
            serde_json::Value::Object(map) => EventData::Map(map),
            _ => EventData::empty(),
        };

        self.emit(AgentEvent::new(
            event_type,
            "hermes",
            &self.session_id,
            event_data,
        ));
    }
}

impl Adapter for HermesAdapter {
    fn framework(&self) -> &str {
        "hermes"
    }

    fn attach(&mut self, _agent: *mut std::ffi::c_void) -> Result<(), AdapterError> {
        self.attached = true;
        info!("Hermes adapter attached");
        Ok(())
    }

    fn detach(&mut self) -> Result<(), AdapterError> {
        self.attached = false;
        info!("Hermes adapter detached");
        Ok(())
    }

    fn is_attached(&self) -> bool {
        self.attached
    }
}
