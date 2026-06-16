//! Runtime patching — monkey-patch Python frameworks and wrap CLI processes.
//!
//! This module provides two main strategies for hooking agent frameworks
//! without modifying their source code:
//!
//! 1. **Python patching** (via PyO3) — inject callbacks into live Python objects
//! 2. **Process wrapping** — spawn CLI agents and parse their stdout/stderr
//!
//! Both strategies emit [`AgentEvent`]s that can be forwarded to the Hub.
//!
//! ## Python Environment Discovery
//!
//! The `python` submodule includes `PythonEnv` which discovers the Python
//! instance used by the target agent framework (venv, registry, /proc, etc.)
//! so PyO3 can load the correct Python DLL without requiring it in PATH.

pub mod process;

#[cfg(feature = "python")]
pub mod python;

pub use process::ProcessWrapper;
#[cfg(feature = "python")]
pub use python::PythonEnv;
