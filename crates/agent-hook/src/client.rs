//! WebSocket Hub client — connects to the central WS Hub and sends events.
//!
//! Supports:
//! - Auto-reconnect with exponential backoff
//! - Buffered send with overflow protection
//! - Thread-safe `emit()` from any thread
//! - Event buffering for offline replay
//! - **Functional options** for ergonomic configuration
//!
//! ## Usage
//!
//! ```no_run
//! use agent_hook::hub::{HubClient, with_url, with_framework, with_buffer_size};
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = HubClient::new([
//!         with_url("ws://127.0.0.1:9210/hook"),
//!         with_framework("hermes"),
//!         with_buffer_size(2048),
//!     ]);
//!
//!     client.connect().await.unwrap();
//!     client.emit(agent_hook::event::events::agent_start("hermes", "s1"));
//!     client.disconnect().await;
//! }
//! ```

use std::collections::VecDeque;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::event::{AgentEvent, EventType};

// ─── Configuration ──────────────────────────────────────────────────────────

/// Hub client configuration.
#[derive(Debug, Clone)]
pub struct HubConfig {
    /// WebSocket Hub URL (e.g. "ws://127.0.0.1:9210/hook").
    pub url: String,

    /// Framework name reported in every event.
    pub framework: String,

    /// Session ID for this connection.
    pub session_id: String,

    /// Max events to buffer before dropping oldest.
    pub buffer_size: usize,

    /// Max reconnect attempts before giving up (0 = infinite).
    pub max_reconnect_attempts: u32,

    /// Initial reconnect delay.
    pub reconnect_delay: Duration,

    /// Max reconnect delay (exponential backoff cap).
    pub max_reconnect_delay: Duration,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:9210/hook".into(),
            framework: "unknown".into(),
            session_id: uuid::Uuid::new_v4().to_string(),
            buffer_size: 1024,
            max_reconnect_attempts: 0,
            reconnect_delay: Duration::from_millis(500),
            max_reconnect_delay: Duration::from_secs(30),
        }
    }
}

// ─── Functional Options ─────────────────────────────────────────────────────

/// A configuration option for [`HubClient`].
///
/// Created by the `with_*` helper functions. Implements `FnOnce(&mut HubConfig)`.
pub struct HubOption {
    apply: Box<dyn FnOnce(&mut HubConfig) + Send>,
}

impl HubOption {
    /// Create a new option from a closure.
    pub fn new(f: impl FnOnce(&mut HubConfig) + Send + 'static) -> Self {
        Self {
            apply: Box::new(f),
        }
    }

    fn apply(self, config: &mut HubConfig) {
        (self.apply)(config);
    }
}

// Allow closures to be used directly as HubOption
impl<F> From<F> for HubOption
where
    F: FnOnce(&mut HubConfig) + Send + 'static,
{
    fn from(f: F) -> Self {
        Self::new(f)
    }
}

/// Set the WebSocket Hub URL.
///
/// # Example
/// ```ignore
/// let client = HubClient::new([with_url("ws://my-hub:9210/hook")]);
/// ```
pub fn with_url(url: impl Into<String> + Send + 'static) -> HubOption {
    HubOption::new(move |c| c.url = url.into())
}

/// Set the framework name.
pub fn with_framework(framework: impl Into<String> + Send + 'static) -> HubOption {
    HubOption::new(move |c| c.framework = framework.into())
}

/// Set the session ID.
pub fn with_session_id(session_id: impl Into<String> + Send + 'static) -> HubOption {
    HubOption::new(move |c| c.session_id = session_id.into())
}

/// Set the max event buffer size.
pub fn with_buffer_size(size: usize) -> HubOption {
    HubOption::new(move |c| c.buffer_size = size)
}

/// Set max reconnect attempts (0 = infinite).
pub fn with_max_reconnect_attempts(attempts: u32) -> HubOption {
    HubOption::new(move |c| c.max_reconnect_attempts = attempts)
}

/// Set the initial reconnect delay.
pub fn with_reconnect_delay(delay: Duration) -> HubOption {
    HubOption::new(move |c| c.reconnect_delay = delay)
}

/// Set the max reconnect delay (backoff cap).
pub fn with_max_reconnect_delay(delay: Duration) -> HubOption {
    HubOption::new(move |c| c.max_reconnect_delay = delay)
}

