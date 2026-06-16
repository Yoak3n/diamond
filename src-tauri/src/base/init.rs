use tauri::{AppHandle, Builder, Manager, RunEvent, generate_handler};
use tauri_plugin_log::{Target, TargetKind};
use crate::base::cmd::*;


pub fn generate_handlers() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static{
    generate_handler![greet, log_example]
}


pub fn configure(builder: Builder<tauri::Wry>) -> Builder<tauri::Wry> {
    let builder = builder.plugin(tauri_plugin_opener::init());

    let builder = builder.plugin(
        tauri_plugin_log::Builder::new()
            .targets([
                // 输出到控制台
                Target::new(TargetKind::Stdout),
                // 输出到前端控制台
                Target::new(TargetKind::Webview),
                // 输出到日志文件
                Target::new(TargetKind::Folder {
                    path: dirs::data_dir().unwrap_or_default().join("tauri-vue-template").join("logs"),
                    file_name: Some("app".into()),
                }),
            ])
            .level(log::LevelFilter::Info)
            .build(),
    );

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = {
        use tauri_plugin_autostart::MacosLauncher;
        builder.plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
    };

    builder.setup(|app| {
        app.manage(crate::base::state::AppState::default());
        crate::base::handle::Handle::global().init(app.handle().clone());
        let _ = crate::base::tray::create_tray_icon(app, false);
        crate::base::lightweight::add_window_listeners(crate::base::window::schema::WindowType::Main);

        Ok(())
    })
}

pub fn app_event_handle(app_handle: &AppHandle, event: RunEvent) {
    match event {
        tauri::RunEvent::Ready | tauri::RunEvent::Resumed => {}
        tauri::RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() {
                api.prevent_exit();
            }
        }
        tauri::RunEvent::WindowEvent { label, event, .. } => {
            // if label == "main" {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let window = app_handle.get_webview_window(&label).unwrap();
                    let _ = window.hide();
                }
                tauri::WindowEvent::Focused(true) => {}
                tauri::WindowEvent::Focused(false) => {}
                tauri::WindowEvent::Destroyed => {}
                _ => {}
            }
            // }
        }
        _ => {}
    }
}