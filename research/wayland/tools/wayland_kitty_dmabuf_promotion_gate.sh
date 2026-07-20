#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="${SOPHIA_DMABUF_PROMOTION_EVIDENCE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/dmabuf-promotion}"
RUNS="${SOPHIA_DMABUF_PROMOTION_RUNS:-3}"

if [[ "$RUNS" != 3 ]]; then
    echo "The controlled DMA-BUF and Kitty acceptance gate requires exactly three independent Kitty runs." >&2
    exit 1
fi
if [[ ! -t 0 ]]; then
    echo "Run the DMA-BUF promotion gate from a dedicated local text TTY." >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"
chmod 700 "$EVIDENCE_DIR"
cd "$ROOT_DIR"

echo "[preflight] Running the controlled output-sized DMA-BUF first-frame proof."
SOPHIA_DMABUF_PRODUCER_FRAMES=3 \
SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE="$EVIDENCE_DIR/controlled-first-frame.log" \
    tools/wayland_dmabuf_first_frame_hardware_proof.sh

echo "[0/3] Running the controlled output-sized 300-frame DMA-BUF lifecycle proof."
SOPHIA_DMABUF_PRODUCER_FRAMES=300 \
SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE="$EVIDENCE_DIR/controlled-lifecycle.log" \
    tools/wayland_dmabuf_first_frame_hardware_proof.sh

for run in 1 2 3; do
    evidence="$EVIDENCE_DIR/kitty-run-${run}.log"
    rm -f "$evidence"
    echo "[$run/3] Run the guarded native Kitty acceptance proof and exit Kitty normally."
    SOPHIA_WAYLAND_KITTY_EVIDENCE="$evidence" \
        tools/wayland_kitty_hardware_proof.sh
done

echo "Controlled DMA-BUF and native Kitty acceptance gate passed. Evidence: $EVIDENCE_DIR"
