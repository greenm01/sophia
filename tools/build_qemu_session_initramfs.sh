#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${SOPHIA_QEMU_OUT_DIR:-$ROOT_DIR/.qemu}"
KERNEL_VERSION="${SOPHIA_QEMU_KERNEL_VERSION:-$(uname -r)}"
KERNEL_IMAGE="${SOPHIA_QEMU_KERNEL:-/boot/vmlinuz-$KERNEL_VERSION}"
INITRAMFS="${SOPHIA_QEMU_INITRAMFS:-$OUT_DIR/sophia-$KERNEL_VERSION.img}"
IMAGE_PROFILE="${SOPHIA_QEMU_IMAGE_PROFILE:-base}"

if [[ "$IMAGE_PROFILE" != base && "$IMAGE_PROFILE" != m8 ]]; then
    echo "SOPHIA_QEMU_IMAGE_PROFILE must be base or m8" >&2
    exit 1
fi

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        exit 1
    fi
}

require_command cargo
require_command dracut
require_command zenity
require_command xterm
require_command readlink
require_command file
require_command ldd
require_command lsinitrd

if [[ ! -r "$KERNEL_IMAGE" ]]; then
    echo "guest kernel is not readable: $KERNEL_IMAGE" >&2
    exit 1
fi
if [[ ! -d "/usr/lib/modules/$KERNEL_VERSION" ]]; then
    echo "guest kernel modules are missing: /usr/lib/modules/$KERNEL_VERSION" >&2
    exit 1
fi

mkdir -p "$OUT_DIR" "$OUT_DIR/dracut-tmp"

BRIDGE_BIN="$ROOT_DIR/target/release/sophia-x11-wm-bridge"
XMONAD_BIN="${SOPHIA_XMONAD_BIN:-}"
if [[ -z "$XMONAD_BIN" ]] && command -v xmonad >/dev/null 2>&1; then
    XMONAD_BIN="$(command -v xmonad)"
fi
if [[ -n "$XMONAD_BIN" && ! -x "$XMONAD_BIN" ]]; then
    echo "configured xmonad binary is not executable: $XMONAD_BIN" >&2
    exit 1
fi
XMONAD_INCLUDE=()
[[ -z "$XMONAD_BIN" ]] || XMONAD_INCLUDE=(--include "$XMONAD_BIN" /usr/bin/xmonad)
(
    cd "$ROOT_DIR"
    cargo build --release --offline -p sophia-cli -p sophia-x11-wm-bridge --features atomic-scanout-live
)

