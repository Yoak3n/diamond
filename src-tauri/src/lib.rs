pub mod base;

use base::init;
pub use base::window::manager::Manager as WM;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init::configure(tauri::Builder::default())
        .invoke_handler(init::generate_handlers())
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(base::init::app_event_handle);
}