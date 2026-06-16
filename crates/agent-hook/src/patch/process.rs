//! CLI process wrapper — spawn agent CLIs and parse stdout/stderr into events.
//!
//! For frameworks that only expose a CLI (Claude Code, Codex, OpenCode, etc.),
//! this module wraps the subprocess and uses configurable regex patterns
//! to extract structured events from output.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

use regex::Regex;
use tracing::{debug, info};

use crate::event::{AgentEvent, EventData, EventType};

// ─── Pattern Rule ───────────────────────────────────────────────────────────

/// A regex-based rule for matching CLI output lines to events.
#[derive(Debug, Clone)]
pub struct OutputPattern {
    /// Regex pattern to match against each output line.
    pub regex: Regex,

    /// Event type to emit on match.
    pub event_type: EventType,

    /// Map from capture group name → event data field name.
    /// Group 0 (full match) is available as "_full".
    pub field_map: HashMap<String, String>,

    /// If true, this pattern also matches against stderr.
    pub match_stderr: bool,
}

impl OutputPattern {
    /// Create a new pattern with a regex string.
    pub fn new(regex_str: &str, event_type: EventType) -> Result<Self, regex::Error> {
        Ok(Self {
            regex: Regex::new(regex_str)?,
            event_type,
            field_map: HashMap::new(),
            match_stderr: false,
        })
    }

    /// Set field mapping (builder pattern).
    pub fn with_fields(mut self, fields: impl IntoIterator<Item = (String, String)>) -> Self {
        self.field_map = fields.into_iter().collect();
        self
    }

    /// Also match stderr lines.
    pub fn with_stderr(mut self) -> Self {
        self.match_stderr = true;
        self
    }

    /// Try to match a line and extract event data.
    fn try_match(&self, line: &str) -> Option<EventData> {
        let caps = self.regex.captures(line)?;

        let mut data = serde_json::Map::new();

        // Always include the full match
        data.insert(
            "_full".into(),
            serde_json::Value::String(caps[0].to_string()),
        );

        // Extract named or numbered groups
        for (field_name, group_name) in &self.field_map {
            let value = if group_name.starts_with('g') {
                // Named group: "g1" → group 1
                group_name[1..].parse::<usize>().ok().and_then(|i| caps.get(i))
            } else {
                caps.name(group_name)
            };

            if let Some(m) = value {
                data.insert(
                    field_name.clone(),
                    serde_json::Value::String(m.as_str().to_string()),
                );
            }
        }

        Some(EventData::Map(data))
    }
}

// ─── Process Wrapper ────────────────────────────────────────────────────────

/// Wraps a CLI agent process and converts stdout/stderr to events.
pub struct ProcessWrapper {
    /// Framework name for events.
    framework: String,

    /// Session ID.
    session_id: String,

    /// Output matching patterns (ordered by priority).
    patterns: Vec<OutputPattern>,

    /// Channel for events from the background reader thread.
    event_tx: mpsc::Sender<AgentEvent>,

    /// Receiving end (used by next_event / drain_events).
    event_rx: mpsc::Receiver<AgentEvent>,
}

impl ProcessWrapper {
    /// Create a new process wrapper.
    pub fn new(framework: impl Into<String>, session_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            framework: framework.into(),
            session_id: session_id.into(),
            patterns: Vec::new(),
            event_tx: tx,
            event_rx: rx,
        }
    }

    /// Add an output matching pattern.
    pub fn add_pattern(mut self, pattern: OutputPattern) -> Self {
        self.patterns.push(pattern);
        self
    }

    /// Add multiple patterns at once.
    pub fn with_patterns(mut self, patterns: Vec<OutputPattern>) -> Self {
        self.patterns.extend(patterns);
        self
    }

    /// Spawn a command and start capturing output.
    ///
    /// Returns a [`RunningProcess`] handle for sending input and waiting for exit.
    pub fn spawn(
        &self,
        command: &str,
        cwd: Option<&str>,
        env: Option<HashMap<String, String>>,
    ) -> Result<RunningProcess, std::io::Error> {
        info!(command = command, "Spawning CLI agent");

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(command);
            c
        };

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        if let Some(vars) = env {
            for (k, v) in vars {
                cmd.env(k, v);
            }
        }

        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        let mut child = cmd.spawn()?;

        // Take stdout/stderr before moving child
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        // Emit agent:start
        self.emit_event(
            EventType::AgentStart,
            EventData::from([("command", serde_json::Value::String(command.into()))]),
        );

        // Spawn stdout reader thread
        let patterns = self.patterns.clone();
        let tx_out = self.event_tx.clone();
        let framework = self.framework.clone();
        let session_id = self.session_id.clone();
        thread::spawn(move || {
            read_output(stdout, false, &patterns, &tx_out, &framework, &session_id);
        });

        // Spawn stderr reader thread
        let patterns_stderr = self.patterns.clone();
        let tx_err = self.event_tx.clone();
        let framework2 = self.framework.clone();
        let session_id2 = self.session_id.clone();
        thread::spawn(move || {
            read_output(stderr, true, &patterns_stderr, &tx_err, &framework2, &session_id2);
        });

        Ok(RunningProcess {
            child,
            stdin: Some(stdin),
            framework: self.framework.clone(),
            session_id: self.session_id.clone(),
            event_tx: self.event_tx.clone(),
        })
    }

    /// Try to receive the next event (non-blocking).
    pub fn next_event(&self) -> Option<AgentEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Drain all pending events.
    pub fn drain_events(&self) -> Vec<AgentEvent> {
        self.event_rx.try_iter().collect()
    }

    /// Internal: emit an event directly.
    fn emit_event(&self, event_type: EventType, data: EventData) {
        let event = AgentEvent::new(
            event_type,
            &self.framework,
            &self.session_id,
            data,
        );
        let _ = self.event_tx.send(event);
    }
}

