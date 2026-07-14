#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${1:-${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-two-xterm.log}}"
STARTUP_BUDGET_MSEC="${SOPHIA_TWO_XTERM_STARTUP_BUDGET_MSEC:-2000}"
COMPOSE_BUDGET_MSEC="${SOPHIA_TWO_XTERM_COMPOSE_BUDGET_MSEC:-25}"

for budget_name in STARTUP_BUDGET_MSEC COMPOSE_BUDGET_MSEC; do
    budget="${!budget_name}"
    if [[ ! "$budget" =~ ^[0-9]+$ ]] || (( budget == 0 )); then
        echo "two-xterm proof requires a positive integer $budget_name" >&2
        exit 1
    fi
done

"$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$EVIDENCE_FILE" >/dev/null

if [[ "$(grep -Ec '^sophia_live_session schema=7 status=running .* secondary_terminal=enabled ' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "two-xterm proof is missing its secondary-terminal startup record" >&2
    exit 1
fi

mapfile -t lines < <(grep -E '^sophia_live_session schema=(10|11) status=bounded_complete ' "$EVIDENCE_FILE" || true)
if [[ "${#lines[@]}" -ne 1 ]]; then
    echo "two-xterm proof expected exactly one current completion record, got ${#lines[@]}" >&2
    exit 1
fi

cpu_layers=""
elapsed_msec=""
cpu_max_compose_msec=""
expected=""
flushed=""
for field in ${lines[0]}; do
    case "$field" in
        cpu_layers=*) cpu_layers="${field#cpu_layers=}" ;;
        elapsed_msec=*) elapsed_msec="${field#elapsed_msec=}" ;;
        cpu_max_compose_msec=*) cpu_max_compose_msec="${field#cpu_max_compose_msec=}" ;;
        input_events_expected=*) expected="${field#input_events_expected=}" ;;
        input_events_flushed=*) flushed="${field#input_events_flushed=}" ;;
    esac
done

if [[ ! "$cpu_layers" =~ ^[0-9]+$ ]] || (( cpu_layers < 2 )); then
    echo "two-xterm proof expected at least two composed CPU layers, got ${cpu_layers:-missing}" >&2
    exit 1
fi
if [[ ! "$expected" =~ ^[0-9]+$ ]] || [[ ! "$flushed" =~ ^[0-9]+$ ]] \
    || (( expected == 0 || flushed != expected )); then
    echo "two-xterm proof expected every injected X11 event to flush, got expected=${expected:-missing} flushed=${flushed:-missing}" >&2
    exit 1
fi
if [[ ! "$elapsed_msec" =~ ^[0-9]+$ ]] || (( elapsed_msec > STARTUP_BUDGET_MSEC )); then
    echo "two-xterm proof exceeded startup-to-echo budget: elapsed_msec=${elapsed_msec:-missing} budget_msec=$STARTUP_BUDGET_MSEC" >&2
    exit 1
fi
if [[ ! "$cpu_max_compose_msec" =~ ^[0-9]+$ ]] \
    || (( cpu_max_compose_msec > COMPOSE_BUDGET_MSEC )); then
    echo "two-xterm proof exceeded CPU composition budget: cpu_max_compose_msec=${cpu_max_compose_msec:-missing} budget_msec=$COMPOSE_BUDGET_MSEC" >&2
    exit 1
fi

echo "sophia_two_xterm_hardware_proof status=passed cpu_layers=$cpu_layers input_events_flushed=$flushed elapsed_msec=$elapsed_msec cpu_max_compose_msec=$cpu_max_compose_msec evidence=$EVIDENCE_FILE"
