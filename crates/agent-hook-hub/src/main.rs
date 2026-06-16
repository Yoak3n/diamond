//! Hook Hub — WS Hub server for agent-hook.
//!
//! Receives events from agent frameworks and broadcasts them to viewers.
//!
//! ## Usage
//!
//! ```bash
//! # Start with defaults (port 9210, 10K event buffer)
//! hook-hub
//!
//! # Custom port and buffer
//! hook-hub --port 8080 --buffer 50000
//!
//! # With debug logging
//! hook-hub --log-level debug
//! ```

use std::net::SocketAddr;

use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

mod api;
mod server;
mod session;
mod store;

use server::AppState;
use session::SessionManager;
use store::EventStore;

// ─── CLI Args ───────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "hook-hub", about = "WS Hub server for agent-hook")]
struct Args {
    /// Port to listen on.
    #[arg(short, long, default_value = "9210")]
    port: u16,

    /// Host address to bind.
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Event buffer size (how many events to keep for replay).
    #[arg(short, long, default_value = "10000")]
    buffer: usize,

    /// Broadcast channel capacity (max concurrent viewers).
    #[arg(long, default_value = "1024")]
    broadcast_capacity: usize,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        host = %args.host,
        port = args.port,
        buffer = args.buffer,
        "Starting Hook Hub"
    );

    // Create shared state
    let state = AppState {
        sessions: SessionManager::new(args.broadcast_capacity),
        store: EventStore::new(args.buffer),
    };

    // Build router
    let app = Router::new()
        // WebSocket endpoints
        .route("/hook", get(server::hook_handler))
        .route("/view", get(server::view_handler))
        // REST API
        .route("/api/status", get(api::status))
        .route("/api/clients", get(api::clients))
        .route("/api/events", get(api::events))
        .route("/api/events/latest", get(api::events_latest))
        .route("/api/emit", post(api::emit))
        // Middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Bind and serve
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .expect("Invalid host:port");

    info!(addr = %addr, "Listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}
