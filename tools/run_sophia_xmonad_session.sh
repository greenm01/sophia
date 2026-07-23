#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_LIVE_SESSION_DISPLAY:-:77}"
STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-xmonad-session-${UID}"
PID_FILE="$STATE_DIR/wrapper.pid"
LOG_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/xmonad-session"
GUARD_LOG="$LOG_DIR/input-guard.log"
RECOVERY_LOG="$LOG_DIR/recovery.log"
SESSION_LOG="$LOG_DIR/session.log"
GUARD_ARMED_FILE="$STATE_DIR/input-guard.armed"
GUARD_TRIGGERED_FILE="$STATE_DIR/input-guard.triggered"

mkdir -p "$STATE_DIR"
chmod 700 "$STATE_DIR"
mkdir -p "$LOG_DIR"
chmod 700 "$LOG_DIR"
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
cargo build --offline --release -p sophia-cli --features atomic-scanout-live
cargo build --offline --release -p sophia-x11-wm-bridge
tools/atomic_scanout_preflight.sh

keyd_was_running=false
tty_state=""
kd_mode=""
guard_pid=""
session_pid=""
cleanup_done=false
terminate_bounded() {
    local target="$1" label="$2"
    if ! kill -0 -- "$target" 2>/dev/null; then
        return 0
    fi
    kill -TERM -- "$target" 2>/dev/null || true
    for _ in {1..40}; do
        if ! kill -0 -- "$target" 2>/dev/null; then
            wait "${target#-}" 2>/dev/null || true
            return 0
        fi
        sleep 0.05
    done
    echo "WARNING: $label did not stop after TERM; sending KILL." >&2
    kill -KILL -- "$target" 2>/dev/null || true
    wait "${target#-}" 2>/dev/null || true
}
cleanup() {
    local status=$?
    if [[ "$cleanup_done" == true ]]; then
        return "$status"
    fi
    cleanup_done=true
    local emergency=false
    [[ ! -s "$GUARD_TRIGGERED_FILE" ]] || emergency=true
    [[ -z "$session_pid" ]] || terminate_bounded "-$session_pid" "Sophia xmonad session"
    session_pid=""
    [[ -z "$guard_pid" ]] || terminate_bounded "$guard_pid" "Sophia input guard"
    guard_pid=""
    rm -f "$PID_FILE"
    if [[ -n "$kd_mode" ]]; then
        python3 "$ROOT_DIR/tools/sophia_tty_mode.py" "$kd_mode" 2>/dev/null || status=1
    fi
    if [[ -n "$tty_state" ]]; then
        stty "$tty_state" 2>/dev/null || status=1
    fi
    if [[ "$keyd_was_running" == true ]]; then
        echo
        echo "Restoring keyd..."
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        fi
    fi
    rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
    if [[ -n "$kd_mode" && -n "$tty_state" ]]; then
        local restored_kd restored_termios
        restored_kd="$(python3 "$ROOT_DIR/tools/sophia_tty_mode.py" get 2>/dev/null || echo unavailable)"
        restored_termios="$(stty -g 2>/dev/null || echo unavailable)"
        printf 'sophia_xmonad_tty_recovery schema=1 kd_mode_before=%s kd_mode_after=%s termios_restored=%s emergency=%s\n' \
            "$kd_mode" "$restored_kd" \
            "$([[ "$restored_termios" == "$tty_state" ]] && echo true || echo false)" \
            "$emergency" >>"$RECOVERY_LOG"
        if [[ "$restored_kd" != "$kd_mode" || "$restored_termios" != "$tty_state" ]]; then
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

tty_state="$(stty -g)"
kd_mode="$(python3 "$ROOT_DIR/tools/sophia_tty_mode.py" get)"

if pgrep -x keyd >/dev/null 2>&1; then
    echo "Temporarily stopping keyd so Sophia can own the keyboard..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

[[ ! -f "$GUARD_LOG" ]] || mv -f "$GUARD_LOG" "$GUARD_LOG.previous"
: >"$GUARD_LOG"
chmod 600 "$GUARD_LOG"
rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
target/release/sophia sophia-session-input-guard \
    --input-devices="$keyboard" \
    --armed-file="$GUARD_ARMED_FILE" \
    --triggered-file="$GUARD_TRIGGERED_FILE" \
    --owner-pid="$$" >>"$GUARD_LOG" 2>&1 &
guard_pid=$!
echo "Safety check: press and release Ctrl-Alt-Backspace once to arm recovery."
echo "During Sophia, press Ctrl-Alt-Backspace again for emergency recovery."
for _ in {1..600}; do
    [[ ! -s "$GUARD_ARMED_FILE" ]] || break
    kill -0 "$guard_pid" 2>/dev/null || {
        echo "Input guard exited before arming; see $GUARD_LOG" >&2
        exit 1
    }
    sleep 0.05
done
[[ -s "$GUARD_ARMED_FILE" ]] || {
    echo "Input guard was not armed within 30 seconds; refusing graphics takeover." >&2
    exit 1
}
echo "Emergency input guard armed."

echo "Starting Sophia with xmonad layout policy on $DISPLAY_NAME."
echo "Use Super+Enter for Kitty or Super+Shift+Q to log out."
echo "Press Ctrl-Alt-Backspace for local emergency recovery."
echo "The outside control plane may also run tools/stop_sophia_xmonad_session.sh."
terminal_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
if [[ -z "$terminal_bin" || ! -x "$terminal_bin" ]]; then
    echo "The graphical session requires Kitty; set SOPHIA_TERMINAL_BIN if it is installed elsewhere." >&2
    exit 1
fi
input_devices="${SOPHIA_OPERATOR_INPUT_DEVICES:-$keyboard}"
[[ ! -f "$SESSION_LOG" ]] || mv -f "$SESSION_LOG" "$SESSION_LOG.previous"
: >"$SESSION_LOG"
chmod 600 "$SESSION_LOG"
setsid env SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    target/release/sophia sophia-live-session \
    --session-mode=normal \
    "--session-app=terminal=$terminal_bin" \
    --session-start=terminal \
    --session-action-app=terminal=terminal \
    --display="$DISPLAY_NAME" \
    --native-scanout \
    --input-devices="$input_devices" \
    --wm-process="$ROOT_DIR/target/release/sophia-x11-wm-bridge" \
    --wm-process-arg="--wm=$xmonad_bin" \
    --wm-process-arg=--profile=xmonad \
    --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux \
    "$@" > >(tee "$SESSION_LOG") 2>&1 &
session_pid=$!
set +e
wait -n "$session_pid" "$guard_pid"
status=$?
set -e
if [[ -s "$GUARD_TRIGGERED_FILE" ]]; then
    echo "Emergency recovery requested."
    exit 130
fi
if ! kill -0 "$session_pid" 2>/dev/null; then
    set +e
    wait "$session_pid"
    status=$?
    set -e
    session_pid=""
else
    echo "Input guard exited unexpectedly; see $GUARD_LOG" >&2
    status=1
fi
exit "$status"
