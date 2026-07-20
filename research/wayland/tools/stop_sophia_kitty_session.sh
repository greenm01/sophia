#!/usr/bin/env bash
set -euo pipefail

STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-kitty-session-${UID}"
PID_FILE="$STATE_DIR/wrapper.pid"
if [[ ! -s "$PID_FILE" ]]; then
    echo "No Sophia Kitty session is recorded."
    exit 0
fi
wrapper_pid="$(<"$PID_FILE")"
if [[ ! "$wrapper_pid" =~ ^[0-9]+$ ]]; then
    echo "Invalid Sophia Kitty session state: $PID_FILE" >&2
    exit 1
fi
if ! kill -0 "$wrapper_pid" 2>/dev/null; then
    rm -f "$PID_FILE"
    echo "Removed stale Sophia Kitty session state."
    exit 0
fi
echo "Stopping Sophia Kitty session (wrapper PID $wrapper_pid)..."
kill -TERM "$wrapper_pid"
for _ in {1..50}; do
    if ! kill -0 "$wrapper_pid" 2>/dev/null; then
        echo "Sophia Kitty session stopped."
        exit 0
    fi
    sleep 0.1
done
echo "Sophia Kitty wrapper did not stop within five seconds." >&2
exit 1
