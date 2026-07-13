#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-two-xterm.log}"

export SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$EVIDENCE_FILE"
# Two X clients need time to complete their independent startup and establish
# a stable frame before the input proof begins. Keep an explicit user override.
: "${SOPHIA_LIVE_SESSION_RUNTIME_MSEC:=12000}"
export SOPHIA_LIVE_SESSION_RUNTIME_MSEC
"$ROOT_DIR/tools/live_session_persistent_hardware_proof.sh" --secondary-terminal "$@"

line="$(grep -E '^sophia_live_session schema=9 status=bounded_complete ' "$EVIDENCE_FILE" | tail -n 1 || true)"
if [[ -z "$line" ]]; then
    echo "two-xterm proof is missing the persistent-session completion record" >&2
    exit 1
fi

cpu_layers=""
for field in ${line}; do
    case "$field" in
        cpu_layers=*) cpu_layers="${field#cpu_layers=}" ;;
    esac
done
if [[ ! "$cpu_layers" =~ ^[0-9]+$ ]] || (( cpu_layers < 2 )); then
    echo "two-xterm proof expected at least two composed CPU layers, got ${cpu_layers:-missing}" >&2
    exit 1
fi

echo "sophia_two_xterm_hardware_proof status=passed cpu_layers=$cpu_layers evidence=$EVIDENCE_FILE"