// ─── Running Process ────────────────────────────────────────────────────────

/// Handle to a running CLI agent process.
pub struct RunningProcess {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
    framework: String,
    session_id: String,
    event_tx: mpsc::Sender<AgentEvent>,
}

impl RunningProcess {
    /// Send input to the process stdin.
    pub fn send_input(&mut self, input: &str) -> Result<(), std::io::Error> {
        use std::io::Write;
        if let Some(ref mut stdin) = self.stdin {
            stdin.write_all(input.as_bytes())?;
            stdin.write_all(b"\n")?;
            stdin.flush()?;
        }
        Ok(())
    }

    /// Wait for the process to exit.
    pub fn wait(&mut self) -> Result<i32, std::io::Error> {
        let status = self.child.wait()?;
        let code = status.code().unwrap_or(-1);

        // Emit agent:end
        let event = AgentEvent::new(
            EventType::AgentEnd,
            &self.framework,
            &self.session_id,
            EventData::from([("exit_code", serde_json::Value::Number(code.into()))]),
        );
        let _ = self.event_tx.send(event);

        Ok(code)
    }

    /// Kill the process.
    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        self.child.kill()
    }

    /// Get the process ID.
    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

// ─── Output Reader ──────────────────────────────────────────────────────────

fn read_output(
    reader: impl std::io::Read,
    is_stderr: bool,
    patterns: &[OutputPattern],
    tx: &mpsc::Sender<AgentEvent>,
    framework: &str,
    session_id: &str,
) {
    let buf_reader = BufReader::new(reader);

    for line in buf_reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        debug!(
            source = if is_stderr { "stderr" } else { "stdout" },
            line = %line,
            "CLI output"
        );

        let mut matched = false;
        for pattern in patterns {
            if !is_stderr || pattern.match_stderr {
                if let Some(data) = pattern.try_match(&line) {
                    let event = AgentEvent::new(
                        pattern.event_type.clone(),
                        framework,
                        session_id,
                        data,
                    );
                    let _ = tx.send(event);
                    matched = true;
                    break;
                }
            }
        }

        // If no pattern matched, emit a raw line event
        if !matched {
            let source = if is_stderr { "stderr" } else { "stdout" };
            let mut data = serde_json::Map::new();
            data.insert("text".into(), serde_json::Value::String(line.clone()));
            data.insert("source".into(), serde_json::Value::String(source.into()));

            let event = AgentEvent::new(
                EventType::Custom("raw_output".into()),
                framework,
                session_id,
                EventData::Map(data),
            );
            let _ = tx.send(event);
        }
    }
}

// ─── Preset Patterns ────────────────────────────────────────────────────────

/// Common output patterns for well-known CLI agents.
pub mod presets {
    use super::*;
    use crate::event::EventType as E;

    /// Claude Code output patterns.
    pub fn claude_code_patterns() -> Vec<OutputPattern> {
        vec![
            OutputPattern::new(r"Tool:\s*(\S+)", E::ToolStart)
                .unwrap()
                .with_fields([("name".into(), "g1".into())]),
            OutputPattern::new(r"Result:\s*(.+)", E::ToolComplete)
                .unwrap()
                .with_fields([("result".into(), "g1".into())]),
            OutputPattern::new(r"\[thinking\]\s*(.+)", E::ThinkingDelta)
                .unwrap()
                .with_fields([("text".into(), "g1".into())]),
            OutputPattern::new(r"Error:\s*(.+)", E::AgentError)
                .unwrap()
                .with_fields([("error".into(), "g1".into())]),
        ]
    }

    /// Generic patterns that work with most CLI tools.
    pub fn generic_patterns() -> Vec<OutputPattern> {
        vec![
            OutputPattern::new(r"(?i)error:\s*(.+)", E::AgentError)
                .unwrap()
                .with_fields([("error".into(), "g1".into())]),
            OutputPattern::new(r"(?i)warning:\s*(.+)", E::SystemWarning)
                .unwrap()
                .with_fields([("message".into(), "g1".into())]),
            OutputPattern::new(r"(?i)running tool:\s*(\S+)", E::ToolStart)
                .unwrap()
                .with_fields([("name".into(), "g1".into())]),
            OutputPattern::new(r"(?i)tool (?:result|output):\s*(.+)", E::ToolComplete)
                .unwrap()
                .with_fields([("result".into(), "g1".into())]),
        ]
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_matching() {
        let pattern = OutputPattern::new(r"Tool:\s*(\S+)", EventType::ToolStart)
            .unwrap()
            .with_fields([("name".into(), "g1".into())]);

        let data = pattern.try_match("Tool: terminal");
        assert!(data.is_some());
        let data = data.unwrap();
        assert_eq!(data.get_str("name"), Some("terminal"));
    }

    #[test]
    fn no_match() {
        let pattern = OutputPattern::new(r"Tool:\s*(\S+)", EventType::ToolStart).unwrap();

        let data = pattern.try_match("Hello world");
        assert!(data.is_none());
    }
}
