//! Python environment discovery — find the Python instance that an agent framework uses.
//!
//! Instead of requiring a system Python in PATH, this module discovers
//! the actual Python executable/library used by the target agent framework
//! and configures PyO3 accordingly.

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

// ─── Python Environment ─────────────────────────────────────────────────────

/// Discovered Python environment details.
#[derive(Debug, Clone)]
pub struct PythonEnv {
    /// Path to the Python executable (e.g. `python.exe`, `python3`).
    pub executable: PathBuf,

    /// Path to the Python shared library (e.g. `python311.dll`, `libpython3.11.so`).
    /// `None` if not found (will try to derive from executable).
    pub library: Option<PathBuf>,

    /// Python version string (e.g. `"3.11.15"`).
    pub version: Option<String>,

    /// Path to the site-packages or venv.
    pub site_packages: Option<PathBuf>,

    /// How this environment was discovered.
    pub source: DiscoverySource,
}

/// How the Python environment was found.
#[derive(Debug, Clone)]
pub enum DiscoverySource {
    /// Agent framework is a Python process — extracted from /proc or WMI.
    AgentProcess { pid: u32 },

    /// Found via virtualenv marker (.venv, venv, .python-version).
    VirtualEnv { path: PathBuf },

    /// Found via Windows registry.
    Registry,

    /// Found via `which python3` / `where python`.
    Which,

    /// Explicitly configured by the user.
    Explicit(PathBuf),

    /// Fallback: system default.
    SystemDefault,
}

impl PythonEnv {
    /// Discover the Python environment used by an agent framework.
    ///
    /// Strategy:
    /// 1. If framework process PID is known, extract its Python path
    /// 2. Look for virtualenv markers in the project directory
    /// 3. Check Windows registry
    /// 4. Fall back to `which python3`
    pub fn discover(
        framework: &str,
        project_dir: Option<&Path>,
        agent_pid: Option<u32>,
    ) -> Option<Self> {
        // 1. Try extracting from agent process
        if let Some(pid) = agent_pid {
            if let Some(env) = Self::from_agent_process(pid) {
                info!(pid, python = %env.executable.display(), "Found Python from agent process");
                return Some(env);
            }
        }

        // 2. Try project directory venv
        if let Some(dir) = project_dir {
            if let Some(env) = Self::from_venv(dir) {
                info!(python = %env.executable.display(), "Found Python from project venv");
                return Some(env);
            }
        }

        // 3. Try framework-specific discovery
        match framework {
            "hermes" => {
                if let Some(env) = Self::discover_hermes() {
                    info!(python = %env.executable.display(), "Found Hermes Python environment");
                    return Some(env);
                }
            }
            "langchain" => {
                if let Some(env) = Self::discover_from_running_process("langchain") {
                    info!(python = %env.executable.display(), "Found LangChain Python environment");
                    return Some(env);
                }
            }
            _ => {}
        }

        // 4. Try Windows registry
        if let Some(env) = Self::from_registry() {
            info!(python = %env.executable.display(), "Found Python from registry");
            return Some(env);
        }

        // 5. Try `where python` / `which python3`
        if let Some(env) = Self::from_which() {
            info!(python = %env.executable.display(), "Found Python from PATH");
            return Some(env);
        }

        warn!(framework, "No Python environment found");
        None
    }

    /// Configure PyO3 to use this discovered environment.
    ///
    /// Call this BEFORE any `Python::with_gil()` or `pyo3::prepare_freethreaded_python()`.
    pub fn configure_pyo3(&self) {
        // Set PYTHONHOME so PyO3 finds the right Python
        if let Some(parent) = self.executable.parent() {
            std::env::set_var("PYTHONHOME", parent);
            debug!(pythonhome = %parent.display(), "Set PYTHONHOME");
        }

        // On Windows, ensure the DLL directory is in PATH
        if cfg!(target_os = "windows") {
            if let Some(dll_dir) = self.dll_directory() {
                let current_path = std::env::var("PATH").unwrap_or_default();
                let new_path = format!("{};{}", dll_dir.display(), current_path);
                std::env::set_var("PATH", &new_path);
                debug!(dll_dir = %dll_dir.display(), "Added DLL directory to PATH");
            }
        }
    }

    /// Get the directory containing the Python DLL (Windows).
    fn dll_directory(&self) -> Option<PathBuf> {
        // Check if library path is explicitly known
        if let Some(ref lib) = self.library {
            return lib.parent().map(|p| p.to_path_buf());
        }

        // Derive from executable: python.exe → python311.dll is in the same dir
        let exe_dir = self.executable.parent()?;
        let dll_names = self
            .version
            .as_ref()
            .map(|v| {
                let major_minor = v.split('.').take(2).collect::<Vec<_>>().join("");
                vec![
                    format!("python{}.dll", major_minor), // python311.dll
                    format!("python{}.dll", v.split('.').next().unwrap_or("3")), // python3.dll
                ]
            })
            .unwrap_or_else(|| {
                vec![
                    "python3.dll".into(),
                    "python311.dll".into(),
                    "python312.dll".into(),
                ]
            });

        for name in &dll_names {
            let path = exe_dir.join(name);
            if path.exists() {
                return Some(exe_dir.to_path_buf());
            }
        }

        None
    }

