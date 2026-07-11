#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEYBOARD_DEVICE="${SOPHIA_OPERATOR_KEYBOARD:-}"
PROOF_TEXT="${SOPHIA_OPERATOR_TEXT:-sophia}"

if [[ -z "$KEYBOARD_DEVICE" ]]; then
    echo "set SOPHIA_OPERATOR_KEYBOARD to one absolute ...-event-kbd path" >&2
    echo "available keyboard paths:" >&2
    find /dev/input/by-id /dev/input/by-path -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null >&2 || true
    exit 1
fi
if [[ "$KEYBOARD_DEVICE" != /* || ! -e "$KEYBOARD_DEVICE" ]]; then
    echo "SOPHIA_OPERATOR_KEYBOARD must name an existing absolute input path" >&2
    exit 1
fi
if [[ ! "$PROOF_TEXT" =~ ^[a-z]{1,24}$ ]]; then
    echo "SOPHIA_OPERATOR_TEXT must contain 1-24 lowercase ASCII letters" >&2
    exit 1
fi

echo "Sophia operator keyboard-to-xterm pixel proof"
echo "Run this from the dedicated hardware TTY after the compositor releases DRM."
echo "When Sophia prints status=ready source=physical, type: $PROOF_TEXT"

SOPHIA_LIVE_SESSION_RUNTIME_MSEC="${SOPHIA_LIVE_SESSION_RUNTIME_MSEC:-15000}" \
    "$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" \
    "--input-devices=$KEYBOARD_DEVICE" \
    "--expect-physical-text=$PROOF_TEXT" \
    "$@"
