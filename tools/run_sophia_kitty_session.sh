#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPAT_DISPLAY="${SOPHIA_COMPAT_DISPLAY:-:178}"
DISPLAY_NUMBER="${COMPAT_DISPLAY#:}"
STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-kitty-session-${UID}"
LOG_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/kitty-session"
PID_FILE="$STATE_DIR/wrapper.pid"
AUTH_FILE="$STATE_DIR/Xauthority"
XORG_CONFIG="$STATE_DIR/xorg.conf"
GUARD_ARMED_FILE="$STATE_DIR/input-guard.armed"
GUARD_TRIGGERED_FILE="$STATE_DIR/input-guard.triggered"
XORG_LOG="$LOG_DIR/Xorg.log"
SESSION_LOG="$LOG_DIR/session.log"
GUARD_LOG="$LOG_DIR/input-guard.log"

if [[ ! "$DISPLAY_NUMBER" =~ ^[0-9]+$ ]]; then
    echo "SOPHIA_COMPAT_DISPLAY must have the form :NUMBER." >&2
    exit 1
fi
if [[ ! -t 0 ]]; then
    echo "Run this interactively from a dedicated local TTY." >&2
    exit 1
fi
for command in cargo kitty mcookie python3 stty xauth xdpyinfo cvt /usr/libexec/Xorg; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Missing required command: $command" >&2
        exit 1
    fi
done
if [[ ! -r /usr/lib/xorg/modules/xlibre-25/drivers/dummy_drv.so ]]; then
    echo "Missing XLibre dummy video driver." >&2
    exit 1
fi

mkdir -p "$STATE_DIR"
chmod 700 "$STATE_DIR"
if [[ -s "$PID_FILE" ]]; then
    previous_pid="$(<"$PID_FILE")"
    if [[ "$previous_pid" =~ ^[0-9]+$ ]] && kill -0 "$previous_pid" 2>/dev/null; then
        echo "A Sophia Kitty session is already running (wrapper PID $previous_pid)." >&2
        echo "Stop it with: tools/stop_sophia_kitty_session.sh" >&2
        exit 1
    fi
    rm -f "$PID_FILE"
fi
if [[ -e "/tmp/.X11-unix/X$DISPLAY_NUMBER" ]]; then
    echo "Compatibility display $COMPAT_DISPLAY is already in use." >&2
    exit 1
fi

active_sessions=()
for process in river niri sway Hyprland kwin_wayland Xorg; do
    if pgrep -x "$process" >/dev/null 2>&1; then
        active_sessions+=("$process")
    fi
