use super::schema::WindowType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    // title should change with window type, so there is no need to set it in config
    pub window_type: WindowType,
    pub inner_size: (f64, f64),
    pub min_inner_size: (f64, f64),
    pub decorations: bool,
    pub transparent: bool,
    pub skip_taskbar: bool,
    pub shadow: bool,
    pub always_on_top: bool,
    pub maximizable: bool,
    pub focused: bool,
    pub center: bool,
    pub float: bool,
}

impl WindowConfig {
    pub fn new(window_type: WindowType) -> Self {
        match window_type {
            WindowType::Main => Self {
                window_type,
                inner_size: (800.0, 600.0),
                min_inner_size: (400.0, 80.0),
                decorations: true,
                transparent: false,
                skip_taskbar: false,
                shadow: false,
                always_on_top: false,
                maximizable: true,
                focused: true,
                center: true,
                float: false,
            },
        }
    }
    pub fn default_config() -> HashMap<WindowType, WindowConfig> {
        HashMap::from([(
            WindowType::Main,
            WindowConfig::new(WindowType::Main),
        )])
    }
}