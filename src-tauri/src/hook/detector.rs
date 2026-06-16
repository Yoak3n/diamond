//! Framework detector — scans the system for installed agent frameworks.
//!
//! Checks common installation paths, config files, and running processes
//! to discover which agent frameworks are available.

use std::path::{Path, PathBuf};
use serde::Serialize;
use log::debug;

// ─── Detected Framework ─────────────────────────────────────────────────────

/// A detected agent framework installation.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedFramework {
    /// Framework identifier (e.g. "hermes", "openclaw").
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Path to the framework's installation or config directory.
    pub install_path: PathBuf,

    /// Path to the framework's config file (if found).
    pub config_path: Option<PathBuf>,

    /// Whether the framework appears to be currently running.
    pub running: bool,

    /// Version string (if detectable).
    pub version: Option<String>,

    /// Detected Python executable path (for Python-based frameworks).
    pub python_path: Option<PathBuf>,

    /// Whether a hook is already registered.
    pub hook_registered: bool,
}

// ─── Detector ───────────────────────────────────────────────────────────────

/// Scan the system for known agent frameworks.
pub fn detect_all() -> Vec<DetectedFramework> {
    let mut results = Vec::new();

    if let Some(fw) = detect_hermes() {
        results.push(fw);
    }
    if let Some(fw) = detect_openclaw() {
        results.push(fw);
    }
    if let Some(fw) = detect_claude_code() {
        results.push(fw);
    }
    if let Some(fw) = detect_codex() {
        results.push(fw);
    }

    results
}

// ─── Hermes Detection ───────────────────────────────────────────────────────

fn detect_hermes() -> Option<DetectedFramework> {
    // Check multiple possible installation paths
    let candidates = find_hermes_installations();
    let hermes_dir = candidates.into_iter().find(|p| p.exists())?;

    debug!("Found Hermes at: {}", hermes_dir.display());

    let config_path = hermes_dir.join("config.yaml");
    let config_exists = config_path.exists();

    // Check for venv Python in multiple locations
    let python_path = find_hermes_python(&hermes_dir);
    let python = python_path.filter(|p| p.exists());

    let running = is_hermes_running(&hermes_dir);
    let hook_registered = check_hermes_hook(&hermes_dir);
    let version = read_hermes_version(&config_path);

    Some(DetectedFramework {
        id: "hermes".into(),
        name: "Hermes Agent Desktop".into(),
        install_path: hermes_dir,
        config_path: if config_exists { Some(config_path) } else { None },
        running,
        version,
        python_path: python,
        hook_registered,
    })
}

/// Find all possible Hermes installation directories.
fn find_hermes_installations() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. AppData/Local/hermes (Windows Desktop app — most common)
    if let Some(local) = dirs::data_local_dir() {
        paths.push(local.join("hermes"));
    }

    // 2. ~/.hermes/hermes-agent (CLI install)
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".hermes").join("hermes-agent"));
        paths.push(home.join(".hermes"));
    }

    // 3. XDG config
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("hermes"));
    }

    paths
}

/// Find the Python executable used by Hermes.
fn find_hermes_python(hermes_dir: &Path) -> Option<PathBuf> {
    let candidates = if cfg!(target_os = "windows") {
        vec![
            hermes_dir.join("venv/Scripts/python.exe"),
            hermes_dir.join("hermes-agent/venv/Scripts/python.exe"),
            hermes_dir.join("Scripts/python.exe"),
        ]
    } else {
        vec![
            hermes_dir.join("venv/bin/python3"),
            hermes_dir.join("hermes-agent/venv/bin/python3"),
            hermes_dir.join("bin/python3"),
        ]
    };

    candidates.into_iter().find(|p| p.exists())
}

/// Check if Hermes gateway is running.
fn is_hermes_running(hermes_dir: &Path) -> bool {
    // Check for gateway.pid file
    let pid_file = hermes_dir.join("gateway.pid");
    if pid_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            let pid: u32 = content.trim().parse().unwrap_or(0);
            if pid > 0 && is_pid_running(pid) {
                return true;
            }
        }
    }

    // Check for gateway_state.json
    let state_file = hermes_dir.join("gateway_state.json");
    if state_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&state_file) {
            if content.contains("\"running\":true") || content.contains("\"state\":\"running\"") {
                return true;
            }
        }
    }

    // Fallback: check for hermes process
    is_process_running("hermes") || is_process_running("hermes-gateway")
}

fn check_hermes_hook(hermes_dir: &Path) -> bool {
    // Check if our hook plugin is registered
    let hooks_dir = hermes_dir.join("hooks");
    if hooks_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&hooks_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.contains("hub") || name.contains("agent-hook") || name.contains("diamond") {
                    return true;
                }
            }
        }
    }

    // Check config.yaml for hook references
    let config_path = hermes_dir.join("config.yaml");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        content.contains("agent-hook") || content.contains("hub-bridge") || content.contains("diamond")
    } else {
        false
    }
}