    // ── Discovery Strategies ──────────────────────────────────────────────

    /// Discover from a running agent process (Windows: WMI / /proc: cmdline).
    fn from_agent_process(pid: u32) -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            Self::from_windows_process(pid)
        }
        #[cfg(target_os = "linux")]
        {
            Self::from_procfs(pid)
        }
        #[cfg(target_os = "macos")]
        {
            // macOS: use `ps` command
            let output = std::process::Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "command="])
                .output()
                .ok()?;
            let cmdline = String::from_utf8_lossy(&output.stdout);
            Self::parse_python_from_cmdline(&cmdline, pid)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            None
        }
    }

    /// Windows: get process command line via WMI.
    #[cfg(target_os = "windows")]
    fn from_windows_process(pid: u32) -> Option<Self> {
        // Use `wmic` to get process command line
        let output = std::process::Command::new("wmic")
            .args([
                "process",
                "where",
                &format!("ProcessId={}", pid),
                "get",
                "CommandLine,ExecutablePath",
                "/format:list",
            ])
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout);
        Self::parse_windows_process_output(&text, pid)
    }

    /// Parse Windows WMI output to extract Python path.
    #[cfg(target_os = "windows")]
    fn parse_windows_process_output(text: &str, pid: u32) -> Option<Self> {
        let mut exe_path = None;
        let mut cmdline = String::new();

        for line in text.lines() {
            let line = line.trim();
            if let Some(path) = line.strip_prefix("CommandLine=") {
                cmdline = path.to_string();
            }
            if let Some(path) = line.strip_prefix("ExecutablePath=") {
                exe_path = Some(PathBuf::from(path));
            }
        }

        // Check if the executable is Python
        if let Some(ref exe) = exe_path {
            let exe_name = exe.file_name()?.to_string_lossy().to_lowercase();
            if exe_name.contains("python") {
                return Some(Self {
                    executable: exe.clone(),
                    library: None,
                    version: Self::extract_version_from_path(exe),
                    site_packages: None,
                    source: DiscoverySource::AgentProcess { pid },
                });
            }
        }

        // Check if command line contains python
        if cmdline.to_lowercase().contains("python") {
            let python_exe = Self::extract_python_from_cmdline(&cmdline)?;
            return Some(Self {
                executable: python_exe,
                library: None,
                version: None,
                site_packages: None,
                source: DiscoverySource::AgentProcess { pid },
            });
        }

        None
    }

    /// Linux: read /proc/[pid]/cmdline.
    #[cfg(target_os = "linux")]
    fn from_procfs(pid: u32) -> Option<Self> {
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        let cmdline = std::fs::read_to_string(&cmdline_path).ok()?;
        Self::parse_python_from_cmdline(&cmdline, pid)
    }

    /// Parse a command line string to find Python executable.
    #[cfg(target_os = "linux")]
    fn parse_python_from_cmdline(cmdline: &str, pid: u32) -> Option<Self> {
        // Look for python/python3/python3.11 in the command line
        let parts: Vec<&str> = cmdline.split_whitespace().collect();
        for part in &parts {
            let lower = part.to_lowercase();
            if lower.contains("python") && !lower.contains("--") {
                let path = PathBuf::from(part);
                if path.exists() {
                    return Some(Self {
                        executable: path.clone(),
                        library: None,
                        version: Self::extract_version_from_path(&path),
                        site_packages: None,
                        source: DiscoverySource::AgentProcess { pid },
                    });
                }
            }
        }
        None
    }

    /// Extract Python version from a path like `python3.11` or `python311.dll`.
    fn extract_version_from_path(path: &Path) -> Option<String> {
        let name = path.file_stem()?.to_string_lossy();
        // Match patterns like "python3.11", "python311", "python3"
        let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 2 {
            let major = digits.chars().next()?;
            let minor = &digits[1..];
            Some(format!("{}.{}", major, minor))
        } else if !digits.is_empty() {
            Some(format!("{}.0", digits))
        } else {
            None
        }
    }

    /// Extract Python executable path from a command line string.
    fn extract_python_from_cmdline(cmdline: &str) -> Option<PathBuf> {
        // Find the first argument that looks like a Python executable
        let parts: Vec<&str> = cmdline.split_whitespace().collect();
        for part in &parts {
            let lower = part.to_lowercase().replace('\\', "/");
            if lower.contains("python") && !lower.starts_with('-') {
                return Some(PathBuf::from(part));
            }
        }
        None
    }

    /// Discover Hermes-specific Python environment.
    ///
    /// Hermes uses a venv at `~/.hermes/hermes-agent/venv/`.
    fn discover_hermes() -> Option<Self> {
        let home = dirs_next::home_dir()?;
        let venv_python = if cfg!(target_os = "windows") {
            home.join(".hermes/hermes-agent/venv/Scripts/python.exe")
        } else {
            home.join(".hermes/hermes-agent/venv/bin/python3")
        };

        if venv_python.exists() {
            let site_packages = if cfg!(target_os = "windows") {
                venv_python
                    .parent()?
                    .join("../Lib/site-packages")
            } else {
                venv_python
                    .parent()?
                    .join("../lib/python3.11/site-packages")
            };

            Some(Self {
                executable: venv_python,
                library: None,
                version: Some("3.11".into()),
                site_packages: if site_packages.exists() {
                    Some(site_packages)
                } else {
                    None
                },
                source: DiscoverySource::Explicit(PathBuf::from("hermes-venv")),
            })
        } else {
            None
        }
    }

    /// Try to find Python from a running process by name.
    fn discover_from_running_process(process_name: &str) -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("wmic")
                .args([
                    "process",
                    "where",
                    &format!("Name like '%{}%'", process_name),
                    "get",
                    "ProcessId,ExecutablePath,CommandLine",
                    "/format:list",
                ])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);

            // Find Python in the command lines
            for block in text.split("\r\n\r\n") {
                if block.to_lowercase().contains("python") {
                    return Self::parse_windows_process_output(block, 0);
                }
            }
        }
        None
    }

    /// Discover from Windows registry.
    fn from_registry() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            use winreg::enums::*;
            use winreg::RegKey;

            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let python_core = hklm
                .open_subkey("SOFTWARE\\Python\\PythonCore")
                .ok()?;

            // Iterate installed versions
            for entry in python_core.enum_keys().filter_map(|k| k.ok()) {
                let version_key = python_core.open_subkey(&entry).ok()?;
                let install_path = version_key
                    .open_subkey("InstallPath")
                    .ok()?;

                let exe: String = install_path.get_value("ExecutabledPath").ok()?;
                let exe_path = PathBuf::from(exe);

                if exe_path.exists() {
                    return Some(Self {
                        executable: exe_path,
                        library: None,
                        version: Some(entry.to_string()),
                        site_packages: None,
                        source: DiscoverySource::Registry,
                    });
                }
            }
        }
        None
    }

    /// Discover from `where python` / `which python3`.
    fn from_which() -> Option<Self> {
        let cmd = if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        };
        let arg = if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3"
        };

        let output = std::process::Command::new(cmd)
            .arg(arg)
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next()?.trim();
        let path = PathBuf::from(first_line);

        if path.exists() {
            return Some(Self {
                executable: path.clone(),
                library: None,
                version: Self::extract_version_from_path(&path),
                site_packages: None,
                source: DiscoverySource::Which,
            });
        }
        None
    }

    /// Discover from virtualenv markers in a directory.
    fn from_venv(project_dir: &Path) -> Option<Self> {
        // Check common venv locations
        let candidates = if cfg!(target_os = "windows") {
            vec![
                project_dir.join(".venv/Scripts/python.exe"),
                project_dir.join("venv/Scripts/python.exe"),
                project_dir.join(".venv/bin/python3"),
                project_dir.join("venv/bin/python3"),
            ]
        } else {
            vec![
                project_dir.join(".venv/bin/python3"),
                project_dir.join("venv/bin/python3"),
            ]
        };

        for candidate in &candidates {
            if candidate.exists() {
                let site_packages = if cfg!(target_os = "windows") {
                    candidate.parent()?.join("../Lib/site-packages")
                } else {
                    candidate.parent()?.join("../lib/python3/site-packages")
                };

                return Some(Self {
                    executable: candidate.clone(),
                    library: None,
                    version: None,
                    site_packages: if site_packages.exists() {
                        Some(site_packages)
                    } else {
                        None
                    },
                    source: DiscoverySource::VirtualEnv {
                        path: candidate.clone(),
                    },
                });
            }
        }

        // Check .python-version file
        let version_file = project_dir.join(".python-version");
        if let Ok(version) = std::fs::read_to_string(&version_file) {
            let version = version.trim();
            // Try to find this Python version
            let cmd = if cfg!(target_os = "windows") {
                "where"
            } else {
                "which"
            };
            let arg = format!("python{}", version.replace('.', ""));
            if let Ok(output) = std::process::Command::new(cmd).arg(&arg).output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = PathBuf::from(first_line.trim());
                    if path.exists() {
                        return Some(Self {
                            executable: path,
                            library: None,
                            version: Some(version.to_string()),
                            site_packages: None,
                            source: DiscoverySource::VirtualEnv {
                                path: version_file,
                            },
                        });
                    }
                }
            }
        }

        None
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_version() {
        assert_eq!(
            PythonEnv::extract_version_from_path(Path::new("python3.11")),
            Some("3.11".into())
        );
        assert_eq!(
            PythonEnv::extract_version_from_path(Path::new("python311.dll")),
            Some("3.11".into())
        );
        assert_eq!(
            PythonEnv::extract_version_from_path(Path::new("python3")),
            Some("3.0".into())
        );
    }

    #[test]
    fn discover_returns_something() {
        // This test verifies the discovery pipeline doesn't panic
        let result = PythonEnv::discover("hermes", None, None);
        // May or may not find Python depending on environment
        println!("Discovery result: {:?}", result);
    }
}
