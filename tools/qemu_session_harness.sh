#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SOPHIA_QEMU_OUT_DIR:-$ROOT_DIR/.qemu}"
KERNEL_VERSION="${SOPHIA_QEMU_KERNEL_VERSION:-$(uname -r)}"
KERNEL_IMAGE="${SOPHIA_QEMU_KERNEL:-/boot/vmlinuz-$KERNEL_VERSION}"
INITRAMFS="${SOPHIA_QEMU_INITRAMFS:-$OUT_DIR/sophia-$KERNEL_VERSION.img}"
EVIDENCE_FILE="${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-session.log}"
QEMU_BIN="${SOPHIA_QEMU_BIN:-qemu-system-x86_64}"
MEMORY_MIB="${SOPHIA_QEMU_MEMORY_MIB:-2048}"
VNC_SOCKET="${SOPHIA_QEMU_VNC_SOCKET:-$OUT_DIR/display.sock}"

cleanup() {
    rm -f "$VNC_SOCKET"
}
trap cleanup EXIT

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
    echo "missing qemu-system-x86_64; on Void install it with:" >&2
    echo "  sudo xbps-install -S qemu-system-amd64" >&2
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
rm -f "$VNC_SOCKET"

echo "sophia_qemu_session schema=1 status=starting isolation=headless display_sink=vnc-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu ticks=300" | tee -a "$EVIDENCE_FILE"

set +e
"$QEMU_BIN" \
    -machine q35,accel=kvm:tcg \
    -smp 2 \
    -m "$MEMORY_MIB" \
    -nodefaults \
    -no-reboot \
    -display none \
    -vnc "unix:$VNC_SOCKET" \
    -monitor none \
    -serial stdio \
    -device virtio-vga \
    -device virtio-keyboard-pci \
    -kernel "$KERNEL_IMAGE" \
    -initrd "$INITRAMFS" \
    -append "console=ttyS0 quiet loglevel=3 rdinit=/sbin/sophia-qemu-init rd.driver.pre=virtio_pci rd.driver.pre=virtio_gpu rd.driver.pre=virtio_input panic=-1" \
    2>&1 | tr -d '\r' | tee -a "$EVIDENCE_FILE"
qemu_status="${PIPESTATUS[0]}"
set -e
cleanup

if [[ "$qemu_status" -ne 0 ]]; then
    echo "sophia_qemu_session schema=1 status=failed qemu_exit=$qemu_status" | tee -a "$EVIDENCE_FILE"
    exit "$qemu_status"
fi

echo "sophia_qemu_session schema=1 status=complete qemu_exit=0" | tee -a "$EVIDENCE_FILE"
"$ROOT_DIR/tools/verify_qemu_session_evidence.sh" "$EVIDENCE_FILE"
