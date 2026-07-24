#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCH_LOG=/tmp/sophia-kitty-tty3-launch.log

export SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=1
set +e
"$ROOT_DIR/tools/start_sophia_kitty_tty3.sh" "$@"
session_status=$?
set -e

if "$ROOT_DIR/tools/verify_sophia_native_composition_pixels.sh" "$LAUNCH_LOG"; then
    evidence_status=0
else
    evidence_status=$?
fi
if (( session_status != 0 )); then
    exit "$session_status"
fi
exit "$evidence_status"
