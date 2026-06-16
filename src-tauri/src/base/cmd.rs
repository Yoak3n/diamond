// Tauri command handlers — define `#[tauri::command]` functions here.

use log::{info, warn, error, debug};

#[tauri::command]
pub fn greet(name: &str) -> String {
    info!("Greet command called with name: {}", name);
    let message = format!("Hello, {}! You've been greeted from Rust!", name);
    debug!("Greet response: {}", message);
    message
}

#[tauri::command]
pub fn log_example(level: &str, message: &str) -> String {
    match level {
        "info" => info!("{}", message),
        "warn" => warn!("{}", message),
        "error" => error!("{}", message),
        "debug" => debug!("{}", message),
        _ => {
            warn!("Unknown log level: {}, defaulting to info", level);
            info!("{}", message);
        }
    }
    format!("Logged {} message: {}", level, message)
}