SOPHIA_BIN="$ROOT_DIR/target/release/sophia"
runtime_files=(
    /usr/lib/libEGL.so.1
    /usr/lib/libEGL_mesa.so.0
    /usr/lib/libGLdispatch.so.0
    /usr/lib/libgbm.so.1
    /usr/lib/libGLESv2.so.2
    /usr/lib/libgallium-*.so
    /usr/lib/libdrm.so.2
    /usr/lib/libinput.so.10
    /usr/lib/libudev.so.1
    /usr/bin/zenity
)
extra_includes=()
required_guest_paths=()
if [[ "$IMAGE_PROFILE" == m8 ]]; then
    extra_includes+=(--include "$ROOT_DIR/tools/fixtures/qemu_zenity_launcher.sh" /usr/bin/sophia-zenity-launcher)
    FIREFOX_BIN="${SOPHIA_FIREFOX_BIN:-$(command -v firefox || true)}"
    VKCUBE_BIN="${SOPHIA_VKCUBE_BIN:-$(command -v vkcube || true)}"
    LVP_ICD="${SOPHIA_LVP_ICD:-/usr/share/vulkan/icd.d/lvp_icd.x86_64.json}"
    if [[ -z "$FIREFOX_BIN" || ! -x "$FIREFOX_BIN" ]]; then
        echo "The M8 QEMU profile requires Firefox; set SOPHIA_FIREFOX_BIN." >&2
        exit 1
    fi
    if [[ -z "$VKCUBE_BIN" || ! -x "$VKCUBE_BIN" ]]; then
        echo "The M8 QEMU profile requires vkcube; install Vulkan-Tools or set SOPHIA_VKCUBE_BIN." >&2
        exit 1
    fi
    if [[ ! -r "$LVP_ICD" ]]; then
        echo "The M8 QEMU profile requires Mesa Lavapipe; install mesa-vulkan-lavapipe or set SOPHIA_LVP_ICD." >&2
        exit 1
    fi
    LVP_LIBRARY="$(sed -n 's/.*"library_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$LVP_ICD" | head -n 1)"
    [[ -n "$LVP_LIBRARY" ]] || { echo "Lavapipe ICD has no library_path" >&2; exit 1; }
    [[ "$LVP_LIBRARY" = /* ]] || LVP_LIBRARY="/usr/lib/$LVP_LIBRARY"
    [[ -r "$LVP_LIBRARY" ]] || { echo "Lavapipe library is missing: $LVP_LIBRARY" >&2; exit 1; }
    firefox_dependencies=()
    while IFS= read -r elf; do
        if ! file -b "$elf" | grep -q '^ELF '; then
            continue
        fi
        dependency_report="$(ldd "$elf" 2>&1 || true)"
        if grep -q 'not found' <<<"$dependency_report"; then
            echo "Firefox dependency resolution failed for $elf:" >&2
            grep 'not found' <<<"$dependency_report" >&2
            exit 1
        fi
        while IFS= read -r dependency; do
            [[ -z "$dependency" ]] || firefox_dependencies+=("$dependency")
        done < <(awk '
            $2 == "=>" && $3 ~ /^\// { print $3 }
            $1 ~ /^\// { print $1 }
        ' <<<"$dependency_report")
    done < <(find /usr/lib/firefox -type f -print)
    mapfile -t firefox_dependencies < <(printf '%s\n' "${firefox_dependencies[@]}" | sort -u)
    runtime_files+=("$FIREFOX_BIN" "$VKCUBE_BIN" /usr/lib/libvulkan.so.1 "$LVP_LIBRARY" "${firefox_dependencies[@]}")
    extra_includes+=(
        --include /usr/lib/firefox /usr/lib/firefox
        --include "$LVP_ICD" /usr/share/vulkan/icd.d/lvp_icd.x86_64.json
        --include "$ROOT_DIR/tools/fixtures/firefox_m8_local_page.html" /usr/share/sophia/firefox_m8_local_page.html
    )
    required_guest_paths=(
        /usr/bin/firefox
        /usr/bin/vkcube
        /usr/bin/xmonad
        /usr/lib/libplds4.so
        /usr/lib/libplc4.so
        /usr/lib/libnspr4.so
        /usr/lib/libnss3.so
        /usr/share/vulkan/icd.d/lvp_icd.x86_64.json
        /usr/share/sophia/firefox_m8_local_page.html
    )
fi
install_files=()
runtime_files+=("$(command -v xterm)")
for file in "${runtime_files[@]}"; do
    if [[ -e "$file" ]]; then
        install_files+=("$file")
    fi
done

XKB_DATA_DIR="$(readlink -f /usr/share/X11/xkb)"
if [[ ! -d "$XKB_DATA_DIR" ]]; then
    echo "xkeyboard-config data is missing: /usr/share/X11/xkb" >&2
    exit 1
fi
dracut --force --no-hostonly --no-hostonly-cmdline --no-early-microcode \
    --kver "$KERNEL_VERSION" \
    --tmpdir "$OUT_DIR/dracut-tmp" \
    --force-drivers "virtio_pci virtio_gpu virtio_input evdev" \
    --install "/bin/sh /usr/bin/chmod /usr/bin/mount /usr/bin/modprobe /usr/bin/pidof /usr/bin/poweroff /usr/bin/sleep /usr/bin/sync ${install_files[*]}" \
    --include "$ROOT_DIR/tools/qemu_guest_init.sh" /sbin/sophia-qemu-init \
    --include "$SOPHIA_BIN" /usr/bin/sophia \
    --include "$BRIDGE_BIN" /usr/bin/sophia-x11-wm-bridge \
    "${XMONAD_INCLUDE[@]}" \
    "${extra_includes[@]}" \
    --include /usr/lib/dri /usr/lib/dri \
    --include /usr/lib/gbm /usr/lib/gbm \
    --include /etc/fonts /etc/fonts \
    --include /usr/share/fonts/cantarell /usr/share/fonts/cantarell \
    --include /usr/share/fonts/noto/NotoSans-Regular.ttf \
      /usr/share/fonts/noto/NotoSans-Regular.ttf \
    --include /var/lib/dbus/machine-id /var/lib/dbus/machine-id \
    --include /usr/share/glvnd /usr/share/glvnd \
    --include /usr/share/libinput /usr/share/libinput \
    --include /usr/share/glib-2.0/schemas /usr/share/glib-2.0/schemas \
    --include /usr/share/icons/Adwaita /usr/share/icons/Adwaita \
    --include "$XKB_DATA_DIR" "$XKB_DATA_DIR" \
    "$INITRAMFS"

if [[ "$IMAGE_PROFILE" == m8 ]]; then
    initramfs_listing="$(lsinitrd "$INITRAMFS")"
    for guest_path in "${required_guest_paths[@]}"; do
        if ! grep -Fq " ${guest_path#/}" <<<"$initramfs_listing"; then
            echo "M8 initramfs is missing required path: $guest_path" >&2
            exit 1
        fi
    done
fi

echo "Sophia QEMU guest initramfs built"
echo "Kernel: $KERNEL_IMAGE"
echo "Initramfs: $INITRAMFS"
