# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Diamond is a Tauri 2.x desktop application — a universal hook management and real-time event viewer for AI agent frameworks (Hermes, OpenClaw, Claude Code, Codex CLI). It detects installed frameworks, registers hooks into them, and streams events through a central WebSocket Hub.

## Build & Development Commands

```bash
pnpm install              # Install frontend dependencies
pnpm tauri dev            # Development mode (Vite + Rust compile + window)
pnpm tauri build          # Production build (frontend + Rust + installer)
pnpm dev                  # Frontend only (Vite dev server on port 1420)
pnpm build                # Type-check (vue-tsc) + Vite bundle
cargo test                # Run Rust tests (workspace level)
cargo build -p hook-hub   # Build just the Hub server binary
```

## Workspace Structure

```
Cargo.toml (workspace root)
├── src-tauri/              Tauri desktop app (main binary)
│   ├── src/base/           Core app infra: window mgmt, tray, state, commands, lightweight mode
│   ├── src/hook/           Agent hook system: detector, manager, hub sidecar control
│   └── src/lib.rs          Tauri builder config, plugin registration, event loop entry
├── crates/agent-hook/      Library: event protocol, WS client, framework adapters, patching
└── crates/agent-hook-hub/  Binary "hook-hub": standalone Axum WS/HTTP Hub server
```

## Architecture & Data Flow

```
Agent Frameworks (Hermes/Claude Code/OpenClaw/Codex)
    ↓ (registered hooks)
agent-hook crate (adapters normalize to AgentEvent)
    ↓ WebSocket
hook-hub binary (Axum, default port 19210)
    ↓ REST API (via tauri-plugin-shell sidecar)
src-tauri (Tauri commands → IPC)
    ↓ invoke()
Vue frontend (App.vue, real-time display)
```

## Key Modules

**`src-tauri/src/base/`** — Core infrastructure:
- `cmd.rs` — All `#[tauri::command]` functions (framework detection, hook registration, hub control)
- `init.rs` — Plugin setup, `generate_handlers()` to register IPC commands, `configure()` for app lifecycle
- `state.rs` — `AppState` with `LightWeightState` and `HubManager` (port 19210)
- `window/manager.rs` — Global `Manager` singleton for window lifecycle (show/close/toggle/destroy)
- `window/config.rs` — Window configuration (size, decorations, transparency, floating)
- `tray.rs` — System tray (Show/Hide, AutoStart, Quit)
- `lightweight.rs` — 10-min idle timer, auto-destroys windows to save resources
- `timer.rs` — Global `Timer` singleton via `delay_timer`, auto-refreshes every minute

**`src-tauri/src/hook/`** — Agent hook system:
- `detector.rs` — Scans for installed frameworks (paths, configs, processes, existing hooks)
- `manager.rs` — Registers/unregisters hooks: creates `.bat`/`.sh` scripts, updates framework configs
- `hub.rs` — `HubManager` starts/stops `hook-hub` sidecar, probes REST API for status

**`crates/agent-hook/src/`** — Hook library:
- `event.rs` — `AgentEvent` with `EventType` enum (50+ types), `EventData` (JSON map or empty)
- `client.rs` — `HubClient` with auto-reconnect, exponential backoff, offline event buffering
- `adapter/` — `Adapter` trait + implementations for Claude Code, Hermes, OpenClaw, LangChain, Generic

**`crates/agent-hook-hub/src/`** — Hub server:
- `main.rs` — CLI args, Axum router: `/hook` (WS), `/view` (WS), `/api/status`, `/api/clients`, `/api/events`
- `server.rs` — WS handlers for hook and view connections
- `store.rs` — Event ring buffer store

## Frontend

Single Vue SPC (`src/App.vue`) with all UI: Hub controls, framework detection list with hook registration, real-time event viewer (3s auto-refresh). All backend communication via `@tauri-apps/api/core` `invoke()`.

## Conventions

- Rust edition 2021; global singletons use `once_cell::OnceCell` + `parking_lot::Mutex`
- Tauri commands defined in `cmd.rs`, registered in `init.rs` → `generate_handlers()`
- Release profile: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `strip = true`
- TypeScript strict mode (`noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`)
- The Hub sidecar binary (`hook-hub`) is declared as an external binary in `tauri.conf.json` at `binaries/hook-hub`
