//! Hook manager — registers and manages hooks for detected frameworks.
//!
//! ## Hermes Desktop Hook Registration
//!
//! Hermes Desktop needs TWO hook systems to cover all modes:
//!
//! 1. **Gateway Event Hooks** (`~/.hermes/hooks/hub_bridge/`)
//!    - `HOOK.yaml` - declares events to listen for
//!    - `handler.py` - Python handler with `async def handle(event_type, context)`
//!    - Covers: Gateway mode (Telegram/Discord/Slack etc.)
//!
//! 2. **Plugin** (`~/.hermes/hermes-agent/plugins/hub-bridge/`)
//!    - `plugin.yaml` - plugin manifest with hooks list
//!    - `__init__.py` - Python module with `register(ctx)` function
//!    - Covers: CLI/TUI mode (desktop direct chat)
//!
//! Both must be created AND the plugin must be enabled in `config.yaml`.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use log::{error, info, warn};
use agent_hook::adapter::hermes_hooks;

use super::detector::DetectedFramework;

// ─── Registration Result ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResult {
    pub framework_id: String,
    pub success: bool,
    pub message: String,
    pub hook_path: Option<PathBuf>,
    /// Whether this registration requires the user to restart the application.
    pub requires_restart: bool,
    /// The name of the application/target that needs to be restarted.
    /// Only meaningful when `requires_restart` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_target: Option<String>,
}

// ─── Register Hook ──────────────────────────────────────────────────────────

/// Register a hook for the given framework to send events to the Hub.
pub fn register_hook(framework: &DetectedFramework, hub_url: &str) -> RegistrationResult {
    let result = match framework.id.as_str() {
        "hermes" => register_hermes_hook(framework, hub_url),
        "openclaw" => register_openclaw_hook(framework, hub_url),
        "claude-code" | "codex" => register_cli_hook(framework, hub_url),
        _ => Err(format!("Unknown framework: {}", framework.id)),
    };

    match result {
        Ok(path) => {
            info!("Hook registered for {} at {}", framework.id, path.display());
            
            // Determine if restart is required and for which target
            let (requires_restart, restart_target) = match framework.id.as_str() {
                "hermes" => {
                    // Hermes Desktop needs restart to load new plugin
                    // Gateway hooks are loaded at startup
                    (true, Some("Hermes Desktop".to_string()))
                }
                "openclaw" => {
                    // OpenClaw plugins are loaded at startup
                    (true, Some("OpenClaw".to_string()))
                }
                "claude-code" | "codex" => {
                    // CLI tools use wrapper scripts, no restart needed
                    (false, None)
                }
                _ => (false, None),
            };
            
            RegistrationResult {
                framework_id: framework.id.clone(),
                success: true,
                message: format!("Hook registered at {}", path.display()),
                hook_path: Some(path),
                requires_restart,
                restart_target,
            }
        }
        Err(e) => {
            error!("Failed to register hook for {}: {}", framework.id, e);
            RegistrationResult {
                framework_id: framework.id.clone(),
                success: false,
                message: e,
                hook_path: None,
                requires_restart: false,
                restart_target: None,
            }
        }
    }
}

