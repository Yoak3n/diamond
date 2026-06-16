//! Hub process manager — starts/stops the Hub server as a Tauri sidecar.

use serde::{Deserialize, Serialize};
use log::{info, warn};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

// ─── Hub Status (from REST API) ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubStatus {
    pub running: bool,
    pub connected_clients: usize,
    pub stored_events: usize,
    pub current_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: u64,
    pub raw_json: String,
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub session_id: u64,
    pub role: String,
    pub remote_addr: String,
    pub connected_at: String,
    pub events_sent: u64,
    pub events_received: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListResponse {
    pub events: Vec<StoredEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientListResponse {
    pub clients: Vec<ClientInfo>,
}

// ─── Hub Manager ────────────────────────────────────────────────────────────

/// Manages the Hub server process via Tauri sidecar.
pub struct HubManager {
    port: u16,
    process_id: std::sync::Mutex<Option<u32>>,
}

impl HubManager {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            process_id: std::sync::Mutex::new(None),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}/hook", self.port)
    }

    pub fn view_url(&self) -> String {
        format!("ws://127.0.0.1:{}/view", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Start the Hub server via Tauri sidecar.
    pub async fn start(&self, app: &AppHandle) -> Result<(), String> {
        if self.is_running() {
            return Err("Hub is already running".into());
        }

        info!("Starting Hub sidecar on port {}", self.port);

        let sidecar = app
            .shell()
            .sidecar("hook-hub")
            .map_err(|e| format!("Failed to resolve sidecar: {}", e))?
            .args(["--port", &self.port.to_string()]);

        let (_rx, child) = sidecar
            .spawn()
            .map_err(|e| format!("Failed to spawn sidecar: {}", e))?;

        let pid = child.pid();
        *self.process_id.lock().map_err(|e| e.to_string())? = Some(pid);
        info!("Hub sidecar spawned (PID: {})", pid);

        // Wait for the port to open (fast TCP check, ~50ms intervals)
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if self.is_running() {
                info!("Hub is ready");
                return Ok(());
            }
        }

        warn!("Hub started but API not responding yet");
        Ok(())
    }

    /// Stop the Hub server.
    pub async fn stop(&self, _app: &AppHandle) -> Result<(), String> {
        let process_id = {
            let mut pid = self.process_id.lock().map_err(|e| e.to_string())?;
            let id = *pid;
            *pid = None;
            id
        };

        if let Some(pid) = process_id {
            info!("Stopping Hub sidecar (PID: {})", pid);
            kill_process(pid);
        } else {
            warn!("No PID tracked");
        }

        // Wait until port closes (fast TCP check, max ~1 second)
        for _ in 0..20 {
            if !port_open(self.port) {
                info!("Hub sidecar stopped");
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Last resort: kill by port
        warn!("Port still open after kill, trying port-based cleanup");
        self.kill_by_port();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        info!("Hub sidecar stopped");
        Ok(())
    }

    /// Kill any process listening on the Hub port.
    #[cfg(target_os = "windows")]
    fn kill_by_port(&self) {
        if let Ok(output) = std::process::Command::new("netstat").args(["-ano"]).output() {
            let output = String::from_utf8_lossy(&output.stdout);
            for line in output.lines() {
                if line.contains(&format!(":{}", self.port)) && line.contains("LISTENING") {
                    if let Some(pid_str) = line.split_whitespace().last() {
                        if let Ok(port_pid) = pid_str.parse::<u32>() {
                            let _ = std::process::Command::new("taskkill")
                                .args(["/F", "/T", "/PID", &port_pid.to_string()])
                                .output();
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn kill_by_port(&self) {
        // Use lsof or ss to find process on port
        if let Ok(output) = std::process::Command::new("lsof")
            .args(["-i", &format!(":{}", self.port), "-t"])
            .output()
        {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid in pids.lines() {
                if let Ok(port_pid) = pid.trim().parse::<u32>() {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &port_pid.to_string()])
                        .output();
                }
            }
        } else if let Ok(output) = std::process::Command::new("ss")
            .args(["-tlnp", &format!("sport = :{}", self.port)])
            .output()
        {
            let output = String::from_utf8_lossy(&output.stdout);
            for line in output.lines() {
                if let Some(pid_match) = line.split("pid=").nth(1) {
                    if let Some(pid_str) = pid_match.split(',').next() {
                        if let Ok(port_pid) = pid_str.trim().parse::<u32>() {
                            let _ = std::process::Command::new("kill")
                                .args(["-9", &port_pid.to_string()])
                                .output();
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn kill_by_port(&self) {
        // macOS uses lsof to find process on port
        if let Ok(output) = std::process::Command::new("lsof")
            .args(["-i", &format!(":{}", self.port), "-t"])
            .output()
        {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid in pids.lines() {
                if let Ok(port_pid) = pid.trim().parse::<u32>() {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &port_pid.to_string()])
                        .output();
                }
            }
        }
    }

    /// Check if the Hub is running (fast TCP port check).
    pub fn is_running(&self) -> bool {
        port_open(self.port)
    }

    /// Probe the Hub's REST API for a full status response.
    fn probe_api(&self) -> bool {
        if !port_open(self.port) {
            return false;
        }
        let url = format!("{}/api/status", self.base_url());
        match http_get(&url) {
            Ok(body) => serde_json::from_str::<HubStatus>(&body)
                .map(|s| s.running)
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    /// Query Hub status via REST API.
    pub async fn status(&self) -> HubStatus {
        let url = format!("{}/api/status", self.base_url());
        match http_get(&url) {
            Ok(body) => serde_json::from_str(&body).unwrap_or(HubStatus {
                running: false,
                connected_clients: 0,
                stored_events: 0,
                current_seq: 0,
            }),
            Err(_) => HubStatus {
                running: false,
                connected_clients: 0,
                stored_events: 0,
                current_seq: 0,
            },
        }
    }

    /// Query stored events via REST API.
    pub async fn events(&self, limit: usize, after_seq: Option<u64>) -> Vec<StoredEvent> {
        if !self.probe_api() {
            return vec![];
        }

        let mut url = format!("{}/api/events?limit={}", self.base_url(), limit);
        if let Some(after) = after_seq {
            url.push_str(&format!("&after_seq={}", after));
        }

        match http_get(&url) {
            Ok(body) => serde_json::from_str::<EventListResponse>(&body)
                .map(|r| r.events)
                .unwrap_or_default(),
            Err(_) => vec![],
        }
    }

    /// Query connected clients via REST API.
    pub async fn clients(&self) -> Vec<ClientInfo> {
        if !self.probe_api() {
            return vec![];
        }

        let url = format!("{}/api/clients", self.base_url());
        match http_get(&url) {
            Ok(body) => serde_json::from_str::<ClientListResponse>(&body)
                .map(|r| r.clients)
                .unwrap_or_default(),
            Err(_) => vec![],
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Fast TCP port check — returns true if the port is accepting connections.
/// Unlike curl, this fails instantly when the port is closed.
fn port_open(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        std::time::Duration::from_millis(200),
    )
    .is_ok()
}

/// Kill a process tree by PID.
#[cfg(target_os = "windows")]
fn kill_process(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
}

#[cfg(target_os = "linux")]
fn kill_process(pid: u32) {
    // Kill process group (negative PID) to include child processes
    let _ = std::process::Command::new("kill")
        .args(["-9", &format!("-{}", pid)])
        .output();
    // Also try pkill as fallback
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-P", &pid.to_string()])
        .output();
}

#[cfg(target_os = "macos")]
fn kill_process(pid: u32) {
    // Kill process group (negative PID) to include child processes
    let _ = std::process::Command::new("kill")
        .args(["-9", &format!("-{}", pid)])
        .output();
    // Also try pkill as fallback
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-P", &pid.to_string()])
        .output();
}

fn http_get(url: &str) -> Result<String, String> {
    let output = std::process::Command::new("curl")
        .args(["-s", "--max-time", "2", url])
        .output()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {}", e))
    } else {
        Err(format!("HTTP GET failed with status: {}", output.status))
    }
}
