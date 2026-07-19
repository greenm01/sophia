#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SOPHIA_QEMU_OUT_DIR:-$ROOT_DIR/.qemu}"
KERNEL_VERSION="${SOPHIA_QEMU_KERNEL_VERSION:-$(uname -r)}"
KERNEL_IMAGE="${SOPHIA_QEMU_KERNEL:-/boot/vmlinuz-$KERNEL_VERSION}"
INITRAMFS="${SOPHIA_QEMU_INITRAMFS:-$OUT_DIR/sophia-$KERNEL_VERSION.img}"
SCENARIO="${SOPHIA_QEMU_SCENARIO:-session}"
TWO_XTERM="${SOPHIA_QEMU_TWO_XTERM:-0}"
if [[ "$SCENARIO" != "session" && "$SCENARIO" != "emergency-recovery" && "$SCENARIO" != "gtk-classic" && "$SCENARIO" != "gtk-confined" && "$SCENARIO" != "xmonad-m7" && "$SCENARIO" != "xmonad-m8-launcher" && "$SCENARIO" != "xmonad-m8-mix" && "$SCENARIO" != "xmonad-m8-soak" ]]; then
    echo "SOPHIA_QEMU_SCENARIO must include a supported session or xmonad scenario" >&2
    exit 1
fi
if [[ "$TWO_XTERM" != "0" && "$TWO_XTERM" != "1" ]]; then
    echo "SOPHIA_QEMU_TWO_XTERM must be 0 or 1" >&2
    exit 1
fi
if [[ "$SCENARIO" == "emergency-recovery" && "$TWO_XTERM" != "0" ]]; then
    echo "SOPHIA_QEMU_TWO_XTERM is only supported by the session scenario" >&2
    exit 1
fi
if [[ "$SCENARIO" == "emergency-recovery" ]]; then
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-emergency-recovery.log"
elif [[ "$SCENARIO" == gtk-* ]]; then
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-$SCENARIO.log"
elif [[ "$SCENARIO" == xmonad-* ]]; then
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-$SCENARIO.log"
else
    DEFAULT_EVIDENCE_FILE="/tmp/sophia-qemu-session.log"
fi
EVIDENCE_FILE="${SOPHIA_QEMU_EVIDENCE:-$DEFAULT_EVIDENCE_FILE}"
QEMU_BIN="${SOPHIA_QEMU_BIN:-qemu-system-x86_64}"
MEMORY_MIB="${SOPHIA_QEMU_MEMORY_MIB:-2048}"
VNC_SOCKET="${SOPHIA_QEMU_VNC_SOCKET:-$OUT_DIR/display.sock}"
QMP_SOCKET="${SOPHIA_QEMU_QMP_SOCKET:-$OUT_DIR/qmp.sock}"
SERIAL_FIFO="${SOPHIA_QEMU_SERIAL_FIFO:-$OUT_DIR/serial.fifo}"
QEMU_PID=""
LOGGER_PID=""

evidence_count() {
    grep -c "$1" "$EVIDENCE_FILE" 2>/dev/null || true
}

wait_for_new_evidence() {
    local pattern=$1
    local baseline=$2
    local attempts=${3:-400}
    local current
    for _ in $(seq 1 "$attempts"); do
        current="$(evidence_count "$pattern")"
        if (( current > baseline )); then
            return 0
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            return 1
        fi
        sleep 0.05
    done
    return 1
}

