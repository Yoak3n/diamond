//! Event store — in-memory ring buffer with optional replay for late-joining viewers.
//!
//! Stores the last N events so that newly connected viewers can catch up
//! on what happened before they joined.

use std::collections::VecDeque;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;
use tracing::debug;

// ─── Stored Event ───────────────────────────────────────────────────────────

/// A stored event with metadata for replay.
#[derive(Debug, Clone, Serialize)]
pub struct StoredEvent {
    /// Monotonic sequence number (global).
    pub seq: u64,

    /// Original JSON from the agent.
    pub raw_json: String,

    /// When the event was received by the Hub.
    pub received_at: chrono::DateTime<chrono::Utc>,
}

// ─── Event Store ────────────────────────────────────────────────────────────

/// In-memory ring buffer event store.
pub struct EventStore {
    events: RwLock<VecDeque<StoredEvent>>,
    max_events: usize,
    next_seq: std::sync::atomic::AtomicU64,
}

impl EventStore {
    /// Create a new store with the given capacity.
    pub fn new(max_events: usize) -> Arc<Self> {
        Arc::new(Self {
            events: RwLock::new(VecDeque::with_capacity(max_events)),
            max_events,
            next_seq: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Append an event to the store.
    ///
    /// Returns the assigned sequence number.
    pub async fn append(&self, raw_json: String) -> u64 {
        let seq = self.next_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stored = StoredEvent {
            seq,
            raw_json,
            received_at: chrono::Utc::now(),
        };

        let mut events = self.events.write().await;
        if events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(stored);
        debug!(seq, "Event stored");
        seq
    }

    /// Get all events since a given sequence number (exclusive).
    ///
    /// Used for replay: client says "I have up to seq N" and gets everything after.
    pub async fn since(&self, after_seq: u64) -> Vec<StoredEvent> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|e| e.seq > after_seq)
            .cloned()
            .collect()
    }

    /// Get the latest N events.
    pub async fn latest(&self, n: usize) -> Vec<StoredEvent> {
        let events = self.events.read().await;
        events
            .iter()
            .rev()
            .take(n)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Get the current sequence number (last assigned).
    pub fn current_seq(&self) -> u64 {
        self.next_seq.load(std::sync::atomic::Ordering::Relaxed) - 1
    }

    /// Total events currently in the store.
    pub async fn len(&self) -> usize {
        self.events.read().await.len()
    }

    /// Check if the store is empty.
    #[allow(dead_code)]
    pub async fn is_empty(&self) -> bool {
        self.events.read().await.is_empty()
    }
}
