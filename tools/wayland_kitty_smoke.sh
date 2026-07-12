#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_WAYLAND_KITTY_EVIDENCE:-/tmp/sophia-wayland-kitty.log}"
RUNTIME_DIR="$(mktemp -d /tmp/sophia-wayland-runtime.XXXXXX)"
trap 'rm -rf "$RUNTIME_DIR"' EXIT
chmod 700 "$RUNTIME_DIR"

cd "$ROOT_DIR"
cargo build -p sophia-cli --features atomic-scanout-live
env XDG_RUNTIME_DIR="$RUNTIME_DIR" LIBGL_ALWAYS_SOFTWARE=1 \
    target/debug/sophia sophia-wayland-session \
    --client=/usr/bin/kitty \
    --client-arg=-o \
    --client-arg=linux_display_server=wayland \
    --max-runtime-ms=3000 2>&1 | tee "$EVIDENCE_FILE"
tools/verify_wayland_kitty_evidence.sh "$EVIDENCE_FILE"
