#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLASSIC_EVIDENCE="${SOPHIA_MILESTONE3_CLASSIC_EVIDENCE:-/tmp/sophia-milestone3-classic.log}"
CONFINED_EVIDENCE="${SOPHIA_MILESTONE3_CONFINED_EVIDENCE:-/tmp/sophia-milestone3-confined.log}"
UPDATE_SIZE="${SOPHIA_MILESTONE3_OUTPUT_SIZE:-1024x768}"
SURFACE_SIZE="${SOPHIA_MILESTONE3_SURFACE_SIZE:-960x640}"

echo "Sophia Milestone 3 paired hardware proof"
echo "This runs two exclusive-DRM sessions: classic shared-X, then fresh confined."

SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$CLASSIC_EVIDENCE" \
    "$ROOT_DIR/tools/live_session_two_xterm_hardware_proof.sh" \
    --namespace-profile=classic-shared --inject-output-size="$UPDATE_SIZE" \
    --inject-surface-resize="$SURFACE_SIZE" "$@"

SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$CONFINED_EVIDENCE" \
    SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
    "$ROOT_DIR/tools/live_session_two_xterm_hardware_proof.sh" \
    --namespace-profile=confined --inject-output-size="$UPDATE_SIZE" \
    --inject-surface-resize="$SURFACE_SIZE" "$@"

"$ROOT_DIR/tools/verify_live_session_milestone3_evidence.sh" \
    "$CLASSIC_EVIDENCE" "$CONFINED_EVIDENCE"
