use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use parking_lot::Mutex;

use crate::base::lightweight::LightWeightState;
use crate::hook::hub::HubManager;

/// Default Hub port. Changed from 9210 to avoid conflicts with QQ.
const DEFAULT_HUB_PORT: u16 = 19210;

#[derive(Clone)]
pub struct AppState {
    pub lightweight: Arc<Mutex<LightWeightState>>,
    pub hub_manager: Arc<tokio::sync::Mutex<HubManager>>,
    /// Tracks hub running state for synchronous access (e.g., tray menu).
    pub hub_running: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            lightweight: Arc::new(Mutex::new(LightWeightState::default())),
            hub_manager: Arc::new(tokio::sync::Mutex::new(HubManager::new(DEFAULT_HUB_PORT))),
            hub_running: Arc::new(AtomicBool::new(false)),
        }
    }
}
