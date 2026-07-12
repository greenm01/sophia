#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_WAYLAND_KITTY_EVIDENCE:-/tmp/sophia-wayland-kitty-hardware.log}"
INPUT_DEVICES="${SOPHIA_INPUT_DEVICES:-}"

if [[ -z "$INPUT_DEVICES" ]]; then
    echo "Set SOPHIA_INPUT_DEVICES to comma-separated keyboard/pointer event paths." >&2
    exit 1
fi
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run the native Wayland Kitty proof from a dedicated text TTY." >&2
    exit 1
fi

cd "$ROOT_DIR"
cargo build --release -p sophia-cli --features atomic-scanout-live
target/release/sophia sophia-wayland-session \
    --client=/usr/bin/kitty \
    --client-arg=-o \
    --client-arg=linux_display_server=wayland \
    --input-devices="$INPUT_DEVICES" \
    --native-scanout \
    --expect-input-presentation \
    --max-input-latency-ms=100 2>&1 | tee "$EVIDENCE_FILE"
SOPHIA_WAYLAND_REQUIRE_DMABUF=1 tools/verify_wayland_kitty_evidence.sh "$EVIDENCE_FILE"
