use std::{collections::HashSet, sync::{Arc,OnceLock}};
use parking_lot::Mutex;

use crate::base::lightweight::LightWeightState;
#[derive(Clone)]
pub struct AppState {
    pub lightweight: Arc<Mutex<LightWeightState>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            lightweight: Arc::new(Mutex::new(LightWeightState::default())),
        }
    }
}