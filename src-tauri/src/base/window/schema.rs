#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowState {
    /// 窗口可见且有焦点
    VisibleFocused,
    /// 窗口可见但无焦点
    // VisibleUnfocused,
    /// 窗口最小化
    Minimized,
    /// 窗口隐藏
    Hidden,
    /// 窗口不存在
    NotExist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowType {
    Main,
}

impl WindowType {
    /// 获取所有窗口类型
    pub fn all() -> [Self; 1] {
        [WindowType::Main]
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "main" => Some(WindowType::Main),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            WindowType::Main => "main",
        }
    }

    pub fn url(&self) -> &'static str {
        match self {
            WindowType::Main => "/",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            WindowType::Main => "Tauri App",
        }
    }
}


impl From<WindowType> for String {
    fn from(window_type: WindowType) -> Self {
        window_type.label().to_string()
    } 
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowOperationResult {
    /// 窗口已显示并获得焦点
    Shown,
    /// 窗口已隐藏
    Hidden,
    /// 创建了新窗口
    Created,
    /// 操作失败
    Failed,
    /// 无需操作
    NoAction,
}