// ─── Hub Client ─────────────────────────────────────────────────────────────

/// Thread-safe WS Hub client.
pub struct HubClient {
    config: HubConfig,
    /// Outbound event channel sender (cloneable, thread-safe).
    tx: mpsc::Sender<AgentEvent>,
    /// Shared state for checking connection status.
    state: Arc<RwLock<ClientState>>,
    /// Event buffer for replay after reconnect.
    buffer: Arc<Mutex<VecDeque<AgentEvent>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClientState {
    Disconnected,
    Connecting,
    Connected,
}

impl HubClient {
    /// Create a new client with functional options.
    ///
    /// # Example
    /// ```no_run
    /// use agent_hook::hub::{HubClient, with_url, with_framework};
    ///
    /// let client = HubClient::new([
    ///     with_url("ws://127.0.0.1:9210/hook"),
    ///     with_framework("hermes"),
    /// ]);
    /// ```
    pub fn new(options: impl IntoIterator<Item = HubOption>) -> Self {
        let mut config = HubConfig::default();
        for opt in options {
            opt.apply(&mut config);
        }
        Self::from_config(config)
    }

    /// Create a client from a pre-built [`HubConfig`].
    ///
    /// Prefer [`HubClient::new()`] with functional options for new code.
    pub fn from_config(config: HubConfig) -> Self {
        let buffer_size = config.buffer_size;
        let (tx, rx) = mpsc::channel(config.buffer_size);

        let client = Self {
            config,
            tx,
            state: Arc::new(RwLock::new(ClientState::Disconnected)),
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(buffer_size))),
        };

        // Spawn the background WS task
        let config = client.config.clone();
        let state = client.state.clone();
        let buffer = client.buffer.clone();
        tokio::spawn(ws_loop(config, rx, state, buffer));

        client
    }

    /// Connect to the Hub (spawns background task).
    pub async fn connect(&self) -> Result<(), HubError> {
        {
            let state = self.state.read().await;
            if *state == ClientState::Connected {
                return Ok(());
            }
        }
        info!(
            url = %self.config.url,
            framework = %self.config.framework,
            session = %self.config.session_id,
            "Hub client connecting"
        );
        Ok(())
    }

    /// Disconnect from the Hub.
    pub async fn disconnect(&self) {
        *self.state.write().await = ClientState::Disconnected;
        info!("Hub client disconnected");
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        *self.state.read().await == ClientState::Connected
    }

    /// Emit an event to the Hub (non-blocking, thread-safe).
    ///
    /// If the buffer is full, the oldest event is dropped.
    pub fn emit(&self, event: AgentEvent) {
        match self.tx.try_send(event.clone()) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("Hub send buffer full, buffering event");
                let buffer = self.buffer.clone();
                tokio::spawn(async move {
                    let mut buf = buffer.lock().await;
                    if buf.len() >= buf.capacity() {
                        buf.pop_front();
                    }
                    buf.push_back(event);
                });
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Hub send channel closed");
            }
        }
    }

    /// Emit an event with builder syntax.
    pub fn emit_event(
        &self,
        event_type: EventType,
        data: crate::event::EventData,
    ) {
        self.emit(AgentEvent::new(
            event_type,
            &self.config.framework,
            &self.config.session_id,
            data,
        ));
    }

    /// Get the current configuration.
    pub fn config(&self) -> &HubConfig {
        &self.config
    }
}

impl From<HubConfig> for HubClient {
    fn from(config: HubConfig) -> Self {
        Self::from_config(config)
    }
}

// ─── WebSocket Loop ─────────────────────────────────────────────────────────

