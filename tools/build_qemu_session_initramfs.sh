#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SOPHIA_QEMU_OUT_DIR:-$ROOT_DIR/.qemu}"
KERNEL_VERSION="${SOPHIA_QEMU_KERNEL_VERSION:-$(uname -r)}"
KERNEL_IMAGE="${SOPHIA_QEMU_KERNEL:-/boot/vmlinuz-$KERNEL_VERSION}"
INITRAMFS="${SOPHIA_QEMU_INITRAMFS:-$OUT_DIR/sophia-$KERNEL_VERSION.img}"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        exit 1
    fi
}

require_command cargo
require_command dracut
require_command xterm

if [[ ! -r "$KERNEL_IMAGE" ]]; then
    echo "guest kernel is not readable: $KERNEL_IMAGE" >&2
    exit 1
fi
if [[ ! -d "/usr/lib/modules/$KERNEL_VERSION" ]]; then
    echo "guest kernel modules are missing: /usr/lib/modules/$KERNEL_VERSION" >&2
    exit 1
fi

mkdir -p "$OUT_DIR" "$OUT_DIR/dracut-tmp"

(
    cd "$ROOT_DIR"
    cargo build --release --offline -p sophia-cli --features atomic-scanout-live
)

SOPHIA_BIN="$ROOT_DIR/target/release/sophia"
runtime_files=(
    /usr/lib/libEGL.so.1
    /usr/lib/libEGL_mesa.so.0
    /usr/lib/libGLdispatch.so.0
    /usr/lib/libgbm.so.1
    /usr/lib/libgallium-*.so
    /usr/lib/libdrm.so.2
    /usr/lib/libinput.so.10
    /usr/lib/libudev.so.1
)
install_files=()
for file in "${runtime_files[@]}"; do
    if [[ -e "$file" ]]; then
        install_files+=("$file")
    fi
done

dracut --force --no-hostonly --no-hostonly-cmdline --no-early-microcode \
    --kver "$KERNEL_VERSION" \
    --tmpdir "$OUT_DIR/dracut-tmp" \
    --force-drivers "virtio_pci virtio_gpu virtio_input evdev" \
    --install "/bin/sh /usr/bin/chmod /usr/bin/mount /usr/bin/modprobe /usr/bin/poweroff /usr/bin/sleep /usr/bin/sync /usr/bin/xterm ${install_files[*]}" \
    --include "$ROOT_DIR/tools/qemu_guest_init.sh" /sbin/sophia-qemu-init \
    --include "$SOPHIA_BIN" /usr/bin/sophia \
    --include /usr/lib/dri /usr/lib/dri \
    --include /usr/lib/gbm /usr/lib/gbm \
    --include /etc/fonts /etc/fonts \
    --include /usr/share/glvnd /usr/share/glvnd \
    --include /usr/share/libinput /usr/share/libinput \
    --include /usr/share/X11/app-defaults /usr/share/X11/app-defaults \
    "$INITRAMFS"

echo "Sophia QEMU guest initramfs built"
echo "Kernel: $KERNEL_IMAGE"
echo "Initramfs: $INITRAMFS"