fn read_hermes_version(config_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(config_path).ok()?;
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("version:") {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    // Try reading from hermes-agent directory
    None
}

// ─── OpenClaw Detection ─────────────────────────────────────────────────────

fn detect_openclaw() -> Option<DetectedFramework> {
    let home = dirs::home_dir()?;
    let openclaw_dir = home.join(".openclaw");
    if !openclaw_dir.exists() {
        return None;
    }

    let config_path = openclaw_dir.join("openclaw.json");
    let config_exists = config_path.exists();

    let running = is_process_running("openclaw");
    let hook_registered = openclaw_dir.join("plugins/hub-bridge").exists();

    let version = if config_exists {
        read_json_version(&config_path)
    } else {
        std::process::Command::new("openclaw")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.lines().next().map(|l| l.trim().to_string())
            })
    };

    Some(DetectedFramework {
        id: "openclaw".into(),
        name: "OpenClaw".into(),
        install_path: openclaw_dir,
        config_path: if config_exists { Some(config_path) } else { None },
        running,
        version,
        python_path: None,
        hook_registered,
    })
}

// ─── Claude Code Detection ──────────────────────────────────────────────────

fn detect_claude_code() -> Option<DetectedFramework> {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("where").arg("claude").output().ok()
    } else {
        std::process::Command::new("which").arg("claude").output().ok()
    };

    let claude_path = output.filter(|o| o.status.success()).and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout);
        s.lines().next().map(|l| PathBuf::from(l.trim()))
    })?;

    let running = is_process_running("claude");
    let version = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.lines().next().map(|l| l.trim().to_string())
        });

    let hook_registered = check_claude_code_hook();

    Some(DetectedFramework {
        id: "claude-code".into(),
        name: "Claude Code".into(),
        install_path: claude_path.parent().unwrap_or(&claude_path).to_path_buf(),
        config_path: None,
        running,
        version,
        python_path: None,
        hook_registered,
    })
}

/// Check if Claude Code hook is registered.
fn check_claude_code_hook() -> bool {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };

    // Check if hook script exists (ps1 on Windows, sh on Unix)
    let hook_dir = home.join(".diamond/hooks/claude-code");
    let script_exists = if cfg!(target_os = "windows") {
        hook_dir.join("hook.ps1").exists()
    } else {
        hook_dir.join("hook.sh").exists()
    };

    if !script_exists {
        return false;
    }

    // Check if .claude/settings.json contains our hook
    let settings_path = home.join(".claude/settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            return content.contains(".diamond") && content.contains("hooks");
        }
    }

    false
}

// ─── Codex Detection ────────────────────────────────────────────────────────

fn detect_codex() -> Option<DetectedFramework> {
    let output = if cfg!(target_os = "windows") {
        std::process::Command::new("where").arg("codex").output().ok()
    } else {
        std::process::Command::new("which").arg("codex").output().ok()
    };

    let codex_path = output.filter(|o| o.status.success()).and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout);
        s.lines().next().map(|l| PathBuf::from(l.trim()))
    })?;

    let running = is_process_running("codex");
    let version = std::process::Command::new("codex")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.lines().next().map(|l| l.trim().to_string())
        });

    let hook_registered = check_codex_hook();

    Some(DetectedFramework {
        id: "codex".into(),
        name: "Codex CLI".into(),
        install_path: codex_path.parent().unwrap_or(&codex_path).to_path_buf(),
        config_path: None,
        running,
        version,
        python_path: None,
        hook_registered,
    })
}

/// Check if Codex hook is registered.
fn check_codex_hook() -> bool {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };

    // Check if hook script exists
    let hook_script = home.join(".diamond/hooks/codex/hook.bat");
    if !hook_script.exists() {
        let hook_script_sh = home.join(".diamond/hooks/codex/hook.sh");
        if !hook_script_sh.exists() {
            return false;
        }
    }

    // Check settings file (Codex may use different config locations)
    let settings_paths = [
        home.join(".codex/settings.json"),
        home.join(".codex/config.json"),
    ];

    for settings_path in &settings_paths {
        if settings_path.exists() {
            if let Ok(content) = std::fs::read_to_string(settings_path) {
                if content.contains(".diamond") && content.contains("hooks") {
                    return true;
                }
            }
        }
    }

    false
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Check if a specific PID is running (Windows).
#[cfg(target_os = "windows")]
fn is_pid_running(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains(&pid.to_string())
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_pid_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_process_running(name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {}.exe", name), "/NH"])
            .output()
            .ok()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                s.contains(name)
            })
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("pgrep")
            .arg("-f")
            .arg(name)
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn read_json_version(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("version").and_then(|v| v.as_str()).map(|s| s.to_string())
}
