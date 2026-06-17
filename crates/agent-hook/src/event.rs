//! Unified event protocol for agent framework hooking.
//!
//! Every event from every framework is normalized into this format
//! before being sent to the WS Hub.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// ─── Event Types ────────────────────────────────────────────────────────────

/// All known event types across all frameworks.
/// Framework adapters map their internal events to these.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    // Gateway lifecycle
    GatewayStart,
    GatewayShutdown,

    // Session lifecycle
    SessionStart,
    SessionEnd,
    SessionReset,

    // Agent lifecycle
    AgentStart,
    AgentStep,
    AgentEnd,
    AgentError,

    // Message flow
    MessageUser,
    MessageStart,
    MessageDelta,
    MessageComplete,
    MessageInterim,
    MessageStreamEnd,

    // Thinking / reasoning
    ThinkingDelta,
    ReasoningAvailable,

    // Tool execution
    ToolStart,
    ToolProgress,
    ToolComplete,
    ToolError,
    ToolGenerating,
    ToolGuardrailHalt,

    // Approval
    ApprovalRequest,
    ApprovalResult,

    // System
    SystemStatus,
    SystemWarning,
    SystemError,
    SystemCompression,

    // Sub-agent
    SubagentStart,
    SubagentProgress,
    SubagentComplete,
    SubagentTool,
    SubagentThinking,

    // Chain (LangChain-style)
    ChainStart,
    ChainEnd,

    // Memory & skills
    MemorySaved,
    MemoryLoaded,
    SkillLoaded,
    SkillSaved,

    // Cron / background
    CronJobStart,
    CronJobEnd,
    BackgroundStarted,
    BackgroundFinished,

    // Custom (framework-specific, passthrough)
    Custom(String),

    // Unknown / unmapped
    Unknown,
}

