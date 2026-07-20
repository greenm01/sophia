#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH=$(readlink -f -- "${BASH_SOURCE[0]}")
ROOT_DIR=$(cd -- "$(dirname -- "$SCRIPT_PATH")/.." && pwd)
KERNEL_RELEASE=$(uname -r)
KERNEL_IMAGE="/boot/vmlinuz-${KERNEL_RELEASE}"
INITRAMFS="${ROOT_DIR}/.qemu/sophia-${KERNEL_RELEASE}.img"
MEMORY_MIB=${SOPHIA_VM_MEMORY_MIB:-4096}
LOG_FILE=${SOPHIA_VM_LOG:-/tmp/sophia-interactive.log}

if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
    echo "Missing qemu-system-x86_64. Install it with:" >&2
    echo "  sudo xbps-install -S qemu-system-amd64" >&2
    exit 1
fi

if [[ ! -r "$KERNEL_IMAGE" ]]; then
    echo "Kernel image not found: $KERNEL_IMAGE" >&2
    exit 1
fi

if [[ ! -r "$INITRAMFS" ]]; then
    echo "Sophia guest image not found: $INITRAMFS" >&2
    echo "Build the QEMU image from $ROOT_DIR before retrying." >&2
    exit 1
fi

echo "Starting the diskless Void Sophia+xmonad guest."
echo "QEMU will open a native display window and grab input on hover."
echo "Release the mouse and keyboard with Ctrl+Alt+G."
echo "Log: $LOG_FILE"
echo "Stop: press Ctrl+C in this terminal"

export GDK_BACKEND=x11
exec qemu-system-x86_64 \
    -machine q35,accel=kvm:tcg \
    -smp 2 \
    -m "$MEMORY_MIB" \
    -nodefaults \
    -no-reboot \
    -display gtk,gl=off,grab-on-hover=on \
    -monitor none \
    -serial "file:${LOG_FILE}" \
    -device virtio-vga,max_outputs=1 \
    -device virtio-keyboard-pci \
    -device qemu-xhci,id=xhci \
    -device usb-mouse,bus=xhci.0 \
    -kernel "$KERNEL_IMAGE" \
    -initrd "$INITRAMFS" \
    -append "console=ttyS0 quiet loglevel=3 rdinit=/sbin/sophia-qemu-init rd.driver.pre=virtio_pci rd.driver.pre=virtio_gpu rd.driver.pre=virtio_input panic=-1 sophia.scenario=xmonad-m8-soak sophia.two_xterm=0"
