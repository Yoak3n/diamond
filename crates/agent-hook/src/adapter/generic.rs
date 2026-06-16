//! Generic adapter — auto-discovers hooks on any agent object via reflection.
//!
//! For frameworks without a specific adapter, this uses configurable
//! method-name-to-event mapping rules.

use std::sync::Arc;

use tracing::info;

use super::{Adapter, AdapterError};
use crate::event::{AgentEvent, EventData, EventType};
use crate::client::HubClient;

/// Method-to-event mapping rule.
#[derive(Debug, Clone)]
pub struct HookRule {
    /// Substring to match in method name (case-insensitive).
    pub method_pattern: String,

    /// Event type to emit.
    pub event_type: EventType,

    /// Argument index to extract as primary data.
    pub arg_index: Option<usize>,

    /// Field name for the extracted argument.
    pub arg_field: String,
}

impl HookRule {
    pub fn new(
        method_pattern: impl Into<String>,
        event_type: EventType,
        arg_index: Option<usize>,
        arg_field: impl Into<String>,
    ) -> Self {
        Self {
            method_pattern: method_pattern.into(),
            event_type,
            arg_index,
            arg_field: arg_field.into(),
        }
    }
}

/// Generic adapter with configurable rules.
pub struct GenericAdapter {
    hub: Arc<HubClient>,
    session_id: String,
    framework: String,
    rules: Vec<HookRule>,
    attached: bool,
}

impl GenericAdapter {
    /// Create with default rules for common method patterns.
    pub fn new(
        hub: Arc<HubClient>,
        framework: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        let rules = vec![
            HookRule::new("chat", EventType::MessageUser, Some(0), "text"),
            HookRule::new("run", EventType::AgentStart, Some(0), "input"),
            HookRule::new("invoke", EventType::ChainStart, Some(0), "input"),
            HookRule::new("predict", EventType::ChainStart, Some(0), "input"),
            HookRule::new("execute", EventType::ToolStart, Some(0), "name"),
            HookRule::new("run_tool", EventType::ToolStart, Some(0), "name"),
            HookRule::new("call_tool", EventType::ToolStart, Some(0), "name"),
        ];

        Self {
            hub,
            session_id: session_id.into(),
            framework: framework.into(),
            rules,
            attached: false,
        }
    }

    /// Add a custom hook rule.
    pub fn add_rule(mut self, rule: HookRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Emit an event.
    fn emit(&self, event: AgentEvent) {
        self.hub.emit(event);
    }

    /// Check if a method name matches any rule.
    pub fn match_method(&self, method_name: &str) -> Option<&HookRule> {
        let lower = method_name.to_lowercase();
        self.rules
            .iter()
            .find(|r| lower.contains(&r.method_pattern))
    }

    /// Emit a method call event.
    pub fn on_method_call(&self, method_name: &str, args: &[String]) {
        if let Some(rule) = self.match_method(method_name) {
            let mut data = serde_json::Map::new();
            data.insert(
                "method".into(),
                serde_json::Value::String(method_name.into()),
            );

            if let Some(idx) = rule.arg_index {
                if let Some(val) = args.get(idx) {
                    data.insert(
                        rule.arg_field.clone(),
                        serde_json::Value::String(val.clone()),
                    );
                }
            }

            self.emit(AgentEvent::new(
                rule.event_type.clone(),
                &self.framework,
                &self.session_id,
                EventData::Map(data),
            ));
        }
    }

    /// Emit a method return event.
    pub fn on_method_return(&self, method_name: &str, result: &str) {
        let lower = method_name.to_lowercase();

        let event_type = if lower.contains("tool") {
            EventType::ToolComplete
        } else if lower.contains("run") || lower.contains("chat") {
            EventType::AgentEnd
        } else {
            EventType::Custom(format!("{}:complete", method_name))
        };

        let mut data = serde_json::Map::new();
        data.insert(
            "method".into(),
            serde_json::Value::String(method_name.into()),
        );
        data.insert(
            "result".into(),
            serde_json::Value::String(result.into()),
        );

        self.emit(AgentEvent::new(
            event_type,
            &self.framework,
            &self.session_id,
            EventData::Map(data),
        ));
    }
}

impl Adapter for GenericAdapter {
    fn framework(&self) -> &str {
        &self.framework
    }

    fn attach(&mut self, _agent: *mut std::ffi::c_void) -> Result<(), AdapterError> {
        self.attached = true;
        info!(
            framework = %self.framework,
            "Generic adapter attached with {} rules",
            self.rules.len()
        );
        Ok(())
    }

    fn detach(&mut self) -> Result<(), AdapterError> {
        self.attached = false;
        info!(framework = %self.framework, "Generic adapter detached");
        Ok(())
    }

    fn is_attached(&self) -> bool {
        self.attached
    }
}
