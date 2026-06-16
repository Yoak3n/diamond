use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    Emitter, Error, Manager as TauriManager, WebviewWindow, WebviewWindowBuilder, Wry,
};

use super::{
    config::WindowConfig,
    schema::{WindowState, WindowType, WindowOperationResult},
    position::adjust_float_window_position,
};

use crate::base::lightweight::add_window_listeners;
use crate::base::{handle,tray::update_menu_visible};

pub struct Manager {
    configs: HashMap<WindowType, WindowConfig>,
    states: Arc<Mutex<HashMap<WindowType, WindowState>>>,
}

fn default_states() -> HashMap<WindowType, WindowState> {
    HashMap::from([(WindowType::Main, WindowState::NotExist)])
}

impl Manager {
    fn new() -> Self {
        Self {
            configs: WindowConfig::default_config(),
            states: Arc::new(Mutex::new(default_states())),
        }
    }

    pub fn global() -> &'static Self {
        static INSTANCE: OnceCell<Manager> = OnceCell::new();
        INSTANCE.get_or_init(Self::new)
    }

    // 获取窗口实例
    pub fn get_window(&self, window_type: WindowType) -> Option<WebviewWindow<Wry>> {
        handle::Handle::global()
            .app_handle()
            .and_then(|app| app.get_webview_window(window_type.label()))
    }

    pub fn update_window_state(&self, window_type: WindowType, state: WindowState) {
        self.states.lock().unwrap().insert(window_type, state);
    }

    pub fn get_cached_window_state(&self, window_type: WindowType) -> WindowState {
        self.states
            .lock()
            .unwrap()
            .get(&window_type)
            .copied()
            .unwrap_or(WindowState::NotExist)
    }

    fn create_window_inner(
        &self,
        window_type: WindowType,
        url_with_args: Option<&str>,
    ) -> Result<WebviewWindow<Wry>, Error> {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let config = self
            .configs
            .get(&window_type)
            .ok_or_else(|| Error::FailedToReceiveMessage)?;

        // 检查是否已存在窗口
        if let Some(existing_window) = self.get_window(window_type) {
            if let Some(u) = url_with_args {
                if let Ok(current_url) = existing_window.url() {
                    let path = current_url.path();
                    let parsed_url = {
                        if let Some(q) = current_url.query() {
                            format!("{}?{}", path, q)
                        } else {
                            path.to_string()
                        }
                    };
                    if parsed_url != u {
                        // 不同运行环境下url并不一致，tauri自带的naviate又需要传完整的url解析结果，所以这里使用redirect事件，让窗口自行跳转
                        let _ = existing_window.emit_to(window_type.label(), "redirect", u);
                    }
                }
            }

            if existing_window.is_minimized().unwrap_or(false) {
                let _ = existing_window.unminimize();
            }
            let _ = existing_window.show();
            let _ = existing_window.set_focus();

            // 更新缓存状态为可见且有焦点
            self.update_window_state(window_type, WindowState::VisibleFocused);

            return Ok(existing_window);
        }

        let mut builder = WebviewWindowBuilder::new(
            &app_handle,
            window_type.label().to_string(),
            tauri::WebviewUrl::App(
                url_with_args
                    .map(|u| {
                        u.parse()
                            .unwrap_or(window_type.url().parse().unwrap_or_default())
                    })
                    .unwrap_or_else(|| window_type.url().into()),
            ),
        )
        .title(config.window_type.title())
        .inner_size(config.inner_size.0, config.inner_size.1)
        .min_inner_size(config.min_inner_size.0, config.min_inner_size.1)
        .decorations(config.decorations)
        .focused(config.focused)
        .skip_taskbar(config.skip_taskbar)
        .always_on_top(config.always_on_top)
        .maximizable(config.maximizable)
        .transparent(config.transparent)
        .shadow(config.shadow);
        if config.center {
            builder = builder.center();
        }
        if config.float {
            let (x, y) = adjust_float_window_position(&app_handle, config);
            builder = builder.position(x, y);  
        }

        #[cfg(target_os = "windows")]
        {
            builder = builder.additional_browser_args("--enable-features=msWebView2EnableDraggableRegions --disable-features=OverscrollHistoryNavigation,msExperimentalScrolling");
        }
        
        let window = builder.build()?;
        window.set_focus()?;
        add_window_listeners(window_type);
        Ok(window)
    }

    fn activate_window(
        &self,
        window: &WebviewWindow<Wry>,
        window_type: WindowType,
    ) -> WindowOperationResult {

        let mut operations_successful = true;

        // 1. 如果窗口最小化，先取消最小化
        if window.is_minimized().unwrap_or(false) {
            if let Err(e) = window.unminimize() {
                println!("取消最小化窗口失败: {:?}", e);
                operations_successful = false;
            }
        }

        // 2. 显示窗口
        if let Err(e) = window.show() {
            println!("显示窗口失败: {:?}", e);
            operations_successful = false;
        }

        // 3. 设置焦点
        if let Err(e) = window.set_focus() {
            println!("设置窗口焦点失败: {:?}", e);
            operations_successful = false;
        }

        // 4. 平台特定的激活策略
        #[cfg(target_os = "windows")]
        {
            // Windows 尝试额外的激活方法
            if let Err(e) = window.set_always_on_top(true) {
                println!("设置窗口置顶失败: {:?}", e);
                operations_successful = false;
            }
            // 立即取消置顶
            if let Err(e) = window.set_always_on_top(false) {
                println!("取消置顶窗口失败: {:?}", e);
                operations_successful = false;
            }
        }

        

        // 更新缓存状态
        if operations_successful {
            self.update_window_state(window_type, WindowState::VisibleFocused);
        }

        if operations_successful {
            WindowOperationResult::Shown
        } else {
            WindowOperationResult::Failed
        }
    }


    pub fn show_window(&self, window_type: WindowType, url: Option<&str>) -> WindowOperationResult {
        // TODO 添加防抖

        let current_state = self.get_cached_window_state(window_type);
                let result = match current_state {
            WindowState::NotExist => {
                match self.create_window_inner(window_type, url) {
                    Ok(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        WindowOperationResult::Created
                    }
                    Err(e) => {
                        println!("创建窗口失败: {:?}", e);
                        WindowOperationResult::Failed
                    }
                }
            }
            WindowState::VisibleFocused => {
                WindowOperationResult::NoAction
            }
            WindowState::Minimized | WindowState::Hidden => {
                if let Some(window) = self.get_window(window_type) {
                    self.activate_window(&window, window_type);
                    WindowOperationResult::Shown
                } else {
                    WindowOperationResult::Failed
                }
            }
        };

                // 更新缓存状态
        if matches!(
            result,
            WindowOperationResult::Created | WindowOperationResult::Shown
        ) {
            self.update_window_state(window_type, WindowState::VisibleFocused);
        }
        result
    }



    pub fn close_window(&self, window_type: WindowType) -> WindowOperationResult {
        let result = match self.get_window(window_type) {
            Some(window) => {
                let operation = window.close();
                match operation {
                    Ok(_) => {
                        println!("窗口已隐藏");
                        WindowOperationResult::Hidden
                    }
                    Err(e) => {
                        println!("隐藏窗口失败: {:?}", e);
                        WindowOperationResult::Failed
                    }
                }
            }
            None => {
                println!("窗口不存在，无需隐藏");
                WindowOperationResult::NoAction
            }
        };

        // 更新缓存状态
        self.update_window_state(window_type, WindowState::Hidden);

        result
    }

    pub fn destroy_window(&self, window_type: WindowType) -> bool {
        match self.get_window(window_type) {
            Some(window) => {
                if let Err(e) = window.destroy() {
                    println!("窗口销毁失败: {:?}", e);
                    return false;
                }
                self.update_window_state(window_type, WindowState::NotExist);
                true
            }
            None => {
                self.update_window_state(window_type, WindowState::NotExist);
                true
            }
        }
    }


    /// 切换窗口显示状态
    pub fn toggle_window(&self, window_type: WindowType) -> WindowOperationResult {
        // TODO 添加防抖


        let current_state = self.get_cached_window_state(window_type);
        // 更新托盘菜单状态
        let update_tray = |visible: bool| {
            if matches!(window_type, WindowType::Main) {
                update_menu_visible(visible);
                // tray::Tray::global().update_menu_visible(visible);
            }
        };

        let result = match current_state {
            WindowState::NotExist => {
                println!("窗口不存在，将创建新窗口");
                match self.create_window_inner(window_type, None) {
                    Ok(_) => {
                        update_tray(true);
                        WindowOperationResult::Created
                    }
                    Err(_) => WindowOperationResult::Failed,
                }
            }
            WindowState::VisibleFocused => {
                println!("窗口可见，将隐藏窗口");
                update_tray(false);
                self.close_window(window_type)
            }
            WindowState::Minimized | WindowState::Hidden => {
                println!("窗口存在但被隐藏或最小化，将激活窗口");
                if let Some(window) = self.get_window(window_type) {
                    update_tray(true);
                    self.activate_window(&window, window_type)
                } else {
                    println!("无法获取窗口实例");
                    WindowOperationResult::Failed
                }
            }
        };

        // 更新缓存状态（注意：hide_window已经处理了隐藏状态的更新）
        match result {
            WindowOperationResult::Created => {
                self.update_window_state(window_type, WindowState::VisibleFocused);
            }
            WindowOperationResult::Shown => {
                self.update_window_state(window_type, WindowState::VisibleFocused);
            }
            // Hidden状态已在hide_window中处理
            _ => {}
        }

        result
    }

    pub fn minimized_window(&self, window_type: WindowType) -> bool {
        match self.get_window(window_type) {
            Some(window) => {
                if window.is_minimized().unwrap_or(false) {
                    return true;
                } else {
                    if let Err(e) = window.minimize() {
                        println!("窗口最小化失败: {:?}", e);
                        return false;
                    }
                    self.update_window_state(window_type, WindowState::Minimized);
                    return true;
                }
            }
            None => return false,
        }
    }

    /// 检查是否所有窗口都已关闭（隐藏或不存在）
    pub fn are_all_windows_closed(&self) -> bool {
        for window_type in &WindowType::all() {
            let state = self.get_cached_window_state(*window_type);
            match state {
                WindowState::VisibleFocused | WindowState::Minimized => {
                    return false; // 有窗口仍然可见或最小化
                }
                WindowState::Hidden | WindowState::NotExist => {
                    // 窗口已隐藏或不存在，继续检查下一个
                }
            }
        }
        true // 所有窗口都已关闭
    }
}