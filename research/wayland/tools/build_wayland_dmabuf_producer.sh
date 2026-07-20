#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-}"

if [[ -z "$OUTPUT" ]]; then
    echo "usage: $0 OUTPUT" >&2
    exit 2
fi
for command in cc pkg-config wayland-scanner; do
    command -v "$command" >/dev/null || {
        echo "Missing required command: $command" >&2
        exit 1
    }
done
pkg-config --exists wayland-client gbm libdrm || {
    echo "Wayland client, GBM, or libdrm development files are unavailable" >&2
    exit 1
}

BUILD_DIR="$(mktemp -d /tmp/sophia-wayland-dmabuf-producer.XXXXXX)"
trap 'rm -rf "$BUILD_DIR"' EXIT
XDG_SHELL_XML="/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml"
DMABUF_XML="/usr/share/wayland-protocols/stable/linux-dmabuf/linux-dmabuf-v1.xml"

for protocol in "$XDG_SHELL_XML" "$DMABUF_XML"; do
    [[ -r "$protocol" ]] || {
        echo "Missing Wayland protocol definition: $protocol" >&2
        exit 1
    }
done

wayland-scanner client-header "$XDG_SHELL_XML" "$BUILD_DIR/xdg-shell-client-protocol.h"
wayland-scanner private-code "$XDG_SHELL_XML" "$BUILD_DIR/xdg-shell-protocol.c"
wayland-scanner client-header "$DMABUF_XML" "$BUILD_DIR/linux-dmabuf-v1-client-protocol.h"
wayland-scanner private-code "$DMABUF_XML" "$BUILD_DIR/linux-dmabuf-v1-protocol.c"

mkdir -p "$(dirname "$OUTPUT")"
cc -std=c11 -O2 -Wall -Wextra -Wpedantic \
    -I"$BUILD_DIR" \
    "$ROOT_DIR/tools/wayland_dmabuf_producer.c" \
    "$BUILD_DIR/xdg-shell-protocol.c" \
    "$BUILD_DIR/linux-dmabuf-v1-protocol.c" \
    $(pkg-config --cflags --libs wayland-client gbm libdrm) \
    -o "$OUTPUT"
