//! Event logger — writes received events to a JSONL file for debugging.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Writes each received event as a single JSON line to a file.
pub struct EventLogger {
    file: Mutex<std::fs::File>,
}

impl EventLogger {
    pub fn new(path: &str) -> Result<Self, String> {
        let path = PathBuf::from(path);
        let file = OpenOptions::new().create(true).write(true).truncate(true).open(&path)
            .map_err(|e| format!("Failed to open log file {:?}: {}", path, e))?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }

    /// Write a raw JSON string as one line to the log file.
    pub fn log(&self, json_str: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{}", json_str);
            let _ = file.flush();
        }
    }
}
