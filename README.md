# Tauri + Vue + TypeScript 模板

基于 Tauri 2.x + Vue 3 + TypeScript 的桌面应用开发模板，内置了窗口管理、系统托盘、轻量模式等常用功能。

## 技术栈

- **前端**: Vue 3 + Vite + TypeScript
- **后端**: Tauri 2.x + Rust
- **包管理**: pnpm (Workspace 模式)

## 自带功能特性

### 窗口管理系统

完整的窗口生命周期管理，支持状态追踪和多窗口扩展。

**窗口状态** (`src-tauri/src/base/window/schema.rs`):
- `VisibleFocused` - 可见且有焦点
- `Minimized` - 最小化
- `Hidden` - 隐藏
- `NotExist` - 不存在

**核心操作** (`src-tauri/src/base/window/manager.rs`):
- `show_window()` - 显示窗口（自动处理创建/激活/恢复）
- `close_window()` - 关闭窗口（实际隐藏到托盘）
- `toggle_window()` - 切换窗口显示状态
- `destroy_window()` - 销毁窗口释放资源

**窗口配置** (`src-tauri/src/base/window/config.rs`):
```rust
// 可配置项
inner_size: (800.0, 600.0),      // 窗口大小
min_inner_size: (400.0, 80.0),   // 最小大小
decorations: true,                // 是否显示标题栏
transparent: false,               // 是否透明
always_on_top: false,             // 是否置顶
skip_taskbar: false,              // 是否隐藏任务栏图标
float: false,                     // 是否启用浮动定位
```

**浮动窗口定位** (`src-tauri/src/base/window/position.rs`):
- 基于鼠标位置自动定位
- 多显示器支持
- 自动边界检测

### 系统托盘

预配置的系统托盘功能 (`src-tauri/src/base/tray.rs`):
- **Show/Hide** - 显示/隐藏主窗口
- **AutoStart** - 开机自启开关（仅桌面端）
- **Quit** - 退出应用
- 左键点击托盘图标切换窗口显示

### 轻量模式

自动资源管理机制 (`src-tauri/src/base/lightweight.rs`):
- 所有窗口关闭后启动 10 分钟计时器
- 超时后自动销毁所有窗口，进入轻量模式
- 窗口获得焦点时自动取消计时器
- 最大程度节省系统资源

### 开机自启

集成 `tauri-plugin-autostart` 插件（仅桌面端）:
- 通过托盘菜单切换
- macOS 使用 LaunchAgent 方式

### 定时任务系统

基于 `delay_timer` 的任务调度 (`src-tauri/src/base/timer.rs`):
- 全局单例 Timer
- 原子操作保证线程安全
- 每分钟自动刷新任务

### 全局状态管理

- `Handle` - 全局 AppHandle 单例 (`src-tauri/src/base/handle.rs`)
- `AppState` - 应用状态管理 (`src-tauri/src/base/state.rs`)
- 使用 `parking_lot::Mutex` 和 `once_cell::OnceCell` 保证线程安全

## 项目结构

```
├── crates/
│   └── addons/          # Rust 扩展 crate
├── src/                 # Vue 前端代码
│   ├── App.vue
│   └── main.ts
├── src-tauri/           # Tauri 后端代码
│   ├── capabilities/    # 权限配置
│   ├── icons/           # 应用图标
│   └── src/
│       ├── base/
│       │   ├── window/  # 窗口管理模块
│       │   ├── cmd.rs   # Tauri 命令定义
│       │   ├── handle.rs
│       │   ├── init.rs  # 初始化配置
│       │   ├── lightweight.rs
│       │   ├── state.rs
│       │   ├── timer.rs
│       │   └── tray.rs
│       ├── lib.rs
│       └── main.rs
├── Cargo.toml           # Rust Workspace 配置
├── package.json
└── vite.config.ts
```

## 需要配置的地方

### 1. 应用标识符

修改 `src-tauri/tauri.conf.json`:
```json
{
  "identifier": "com.your-company.your-app"
}
```

### 2. 窗口配置

修改 `src-tauri/src/base/window/config.rs` 中的 `WindowConfig::new()`:
```rust
WindowType::Main => Self {
    inner_size: (1024.0, 768.0),  // 调整默认窗口大小
    decorations: true,             // false 可隐藏标题栏
    // ... 其他配置
}
```

### 3. 添加新窗口类型

在 `src-tauri/src/base/window/schema.rs` 中扩展 `WindowType` 枚举:
```rust
pub enum WindowType {
    Main,
    Settings,  // 新增
}
```

然后在 `config.rs` 中添加对应配置。

### 4. Tauri 命令

在 `src-tauri/src/base/cmd.rs` 中添加新命令:
```rust
#[tauri::command]
pub fn your_command(param: &str) -> String {
    // 实现
}
```

并在 `init.rs` 的 `generate_handlers()` 中注册。

### 5. 轻量模式超时时间

修改 `src-tauri/src/base/lightweight.rs`:
```rust
.set_frequency_once_by_minutes(10)  // 修改超时分钟数
```

### 6. 权限配置

修改 `src-tauri/capabilities/default.json` 添加所需权限:
```json
{
  "permissions": [
    "core:default",
    "opener:default",
    "fs:default"  // 示例：添加文件系统权限
  ]
}
```

### 7. 应用图标

替换 `src-tauri/icons/` 目录下的图标文件。

### 8. 构建优化

`Cargo.toml` 已配置 Release 优化:
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

## 开发指南

### 环境要求

- Node.js >= 18
- pnpm
- Rust 工具链
- Tauri CLI (已包含在 devDependencies)

### 安装依赖

```bash
pnpm install
```

### 开发运行

```bash
pnpm tauri dev
```

### 构建打包

```bash
pnpm tauri build
```

## 推荐 IDE 配置

- [VS Code](https://code.visualstudio.com/)
- [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
