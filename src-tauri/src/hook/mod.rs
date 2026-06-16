//! Hook management — detect frameworks, register hooks, run embedded Hub.
//!
//! This module provides the bridge between the Tauri app and the agent-hook ecosystem:
//! - **detector**: Scan the system for installed agent frameworks
//! - **manager**: Register/unregister hooks for detected frameworks
//! - **hub**: Embedded WS Hub server that receives and broadcasts events

pub mod detector;
pub mod manager;
pub mod hub;
