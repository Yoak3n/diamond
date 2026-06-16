use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

pub struct Handle {
    handle: Arc<Mutex<Option<AppHandle>>>,
}
impl Default for Handle {
    fn default() -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl Handle {
    pub fn global() -> &'static Self {
        static APP_HANDLE: OnceCell<Handle> = OnceCell::new();
        APP_HANDLE.get_or_init(|| Self::default())
    }

    pub fn init(&self, input: AppHandle) {
        let mut handle = self.handle.lock().unwrap();
        handle.replace(input);
    }

    pub fn app_handle(&self) -> Option<AppHandle> {
        self.handle.lock().unwrap().clone()
    }
}