impl EventType {
    /// Parse from a string, falling back to Custom/Unknown.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "gateway:start" => Self::GatewayStart,
            "gateway:shutdown" => Self::GatewayShutdown,
            "session:start" => Self::SessionStart,
            "session:end" => Self::SessionEnd,
            "session:reset" => Self::SessionReset,
            "agent:start" => Self::AgentStart,
            "agent:step" => Self::AgentStep,
            "agent:end" => Self::AgentEnd,
            "agent:error" => Self::AgentError,
            "message:user" => Self::MessageUser,
            "message:start" => Self::MessageStart,
            "message:delta" => Self::MessageDelta,
            "message:complete" => Self::MessageComplete,
            "message:interim" => Self::MessageInterim,
            "message:stream_end" => Self::MessageStreamEnd,
            "thinking:delta" => Self::ThinkingDelta,
            "reasoning:available" => Self::ReasoningAvailable,
            "tool:start" => Self::ToolStart,
            "tool:progress" => Self::ToolProgress,
            "tool:complete" => Self::ToolComplete,
            "tool:error" => Self::ToolError,
            "tool:generating" => Self::ToolGenerating,
            "tool:guardrail_halt" => Self::ToolGuardrailHalt,
            "approval:request" => Self::ApprovalRequest,
            "approval:result" => Self::ApprovalResult,
            "system:status" => Self::SystemStatus,
            "system:warning" => Self::SystemWarning,
            "system:error" => Self::SystemError,
            "system:compression" => Self::SystemCompression,
            "subagent:start" => Self::SubagentStart,
            "subagent:progress" => Self::SubagentProgress,
            "subagent:complete" => Self::SubagentComplete,
            "subagent:tool" => Self::SubagentTool,
            "subagent:thinking" => Self::SubagentThinking,
            "chain:start" => Self::ChainStart,
            "chain:end" => Self::ChainEnd,
            "memory:saved" => Self::MemorySaved,
            "memory:loaded" => Self::MemoryLoaded,
            "skill:loaded" => Self::SkillLoaded,
            "skill:saved" => Self::SkillSaved,
            "cron:job_start" => Self::CronJobStart,
            "cron:job_end" => Self::CronJobEnd,
            "background:started" => Self::BackgroundStarted,
            "background:finished" => Self::BackgroundFinished,
            other => {
                if other.starts_with("custom:") {
                    Self::Custom(other[7..].to_string())
                } else {
                    Self::Unknown
                }
            }
        }
    }

    /// Serialize back to wire format.
    pub fn as_str(&self) -> String {
        match self {
            Self::GatewayStart => "gateway:start".into(),
            Self::GatewayShutdown => "gateway:shutdown".into(),
            Self::SessionStart => "session:start".into(),
            Self::SessionEnd => "session:end".into(),
            Self::SessionReset => "session:reset".into(),
            Self::AgentStart => "agent:start".into(),
            Self::AgentStep => "agent:step".into(),
            Self::AgentEnd => "agent:end".into(),
            Self::AgentError => "agent:error".into(),
            Self::MessageUser => "message:user".into(),
            Self::MessageStart => "message:start".into(),
            Self::MessageDelta => "message:delta".into(),
            Self::MessageComplete => "message:complete".into(),
            Self::MessageInterim => "message:interim".into(),
            Self::MessageStreamEnd => "message:stream_end".into(),
            Self::ThinkingDelta => "thinking:delta".into(),
            Self::ReasoningAvailable => "reasoning:available".into(),
            Self::ToolStart => "tool:start".into(),
            Self::ToolProgress => "tool:progress".into(),
            Self::ToolComplete => "tool:complete".into(),
            Self::ToolError => "tool:error".into(),
            Self::ToolGenerating => "tool:generating".into(),
            Self::ToolGuardrailHalt => "tool:guardrail_halt".into(),
            Self::ApprovalRequest => "approval:request".into(),
            Self::ApprovalResult => "approval:result".into(),
            Self::SystemStatus => "system:status".into(),
            Self::SystemWarning => "system:warning".into(),
            Self::SystemError => "system:error".into(),
            Self::SystemCompression => "system:compression".into(),
            Self::SubagentStart => "subagent:start".into(),
            Self::SubagentProgress => "subagent:progress".into(),
            Self::SubagentComplete => "subagent:complete".into(),
            Self::SubagentTool => "subagent:tool".into(),
            Self::SubagentThinking => "subagent:thinking".into(),
            Self::ChainStart => "chain:start".into(),
            Self::ChainEnd => "chain:end".into(),
            Self::MemorySaved => "memory:saved".into(),
            Self::MemoryLoaded => "memory:loaded".into(),
            Self::SkillLoaded => "skill:loaded".into(),
            Self::SkillSaved => "skill:saved".into(),
            Self::CronJobStart => "cron:job_start".into(),
            Self::CronJobEnd => "cron:job_end".into(),
            Self::BackgroundStarted => "background:started".into(),
            Self::BackgroundFinished => "background:finished".into(),
            Self::Custom(name) => format!("custom:{}", name),
            Self::Unknown => "unknown".into(),
        }
    }
}

impl Serialize for EventType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(EventType::from_str_lossy(&s))
    }
}

/// A single normalized event from any agent framework.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Event type (e.g. "tool:start", "message:delta").
    #[serde(rename = "event")]
    pub event_type: EventType,

    /// Source framework identifier (e.g. "hermes", "langchain", "claude_code").
    pub framework: String,

    /// Session ID — ties events to a conversation.
    pub session_id: String,

    /// Turn ID — ties events to a single user turn (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,

    /// ISO-8601 timestamp.
    pub timestamp: DateTime<Utc>,

    /// Event-specific payload.
    #[serde(flatten)]
    pub data: EventData,
}

impl AgentEvent {
    /// Create a new event with auto-generated ID and current timestamp.
    pub fn new(
        event_type: EventType,
        framework: impl Into<String>,
        session_id: impl Into<String>,
        data: EventData,
    ) -> Self {
        Self {
            event_type,
            framework: framework.into(),
            session_id: session_id.into(),
            turn_id: None,
            timestamp: Utc::now(),
            data,
        }
    }

    /// Set turn ID (builder pattern).
    pub fn with_turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }

    /// Serialize to JSON bytes (newline-delimited).
    pub fn to_json_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(self).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    /// Deserialize from JSON string.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

// ─── Event Data ─────────────────────────────────────────────────────────────

