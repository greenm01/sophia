#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_LIVE_SESSION_DISPLAY:-:77}"
STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-xmonad-session-${UID}"
PID_FILE="$STATE_DIR/wrapper.pid"

mkdir -p "$STATE_DIR"
chmod 700 "$STATE_DIR"
if [[ -s "$PID_FILE" ]]; then
    previous_pid="$(<"$PID_FILE")"
    if [[ "$previous_pid" =~ ^[0-9]+$ ]] && kill -0 "$previous_pid" 2>/dev/null; then
        echo "A Sophia xmonad session is already running (wrapper PID $previous_pid)." >&2
        echo "Stop it with: tools/stop_sophia_xmonad_session.sh" >&2
        exit 1
    fi
    rm -f "$PID_FILE"
fi

if [[ ! -t 0 ]]; then
    echo "Run this interactively from a dedicated local TTY." >&2
    exit 1
fi

active_sessions=()
for process in river niri sway Hyprland kwin_wayland Xorg; do
    if pgrep -x "$process" >/dev/null 2>&1; then
        active_sessions+=("$process")
    fi
done
if (( ${#active_sessions[@]} > 0 )); then
    echo "Refusing to take over a TTY while a graphical session is active." >&2
    echo "Still active: ${active_sessions[*]}" >&2
    exit 1
fi

keyboard="${SOPHIA_OPERATOR_KEYBOARD:-}"
if [[ -z "$keyboard" ]]; then
    mapfile -t keyboards < <(
        find /dev/input/by-id /dev/input/by-path \
            -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null \
            | sort -u
    )
    if (( ${#keyboards[@]} != 1 )); then
        echo "Expected exactly one keyboard path, found ${#keyboards[@]}." >&2
        printf '  %s\n' "${keyboards[@]}" >&2
        echo "Set SOPHIA_OPERATOR_KEYBOARD to the keyboard you will use." >&2
        exit 1
    fi
    keyboard="${keyboards[0]}"
fi
if [[ ! -r "$keyboard" ]]; then
    echo "Keyboard is not readable: $keyboard" >&2
    exit 1
fi

xmonad_bin="${SOPHIA_XMONAD_BIN:-}"
if [[ -z "$xmonad_bin" ]] && command -v xmonad >/dev/null 2>&1; then
    xmonad_bin="$(command -v xmonad)"
fi
if [[ -z "$xmonad_bin" ]]; then
    xmonad_source="${SOPHIA_XMONAD_SOURCE:-$HOME/src/xmonad}"
    xmonad_out="${SOPHIA_XMONAD_NIX_OUT:-/tmp/sophia-xmonad}"
    if [[ ! -x "$xmonad_out/bin/xmonad" ]]; then
        if [[ ! -f "$xmonad_source/flake.nix" ]]; then
            echo "xmonad not found; set SOPHIA_XMONAD_BIN or SOPHIA_XMONAD_SOURCE." >&2
            exit 1
        fi
        nix build "$xmonad_source#defaultPackage.x86_64-linux" \
            --out-link "$xmonad_out"
    fi
    xmonad_bin="$xmonad_out/bin/xmonad"
fi

cd "$ROOT_DIR"
cargo build --offline -p sophia-cli --features atomic-scanout-live
cargo build --offline -p sophia-x11-wm-bridge
tools/atomic_scanout_preflight.sh

keyd_was_running=false
session_pid=""
cleanup_done=false
cleanup() {
    local status=$?
    if [[ "$cleanup_done" == true ]]; then
        return "$status"
    fi
    cleanup_done=true
    if [[ -n "$session_pid" ]] && kill -0 "$session_pid" 2>/dev/null; then
        kill -TERM -- "-$session_pid" 2>/dev/null || true
        for _ in {1..20}; do
            kill -0 "$session_pid" 2>/dev/null || break
            sleep 0.1
        done
        if kill -0 "$session_pid" 2>/dev/null; then
            kill -KILL -- "-$session_pid" 2>/dev/null || true
        fi
        wait "$session_pid" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
    if [[ "$keyd_was_running" == true ]]; then
        echo
        echo "Restoring keyd..."
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        fi
    fi
    return "$status"
}
stop_from_signal() {
    local status="$1"
    exit "$status"
}
trap cleanup EXIT
trap 'stop_from_signal 130' INT
trap 'stop_from_signal 143' TERM
printf '%s\n' "$$" >"$PID_FILE"

if pgrep -x keyd >/dev/null 2>&1; then
    echo "Temporarily stopping keyd so Sophia can own the keyboard..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

echo "Starting Sophia with xmonad layout policy on $DISPLAY_NAME."
echo "Exit xterm, or run tools/stop_sophia_xmonad_session.sh from another TTY."
setsid env SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    target/debug/sophia sophia-live-session \
    --display="$DISPLAY_NAME" \
    --native-scanout \
    --input-devices="$keyboard" \
    --wm-process="$ROOT_DIR/target/debug/sophia-x11-wm-bridge" \
    --wm-process-arg="--wm=$xmonad_bin" \
    --wm-process-arg=--profile=xmonad \
    --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux \
    --terminal-exec=/bin/sh \
    --terminal-exec-arg=-i \
    "$@" &
session_pid=$!
set +e
wait "$session_pid"
status=$?
set -e
session_pid=""
exit "$status"
