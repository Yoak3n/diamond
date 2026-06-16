// Tauri command handlers — define `#[tauri::command]` functions here.

use log::{debug, error, info, warn};
use tauri::Manager;

use crate::hook;
use super::window::{manager::Manager as WM, schema::WindowType};

// ─── Existing Commands ──────────────────────────────────────────────────────

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

// ─── Hook Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn detect_frameworks() -> Vec<hook::detector::DetectedFramework> {
    info!("Scanning for installed agent frameworks...");
    let frameworks = hook::detector::detect_all();
    info!("Found {} framework(s)", frameworks.len());
    frameworks
}

#[tauri::command]
pub fn register_hook(framework_id: &str, hub_url: &str) -> hook::manager::RegistrationResult {
    info!("Registering hook for {} -> {}", framework_id, hub_url);

    let frameworks = hook::detector::detect_all();
    match frameworks.iter().find(|f| f.id == framework_id) {
        Some(fw) => hook::manager::register_hook(fw, hub_url),
        None => hook::manager::RegistrationResult {
            framework_id: framework_id.to_string(),
            success: false,
            message: format!("Framework '{}' not found", framework_id),
            hook_path: None,
            requires_restart: false,
            restart_target: None,
        },
    }
}

#[tauri::command]
pub fn unregister_hook(framework_id: &str) -> hook::manager::RegistrationResult {
    info!("Unregistering hook for {}", framework_id);

    let frameworks = hook::detector::detect_all();
    match frameworks.iter().find(|f| f.id == framework_id) {
        Some(fw) => hook::manager::unregister_hook(fw),
        None => hook::manager::RegistrationResult {
            framework_id: framework_id.to_string(),
            success: false,
            message: format!("Framework '{}' not found", framework_id),
            hook_path: None,
            requires_restart: false,
            restart_target: None,
        },
    }
}

// ─── Hub Commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn hub_start(app: tauri::AppHandle) -> Result<String, String> {
    let state = app.state::<crate::base::state::AppState>();
    let manager = state.hub_manager.lock().await;

    if manager.is_running() {
        // Sync atomic flag even if already running
        state.hub_running.store(true, std::sync::atomic::Ordering::Relaxed);
        return Ok("Hub is already running".into());
    }

    let port = manager.port();
    manager.start(&app).await?;
    drop(manager);
    super::tray::sync_hub_check().await;
    Ok(format!("Hub started on port {}", port))
}

#[tauri::command]
pub async fn hub_stop(app: tauri::AppHandle) -> Result<String, String> {
    let state = app.state::<crate::base::state::AppState>();
    let manager = state.hub_manager.lock().await;

    manager.stop(&app).await?;
    drop(manager);
    super::tray::sync_hub_check().await;
    Ok("Hub stopped".into())
}

#[tauri::command]
pub async fn hub_status(app: tauri::AppHandle) -> hook::hub::HubStatus {
    let state = app.state::<crate::base::state::AppState>();
    let manager = state.hub_manager.lock().await;
    manager.status().await
}

#[tauri::command]
pub async fn hub_ws_url(app: tauri::AppHandle) -> String {
    let state = app.state::<crate::base::state::AppState>();
    let manager = state.hub_manager.lock().await;
    manager.ws_url()
}

#[tauri::command]
pub async fn hub_events(
    app: tauri::AppHandle,
    limit: usize,
    after_seq: Option<u64>,
) -> Vec<hook::hub::StoredEvent> {
    let state = app.state::<crate::base::state::AppState>();
    let manager = state.hub_manager.lock().await;
    manager.events(limit, after_seq).await
}

#[tauri::command]
pub async fn hub_clients(app: tauri::AppHandle) -> Vec<hook::hub::ClientInfo> {
    let state = app.state::<crate::base::state::AppState>();
    let manager = state.hub_manager.lock().await;
    manager.clients().await
}

// ─── Window Commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn window_minimize() -> bool {
    WM::global().minimized_window(WindowType::Main)
}

#[tauri::command]
pub fn window_toggle_maximize() -> Result<(), String> {
    if let Some(window) = WM::global().get_window(WindowType::Main) {
        if window.is_maximized().unwrap_or(false) {
            window.unmaximize().map_err(|e| e.to_string())?;
        } else {
            window.maximize().map_err(|e| e.to_string())?;
        }
        Ok(())
    } else {
        Err("Window not found".into())
    }
}

#[tauri::command]
pub fn window_close() {
    let _ = WM::global().close_window(WindowType::Main);
}
