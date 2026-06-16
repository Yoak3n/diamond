//! Connection session management — tracks connected clients and their metadata.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info};

// ─── Session ID ─────────────────────────────────────────────────────────────

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Unique session identifier for a connected client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct SessionId(pub u64);

impl SessionId {
    pub fn next() -> Self {
        Self(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sess:{}", self.0)
    }
}

// ─── Client Role ────────────────────────────────────────────────────────────

/// Role of a connected client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(dead_code)]
pub enum ClientRole {
    /// Agent framework emitting events (Hermes, LangChain, OpenClaw, etc.)
    Agent {
        framework: String,
        session_id: String,
    },

    /// Dashboard or monitoring tool receiving events.
    Viewer,

    /// Hook client that both sends and receives.
    Bidirectional {
        framework: String,
    },
}

// ─── Client Info ────────────────────────────────────────────────────────────

/// Metadata about a connected client.
#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub session_id: SessionId,
    pub role: ClientRole,
    pub remote_addr: String,
    pub connected_at: DateTime<Utc>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub events_sent: u64,
    pub events_received: u64,
}

impl ClientInfo {
    pub fn new(session_id: SessionId, role: ClientRole, remote_addr: String) -> Self {
        Self {
            session_id,
            role,
            remote_addr,
            connected_at: Utc::now(),
            last_event_at: None,
            events_sent: 0,
            events_received: 0,
        }
    }
}

// ─── Session Manager ────────────────────────────────────────────────────────

/// Manages all connected clients and the event broadcast channel.
pub struct SessionManager {
    /// Active clients mapped by session ID.
    clients: RwLock<HashMap<SessionId, ClientInfo>>,

    /// Broadcast channel for events. Sender side is held here;
    /// each WS connection clones a receiver.
    event_tx: broadcast::Sender<String>,
}

impl SessionManager {
    /// Create a new session manager with the given broadcast capacity.
    pub fn new(event_buffer: usize) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(event_buffer);
        Arc::new(Self {
            clients: RwLock::new(HashMap::new()),
            event_tx,
        })
    }

    /// Register a new client and return its session ID.
    pub async fn register(
        &self,
        role: ClientRole,
        remote_addr: String,
    ) -> SessionId {
        let id = SessionId::next();
        let info = ClientInfo::new(id, role.clone(), remote_addr);
        self.clients.write().await.insert(id, info);

        let role_desc = match &role {
            ClientRole::Agent { framework, session_id } => {
                format!("agent:{}:{}", framework, session_id)
            }
            ClientRole::Viewer => "viewer".into(),
            ClientRole::Bidirectional { framework } => {
                format!("bidir:{}", framework)
            }
        };
        info!(session = %id, role = %role_desc, "Client connected");

        id
    }

    /// Unregister a client.
    pub async fn unregister(&self, id: SessionId) {
        if let Some(info) = self.clients.write().await.remove(&id) {
            let role_desc = match &info.role {
                ClientRole::Agent { framework, .. } => format!("agent:{}", framework),
                ClientRole::Viewer => "viewer".into(),
                ClientRole::Bidirectional { framework } => format!("bidir:{}", framework),
            };
            info!(
                session = %id,
                role = %role_desc,
                sent = info.events_sent,
                received = info.events_received,
                "Client disconnected"
            );
        }
    }

    /// Update last event timestamp and increment sent counter.
    pub async fn mark_event_sent(&self, id: SessionId) {
        if let Some(client) = self.clients.write().await.get_mut(&id) {
            client.last_event_at = Some(Utc::now());
            client.events_sent += 1;
        }
    }

    /// Increment received counter for a viewer.
    pub async fn mark_event_received(&self, id: SessionId) {
        if let Some(client) = self.clients.write().await.get_mut(&id) {
            client.events_received += 1;
        }
    }

    /// Broadcast a serialized event to all connected viewers.
    ///
    /// Returns the number of subscribers that received the event.
    pub fn broadcast(&self, event_json: &str) -> usize {
        match self.event_tx.send(event_json.to_string()) {
            Ok(n) => {
                debug!(subscribers = n, "Event broadcast");
                n
            }
            Err(_) => 0, // No subscribers
        }
    }

    /// Get a new event receiver for a newly connected viewer.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.event_tx.subscribe()
    }

    /// Get info about all connected clients.
    pub async fn list_clients(&self) -> Vec<ClientInfo> {
        self.clients.read().await.values().cloned().collect()
    }

    /// Count of currently connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}