send_chord_and_wait() {
    local chord=$1
    local pattern=$2
    local label=$3
    local baseline
    baseline="$(evidence_count "$pattern")"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence "$pattern" "$baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=action_evidence_timeout action=$label chord=$chord" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

send_launch_and_wait() {
    local chord=$1
    local pattern=$2
    local label=$3
    local layout_baseline
    local focus_baseline
    layout_baseline="$(evidence_count '^sophia_live_wm schema=1 status=layout_committed ')"
    focus_baseline="$(evidence_count '^sophia_live_wm schema=1 status=focus_reconciled .* target=surface .*outcome=')"
    send_chord_and_wait "$chord" "$pattern" "$label"
    if ! wait_for_new_evidence '^sophia_live_wm schema=1 status=layout_committed ' "$layout_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=action_layout_timeout action=$label chord=$chord" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    if ! wait_for_new_evidence '^sophia_live_wm schema=1 status=focus_reconciled .* target=surface .*outcome=' "$focus_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=action_focus_timeout action=$label chord=$chord" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

send_close_and_wait() {
    local app=$1
    local action_baseline
    local exit_baseline
    action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$')"
    exit_baseline="$(evidence_count "^sophia_session_app schema=1 status=exited id=$app ")"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+c
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+shift+c app=$app" | tee -a "$EVIDENCE_FILE"
    if ! wait_for_new_evidence '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$action_baseline" \
        || ! wait_for_new_evidence "^sophia_session_app schema=1 status=exited id=$app " "$exit_baseline" 800; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=application_close_timeout app=$app" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

send_firefox_close_and_wait() {
    local exit_baseline
    local action_baseline
    exit_baseline="$(evidence_count '^sophia_session_app schema=1 status=exited id=firefox ')"
    action_baseline="$(evidence_count '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$')"
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+shift+c
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+shift+c app=firefox" | tee -a "$EVIDENCE_FILE"
    wait_for_new_evidence '^sophia_live_wm schema=1 status=session_action_committed .* action=CloseFocused$' "$action_baseline"

    # Firefox can expose more than one managed top-level window. Closing the
    # focused one may leave the browser process alive, so use its native quit
    # chord while cycling the remaining managed surfaces. Ctrl+Q exits
    # the browser process instead of closing only one of its top-levels.
    for _ in $(seq 1 8); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+q
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=ctrl+q app=firefox" | tee -a "$EVIDENCE_FILE"
        if wait_for_new_evidence '^sophia_session_app schema=1 status=exited id=firefox ' "$exit_baseline" 80; then
            return 0
        fi
        local focus_baseline
        focus_baseline="$(evidence_count '^sophia_live_wm schema=1 status=focus_reconciled ')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
        wait_for_new_evidence '^sophia_live_wm schema=1 status=focus_reconciled ' "$focus_baseline" || true
    done
    echo "sophia_qemu_xmonad schema=1 status=failed reason=application_close_timeout app=firefox" | tee -a "$EVIDENCE_FILE"
    return 1
}

wait_for_firefox_stage() {
    local stage=$1
    if ! wait_for_new_evidence "^sophia_firefox_m8 schema=1 status=stage_complete stage=$stage " 0 800; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=$stage" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
}

run_firefox_m8_interactions() {
    local page_focus_baseline
    local clipboard_complete=false
    local primary_complete=false
    local dialog_complete=false
    wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=page_ready ' 0 800
    for _ in $(seq 1 10); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+l
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" f6
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+a
        "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" --no-return sophia
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+a
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+c
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" tab
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+v
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=clipboard ' 0 20; then
            clipboard_complete=true
            break
        fi
        page_focus_baseline="$(evidence_count '^sophia_live_wm schema=1 status=focus_reconciled ')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+j phase=firefox-input-refocus" | tee -a "$EVIDENCE_FILE"
        wait_for_new_evidence '^sophia_live_wm schema=1 status=focus_reconciled ' "$page_focus_baseline" 400 || true
    done
    if [[ "$clipboard_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=clipboard" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage clipboard
    for _ in $(seq 1 10); do
        # Firefox's native PRIMARY paste gesture is a middle click. The fixture
        # expands the target over its content area for this stage; sweep a
        # bounded grid because the browser shares two outputs with other apps.
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" -4096 -4096 1 middle
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 320 400 1 middle
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 640 0 1 middle
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 640 0 1 middle
        "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 640 0 1 middle
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+v
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=primary ' 0 20; then
            primary_complete=true
            break
        fi
        page_focus_baseline="$(evidence_count '^sophia_live_wm schema=1 status=focus_reconciled ')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+j phase=firefox-primary-refocus" | tee -a "$EVIDENCE_FILE"
        wait_for_new_evidence '^sophia_live_wm schema=1 status=focus_reconciled ' "$page_focus_baseline" 400 || true
    done
    if [[ "$primary_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=primary" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage primary
    "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+spc
    echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+spc phase=firefox-resize" | tee -a "$EVIDENCE_FILE"
    wait_for_firefox_stage resize
    for _ in $(seq 1 10); do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ctrl+l
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" f6
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" tab
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ret
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" ret
        if wait_for_new_evidence '^sophia_firefox_m8 schema=1 status=stage_complete stage=dialog ' 0 20; then
            dialog_complete=true
            break
        fi
        page_focus_baseline="$(evidence_count '^sophia_live_wm schema=1 status=focus_reconciled ')"
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" meta_l+j
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=meta_l+j phase=firefox-dialog-refocus" | tee -a "$EVIDENCE_FILE"
        wait_for_new_evidence '^sophia_live_wm schema=1 status=focus_reconciled ' "$page_focus_baseline" 400 || true
    done
    if [[ "$dialog_complete" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=firefox_stage_timeout stage=dialog" | tee -a "$EVIDENCE_FILE"
        return 1
    fi
    wait_for_firefox_stage dialog
    echo "sophia_qemu_firefox_m8 schema=1 status=interactions_complete keyboard=true clipboard=true primary=true resize=true dialog=true" | tee -a "$EVIDENCE_FILE"
}

cleanup() {
    if [[ -n "$QEMU_PID" ]] && kill -0 "$QEMU_PID" 2>/dev/null; then
        kill "$QEMU_PID" 2>/dev/null || true
        wait "$QEMU_PID" 2>/dev/null || true
    fi
    if [[ -n "$LOGGER_PID" ]] && kill -0 "$LOGGER_PID" 2>/dev/null; then
        kill "$LOGGER_PID" 2>/dev/null || true
        wait "$LOGGER_PID" 2>/dev/null || true
    fi
    rm -f "$VNC_SOCKET" "$QMP_SOCKET" "$SERIAL_FIFO"
}
trap cleanup EXIT

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
    echo "missing qemu-system-x86_64; on Void install it with:" >&2
    echo "  sudo xbps-install -S qemu-system-amd64" >&2
    exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
    echo "missing python3; on Void install it with:" >&2
    echo "  sudo xbps-install -S python3" >&2
    exit 1
fi
if [[ ! -r "$KERNEL_IMAGE" ]]; then
    echo "guest kernel is not readable: $KERNEL_IMAGE" >&2
    exit 1
fi
if [[ ! -r "$INITRAMFS" ]]; then
    echo "guest initramfs is not readable: $INITRAMFS" >&2
    echo "build it first with tools/build_qemu_session_initramfs.sh" >&2
    exit 1
fi
if [[ ! "$MEMORY_MIB" =~ ^[0-9]+$ ]] || (( MEMORY_MIB < 512 || MEMORY_MIB > 16384 )); then
    echo "SOPHIA_QEMU_MEMORY_MIB must be from 512 through 16384" >&2
    exit 1
fi

mkdir -p "$(dirname "$EVIDENCE_FILE")"
: > "$EVIDENCE_FILE"
rm -f "$VNC_SOCKET" "$QMP_SOCKET" "$SERIAL_FIFO"
mkfifo "$SERIAL_FIFO"

if [[ "$SCENARIO" == "emergency-recovery" ]]; then
    echo "sophia_qemu_recovery schema=1 status=starting isolation=headless control=qmp-unix host_drm=none host_vt=none keyboard=virtio chord=ctrl-alt-backspace" | tee -a "$EVIDENCE_FILE"
elif [[ "$SCENARIO" == gtk-* ]]; then
    echo "sophia_qemu_gtk schema=1 status=starting isolation=headless control=qmp-unix host_drm=none host_vt=none keyboard=virtio mouse=virtio scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
elif [[ "$SCENARIO" == xmonad-* ]]; then
    echo "sophia_qemu_xmonad schema=1 status=starting isolation=headless control=qmp-unix profile=xmonad windows=2" | tee -a "$EVIDENCE_FILE"
else
    echo "sophia_qemu_session schema=3 status=starting isolation=headless display_sink=vnc-unix control=qmp-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu gpu_devices=2 gpu_heads=2 keyboard=virtio mouse=virtio ticks=300" | tee -a "$EVIDENCE_FILE"
fi

while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s\n' "${line%$'\r'}"
done < "$SERIAL_FIFO" | tee -a "$EVIDENCE_FILE" &
LOGGER_PID=$!

"$QEMU_BIN" \
    -machine q35,accel=kvm:tcg \
    -smp 2 \
    -m "$MEMORY_MIB" \
    -nodefaults \
    -no-reboot \
    -display none \
    -vnc "unix:$VNC_SOCKET" \
    -monitor none \
    -qmp "unix:$QMP_SOCKET,server=on,wait=off" \
    -serial stdio \
    -device virtio-vga,max_outputs=1 \
    -device virtio-gpu-pci,max_outputs=1 \
    -device virtio-keyboard-pci \
    -device virtio-mouse-pci \
    -kernel "$KERNEL_IMAGE" \
    -initrd "$INITRAMFS" \
    -append "console=ttyS0 quiet loglevel=3 rdinit=/sbin/sophia-qemu-init rd.driver.pre=virtio_pci rd.driver.pre=virtio_gpu rd.driver.pre=virtio_input panic=-1 sophia.scenario=$SCENARIO sophia.two_xterm=$TWO_XTERM" \
    > "$SERIAL_FIFO" 2>&1 &
QEMU_PID=$!

if [[ "$SCENARIO" == "emergency-recovery" ]]; then
    guard_ready=false
    for _ in $(seq 1 600); do
        if grep -q '^sophia_session_input_guard schema=1 status=ready ' "$EVIDENCE_FILE"; then
            guard_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            break
        fi
        sleep 0.05
    done
    if [[ "$guard_ready" != true ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=input_guard_readiness_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    if ! "$ROOT_DIR/tools/qemu_qmp_emergency_chord.py" "$QMP_SOCKET"; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=qmp_arm_input_send" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_recovery_input schema=1 status=sent phase=arm source=qmp device=virtio-keyboard chord=ctrl-alt-backspace events=6" | tee -a "$EVIDENCE_FILE"

    recovery_ready=false
    for _ in $(seq 1 600); do
        if grep -q '^sophia_session_input_guard schema=1 status=armed$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=2 status=poller_ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE"; then
            recovery_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then
            break
        fi
        sleep 0.05
    done
    if [[ "$recovery_ready" != true ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=armed_session_readiness_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    if ! "$ROOT_DIR/tools/qemu_qmp_emergency_chord.py" "$QMP_SOCKET"; then
        echo "sophia_qemu_recovery schema=1 status=failed reason=qmp_trigger_input_send" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_recovery_input schema=1 status=sent phase=trigger source=qmp device=virtio-keyboard chord=ctrl-alt-backspace events=6" | tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup

    if [[ "$qemu_status" -ne 0 ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed qemu_exit=$qemu_status" | tee -a "$EVIDENCE_FILE"
        exit "$qemu_status"
    fi
    if [[ "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_recovery schema=1 status=failed serial_logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
        exit "$logger_status"
    fi

    echo "sophia_qemu_recovery schema=1 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/verify_qemu_emergency_recovery_evidence.sh" "$EVIDENCE_FILE"
    exit 0
fi

if [[ "$SCENARIO" == xmonad-* ]]; then
    ready=false
    for _ in $(seq 1 800); do
        if grep -q '^sophia_live_wm schema=1 status=ready ' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=focus_ready$' "$EVIDENCE_FILE" \
            && grep -q '^sophia_live_wm schema=1 status=layout_committed ' "$EVIDENCE_FILE"; then
            ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$ready" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=readiness_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    chords=("meta_l+j" "meta_l+k" "meta_l+spc" "meta_l+2" "meta_l+shift+1")
    for chord in "${chords[@]}"; do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
        sleep 1
    done
    restart_layout_baseline="$(grep -c '^sophia_live_wm schema=1 status=layout_committed ' "$EVIDENCE_FILE" || true)"
    restart_focus_baseline="$(grep -c '^sophia_live_wm schema=1 status=focus_reconciled ' "$EVIDENCE_FILE" || true)"
    restarted=false
    for _ in $(seq 1 400); do
        if grep -q '^sophia_live_wm schema=1 status=restarted .*preserved_layout=true' "$EVIDENCE_FILE"; then
            restarted=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$restarted" != true ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=restart_recovery_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    recovery_layout=false
    for _ in $(seq 1 400); do
        current_layout_count="$(grep -c '^sophia_live_wm schema=1 status=layout_committed ' "$EVIDENCE_FILE" || true)"
        if (( current_layout_count > restart_layout_baseline )); then
            recovery_layout=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$recovery_layout" != true ]] && ! grep -q '^sophia_live_wm schema=1 status=layout_timeout .*preserved_layout=true' "$EVIDENCE_FILE"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=restart_layout_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if ! wait_for_new_evidence '^sophia_live_wm schema=1 status=focus_reconciled ' "$restart_focus_baseline"; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=restart_focus_timeout" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if [[ "$SCENARIO" == "xmonad-m8-soak" ]]; then
        soak_started=$SECONDS
        cycles=0
        while (( SECONDS - soak_started < 1800 )); do
            for chord in meta_l+j meta_l+k meta_l+spc meta_l+2 meta_l+shift+1; do
                "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
                echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
            done
            send_launch_and_wait meta_l+shift+ret '^sophia_session_app schema=1 status=started id=terminal source=action' terminal-launch
            send_close_and_wait terminal
            send_launch_and_wait meta_l+f '^sophia_session_app schema=1 status=started id=firefox source=action' firefox-launch
            if (( cycles == 0 )); then
                run_firefox_m8_interactions
            fi
            send_firefox_close_and_wait
            send_launch_and_wait meta_l+p '^sophia_session_app schema=1 status=started id=launcher source=action' launcher-launch
            send_close_and_wait launcher
            cycles=$((cycles + 1))
            echo "sophia_qemu_m8_soak schema=1 status=cycle_complete cycle=$cycles terminal_restarts=$cycles firefox_restarts=$cycles launcher_restarts=$cycles close_actions=$((cycles * 3))" | tee -a "$EVIDENCE_FILE"
            sleep 75
        done
        if (( cycles < 20 )); then
            echo "sophia_qemu_m8_soak schema=1 status=failed reason=insufficient_cycles cycles=$cycles" | tee -a "$EVIDENCE_FILE"
            exit 1
        fi
        chords=("meta_l+shift+q")
    elif [[ "$SCENARIO" == "xmonad-m8-mix" ]]; then
        send_launch_and_wait meta_l+shift+ret '^sophia_session_app schema=1 status=started id=terminal source=action' terminal-launch
        send_close_and_wait terminal
        send_launch_and_wait meta_l+f '^sophia_session_app schema=1 status=started id=firefox source=action' firefox-launch
        run_firefox_m8_interactions
        send_firefox_close_and_wait
        send_launch_and_wait meta_l+p '^sophia_session_app schema=1 status=started id=launcher source=action' launcher-launch
        send_close_and_wait launcher
        chords=("meta_l+shift+q")
    else
        chords=("meta_l+shift+ret" "meta_l+shift+c" "meta_l+shift+q")
    fi
    for chord in "${chords[@]}"; do
        "$ROOT_DIR/tools/qemu_qmp_chord.py" "$QMP_SOCKET" "$chord"
        echo "sophia_qemu_xmonad_input schema=1 status=sent chord=$chord" | tee -a "$EVIDENCE_FILE"
        sleep 1
    done


    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup
    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_xmonad schema=1 status=failed reason=guest_exit qemu_exit=$qemu_status logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    case "$SCENARIO" in
        xmonad-m7) "$ROOT_DIR/tools/verify_qemu_xmonad_m7_evidence.sh" "$EVIDENCE_FILE" ;;
        xmonad-m8-launcher) "$ROOT_DIR/tools/verify_qemu_xmonad_m8_launcher_evidence.sh" "$EVIDENCE_FILE" ;;
        xmonad-m8-mix) "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$EVIDENCE_FILE" ;;
        xmonad-m8-soak) "$ROOT_DIR/tools/verify_qemu_xmonad_m8_soak_evidence.sh" "$EVIDENCE_FILE" ;;
    esac
    echo "sophia_qemu_xmonad schema=1 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
    exit 0
fi
if [[ "$SCENARIO" == gtk-* ]]; then
    input_ready=false
    for _ in $(seq 1 600); do
        if grep -q '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$EVIDENCE_FILE"; then
            input_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$input_ready" != true ]]; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=input_readiness_timeout scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 1 1 1
    echo "sophia_qemu_gtk_pointer schema=1 status=sent phase=entry_focus source=qmp clicks=1" | tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" sophia
    echo "sophia_qemu_gtk_input schema=1 status=sent source=qmp text=sophia events=14" | tee -a "$EVIDENCE_FILE"

    pointer_ready=false
    for _ in $(seq 1 200); do
        if grep -q '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' "$EVIDENCE_FILE"; then
            pointer_ready=true
            break
        fi
        if ! kill -0 "$QEMU_PID" 2>/dev/null; then break; fi
        sleep 0.05
    done
    if [[ "$pointer_ready" != true ]]; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=pointer_readiness_timeout scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi

    "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET" 0 0 1
    echo "sophia_qemu_gtk_pointer schema=1 status=sent phase=focused_select source=qmp clicks=1" | tee -a "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET"
    echo "sophia_qemu_gtk_input schema=1 status=sent source=qmp action=submit events=2" | tee -a "$EVIDENCE_FILE"

    set +e
    wait "$QEMU_PID"
    qemu_status=$?
    QEMU_PID=""
    wait "$LOGGER_PID"
    logger_status=$?
    LOGGER_PID=""
    set -e
    cleanup

    if [[ "$qemu_status" -ne 0 || "$logger_status" -ne 0 ]]; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=guest_exit scenario=$SCENARIO qemu_exit=$qemu_status logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    if ! grep -q "^sophia_qemu_guest schema=1 status=complete scenario=$SCENARIO$" "$EVIDENCE_FILE" \
        || ! grep -q '^sophia_x_application_session schema=1 status=passed class=gtk3_software client=zenity .*protocol_errors=0 first_error=none physical_text=true pointer_button=true surface_resize=committed buffer_path=cpu_shm native_presentation=enabled cleanup=clean$' "$EVIDENCE_FILE"; then
        echo "sophia_qemu_gtk schema=1 status=failed reason=semantic_evidence scenario=$SCENARIO" | tee -a "$EVIDENCE_FILE"
        exit 1
    fi
    echo "sophia_qemu_gtk schema=1 status=complete scenario=$SCENARIO qemu_exit=0" | tee -a "$EVIDENCE_FILE"
    exit 0
fi

input_ready=false
for _ in $(seq 1 600); do
    if grep -q '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$EVIDENCE_FILE"; then
        input_ready=true
        break
    fi
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        break
    fi
    sleep 0.05
done
if [[ "$input_ready" != true ]]; then
    echo "sophia_qemu_session schema=3 status=failed reason=input_readiness_timeout" | tee -a "$EVIDENCE_FILE"
    exit 1
fi

if ! "$ROOT_DIR/tools/qemu_qmp_type.py" "$QMP_SOCKET" sophia; then
    echo "sophia_qemu_session schema=3 status=failed reason=qmp_input_send" | tee -a "$EVIDENCE_FILE"
    exit 1
fi
echo "sophia_qemu_input schema=1 status=sent source=qmp device=virtio-keyboard text=sophia events=14" | tee -a "$EVIDENCE_FILE"

pointer_ready=false
for _ in $(seq 1 100); do
    if grep -q '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' "$EVIDENCE_FILE"; then
        pointer_ready=true
        break
    fi
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        break
    fi
    sleep 0.05
done
if [[ "$pointer_ready" != true ]]; then
    echo "sophia_qemu_session schema=3 status=failed reason=pointer_readiness_timeout" | tee -a "$EVIDENCE_FILE"
    exit 1
fi

if ! "$ROOT_DIR/tools/qemu_qmp_pointer.py" "$QMP_SOCKET"; then
    echo "sophia_qemu_session schema=3 status=failed reason=qmp_pointer_send" | tee -a "$EVIDENCE_FILE"
    exit 1
fi
echo "sophia_qemu_pointer schema=1 status=sent source=qmp device=virtio-mouse action=select commands=5" | tee -a "$EVIDENCE_FILE"

set +e
wait "$QEMU_PID"
qemu_status=$?
QEMU_PID=""
wait "$LOGGER_PID"
logger_status=$?
LOGGER_PID=""
set -e
cleanup

if [[ "$qemu_status" -ne 0 ]]; then
    echo "sophia_qemu_session schema=3 status=failed qemu_exit=$qemu_status" | tee -a "$EVIDENCE_FILE"
    exit "$qemu_status"
fi
if [[ "$logger_status" -ne 0 ]]; then
    echo "sophia_qemu_session schema=3 status=failed serial_logger_exit=$logger_status" | tee -a "$EVIDENCE_FILE"
    exit "$logger_status"
fi

echo "sophia_qemu_session schema=3 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
if [[ "$TWO_XTERM" == "1" ]]; then
    SOPHIA_QEMU_REQUIRE_TWO_XTERM=1 \
        "$ROOT_DIR/tools/verify_qemu_session_evidence.sh" "$EVIDENCE_FILE"
else
    "$ROOT_DIR/tools/verify_qemu_session_evidence.sh" "$EVIDENCE_FILE"
fi
