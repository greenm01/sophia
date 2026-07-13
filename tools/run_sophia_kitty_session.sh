#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_RUNTIME_DIR:-/tmp}/sophia-kitty-session-${UID}"
LOG_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/kitty-session"
PID_FILE="$STATE_DIR/wrapper.pid"
GUARD_ARMED_FILE="$STATE_DIR/input-guard.armed"
GUARD_TRIGGERED_FILE="$STATE_DIR/input-guard.triggered"
SESSION_LOG="$LOG_DIR/session.log"
GUARD_LOG="$LOG_DIR/input-guard.log"

if [[ ! -t 0 ]]; then
    echo "Run this interactively from a dedicated local TTY." >&2
    exit 1
fi
for command in cargo kitty python3 stty; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Missing required command: $command" >&2
        exit 1
    fi
done

mkdir -p "$STATE_DIR" "$LOG_DIR"
chmod 700 "$STATE_DIR" "$LOG_DIR"
if [[ -s "$PID_FILE" ]]; then
    previous_pid="$(<"$PID_FILE")"
    if [[ "$previous_pid" =~ ^[0-9]+$ ]] && kill -0 "$previous_pid" 2>/dev/null; then
        echo "A Sophia Kitty session is already running (wrapper PID $previous_pid)." >&2
        echo "Stop it with: tools/stop_sophia_kitty_session.sh" >&2
        exit 1
    fi
    rm -f "$PID_FILE"
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
input_devices="${SOPHIA_INPUT_DEVICES:-$keyboard}"
require_dmabuf="${SOPHIA_KITTY_REQUIRE_DMABUF:-0}"
expected_keycodes="${SOPHIA_KITTY_EXPECT_KEYCODES:-}"
expect_pointer_input="${SOPHIA_KITTY_EXPECT_POINTER_INPUT:-0}"
expect_input_presentation="${SOPHIA_KITTY_EXPECT_INPUT_PRESENTATION:-0}"
max_input_latency_ms="${SOPHIA_KITTY_MAX_INPUT_LATENCY_MS:-}"

for value in "$require_dmabuf" "$expect_pointer_input" "$expect_input_presentation"; do
    if [[ "$value" != 0 && "$value" != 1 ]]; then
        echo "Sophia Kitty policy booleans must be 0 or 1." >&2
        exit 1
    fi
done
if [[ -n "$max_input_latency_ms" && ! "$max_input_latency_ms" =~ ^[0-9]+$ ]]; then
    echo "SOPHIA_KITTY_MAX_INPUT_LATENCY_MS must be an integer." >&2
    exit 1
fi

for log in "$SESSION_LOG" "$GUARD_LOG"; do
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

tty_state="$(stty -g)"
kd_mode=""
keyd_was_running=false
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
        terminate_bounded "-$session_pid" "Sophia Wayland session"
    fi
    if [[ -n "$kd_mode" ]]; then
        python3 "$ROOT_DIR/tools/sophia_tty_mode.py" "$kd_mode" 2>/dev/null || true
    fi
    stty "$tty_state" 2>/dev/null || true
    rm -f "$PID_FILE" "$GUARD_ARMED_FILE" "$GUARD_TRIGGERED_FILE"
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

echo "Starting native Wayland Kitty under Sophia (no X server)."
echo "Type exit normally, press Ctrl-Alt-Backspace for emergency recovery, or run 'sophia stop' externally."
kd_mode="$(python3 "$ROOT_DIR/tools/sophia_tty_mode.py" get)"
python3 "$ROOT_DIR/tools/sophia_tty_mode.py" graphics
stty raw -echo
session_command=(
    target/release/sophia sophia-wayland-session
    --client="${SOPHIA_KITTY_BIN:-kitty}"
    --client-arg=-o
    --client-arg=linux_display_server=wayland
    --client-arg=-o
    --client-arg=remember_window_size=no
    --native-scanout
    --input-devices="$input_devices"
)
session_command+=("$@")
if [[ "$require_dmabuf" == 1 ]]; then
    session_command+=(--experimental-dmabuf)
fi
if [[ -n "$expected_keycodes" ]]; then
    session_command+=("--expect-keycodes=$expected_keycodes")
fi
if [[ "$expect_pointer_input" == 1 ]]; then
    session_command+=(--expect-pointer-input)
fi
if [[ "$expect_input_presentation" == 1 ]]; then
    session_command+=(--expect-input-presentation)
fi
if [[ -n "$max_input_latency_ms" ]]; then
    session_command+=("--max-input-latency-ms=$max_input_latency_ms")
fi
dmabuf_requested=false
for argument in "${session_command[@]}"; do
    if [[ "$argument" == --experimental-dmabuf ]]; then
        dmabuf_requested=true
        break
    fi
done
printf 'sophia_wayland_wrapper schema=1 dmabuf_requested=%s expected_keycodes=%s expect_pointer_input=%s expect_input_presentation=%s arguments=%q\n' \
    "$dmabuf_requested" "${expected_keycodes:-none}" "$expect_pointer_input" \
    "$expect_input_presentation" "${session_command[*]}" >>"$SESSION_LOG"
setsid env SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
    "${session_command[@]}" >>"$SESSION_LOG" 2>&1 &
session_pid=$!
set +e
wait -n "$session_pid" "$guard_pid"
status=$?
set -e

if [[ -s "$GUARD_TRIGGERED_FILE" ]]; then
    echo "Emergency recovery requested; restoring TTY."
    status=0
elif ! kill -0 "$session_pid" 2>/dev/null; then
    wait "$session_pid" || status=$?
fi
exit "$status"
