#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia"
EVIDENCE_DIR="$STATE_DIR/dmabuf-promotion"
EVIDENCE_FILE="$EVIDENCE_DIR/controlled-lifecycle.log"
DIAGNOSTIC=false

usage() {
    echo "usage: $0 [--diagnostic]" >&2
}

while (( $# > 0 )); do
    case "$1" in
        --diagnostic)
            DIAGNOSTIC=true
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
    shift
done

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
if [[ "$DIAGNOSTIC" == true ]]; then
    echo "  mode:     GDB allocator/lifecycle diagnostic"
    exec env SOPHIA_DMABUF_DIAGNOSTIC_FRAMES=300 \
        tools/diagnose_void_dmabuf_heap.sh
fi

exec env \
    SOPHIA_DMABUF_PRODUCER_FRAMES=300 \
    SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE="$EVIDENCE_FILE" \
    tools/wayland_dmabuf_first_frame_hardware_proof.sh