/// Event-specific payload. The shape varies by event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    /// Generic key-value payload.
    Map(serde_json::Map<String, serde_json::Value>),

    /// Empty payload.
    Empty,
}

impl EventData {
    /// Create from a JSON value (must be an object).
    pub fn from_json_value(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::Object(map) => Self::Map(map),
            _ => Self::Empty,
        }
    }

    /// Create an empty payload.
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Get a field by key.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        match self {
            Self::Map(map) => map.get(key),
            Self::Empty => None,
        }
    }

    /// Get a string field.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    /// Get an integer field.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    /// Get a boolean field.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Insert a key-value pair.
    pub fn insert(&mut self, key: impl Into<String>, value: serde_json::Value) {
        match self {
            Self::Map(map) => {
                map.insert(key.into(), value);
            }
            Self::Empty => {
                let mut map = serde_json::Map::new();
                map.insert(key.into(), value);
                *self = Self::Map(map);
            }
        }
    }

    /// Convert to JSON value.
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::Map(map) => serde_json::Value::Object(map.clone()),
            Self::Empty => serde_json::Value::Null,
        }
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for EventData {
    fn from(map: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::Map(map)
    }
}

impl FromIterator<(String, serde_json::Value)> for EventData {
    fn from_iter<I: IntoIterator<Item = (String, serde_json::Value)>>(iter: I) -> Self {
        Self::Map(serde_json::Map::from_iter(iter))
    }
}

impl<const N: usize> From<[(&str, serde_json::Value); N]> for EventData {
    fn from(arr: [(&str, serde_json::Value); N]) -> Self {
        Self::Map(serde_json::Map::from_iter(
            arr.into_iter().map(|(k, v)| (k.to_string(), v)),
        ))
    }
}

// ─── Convenience constructors ───────────────────────────────────────────────

/// Helper to build common events quickly.
pub mod events {
    use super::*;

