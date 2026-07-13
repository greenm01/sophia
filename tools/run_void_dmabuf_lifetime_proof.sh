#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia"
EVIDENCE_DIR="$STATE_DIR/dmabuf-promotion"
EVIDENCE_FILE="$EVIDENCE_DIR/controlled-lifecycle.log"

if [[ ! -r /etc/os-release ]] || ! grep -Eq '^ID="?void"?$' /etc/os-release; then
    echo "This helper supports Void Linux only." >&2
    exit 1
fi
if [[ ! -t 0 ]] || [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run this proof from a dedicated local text TTY." >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"
chmod 700 "$STATE_DIR" "$EVIDENCE_DIR"

echo "Sophia DMA-BUF 300-frame lifetime proof"
echo "  evidence: $EVIDENCE_FILE"

cd "$ROOT_DIR"
exec env \
    SOPHIA_DMABUF_PRODUCER_FRAMES=300 \
    SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE="$EVIDENCE_FILE" \
    tools/wayland_dmabuf_first_frame_hardware_proof.sh
