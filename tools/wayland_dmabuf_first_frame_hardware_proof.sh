#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE:-/tmp/sophia-wayland-dmabuf-first-frame.log}"
FRAME_COUNT="${SOPHIA_DMABUF_PRODUCER_FRAMES:-3}"
RENDER_NODE="${SOPHIA_DMABUF_RENDER_NODE:-}"

if [[ ! -t 0 ]]; then
    echo "Run the DMA-BUF first-frame proof from a dedicated local text TTY." >&2
    exit 1
fi
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Refusing DMA-BUF KMS proof while another graphical display is active." >&2
    exit 1
fi
if [[ ! "$FRAME_COUNT" =~ ^[0-9]+$ ]] || (( FRAME_COUNT < 2 || FRAME_COUNT > 1000 )); then
    echo "SOPHIA_DMABUF_PRODUCER_FRAMES must be an integer from 2 to 1000." >&2
    exit 1
fi
if [[ -z "$RENDER_NODE" ]]; then
    render_nodes=(/dev/dri/renderD*)
    if (( ${#render_nodes[@]} != 1 )) || [[ ! -e "${render_nodes[0]}" ]]; then
        echo "Expected exactly one render node; set SOPHIA_DMABUF_RENDER_NODE explicitly." >&2
        exit 1
    fi
    RENDER_NODE="${render_nodes[0]}"
fi
if [[ "$RENDER_NODE" != /dev/dri/renderD* || ! -r "$RENDER_NODE" || ! -w "$RENDER_NODE" ]]; then
    echo "DMA-BUF render node is not usable: $RENDER_NODE" >&2
    exit 1
fi

RUNTIME_DIR="$(mktemp -d /tmp/sophia-wayland-dmabuf-runtime.XXXXXX)"
PRODUCER="$(mktemp /tmp/sophia-wayland-dmabuf-producer.XXXXXX)"
trap 'rm -rf "$RUNTIME_DIR" "$PRODUCER"' EXIT
chmod 700 "$RUNTIME_DIR"
mkdir -p "$(dirname "$EVIDENCE_FILE")"

cd "$ROOT_DIR"
echo "Sophia DMA-BUF first-frame proof"
echo "  render node: $RENDER_NODE"
echo "  frames:      $FRAME_COUNT"
echo "  evidence:    $EVIDENCE_FILE"

cargo build --release --offline -p sophia-cli --features atomic-scanout-live
tools/atomic_scanout_preflight.sh
tools/build_wayland_dmabuf_producer.sh "$PRODUCER"

env XDG_RUNTIME_DIR="$RUNTIME_DIR" SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    timeout --foreground 30s target/release/sophia sophia-wayland-session \
        --client="$PRODUCER" \
        --client-arg=--render-node \
        --client-arg="$RENDER_NODE" \
        --client-arg=--frames \
        --client-arg="$FRAME_COUNT" \
        --native-scanout \
        --experimental-dmabuf >"$EVIDENCE_FILE" 2>&1

SOPHIA_WAYLAND_REQUIRE_DMABUF=1 \
    tools/verify_wayland_kitty_evidence.sh "$EVIDENCE_FILE"

echo "Sophia DMA-BUF first-frame proof passed: $EVIDENCE_FILE"
