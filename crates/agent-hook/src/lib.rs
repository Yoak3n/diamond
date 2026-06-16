//! # agent-hook
//!
//! Universal agent framework hooking — unified event protocol, WS hub client,
//! and runtime patching for Python frameworks and CLI tools.
//!
//! ## Overview
//!
//! This crate provides a complete solution for hooking into multiple AI agent
//! frameworks (Hermes, LangChain, Claude Code, etc.) without modifying their
//! source code. Events from all frameworks are normalized into a unified format
//! and sent to a central WebSocket Hub.
//!
//! ## Architecture
//!
//! ```text
//! Agent Frameworks          agent-hook crate              WS Hub
//! ┌─────────────┐      ┌─────────────────────┐      ┌──────────┐
//! │  Hermes     │──┐   │                     │      │          │
//! │  LangChain  │──┼──▶│  Adapters → Events  │──WS─▶│  Hub     │
//! │  Claude Code│──┤   │                     │      │          │
//! │  AutoGen    │──┘   │  (patch + process)  │      │          │
//! └─────────────┘      └─────────────────────┘      └──────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```no_run
//! use agent_hook::client::{HubClient, with_url, with_framework};
//! use agent_hook::event::events;
//!
//! #[tokio::main]
//! async fn main() {
//!     let hub = HubClient::new([
//!         with_url("ws://127.0.0.1:9210/hook"),
//!         with_framework("hermes"),
//!     ]);
//!
//!     hub.connect().await.unwrap();
//!
//!     // Emit events
//!     hub.emit(events::agent_start("hermes", "session_1"));
//!     hub.emit(events::tool_start("hermes", "session_1", "terminal", None));
//!     hub.emit(events::message_delta("hermes", "session_1", "Hello!"));
//!
//!     hub.disconnect().await;
//! }
//! ```

pub mod event;
pub mod client;
pub mod patch;
pub mod adapter;

// Re-exports for convenience
pub use event::{AgentEvent, EventData, EventType};
pub use client::{HubClient, HubConfig, with_url, with_framework, with_session_id, with_buffer_size, with_max_reconnect_attempts, with_reconnect_delay, with_max_reconnect_delay};

#[cfg(feature = "python")]
pub use patch::PythonEnv;
