#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-two-xterm.log}"

export SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$EVIDENCE_FILE"
# Two X clients need time to complete their independent startup and establish
# a stable frame before the input proof begins. Keep an explicit user override.
: "${SOPHIA_LIVE_SESSION_RUNTIME_MSEC:=20000}"
export SOPHIA_LIVE_SESSION_RUNTIME_MSEC
"$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" --secondary-terminal "$@"
"$ROOT_DIR/tools/verify_live_session_two_xterm_evidence.sh" "$EVIDENCE_FILE"
