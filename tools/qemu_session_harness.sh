#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SOPHIA_QEMU_OUT_DIR:-$ROOT_DIR/.qemu}"
KERNEL_VERSION="${SOPHIA_QEMU_KERNEL_VERSION:-$(uname -r)}"
KERNEL_IMAGE="${SOPHIA_QEMU_KERNEL:-/boot/vmlinuz-$KERNEL_VERSION}"
INITRAMFS="${SOPHIA_QEMU_INITRAMFS:-$OUT_DIR/sophia-$KERNEL_VERSION.img}"
SCENARIO="${SOPHIA_QEMU_SCENARIO:-session}"
TWO_XTERM="${SOPHIA_QEMU_TWO_XTERM:-0}"
if [[ "$SCENARIO" != "session" && "$SCENARIO" != "emergency-recovery" ]]; then
    echo "SOPHIA_QEMU_SCENARIO must be session or emergency-recovery" >&2
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
            && grep -q '^sophia_live_session_input_pipeline schema=1 status=poller_ready ' "$EVIDENCE_FILE" \
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
