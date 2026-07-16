#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_M4_DISPLAY:-:184}"
RUNTIME_MSEC="${SOPHIA_M4_RUNTIME_MSEC:-6000}"
EVIDENCE_FILE="${SOPHIA_M4_NATIVE_EGL_EVIDENCE:-${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/milestone4/native-egl-mixed.log}"

if [[ ! -t 0 ]]; then
    echo "Run this diagnostic interactively from a dedicated local text TTY." >&2
    exit 1
fi
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "A graphical display is active in this shell; use a dedicated text TTY." >&2
    exit 1
fi
command -v xterm >/dev/null || {
    echo "xterm is required for the native EGL mixed diagnostic." >&2
    exit 1
}
command -v vkcube >/dev/null || {
    echo "vkcube is required for the native EGL mixed diagnostic." >&2
    exit 1
}

cargo build --quiet --release --offline --manifest-path "$ROOT_DIR/Cargo.toml" \
    -p sophia-cli --features "atomic-scanout-live"
"$ROOT_DIR/tools/atomic_scanout_preflight.sh"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
set +e
SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    "$ROOT_DIR/target/release/sophia" native-egl-vkcube-mixed-smoke \
        --display="$DISPLAY_NAME" --max-runtime-ms="$RUNTIME_MSEC" \
    2>&1 | tee "$EVIDENCE_FILE"
smoke_status="${PIPESTATUS[0]}"
set -e
if (( smoke_status == 0 )); then
    "$ROOT_DIR/tools/verify_native_egl_mixed_evidence.sh" "$EVIDENCE_FILE"
fi
exit "$smoke_status"
