#!/usr/bin/env bash
# Build the hook-hub sidecar binary and place it where Tauri expects it.
# Usage: ./scripts/build-sidecar.sh [--release]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PROFILE="debug"
CARGO_FLAGS=""
if [[ "${1:-}" == "--release" ]]; then
    PROFILE="release"
    CARGO_FLAGS="--release"
fi

# Detect target triple
TARGET=$(rustc -vV | grep "^host:" | awk '{print $2}')
EXT=""
if [[ "$TARGET" == *"-windows-"* ]]; then
    EXT=".exe"
fi

SRC="$PROJECT_ROOT/target/$PROFILE/hook-hub$EXT"
DEST="$PROJECT_ROOT/src-tauri/binaries/hook-hub-${TARGET}${EXT}"

echo "Building agent-hook-hub ($PROFILE)..."
cargo build -p agent-hook-hub $CARGO_FLAGS

mkdir -p "$PROJECT_ROOT/src-tauri/binaries"
cp "$SRC" "$DEST"

echo "Copied: $SRC -> $DEST"
echo "Done."
