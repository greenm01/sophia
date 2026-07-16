#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="${SOPHIA_M4_EVIDENCE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/milestone4}"
DISPLAY_NAME="${SOPHIA_M4_DISPLAY:-:184}"
RUNTIME_MSEC="${SOPHIA_M4_RUNTIME_MSEC:-6000}"
SOFTWARE_EVIDENCE="$EVIDENCE_DIR/software-xterm.log"
GPU_EVIDENCE="$EVIDENCE_DIR/gpu-vkcube.log"

if [[ ! -t 0 ]]; then
    echo "Run this proof interactively from a dedicated local text TTY." >&2
    exit 1
fi
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "A graphical display is active in this shell; use a dedicated text TTY." >&2
    exit 1
fi
command -v xterm >/dev/null || {
    echo "xterm is required for the Milestone 4 proof." >&2
    exit 1
}
VKCUBE="$(command -v vkcube || true)"
if [[ -z "$VKCUBE" ]]; then
    echo "vkcube is required for the Milestone 4 GPU proof." >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"
: >"$SOFTWARE_EVIDENCE"
: >"$GPU_EVIDENCE"

echo "Sophia Milestone 4 software + Vulkan hardware proof"
echo "This proof requires exclusive DRM/KMS ownership on the active TTY."
echo "Evidence: $EVIDENCE_DIR"

cargo build --quiet --release --offline --manifest-path "$ROOT_DIR/Cargo.toml" \
    -p sophia-cli --features "atomic-scanout-live"
"$ROOT_DIR/tools/atomic_scanout_preflight.sh"

SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$SOFTWARE_EVIDENCE" \
SOPHIA_LIVE_SESSION_DISPLAY="$DISPLAY_NAME" \
SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$RUNTIME_MSEC" \
    "$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" \
        --inject-surface-resize=800x600

set +e
(
    cd "$ROOT_DIR"
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        "$ROOT_DIR/target/release/sophia" sophia-live-session \
        --display="$DISPLAY_NAME" --native-scanout \
        --max-runtime-ms="$RUNTIME_MSEC" --secondary-terminal \
        --terminal-exec="$VKCUBE" \
        --m4-first-acquire-delay-ms=150 \
        --m4-reject-first-present
) 2>&1 | tee "$GPU_EVIDENCE"
proof_status="${PIPESTATUS[0]}"
set -e

if (( proof_status == 0 )); then
    "$ROOT_DIR/tools/verify_live_session_milestone4_evidence.sh" "$GPU_EVIDENCE"
fi

exit "$proof_status"
