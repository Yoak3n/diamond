use super::super::window::config::WindowConfig;
use mouse_position::mouse_position::Mouse;
use tauri::AppHandle;

pub fn adjust_float_window_position(app_handle: &AppHandle, config: &WindowConfig) -> (f64, f64) {
    let postion = if let Mouse::Position { x, y } = Mouse::get_mouse_position() {
        let half_width = config.inner_size.0 / 2.0;
        let window_width = config.inner_size.0;
        let window_height = config.inner_size.1;

        // 获取屏幕尺寸进行边界检测 - 支持多屏幕
        let (screen_width, screen_height, screen_x, screen_y) =
            if let Ok(monitors) = app_handle.available_monitors() {
                // 查找鼠标所在的显示器
                let mouse_x = x as f64;
                let mouse_y = y as f64;

                let mut found_monitor = None;
                for monitor in monitors {
                    let pos = monitor.position();
                    let size = monitor.size();
                    let monitor_x = pos.x as f64;
                    let monitor_y = pos.y as f64;
                    let monitor_width = size.width as f64;
                    let monitor_height = size.height as f64;

                    // 检查鼠标是否在当前显示器范围内
                    if mouse_x >= monitor_x
                        && mouse_x < monitor_x + monitor_width
                        && mouse_y >= monitor_y
                        && mouse_y < monitor_y + monitor_height
                    {
                        found_monitor = Some((monitor_width, monitor_height, monitor_x, monitor_y));
                        break;
                    }
                }

                // 如果找到了匹配的显示器，使用它；否则使用主显示器
                if let Some(monitor_info) = found_monitor {
                    monitor_info
                } else if let Some(primary) = app_handle.primary_monitor().ok().flatten() {
                    let pos = primary.position();
                    let size = primary.size();
                    (
                        size.width as f64,
                        size.height as f64,
                        pos.x as f64,
                        pos.y as f64,
                    )
                } else {
                    // 如果无法获取屏幕尺寸，使用默认值
                    (1920.0, 1080.0, 0.0, 0.0)
                }
            } else {
                // 如果无法获取屏幕尺寸，使用默认值
                (1920.0, 1080.0, 0.0, 0.0)
            };

        let initial_x = x as f64 - half_width;
        let initial_y = y as f64 - 20.0;

        let margin = 10.0; // 保留10像素边距
        let adjusted_x = initial_x
            .max(screen_x + margin)
            .min(screen_x + screen_width - window_width - margin);
        let adjusted_y = initial_y
            .max(screen_y + margin)
            .min(screen_y + screen_height - window_height - margin);

        (adjusted_x, adjusted_y)
    } else {
        // 如果无法获取鼠标位置，使用默认值
        (0.0, 0.0)
    };

    postion
}