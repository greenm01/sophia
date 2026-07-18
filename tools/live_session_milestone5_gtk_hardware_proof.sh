#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_dir="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia/milestone5-gtk"
runtime_dir="${XDG_RUNTIME_DIR:-/tmp}/sophia-milestone5-gtk-${UID}"
classic="${SOPHIA_M5_GTK_CLASSIC_EVIDENCE:-$state_dir/classic.log}"
confined="${SOPHIA_M5_GTK_CONFINED_EVIDENCE:-$state_dir/confined.log}"
guard_log="$state_dir/input-guard.log"
recovery_log="$state_dir/recovery.log"
guard_armed_file="$runtime_dir/input-guard.armed"
guard_triggered_file="$runtime_dir/input-guard.triggered"
devices="${SOPHIA_M5_GTK_INPUT_DEVICES:-}"
runtime_msec="${SOPHIA_M5_GTK_RUNTIME_MSEC:-30000}"
mode="${SOPHIA_M5_GTK_MODE:-paired}"
diagnostic="${SOPHIA_M5_GTK_DIAGNOSTIC:-0}"

keyd_was_running=false
tty_state=""
kd_mode=""
guard_pid=""
session_pid=""
cleanup_done=false

discover_stable_pointers() {
    local path event ev prop
    while IFS= read -r path; do
        event="$(basename "$(readlink -f "$path")")"
        [[ "$event" == event* ]] || continue
        ev="$(tr -d '[:space:]' <"/sys/class/input/$event/device/capabilities/ev" 2>/dev/null || true)"
        prop="$(tr -d '[:space:]' <"/sys/class/input/$event/device/properties" 2>/dev/null || echo 0)"
        [[ "$ev" =~ ^[[:xdigit:]]+$ && "$prop" =~ ^[[:xdigit:]]+$ ]] || continue

        # Stable by-path names are not standardized for touchpads; a
        # pointer-capable node may use a generic `*-event` name instead of a
        # mouse or touchpad suffix. Select relative devices, plus absolute
        # pointer/button-pad devices, from kernel capabilities.
        if (( (16#$ev & 4) != 0 || ((16#$ev & 8) != 0 && (16#$prop & 5) != 0) )); then
            printf '%s\n' "$path"
        fi
    done < <(
        find /dev/input/by-path -maxdepth 1 -type l -name '*-event*' -print 2>/dev/null | sort -u
    )
}

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
    local status="${1:-$?}"
    if [[ "$cleanup_done" == true ]]; then
        return "$status"
    fi
    cleanup_done=true
    local emergency=false
    [[ ! -s "$guard_triggered_file" ]] || emergency=true

    [[ -z "$session_pid" ]] || terminate_bounded "-$session_pid" "Sophia X session"
    session_pid=""
    [[ -z "$guard_pid" ]] || terminate_bounded "$guard_pid" "Sophia input guard"
    guard_pid=""

    if [[ -n "$kd_mode" ]]; then
        python3 "$root/tools/sophia_tty_mode.py" "$kd_mode" 2>/dev/null || status=1
    fi
    if [[ -n "$tty_state" ]]; then
        stty "$tty_state" 2>/dev/null || status=1
    fi
    if [[ "$keyd_was_running" == true ]]; then
        echo "Restoring keyd..."
        if ! sudo sv up keyd; then
            echo "WARNING: keyd could not be restored; run: sudo sv up keyd" >&2
            status=1
        else
            for _ in {1..200}; do
                pgrep -x keyd >/dev/null 2>&1 && break
                sleep 0.05
            done
        fi
    fi

    rm -f "$guard_armed_file" "$guard_triggered_file"
    if [[ -n "$kd_mode" && -n "$tty_state" ]]; then
        local restored_kd restored_termios keyd_restored processes recovery_status
        restored_kd="$(python3 "$root/tools/sophia_tty_mode.py" get 2>/dev/null || echo unavailable)"
        restored_termios="$(stty -g 2>/dev/null || echo unavailable)"
        keyd_restored=true
        if [[ "$keyd_was_running" == true ]] && ! pgrep -x keyd >/dev/null 2>&1; then
            keyd_restored=false
        fi
        processes=0
        if pgrep -af 'target/release/sophia (sophia-live-session|sophia-session-input-guard)' >/dev/null 2>&1; then
            processes=1
        fi
        recovery_status=complete
        if [[ "$restored_kd" != "$kd_mode" || "$restored_termios" != "$tty_state" || "$keyd_restored" != true || "$processes" != 0 ]]; then
            recovery_status=failed
            status=1
        fi
        printf 'sophia_x_tty_recovery schema=1 status=%s kd_mode_before=%s kd_mode_after=%s termios_restored=%s keyd_restored=%s processes=%s emergency=%s\n' \
            "$recovery_status" "$kd_mode" "$restored_kd" \
            "$([[ "$restored_termios" == "$tty_state" ]] && echo true || echo false)" \
            "$keyd_restored" "$processes" "$emergency" >>"$recovery_log"
    fi
    return "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ ! -t 0 ]]; then
    echo "Run this proof interactively from a dedicated local text TTY." >&2
    exit 1
fi
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run this proof from a dedicated text TTY with no graphical session." >&2
    exit 1
fi
for command in cargo zenity python3 stty setsid; do
    command -v "$command" >/dev/null || {
        echo "Missing required command: $command" >&2
        exit 1
    }
done
[[ "$runtime_msec" =~ ^[0-9]+$ ]] && (( runtime_msec >= 15000 && runtime_msec <= 120000 )) || {
    echo "SOPHIA_M5_GTK_RUNTIME_MSEC must be 15000-120000" >&2
    exit 1
}
[[ "$diagnostic" == 0 || "$diagnostic" == 1 ]] || {
    echo "SOPHIA_M5_GTK_DIAGNOSTIC must be 0 or 1" >&2
    exit 1
}
case "$mode" in
    paired|classic|confined) ;;
    *)
        echo "SOPHIA_M5_GTK_MODE must be paired, classic, or confined" >&2
        exit 1
        ;;
esac


if [[ -z "$devices" ]]; then
    mapfile -t keyboards < <(
        find /dev/input/by-path -maxdepth 1 -type l -name '*-event-kbd' -print 2>/dev/null | sort -u
    )
    mapfile -t pointers < <(
        discover_stable_pointers
    )
    (( ${#keyboards[@]} == 1 )) || {
        echo "Expected exactly one stable keyboard path; found ${#keyboards[@]}." >&2
        echo "Set SOPHIA_M5_GTK_INPUT_DEVICES explicitly if this machine exposes more than one." >&2
        exit 1
    }
    (( ${#pointers[@]} >= 1 )) || {
        echo "No stable mouse or touchpad event path was found." >&2
        echo "Set SOPHIA_M5_GTK_INPUT_DEVICES explicitly." >&2
        exit 1
    }
    selected=("${keyboards[0]}" "${pointers[@]}")
    devices="$(IFS=,; echo "${selected[*]}")"
fi

IFS=',' read -r -a input_paths <<<"$devices"
for path in "${input_paths[@]}"; do
    [[ "$path" == /dev/input/* && -e "$path" && -r "$path" ]] || {
        echo "Input device is not a readable /dev/input path: $path" >&2
        exit 1
    }
done
keyboard="${input_paths[0]}"

active_sessions=()
for process in river niri sway Hyprland kwin_wayland Xorg; do
    pgrep -x "$process" >/dev/null 2>&1 && active_sessions+=("$process")
done
if (( ${#active_sessions[@]} > 0 )); then
    echo "Refusing DRM takeover while another graphical session is active: ${active_sessions[*]}" >&2
    exit 1
fi

mkdir -p "$state_dir" "$runtime_dir"
chmod 700 "$state_dir" "$runtime_dir"
for log in "$classic" "$confined" "$guard_log" "$recovery_log"; do
    mkdir -p "$(dirname "$log")"
    [[ ! -f "$log" ]] || mv -f "$log" "$log.previous"
    : >"$log"
    chmod 600 "$log"
done
rm -f "$guard_armed_file" "$guard_triggered_file"

cd "$root"
cargo build --quiet --release --offline -p sophia-cli --features atomic-scanout-live
tools/atomic_scanout_preflight.sh

tty_state="$(stty -g)"
kd_mode="$(python3 "$root/tools/sophia_tty_mode.py" get)"

if pgrep -x keyd >/dev/null 2>&1; then
    command -v sudo >/dev/null && command -v sv >/dev/null || {
        echo "keyd owns the keyboard, but sudo/sv is unavailable to release it" >&2
        exit 1
    }
    echo "Temporarily stopping keyd so Sophia can receive physical input..."
    sudo -v
    sudo sv down keyd
    keyd_was_running=true
fi

target/release/sophia sophia-session-input-guard \
    --input-devices="$keyboard" \
    --armed-file="$guard_armed_file" \
    --triggered-file="$guard_triggered_file" \
    --owner-pid="$$" >>"$guard_log" 2>&1 &
guard_pid=$!
echo "Safety check: press and release Ctrl-Alt-Backspace once to arm recovery."
echo "During either GTK proof, press Ctrl-Alt-Backspace again for emergency recovery."
for _ in {1..600}; do
    [[ ! -s "$guard_armed_file" ]] || break
    kill -0 "$guard_pid" 2>/dev/null || {
        echo "Input guard exited before arming; see $guard_log" >&2
        exit 1
    }
    sleep 0.05
done
[[ -s "$guard_armed_file" ]] || {
    echo "Input guard was not armed within 30 seconds; refusing graphics takeover." >&2
    exit 1
}
echo "Emergency input guard armed."

echo "Sophia optional GTK3 paired hardware compatibility proof"
echo "Input devices: $devices"
echo "Classic evidence: $classic"
echo "Confined evidence: $confined"
echo "For each dialog: type sophia without Return, then move the pointer and click OK."
echo "Do not press Enter; the physical pointer click is part of the acceptance gate."

python3 "$root/tools/sophia_tty_mode.py" graphics
stty raw -echo

run_profile() {
    local profile="$1" evidence="$2"
    shift 2
    local diagnostic_env=()
    if [[ "$diagnostic" == 1 ]]; then
        diagnostic_env=(SOPHIA_X11_AUTHORITY_TRACE=1 SOPHIA_LIVE_SESSION_DIAGNOSTIC=1)
    fi
    setsid env SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE="$evidence" \
      SOPHIA_LIVE_SESSION_RUNTIME_MSEC="$runtime_msec" \
      SOPHIA_LIVE_SESSION_SKIP_BUILD=1 SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1 \
      "${diagnostic_env[@]}" \
      "$root/tools/live_session_persistent_hardware_proof.sh" \
      --namespace-profile="$profile" --software-client-rendering --client=zenity --client-arg=--entry \
      --client-arg=--title --client-arg='Sophia GTK proof' \
      --client-arg=--text --client-arg='Type sophia, then click OK' \
      --expect-client-stdout=$'sophia\n' --require-client-normal-exit \
      --expect-physical-text=sophia --expect-physical-pointer --exit-after-input-proof \
      --inject-surface-resize=640x360 --input-devices="$devices" "$@" &
    session_pid=$!
    set +e
    wait -n "$session_pid" "$guard_pid"
    local status=$?
    set -e
    if [[ -s "$guard_triggered_file" ]]; then
        echo "Emergency recovery requested."
        return 130
    fi
    if kill -0 "$guard_pid" 2>/dev/null && ! kill -0 "$session_pid" 2>/dev/null; then
        wait "$session_pid" || status=$?
        session_pid=""
        return "$status"
    fi
    echo "Input guard exited unexpectedly; see $guard_log" >&2
    return 1
}

case "$mode" in
    paired)
        run_profile classic-shared "$classic" "$@"
        run_profile confined "$confined" "$@"
        ;;
    classic) run_profile classic-shared "$classic" "$@" ;;
    confined) run_profile confined "$confined" "$@" ;;
esac
cleanup 0
if [[ "$mode" == paired ]]; then
    "$root/tools/verify_live_session_milestone5_gtk_evidence.sh" "$classic" "$confined"
else
    echo "Milestone 5 GTK diagnostic profile completed: $mode"
fi
"$root/tools/verify_live_session_milestone5_tty_recovery.sh" "$recovery_log"