done
if (( ${#active_sessions[@]} > 0 )); then
    echo "Refusing to take over a TTY while another graphical session is active." >&2
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

mkdir -p "$LOG_DIR"
chmod 700 "$LOG_DIR"
for log in "$XORG_LOG" "$SESSION_LOG" "$GUARD_LOG"; do
    if [[ -f "$log" ]]; then
        mv -f "$log" "$log.previous"
    fi
    : >"$log"
    chmod 600 "$log"
done
rm -f "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"

cd "$ROOT_DIR"
cargo build --release --offline -p sophia-cli --features atomic-scanout-live
tools/atomic_scanout_preflight.sh

modeline="$(cvt 1280 720 60 | sed -n 's/^Modeline //p')"
if [[ -z "$modeline" ]]; then
    echo "Could not generate the XLibre dummy display mode." >&2
    exit 1
fi
cat >"$XORG_CONFIG" <<EOF
Section "ServerFlags"
    Option "AutoAddDevices" "false"
    Option "AutoEnableDevices" "false"
    Option "DontVTSwitch" "true"
    Option "DontZap" "true"
EndSection
Section "Device"
    Identifier "SophiaDummy"
    Driver "dummy"
    VideoRam 256000
EndSection
Section "Monitor"
    Identifier "SophiaMonitor"
    HorizSync 5.0-1000.0
    VertRefresh 5.0-200.0
    Modeline $modeline
EndSection
Section "Screen"
    Identifier "SophiaScreen"
    Device "SophiaDummy"
    Monitor "SophiaMonitor"
    DefaultDepth 24
    SubSection "Display"
        Depth 24
        Modes "1280x720_60.00"
        Virtual 1280 720
    EndSubSection
EndSection
EOF
chmod 600 "$XORG_CONFIG"
: >"$AUTH_FILE"
chmod 600 "$AUTH_FILE"
xauth -f "$AUTH_FILE" add "$COMPAT_DISPLAY" MIT-MAGIC-COOKIE-1 "$(mcookie)"

tty_state="$(stty -g)"
kd_mode=""
keyd_was_running=false
xorg_pid=""
session_pid=""
guard_pid=""
cleanup_done=false
terminate_bounded() {
    local target="$1"
    local label="$2"
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
    if [[ -n "$guard_pid" ]]; then
        terminate_bounded "$guard_pid" "Sophia input guard"
    fi
    if [[ -n "$session_pid" ]]; then
        terminate_bounded "-$session_pid" "Sophia session process group"
    fi
    if [[ -n "$xorg_pid" ]]; then
        terminate_bounded "$xorg_pid" "XLibre"
    fi
    if [[ -n "$kd_mode" ]]; then
        python3 "$ROOT_DIR/tools/sophia_tty_mode.py" "$kd_mode" 2>/dev/null || true
    fi
    stty "$tty_state" 2>/dev/null || true
    rm -f "$PID_FILE" "$AUTH_FILE" "$XORG_CONFIG" \
        "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
    if [[ "$keyd_was_running" == true ]]; then
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        fi
    fi
    return "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
printf '%s\n' "$$" >"$PID_FILE"

if pgrep -x keyd >/dev/null 2>&1; then
    echo "Temporarily stopping keyd so Sophia can own the keyboard..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

target/release/sophia sophia-session-input-guard \
    --input-devices="$keyboard" \
    --armed-file="$GUARD_ARMED_FILE" \
    --triggered-file="$GUARD_TRIGGERED_FILE" \
    --owner-pid="$$" >>"$GUARD_LOG" 2>&1 &
guard_pid=$!
echo "Safety check: press and release Ctrl-Alt-Backspace once to arm recovery."
echo "During Sophia, press Ctrl-Alt-Backspace again to exit and restore this TTY."
for _ in {1..600}; do
    if [[ -s "$GUARD_ARMED_FILE" ]]; then
        break
    fi
    if ! kill -0 "$guard_pid" 2>/dev/null; then
        echo "Sophia input guard exited before arming. See $GUARD_LOG" >&2
        exit 1
    fi
    sleep 0.05
done
if [[ ! -s "$GUARD_ARMED_FILE" ]]; then
    echo "Sophia input guard was not armed within 30 seconds; refusing graphics takeover." >&2
    exit 1
fi
echo "Emergency input guard armed."

XAUTHORITY="$AUTH_FILE" /usr/libexec/Xorg "$COMPAT_DISPLAY" \
    -config "$XORG_CONFIG" \
    -auth "$AUTH_FILE" \
    -modulepath /usr/lib/xorg/modules/xlibre-25 \
    -nolisten tcp -novtswitch -sharevts \
    -logfile "$XORG_LOG" >"$STATE_DIR/xorg.stdout.log" 2>&1 &
xorg_pid=$!
for _ in {1..100}; do
    if ! kill -0 "$xorg_pid" 2>/dev/null; then
        echo "XLibre exited before becoming ready. See $XORG_LOG" >&2
        exit 1
    fi
    if XAUTHORITY="$AUTH_FILE" xdpyinfo -display "$COMPAT_DISPLAY" >/dev/null 2>&1; then
        break
    fi
    sleep 0.05
done
if ! XAUTHORITY="$AUTH_FILE" xdpyinfo -display "$COMPAT_DISPLAY" >/dev/null 2>&1; then
    echo "XLibre did not become ready. See $XORG_LOG" >&2
    exit 1
fi

echo "Starting a normal Kitty session under Sophia."
echo "Type exit normally, press Ctrl-Alt-Backspace for emergency recovery, or run 'sophia stop' externally."
kd_mode="$(python3 "$ROOT_DIR/tools/sophia_tty_mode.py" get)"
python3 "$ROOT_DIR/tools/sophia_tty_mode.py" graphics
stty raw -echo
setsid env \
    XAUTHORITY="$AUTH_FILE" \
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    target/release/sophia sophia-live-session \
    --client-backend=xlibre-compat \
    --compat-display="$COMPAT_DISPLAY" \
    --client="${SOPHIA_KITTY_BIN:-kitty}" \
    --client-arg=-o \
    --client-arg=linux_display_server=x11 \
    --client-arg=-o \
    --client-arg=remember_window_size=no \
    --client-arg=-o \
    --client-arg=initial_window_width=1280 \
    --client-arg=-o \
    --client-arg=initial_window_height=720 \
    --native-scanout \
    --input-devices="$keyboard" \
    "$@" >"$SESSION_LOG" 2>&1 &
session_pid=$!
set +e
wait -n "$session_pid" "$guard_pid"
status=$?
set -e
if [[ -s "$GUARD_TRIGGERED_FILE" ]]; then
    echo "Emergency input guard triggered; restoring the TTY."
    status=130
elif ! kill -0 "$session_pid" 2>/dev/null; then
    session_pid=""
elif ! kill -0 "$guard_pid" 2>/dev/null; then
    echo "Sophia input guard exited unexpectedly. See $GUARD_LOG" >&2
    status=1
fi
if (( status != 0 )); then
    echo "Sophia Kitty session stopped with status $status. See $SESSION_LOG, $XORG_LOG, and $GUARD_LOG" >&2
fi
exit "$status"
