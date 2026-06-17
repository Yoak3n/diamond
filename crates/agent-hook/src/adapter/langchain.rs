//! LangChain adapter — hooks into LangChain's callback system.
//!
//! LangChain has a mature callback system (`BaseCallbackHandler`).
//! This adapter provides a handler class that normalizes LangChain
//! events into the unified protocol.

use std::sync::Arc;

use tracing::info;

use super::{Adapter, AdapterError};
use crate::event::{AgentEvent, EventData, EventType};
use crate::client::HubClient;

/// Adapter for LangChain framework.
pub struct LangChainAdapter {
    hub: Arc<HubClient>,
    session_id: String,
    attached: bool,
}

impl LangChainAdapter {
    pub fn new(hub: Arc<HubClient>, session_id: impl Into<String>) -> Self {
        Self {
            hub,
            session_id: session_id.into(),
            attached: false,
        }
    }

    fn emit(&self, event: AgentEvent) {
        self.hub.emit(event);
    }

    /// Called when LLM starts generating.
    pub fn on_llm_start(&self, model_name: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::AgentStep,
            "langchain",
            &self.session_id,
            EventData::from([
                ("model", serde_json::Value::String(model_name.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called with each new token.
    pub fn on_llm_new_token(&self, token: &str) {
        self.emit(AgentEvent::new(
            EventType::MessageDelta,
            "langchain",
            &self.session_id,
            EventData::from([("text", serde_json::Value::String(token.into()))]),
        ));
    }

    /// Called when LLM finishes.
    pub fn on_llm_end(&self, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::MessageStreamEnd,
            "langchain",
            &self.session_id,
            EventData::from([("tool_call_id", serde_json::Value::String(run_id.into()))]),
        ));
    }

    /// Called when LLM encounters an error.
    pub fn on_llm_error(&self, error: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::AgentError,
            "langchain",
            &self.session_id,
            EventData::from([
                ("error", serde_json::Value::String(error.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called when a tool starts.
    pub fn on_tool_start(&self, name: &str, input: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::ToolStart,
            "langchain",
            &self.session_id,
            EventData::from([
                ("tool_name", serde_json::Value::String(name.into())),
                ("tool_input", serde_json::Value::String(input.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called when a tool finishes.
    pub fn on_tool_end(&self, name: &str, output: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::ToolComplete,
            "langchain",
            &self.session_id,
            EventData::from([
                ("tool_name", serde_json::Value::String(name.into())),
                ("tool_response", serde_json::Value::String(output.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called on tool error.
    pub fn on_tool_error(&self, name: &str, error: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::ToolError,
            "langchain",
            &self.session_id,
            EventData::from([
                ("tool_name", serde_json::Value::String(name.into())),
                ("error", serde_json::Value::String(error.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called when a chain starts.
    pub fn on_chain_start(&self, name: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::ChainStart,
            "langchain",
            &self.session_id,
            EventData::from([
                ("name", serde_json::Value::String(name.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called when a chain ends.
    pub fn on_chain_end(&self, name: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::ChainEnd,
            "langchain",
            &self.session_id,
            EventData::from([
                ("name", serde_json::Value::String(name.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called when an agent selects an action (tool to call).
    pub fn on_agent_action(&self, tool_name: &str, tool_input: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::ToolStart,
            "langchain",
            &self.session_id,
            EventData::from([
                ("tool_name", serde_json::Value::String(tool_name.into())),
                ("tool_input", serde_json::Value::String(tool_input.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }

    /// Called when an agent finishes.
    pub fn on_agent_finish(&self, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::AgentEnd,
            "langchain",
            &self.session_id,
            EventData::from([("tool_call_id", serde_json::Value::String(run_id.into()))]),
        ));
    }

    /// Called when a chat model starts generating.
    pub fn on_chat_model_start(&self, model_name: &str, run_id: &str) {
        self.emit(AgentEvent::new(
            EventType::AgentStep,
            "langchain",
            &self.session_id,
            EventData::from([
                ("model", serde_json::Value::String(model_name.into())),
                ("tool_call_id", serde_json::Value::String(run_id.into())),
            ]),
        ));
    }
}

impl Adapter for LangChainAdapter {
    fn framework(&self) -> &str {
        "langchain"
    }

    fn attach(&mut self, _agent: *mut std::ffi::c_void) -> Result<(), AdapterError> {
        self.attached = true;
        info!("LangChain adapter attached");
        Ok(())
    }

    fn detach(&mut self) -> Result<(), AdapterError> {
        self.attached = false;
        info!("LangChain adapter detached");
        Ok(())
    }

    fn is_attached(&self) -> bool {
        self.attached
    }
}
