#!/usr/bin/env bash
set -euo pipefail

STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-xmonad-session-${UID}"
PID_FILE="$STATE_DIR/wrapper.pid"

if [[ ! -s "$PID_FILE" ]]; then
    echo "No Sophia xmonad session is recorded."
    exit 0
fi

wrapper_pid="$(<"$PID_FILE")"
if [[ ! "$wrapper_pid" =~ ^[0-9]+$ ]]; then
    echo "Invalid Sophia xmonad session state: $PID_FILE" >&2
    exit 1
fi
if ! kill -0 "$wrapper_pid" 2>/dev/null; then
    rm -f "$PID_FILE"
    echo "Removed stale Sophia xmonad session state."
    exit 0
fi

echo "Stopping Sophia xmonad session (wrapper PID $wrapper_pid)..."
kill -TERM "$wrapper_pid"
for _ in {1..50}; do
    if ! kill -0 "$wrapper_pid" 2>/dev/null; then
        echo "Sophia xmonad session stopped."
        exit 0
    fi
    sleep 0.1
done

echo "Sophia xmonad wrapper did not stop within five seconds." >&2
echo "Inspect it with: ps -o pid,ppid,pgid,tty,stat,args -p $wrapper_pid" >&2
exit 1
