use anyhow::Result;
use tauri::{
    AppHandle, Manager, Runtime, Wry,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

use crate::base::window::schema::WindowType;

use super::handle::Handle;
use super::window::manager::Manager as WM;
pub fn create_tray_icon<R: Runtime>(app: &tauri::App<R>, visible: bool) -> Result<()> {
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show_i = MenuItem::with_id(
        app,
        "show",
        if visible { "Hide" } else { "Show" },
        true,
        None::<&str>,
    )?;
    let hub_i = CheckMenuItem::with_id(app, "hub", "Hub Server", true, false, None::<&str>)?;
    let menu = Menu::with_items(app, &[&hub_i, &show_i, &quit_i])?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let auto_i = CheckMenuItem::with_id(
            app,
            "autostart",
            "AutoStart",
            true,
            {
                use tauri_plugin_autostart::ManagerExt;
                app.autolaunch().is_enabled().unwrap_or(false)
            },
            None::<&str>,
        )?;
        menu.insert_items(&[&auto_i], 0)?;
    }

    let _tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "show" => {
                WM::global().toggle_window(WindowType::Main);
            }
            "hub" => {
                tauri::async_runtime::spawn(async {
                    super::tray::toggle_hub().await;
                });
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            "autostart" => {
                let autostart_manager = {
                    use tauri_plugin_autostart::ManagerExt;
                    app.autolaunch()
                };
                let currently_enabled = autostart_manager.is_enabled().unwrap_or(false);
                let new_state = if currently_enabled {
                    autostart_manager.disable().is_ok() && false
                } else {
                    autostart_manager.enable().is_ok()
                };
                if let Some(item) = app.menu().and_then(|m| m.get("autostart")) {
                    if let Some(check_item) = item.as_check_menuitem() {
                        let _ = check_item.set_checked(new_state);
                    }
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|_, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                WM::global().toggle_window(WindowType::Main);
            }
        })
        .build(app)?;

    // Sync hub check state after tray icon is created
    tauri::async_runtime::spawn(async {
        sync_hub_check().await;
    });

    Ok(())
}

pub fn update_menu_visible(visible: bool) {
    let app = Handle::global();
    let app_handle = app.app_handle().unwrap();
    let tray = app_handle.tray_by_id("main").unwrap();
    tray.set_menu(Some(create_tray_menu(&app_handle, visible).unwrap()))
        .unwrap();
    // Sync hub check state after menu rebuild
    tauri::async_runtime::spawn(async {
        sync_hub_check().await;
    });
}

fn create_tray_menu(app_handle: &AppHandle, visiable: bool) -> Result<Menu<Wry>> {
    let quit_i = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;
    let show_i = MenuItem::with_id(
        app_handle,
        "show",
        if visiable { "Hide" } else { "Show" },
        true,
        None::<&str>,
    )?;
    // Get hub state synchronously from atomic flag
    let hub_running = app_handle
        .try_state::<super::state::AppState>()
        .map(|state| state.hub_running.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false);
    let hub_i = CheckMenuItem::with_id(app_handle, "hub", "Hub Server", true, hub_running, None::<&str>)?;
    let menu = Menu::with_items(app_handle, &[&hub_i, &show_i, &quit_i])?;

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let auto_i = CheckMenuItem::with_id(
            app_handle,
            "autostart",
            "AutoStart",
            true,
            {
                use tauri_plugin_autostart::ManagerExt;
                app_handle.autolaunch().is_enabled().unwrap_or(false)
            },
            None::<&str>,
        )?;
        menu.insert_items(&[&auto_i], 0)?;
    }

    Ok(menu)
}

/// Toggle hub server and update tray check state.
pub async fn toggle_hub() {
    let app_handle = Handle::global().app_handle().unwrap();
    let state = app_handle.state::<super::state::AppState>();
    let manager = state.hub_manager.lock().await;
    let running = manager.is_running();
    let result = if running {
        manager.stop(&app_handle).await
    } else {
        manager.start(&app_handle).await
    };
    match result {
        Ok(()) => {
            let new_state = !running;
            state.hub_running.store(new_state, std::sync::atomic::Ordering::Relaxed);
            log::info!("Hub {}", if new_state { "started" } else { "stopped" });
            set_tray_check(&app_handle, "hub", new_state);
        }
        Err(e) => {
            log::warn!("Hub toggle failed: {}", e);
        }
    }
}

/// Sync hub check state with actual hub state.
pub async fn sync_hub_check() {
    let app_handle = Handle::global().app_handle().unwrap();
    let state = app_handle.state::<super::state::AppState>();
    let manager = state.hub_manager.lock().await;
    let running = manager.is_running();
    state.hub_running.store(running, std::sync::atomic::Ordering::Relaxed);
    set_tray_check(&app_handle, "hub", running);
}

fn set_tray_check(app: &AppHandle, id: &str, checked: bool) {
    if let Some(item) = app.menu().and_then(|m| m.get(id)) {
        if let Some(check_item) = item.as_check_menuitem() {
            let _ = check_item.set_checked(checked);
        }
    }
}