async fn ws_loop(
    config: HubConfig,
    mut rx: mpsc::Receiver<AgentEvent>,
    state: Arc<RwLock<ClientState>>,
    buffer: Arc<Mutex<VecDeque<AgentEvent>>>,
) {
    let mut reconnect_attempts = 0u32;
    let mut current_delay = config.reconnect_delay;

    loop {
        {
            let s = state.read().await;
            if *s == ClientState::Disconnected && reconnect_attempts > 0 {
                break;
            }
        }

        *state.write().await = ClientState::Connecting;

        match connect_async(&config.url).await {
            Ok((ws_stream, _)) => {
                info!(url = %config.url, "Connected to Hub");
                reconnect_attempts = 0;
                current_delay = config.reconnect_delay;
                *state.write().await = ClientState::Connected;

                {
                    let mut buf = buffer.lock().await;
                    while let Some(event) = buf.pop_front() {
                        buf.push_front(event);
                        break;
                    }
                }

                let result = run_ws_session(ws_stream, &mut rx, &buffer, &state).await;

                if let Err(e) = result {
                    warn!(error = %e, "WS session error, will reconnect");
                }

                *state.write().await = ClientState::Disconnected;
            }
            Err(e) => {
                error!(error = %e, "Failed to connect to Hub");
                *state.write().await = ClientState::Disconnected;
            }
        }

        reconnect_attempts += 1;
        if config.max_reconnect_attempts > 0 && reconnect_attempts >= config.max_reconnect_attempts {
            error!(
                attempts = reconnect_attempts,
                "Max reconnect attempts reached, giving up"
            );
            break;
        }

        debug!(
            attempt = reconnect_attempts,
            delay_ms = current_delay.as_millis() as u64,
            "Reconnecting..."
        );
        sleep(current_delay).await;
        current_delay = (current_delay * 2).min(config.max_reconnect_delay);
    }
}

async fn run_ws_session(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    rx: &mut mpsc::Receiver<AgentEvent>,
    buffer: &Arc<Mutex<VecDeque<AgentEvent>>>,
    state: &Arc<RwLock<ClientState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut ws_sink, mut ws_source) = ws_stream.split();

    // Flush buffered events first
    {
        let mut buf = buffer.lock().await;
        while let Some(event) = buf.pop_front() {
            let json = serde_json::to_string(&event)?;
            if let Err(e) = ws_sink.send(Message::Text(json.into())).await {
                error!(error = %e, "Failed to send buffered event");
                buf.push_front(event);
                break;
            }
        }
    }

    loop {
        tokio::select! {
            msg = ws_source.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!(text = %text, "Received from Hub");
                    }
                    Some(Ok(Message::Binary(_))) => {}
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Frame(_))) => {}
                    Some(Ok(Message::Close(_))) => {
                        info!("Hub closed connection");
                        break;
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "WS receive error");
                        break;
                    }
                    None => break,
                }
            }

            event = rx.recv() => {
                match event {
                    Some(event) => {
                        let json = serde_json::to_string(&event)?;
                        if let Err(e) = ws_sink.send(Message::Text(json.into())).await {
                            error!(error = %e, "Failed to send event, buffering");
                            let mut buf = buffer.lock().await;
                            buf.push_back(event);
                            break;
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    *state.write().await = ClientState::Disconnected;
    Ok(())
}

// ─── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    #[error("Client is shutting down")]
    ShuttingDown,
}

impl From<tokio_tungstenite::tungstenite::Error> for HubError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        HubError::WebSocket(e.to_string())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = HubConfig::default();
        assert_eq!(config.url, "ws://127.0.0.1:9210/hook");
        assert_eq!(config.framework, "unknown");
        assert_eq!(config.buffer_size, 1024);
    }

    #[test]
    fn functional_options() {
        let config = apply_options([
            with_url("ws://my-hub:9210/hook"),
            with_framework("openclaw"),
            with_buffer_size(512),
        ]);
        assert_eq!(config.url, "ws://my-hub:9210/hook");
        assert_eq!(config.framework, "openclaw");
        assert_eq!(config.buffer_size, 512);
    }

    #[test]
    fn option_from_closure() {
        let opt = HubOption::new(|c| {
            c.max_reconnect_attempts = 10;
            c.reconnect_delay = Duration::from_secs(1);
        });
        let mut config = HubConfig::default();
        opt.apply(&mut config);
        assert_eq!(config.max_reconnect_attempts, 10);
        assert_eq!(config.reconnect_delay, Duration::from_secs(1));
    }

    /// Helper: apply options to a default config (for testing).
    fn apply_options(options: impl IntoIterator<Item = HubOption>) -> HubConfig {
        let mut config = HubConfig::default();
        for opt in options {
            opt.apply(&mut config);
        }
        config
    }
}