    pub fn tool_start(
        framework: &str,
        session_id: &str,
        name: &str,
        arguments: Option<&str>,
    ) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        if let Some(args) = arguments {
            data.insert("arguments".into(), serde_json::Value::String(args.into()));
        }
        AgentEvent::new(EventType::ToolStart, framework, session_id, EventData::Map(data))
    }

    pub fn tool_complete(
        framework: &str,
        session_id: &str,
        name: &str,
        result: &str,
        success: bool,
    ) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        data.insert("result".into(), serde_json::Value::String(result.into()));
        data.insert("success".into(), serde_json::Value::Bool(success));
        AgentEvent::new(EventType::ToolComplete, framework, session_id, EventData::Map(data))
    }

    pub fn tool_error(framework: &str, session_id: &str, name: &str, error: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        data.insert("error".into(), serde_json::Value::String(error.into()));
        AgentEvent::new(EventType::ToolError, framework, session_id, EventData::Map(data))
    }

    pub fn message_delta(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::MessageDelta, framework, session_id, EventData::Map(data))
    }

    pub fn message_user(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::MessageUser, framework, session_id, EventData::Map(data))
    }

    pub fn message_complete(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::MessageComplete, framework, session_id, EventData::Map(data))
    }

    pub fn agent_start(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::AgentStart, framework, session_id, EventData::empty())
    }

    pub fn agent_end(framework: &str, session_id: &str, response: Option<&str>) -> AgentEvent {
        let mut data = serde_json::Map::new();
        if let Some(resp) = response {
            data.insert("response".into(), serde_json::Value::String(resp.into()));
        }
        AgentEvent::new(EventType::AgentEnd, framework, session_id, EventData::Map(data))
    }

    pub fn agent_error(framework: &str, session_id: &str, error: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("error".into(), serde_json::Value::String(error.into()));
        AgentEvent::new(EventType::AgentError, framework, session_id, EventData::Map(data))
    }

    pub fn agent_step(
        framework: &str,
        session_id: &str,
        iteration: u32,
        prev_tools: Vec<serde_json::Value>,
    ) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("iteration".into(), serde_json::Value::Number(iteration.into()));
        data.insert("prev_tools".into(), serde_json::Value::Array(prev_tools));
        AgentEvent::new(EventType::AgentStep, framework, session_id, EventData::Map(data))
    }

    pub fn thinking_delta(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::ThinkingDelta, framework, session_id, EventData::Map(data))
    }

    pub fn session_start(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::SessionStart, framework, session_id, EventData::empty())
    }

    pub fn session_end(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::SessionEnd, framework, session_id, EventData::empty())
    }

    pub fn system_status(framework: &str, session_id: &str, message: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("message".into(), serde_json::Value::String(message.into()));
        AgentEvent::new(EventType::SystemStatus, framework, session_id, EventData::Map(data))
    }

    pub fn message_start(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::MessageStart, framework, session_id, EventData::empty())
    }

    pub fn message_stream_end(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::MessageStreamEnd, framework, session_id, EventData::empty())
    }

    pub fn message_interim(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::MessageInterim, framework, session_id, EventData::Map(data))
    }

    pub fn tool_progress(framework: &str, session_id: &str, name: &str, message: Option<&str>) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("tool_name".into(), serde_json::Value::String(name.into()));
        if let Some(msg) = message {
            data.insert("message".into(), serde_json::Value::String(msg.into()));
        }
        AgentEvent::new(EventType::ToolProgress, framework, session_id, EventData::Map(data))
    }

    pub fn tool_generating(framework: &str, session_id: &str, name: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("tool_name".into(), serde_json::Value::String(name.into()));
        AgentEvent::new(EventType::ToolGenerating, framework, session_id, EventData::Map(data))
    }

    pub fn tool_guardrail_halt(framework: &str, session_id: &str, name: &str, reason: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("tool_name".into(), serde_json::Value::String(name.into()));
        data.insert("reason".into(), serde_json::Value::String(reason.into()));
        AgentEvent::new(EventType::ToolGuardrailHalt, framework, session_id, EventData::Map(data))
    }

    pub fn approval_request(framework: &str, session_id: &str, tool_name: &str, message: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("tool_name".into(), serde_json::Value::String(tool_name.into()));
        data.insert("message".into(), serde_json::Value::String(message.into()));
        AgentEvent::new(EventType::ApprovalRequest, framework, session_id, EventData::Map(data))
    }

    pub fn approval_result(framework: &str, session_id: &str, approved: bool, reason: Option<&str>) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("approved".into(), serde_json::Value::Bool(approved));
        if let Some(r) = reason {
            data.insert("reason".into(), serde_json::Value::String(r.into()));
        }
        AgentEvent::new(EventType::ApprovalResult, framework, session_id, EventData::Map(data))
    }

    pub fn system_warning(framework: &str, session_id: &str, message: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("message".into(), serde_json::Value::String(message.into()));
        AgentEvent::new(EventType::SystemWarning, framework, session_id, EventData::Map(data))
    }

    pub fn system_error(framework: &str, session_id: &str, message: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("message".into(), serde_json::Value::String(message.into()));
        AgentEvent::new(EventType::SystemError, framework, session_id, EventData::Map(data))
    }

    pub fn system_compression(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::SystemCompression, framework, session_id, EventData::empty())
    }

    pub fn subagent_start(framework: &str, session_id: &str, name: Option<&str>) -> AgentEvent {
        let mut data = serde_json::Map::new();
        if let Some(n) = name {
            data.insert("name".into(), serde_json::Value::String(n.into()));
        }
        AgentEvent::new(EventType::SubagentStart, framework, session_id, EventData::Map(data))
    }

    pub fn subagent_progress(framework: &str, session_id: &str, message: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("message".into(), serde_json::Value::String(message.into()));
        AgentEvent::new(EventType::SubagentProgress, framework, session_id, EventData::Map(data))
    }

    pub fn subagent_complete(framework: &str, session_id: &str, result: Option<&str>) -> AgentEvent {
        let mut data = serde_json::Map::new();
        if let Some(r) = result {
            data.insert("result".into(), serde_json::Value::String(r.into()));
        }
        AgentEvent::new(EventType::SubagentComplete, framework, session_id, EventData::Map(data))
    }

    pub fn subagent_tool(framework: &str, session_id: &str, tool_name: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("tool_name".into(), serde_json::Value::String(tool_name.into()));
        AgentEvent::new(EventType::SubagentTool, framework, session_id, EventData::Map(data))
    }

    pub fn subagent_thinking(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::SubagentThinking, framework, session_id, EventData::Map(data))
    }

    pub fn chain_start(framework: &str, session_id: &str, name: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        AgentEvent::new(EventType::ChainStart, framework, session_id, EventData::Map(data))
    }

    pub fn chain_end(framework: &str, session_id: &str, name: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        AgentEvent::new(EventType::ChainEnd, framework, session_id, EventData::Map(data))
    }

    pub fn memory_saved(framework: &str, session_id: &str, key: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("key".into(), serde_json::Value::String(key.into()));
        AgentEvent::new(EventType::MemorySaved, framework, session_id, EventData::Map(data))
    }

    pub fn memory_loaded(framework: &str, session_id: &str, key: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("key".into(), serde_json::Value::String(key.into()));
        AgentEvent::new(EventType::MemoryLoaded, framework, session_id, EventData::Map(data))
    }

    pub fn skill_loaded(framework: &str, session_id: &str, name: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        AgentEvent::new(EventType::SkillLoaded, framework, session_id, EventData::Map(data))
    }

    pub fn skill_saved(framework: &str, session_id: &str, name: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("name".into(), serde_json::Value::String(name.into()));
        AgentEvent::new(EventType::SkillSaved, framework, session_id, EventData::Map(data))
    }

    pub fn cron_job_start(framework: &str, session_id: &str, job_id: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("job_id".into(), serde_json::Value::String(job_id.into()));
        AgentEvent::new(EventType::CronJobStart, framework, session_id, EventData::Map(data))
    }

    pub fn cron_job_end(framework: &str, session_id: &str, job_id: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("job_id".into(), serde_json::Value::String(job_id.into()));
        AgentEvent::new(EventType::CronJobEnd, framework, session_id, EventData::Map(data))
    }

    pub fn background_started(framework: &str, session_id: &str, task_id: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("task_id".into(), serde_json::Value::String(task_id.into()));
        AgentEvent::new(EventType::BackgroundStarted, framework, session_id, EventData::Map(data))
    }

    pub fn background_finished(framework: &str, session_id: &str, task_id: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("task_id".into(), serde_json::Value::String(task_id.into()));
        AgentEvent::new(EventType::BackgroundFinished, framework, session_id, EventData::Map(data))
    }

    pub fn gateway_start(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::GatewayStart, framework, session_id, EventData::empty())
    }

    pub fn gateway_shutdown(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::GatewayShutdown, framework, session_id, EventData::empty())
    }

    pub fn session_reset(framework: &str, session_id: &str) -> AgentEvent {
        AgentEvent::new(EventType::SessionReset, framework, session_id, EventData::empty())
    }

    pub fn reasoning_available(framework: &str, session_id: &str, text: &str) -> AgentEvent {
        let mut data = serde_json::Map::new();
        data.insert("text".into(), serde_json::Value::String(text.into()));
        AgentEvent::new(EventType::ReasoningAvailable, framework, session_id, EventData::Map(data))
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_roundtrip() {
        let cases = vec![
            "gateway:start",
            "session:start",
            "agent:start",
            "agent:step",
            "tool:start",
            "tool:complete",
            "tool:error",
            "message:delta",
            "message:complete",
            "thinking:delta",
            "approval:request",
            "system:status",
            "subagent:start",
            "chain:start",
            "memory:saved",
            "custom:my_event",
        ];
        for wire in cases {
            let parsed = EventType::from_str_lossy(wire);
            let back = parsed.as_str();
            assert_eq!(wire, back, "roundtrip failed for {:?}", wire);
        }
    }

    #[test]
    fn event_serialization() {
        let event = events::tool_start("hermes", "sess_123", "terminal", Some(r#"{"command":"ls"}"#));
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""event":"tool:start""#));
        assert!(json.contains(r#""framework":"hermes""#));
        assert!(json.contains(r#""name":"terminal""#));
    }

    #[test]
    fn event_data_getters() {
        let mut data = EventData::empty();
        data.insert("name", serde_json::Value::String("test".into()));
        assert_eq!(data.get_str("name"), Some("test"));
        assert_eq!(data.get_str("missing"), None);
    }
}
