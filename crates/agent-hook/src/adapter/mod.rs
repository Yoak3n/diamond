//! Framework adapters — bridge specific agent frameworks to the unified event protocol.
//!
//! Each adapter knows how to:
//! 1. Register hooks/callbacks on a framework instance
//! 2. Normalize framework events into [`AgentEvent`]
//! 3. Forward events to the Hub

pub mod claude_code;
pub mod generic;
pub mod hermes;
pub mod hermes_hooks;
pub mod langchain;
pub mod openclaw;

pub use claude_code::ClaudeCodeAdapter;
pub use generic::GenericAdapter;
pub use hermes::HermesAdapter;
pub use langchain::LangChainAdapter;
pub use openclaw::OpenClawAdapter;

/// Common interface for all framework adapters.
pub trait Adapter: Send + Sync {
    /// Framework identifier (e.g. "hermes", "langchain", "openclaw").
    fn framework(&self) -> &str;

    /// Attach hooks to the agent instance.
    fn attach(&mut self, agent: *mut std::ffi::c_void) -> Result<(), AdapterError>;

    /// Detach hooks (restore original behavior).
    fn detach(&mut self) -> Result<(), AdapterError>;

    /// Check if currently attached.
    fn is_attached(&self) -> bool;
}

// ─── Adapter Error ──────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Framework not found: {0}")]
    NotFound(String),

    #[error("Agent instance is null")]
    NullAgent,

    #[error("Callback injection failed: {0}")]
    InjectionFailed(String),

    #[error("Python error: {0}")]
    Python(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
