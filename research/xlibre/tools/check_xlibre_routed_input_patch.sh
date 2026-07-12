#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XSERVER_SRC="${XSERVER_SRC:-/home/niltempus/src/xserver}"
PATCH_FILE="${SOPHIA_ROUTED_INPUT_PATCH:-$ROOT_DIR/patches/xlibre/0001-add-sophia-routed-input-extension.patch}"
CHECK_DIR="${SOPHIA_XLIBRE_PATCH_CHECK_DIR:-/tmp/sophia-xserver-routed-input-check}"
BUILD_DIR="${SOPHIA_XLIBRE_PATCH_BUILD_DIR:-/tmp/sophia-xserver-routed-input-build}"

if [ ! -d "$XSERVER_SRC" ]; then
    echo "missing XLibre source tree: $XSERVER_SRC" >&2
    exit 1
fi

if [ ! -f "$PATCH_FILE" ]; then
    echo "missing routed-input patch: $PATCH_FILE" >&2
    exit 1
fi

git -C "$XSERVER_SRC" apply --check "$PATCH_FILE"

rm -rf "$CHECK_DIR" "$BUILD_DIR"
rsync -a --delete --exclude .git "$XSERVER_SRC/" "$CHECK_DIR/"
git -C "$CHECK_DIR" apply "$PATCH_FILE"

meson setup "$BUILD_DIR" "$CHECK_DIR" \
    -Dxvfb=true \
    -Dxorg=false \
    -Dxnest=false \
    -Dxephyr=false \
    -Dnamespace=true \
    -Dglamor=false \
    -Dglx=false \
    -Dxdmcp=false \
    -Dxdm-auth-1=false \
    -Dudev=false \
    -Dudev_kms=false \
    -Ddrm=false \
    -Ddri1=false \
    -Ddri2=false \
    -Ddri3=false \
    -Dtests=false

ninja -C "$BUILD_DIR" hw/vfb/Xvfb
