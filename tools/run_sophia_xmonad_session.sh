#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_LIVE_SESSION_DISPLAY:-:77}"
SESSION_PROFILE="${SOPHIA_TTY_PROFILE:-xmonad}"
if [[ "$SESSION_PROFILE" != xmonad && "$SESSION_PROFILE" != kitty ]]; then
    echo "SOPHIA_TTY_PROFILE must be xmonad or kitty." >&2
    exit 1
fi
SESSION_LABEL="Sophia $SESSION_PROFILE session"
STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-${SESSION_PROFILE}-session-${UID}"
PID_FILE="$STATE_DIR/wrapper.pid"
LOG_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/${SESSION_PROFILE}-session"
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
        echo "A $SESSION_LABEL is already running (wrapper PID $previous_pid)." >&2
        echo "Stop it with: tools/stop_sophia_${SESSION_PROFILE}_session.sh" >&2
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

input_seat="${SOPHIA_OPERATOR_INPUT_SEAT:-seat0}"
input_devices="${SOPHIA_OPERATOR_INPUT_DEVICES:-}"
input_source_args=()
if [[ -n "$input_devices" ]]; then
    input_source_args+=("--input-devices=$input_devices")
else
    input_source_args+=("--input-seat=$input_seat")
fi

xmonad_bin=""
if [[ "$SESSION_PROFILE" == xmonad ]]; then
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
fi

cd "$ROOT_DIR"
cargo build --offline --release -p sophia-cli --features atomic-scanout-live
if [[ "$SESSION_PROFILE" == xmonad ]]; then
    cargo build --offline --release -p sophia-x11-wm-bridge
fi
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
    [[ -z "$session_pid" ]] || terminate_bounded "-$session_pid" "$SESSION_LABEL"
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
        printf 'sophia_tty_recovery schema=2 profile=%s kd_mode_before=%s kd_mode_after=%s termios_restored=%s emergency=%s\n' \
            "$SESSION_PROFILE" \
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
    "${input_source_args[@]}" \
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

if [[ "$SESSION_PROFILE" == xmonad ]]; then
    echo "Starting Sophia with experimental xmonad layout policy on $DISPLAY_NAME."
    echo "Use Super+Enter for Kitty or Super+Shift+Q to log out."
else
    echo "Starting the supported Kitty-only Sophia input session on $DISPLAY_NAME."
    echo "xmonad and Super+Enter are intentionally disabled for this input gate."
    echo "Exit Kitty normally to return to tty3."
fi
echo "Press Ctrl-Alt-Backspace for local emergency recovery."
echo "The outside control plane may also run tools/stop_sophia_${SESSION_PROFILE}_session.sh."
terminal_bin="${SOPHIA_TERMINAL_BIN:-$(command -v kitty || true)}"
if [[ -z "$terminal_bin" || ! -x "$terminal_bin" ]]; then
    echo "The graphical session requires Kitty; set SOPHIA_TERMINAL_BIN if it is installed elsewhere." >&2
    exit 1
fi
[[ ! -f "$SESSION_LOG" ]] || mv -f "$SESSION_LOG" "$SESSION_LOG.previous"
: >"$SESSION_LOG"
chmod 600 "$SESSION_LOG"
session_args=(
    sophia-live-session
    --session-mode=normal
    "--session-app=terminal=$terminal_bin"
    --session-start=terminal
    --display="$DISPLAY_NAME"
    --native-scanout
    "${input_source_args[@]}"
)
if [[ "$SESSION_PROFILE" == xmonad ]]; then
    session_args+=(
        --session-action-app=terminal=terminal
        --wm-process="$ROOT_DIR/target/release/sophia-x11-wm-bridge"
        --wm-process-arg="--wm=$xmonad_bin"
        --wm-process-arg=--profile=xmonad
        --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux
    )
else
    session_args+=(
        --session-app-arg=terminal=--config
        --session-app-arg=terminal=NONE
        --session-app-arg=terminal=--override
        --session-app-arg=terminal=linux_display_server=x11
        --session-app-arg=terminal=--override
        --session-app-arg=terminal=background_opacity=1
        --session-app-arg=terminal=--title
        "--session-app-arg=terminal=Sophia Kitty TTY3"
        --exit-when-startup-exits
        --startup-ready-timeout-ms=8000
    )
fi
session_args+=("$@")
session_environment=(SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1)
if [[ "$SESSION_PROFILE" == kitty ]]; then
    session_environment+=(
        DBUS_SESSION_BUS_ADDRESS=unix:path=/dev/null
        SOPHIA_LIVE_SESSION_DIAGNOSTIC=1
        SOPHIA_NATIVE_COMPOSITION_PIXEL_TRACE=1
        SOPHIA_X11_AUTHORITY_TRACE=1
    )
fi
session_command=(
    env
    -u WAYLAND_DISPLAY
    -u WAYLAND_SOCKET
    "${session_environment[@]}"
    target/release/sophia
    "${session_args[@]}"
)
python3 "$ROOT_DIR/tools/sophia_tty_mode.py" graphics
stty raw -echo
setsid "${session_command[@]}" > >(tee "$SESSION_LOG") 2>&1 &
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