/// Remove a hook for the given framework.
pub fn unregister_hook(framework: &DetectedFramework) -> RegistrationResult {
    let result = match framework.id.as_str() {
        "hermes" => unregister_hermes_hook(framework),
        "openclaw" => unregister_openclaw_hook(framework),
        "claude-code" | "codex" => unregister_cli_hook(framework),
        _ => Err(format!("Unknown framework: {}", framework.id)),
    };

    match result {
        Ok(()) => {
            info!("Hook unregistered for {}", framework.id);
            
            // Unregistering also requires restart for the same frameworks
            let (requires_restart, restart_target) = match framework.id.as_str() {
                "hermes" => (true, Some("Hermes Desktop".to_string())),
                "openclaw" => (true, Some("OpenClaw".to_string())),
                _ => (false, None),
            };
            
            RegistrationResult {
                framework_id: framework.id.clone(),
                success: true,
                message: "Hook removed".into(),
                hook_path: None,
                requires_restart,
                restart_target,
            }
        }
        Err(e) => {
            error!("Failed to unregister hook for {}: {}", framework.id, e);
            RegistrationResult {
                framework_id: framework.id.clone(),
                success: false,
                message: e,
                hook_path: None,
                requires_restart: false,
                restart_target: None,
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  HERMES DESKTOP HOOK REGISTRATION
// ═══════════════════════════════════════════════════════════════════════════

/// Register both Gateway Event Hook AND Plugin for Hermes Desktop.
fn register_hermes_hook(framework: &DetectedFramework, hub_url: &str) -> Result<PathBuf, String> {
    let result = hermes_hooks::install_hermes_hooks(&framework.install_path, hub_url)
        .map_err(|e| format!("Failed to install Hermes hooks: {}", e))?;

    // Log warnings if any
    for warning in &result.warnings {
        warn!("Hermes hook installation warning: {}", warning);
    }

    info!(
        "Hermes hooks registered:\n  Gateway: {:?}\n  Plugin:  {:?}\n  Config updated: {}",
        result.gateway_hook_dir, result.plugin_dir, result.config_updated
    );

    let hook_dir = result
        .gateway_hook_dir
        .or(result.plugin_dir)
        .ok_or_else(|| "No hook directory created".to_string())?;

    Ok(hook_dir)
}

fn unregister_hermes_hook(framework: &DetectedFramework) -> Result<(), String> {
    let removed = hermes_hooks::uninstall_hermes_hooks(&framework.install_path)
        .map_err(|e| format!("Failed to uninstall Hermes hooks: {}", e))?;

    for item in &removed {
        info!("Removed: {}", item);
    }

    Ok(())
}

// Old template code removed - now using agent_hook::adapter::hermes_hooks

// ═══════════════════════════════════════════════════════════════════════════
//  OPENCLAW HOOK REGISTRATION
// ═══════════════════════════════════════════════════════════════════════════

fn register_openclaw_hook(framework: &DetectedFramework, hub_url: &str) -> Result<PathBuf, String> {
    let plugin_dir = framework.install_path.join("plugins").join("hub-bridge");
    std::fs::create_dir_all(&plugin_dir)
        .map_err(|e| format!("Failed to create plugin dir: {}", e))?;

    let mut adapter = agent_hook::adapter::OpenClawAdapter::new(hub_url);
    let source = adapter.generate_plugin();

    std::fs::write(plugin_dir.join("index.ts"), source)
        .map_err(|e| format!("Failed to write plugin: {}", e))?;

    let manifest = serde_json::json!({
        "name": "hub-bridge",
        "version": "0.1.0",
        "description": "Bridges OpenClaw events to a WS Hub",
        "entry": "index.ts",
        "enabled": true
    });
    std::fs::write(
        plugin_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .map_err(|e| format!("Failed to write manifest: {}", e))?;

    Ok(plugin_dir)
}

fn unregister_openclaw_hook(framework: &DetectedFramework) -> Result<(), String> {
    let plugin_dir = framework.install_path.join("plugins").join("hub-bridge");
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir)
            .map_err(|e| format!("Failed to remove plugin: {}", e))?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
//  CLI HOOK REGISTRATION (Claude Code / Codex)
// ═══════════════════════════════════════════════════════════════════════════

/// Register hooks for Claude Code via `.claude/settings.json`.
///
/// Claude Code has a native hook system with events like:
/// - SessionStart, PreToolUse, PostToolUse, Stop, etc.
///
/// We create a hook script and register it in the settings file.
fn register_cli_hook(framework: &DetectedFramework, hub_url: &str) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    
    // 1. Create hook script directory
    let hook_script_dir = home.join(".diamond").join("hooks").join(&framework.id);
    std::fs::create_dir_all(&hook_script_dir)
        .map_err(|e| format!("Failed to create hooks dir: {}", e))?;
    
    // 2. Create the hook script that sends events to Hub
    let hook_script = create_cli_hook_script(&hook_script_dir, hub_url)?;
    
    // 3. Update .claude/settings.json
    let settings_path = home.join(".claude").join("settings.json");
    update_claude_settings(&settings_path, &hook_script, hub_url)?;
    
    Ok(hook_script)
}

/// Create the hook script that sends events to the Hub.
fn create_cli_hook_script(dir: &std::path::Path, hub_url: &str) -> Result<std::path::PathBuf, String> {
    let http_url = hub_url.replace("ws://", "http://").replace("/hook", "");

    let (script_path, script_content) = if cfg!(target_os = "windows") {
        // Use PowerShell on Windows — bat's `for /f ... ('more')` hangs on piped stdin
        (
            dir.join("hook.ps1"),
            format!(
                r#"# Diamond hook for Claude Code
# Sends events to Hub at {http_url}
# Event type is passed as first argument, JSON input comes from stdin

param(
    [Parameter(Position=0, Mandatory=$true)]
    [string]$EventType
)

$inputText = [Console]::In.ReadToEnd().Trim()
if ([string]::IsNullOrEmpty($inputText)) {{ $inputText = "{{}}" }}

try {{
    $data = $inputText | ConvertFrom-Json
    $body = @{{
        event = $EventType
        framework = "claude-code"
        data = $data
    }} | ConvertTo-Json -Depth 10 -Compress

    Invoke-RestMethod -Uri "{http_url}/api/emit" -Method Post -ContentType "application/json" -Body $body -ErrorAction SilentlyContinue | Out-Null
}} catch {{}}
"#
            )
        )
    } else {
        (
            dir.join("hook.sh"),
            format!(
                r#"#!/bin/bash
# Diamond hook for Claude Code
# Sends events to Hub at {http_url}
# Event type is passed as first argument, JSON input comes from stdin

EVENT_TYPE="$1"
INPUT=$(cat)

curl -s -X POST "{http_url}/api/emit" \
  -H "Content-Type: application/json" \
  -d "{{\"event\": \"$EVENT_TYPE\", \"framework\": \"claude-code\", \"data\": $INPUT}}" \
  > /dev/null 2>&1
"#
            )
        )
    };

    std::fs::write(&script_path, &script_content)
        .map_err(|e| format!("Failed to write hook script: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
    }

    Ok(script_path)
}

/// Update `.claude/settings.json` to register hooks.
fn update_claude_settings(settings_path: &std::path::Path, hook_script: &std::path::Path, _hub_url: &str) -> Result<(), String> {
    // Read existing settings or create new
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings: {}", e))?
    } else {
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .claude dir: {}", e))?;
        }
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .ok_or("Settings is not a JSON object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or("hooks is not a JSON object")?;

    let script_str = hook_script.to_string_lossy().to_string();

    // On Windows, use powershell.exe as the command with the ps1 script as an arg
    // On Unix, use the script directly
    let make_hook_entry = |event: &str| -> serde_json::Value {
        if cfg!(target_os = "windows") {
            serde_json::json!({
                "hooks": [{
                    "type": "command",
                    "command": "powershell.exe",
                    "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &script_str, event],
                    "timeout": 5
                }]
            })
        } else {
            serde_json::json!({
                "hooks": [{
                    "type": "command",
                    "command": &script_str,
                    "args": [event],
                    "timeout": 5
                }]
            })
        }
    };

    // The identifier used to detect if our hook is already registered
    let hook_id = if cfg!(target_os = "windows") { "hook.ps1" } else { "hook.sh" };

    let events_to_hook = vec!["SessionStart", "PreToolUse", "PostToolUse", "Stop"];

    for event in events_to_hook {
        let entry = hooks
            .entry(event)
            .or_insert_with(|| serde_json::json!([]));

        let already_exists = entry.as_array()
            .map(|arr| arr.iter().any(|h| {
                h.get("hooks")
                    .and_then(|hooks| hooks.as_array())
                    .map(|hooks| hooks.iter().any(|h| {
                        h.get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c.contains(hook_id))
                            .unwrap_or(false)
                    }))
                    .unwrap_or(false)
            }))
            .unwrap_or(false);

        if !already_exists {
            if let Some(arr) = entry.as_array_mut() {
                arr.push(make_hook_entry(event));
            } else {
                *entry = serde_json::json!([make_hook_entry(event)]);
            }
        }
    }

    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    std::fs::write(settings_path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    info!("Updated Claude Code settings at {}", settings_path.display());
    Ok(())
}

/// Unregister hooks for Claude Code.
fn unregister_cli_hook(framework: &DetectedFramework) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    
    // Remove hook script directory
    let hook_script_dir = home.join(".diamond").join("hooks").join(&framework.id);
    if hook_script_dir.exists() {
        std::fs::remove_dir_all(&hook_script_dir)
            .map_err(|e| format!("Failed to remove hooks dir: {}", e))?;
    }
    
    // Remove hooks from .claude/settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse settings: {}", e))?;
        
        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            let script_pattern = format!(".diamond/hooks/{}", framework.id);
            
            // Remove hooks from each event
            for event in ["SessionStart", "PreToolUse", "PostToolUse", "Stop"] {
                if let Some(entry) = hooks.get_mut(event) {
                    if let Some(arr) = entry.as_array_mut() {
                        arr.retain(|h| {
                            !h.get("hooks")
                                .and_then(|hooks| hooks.as_array())
                                .map(|hooks| hooks.iter().any(|h| {
                                    h.get("command")
                                        .and_then(|c| c.as_str())
                                        .map(|c| c.contains(&script_pattern))
                                        .unwrap_or(false)
                                }))
                                .unwrap_or(false)
                        });
                        
                        // Remove empty arrays
                        if arr.is_empty() {
                            hooks.remove(event);
                        }
                    }
                }
            }
        }
        
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        std::fs::write(&settings_path, content)
            .map_err(|e| format!("Failed to write settings: {}", e))?;
    }
    
    Ok(())
}
