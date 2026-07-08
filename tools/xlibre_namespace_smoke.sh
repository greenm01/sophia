#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XSERVER_SRC="${XSERVER_SRC:-/home/niltempus/src/xserver}"
PATCH_FILE="${SOPHIA_ROUTED_INPUT_PATCH:-$ROOT_DIR/patches/xlibre/0001-add-sophia-routed-input-extension.patch}"
PATCHED_SRC="${SOPHIA_XLIBRE_SMOKE_PATCHED_SRC:-/tmp/sophia-xserver-routed-input-smoke-src}"
BUILD_DIR="${XLIBRE_BUILD_DIR:-/tmp/sophia-xlibre-build}"
WORK_DIR="${SOPHIA_XLIBRE_SMOKE_DIR:-/tmp/sophia-xlibre-smoke}"
DISPLAY_NAME="${SOPHIA_XLIBRE_DISPLAY:-:120}"
ROOT_COOKIE="${SOPHIA_XLIBRE_ROOT_COOKIE:-00112233445566778899aabbccddeeff}"
CLIENT_COOKIE="${SOPHIA_XLIBRE_CLIENT_COOKIE:-102132435465768798a9babbdcddedef}"
XVFB="$BUILD_DIR/hw/vfb/Xvfb"
ACTIVE_XSERVER_SRC="$XSERVER_SRC"

if [ ! -d "$XSERVER_SRC" ]; then
    echo "missing XLibre source tree: $XSERVER_SRC" >&2
    exit 1
fi

if ! grep -Rqs "SOPHIA-ROUTED-INPUT" "$XSERVER_SRC/Xext"; then
    if [ ! -f "$PATCH_FILE" ]; then
        echo "missing routed-input patch: $PATCH_FILE" >&2
        exit 1
    fi

    echo "source tree lacks SOPHIA-ROUTED-INPUT; preparing patched smoke tree"
    rm -rf "$PATCHED_SRC"
    rsync -a --delete "$XSERVER_SRC"/ "$PATCHED_SRC"/
    git -C "$PATCHED_SRC" apply "$PATCH_FILE"
    ACTIVE_XSERVER_SRC="$PATCHED_SRC"
    BUILD_DIR="${XLIBRE_BUILD_DIR:-/tmp/sophia-xlibre-routed-input-smoke-build}"
    XVFB="$BUILD_DIR/hw/vfb/Xvfb"
fi

if [ ! -f "$BUILD_DIR/build.ninja" ]; then
    meson setup "$BUILD_DIR" "$ACTIVE_XSERVER_SRC" \
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
else
    meson configure "$BUILD_DIR" -Dxdm-auth-1=false >/dev/null
fi

ninja -C "$BUILD_DIR" hw/vfb/Xvfb

(
    cd "$ROOT_DIR"
    cargo build -q -p sophia-wm-demo
)

mkdir -p "$WORK_DIR"
rm -f "$WORK_DIR/root.xauth" "$WORK_DIR/client.xauth" "$WORK_DIR/client-denied.log"
touch "$WORK_DIR/root.xauth" "$WORK_DIR/client.xauth"

cat >"$WORK_DIR/ns.conf" <<EOF
auth MIT-MAGIC-COOKIE-1 $ROOT_COOKIE

namespace sophia_untrusted root
  auth MIT-MAGIC-COOKIE-1 $CLIENT_COOKIE
  allow mouse-motion
  allow shape
  allow xinput
EOF

xauth -f "$WORK_DIR/root.xauth" add "$DISPLAY_NAME" MIT-MAGIC-COOKIE-1 "$ROOT_COOKIE"
xauth -f "$WORK_DIR/client.xauth" add "$DISPLAY_NAME" MIT-MAGIC-COOKIE-1 "$CLIENT_COOKIE"

"$XVFB" "$DISPLAY_NAME" -screen 0 800x600x24 -nolisten tcp -namespace "$WORK_DIR/ns.conf" &
server_pid=$!
client_pid=""

cleanup() {
    if [ -n "$client_pid" ]; then
        kill "$client_pid" 2>/dev/null || true
        wait "$client_pid" 2>/dev/null || true
    fi

    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
}
trap cleanup EXIT

sleep 1

(
    cd "$ROOT_DIR"
    env DISPLAY="$DISPLAY_NAME" XAUTHORITY="$WORK_DIR/client.xauth" \
        cargo run -q -p sophia-cli -- x-test-client --seconds=60
) &
client_pid=$!

sleep 2

echo "root bridge smoke:"
(
    cd "$ROOT_DIR"
    env DISPLAY="$DISPLAY_NAME" XAUTHORITY="$WORK_DIR/root.xauth" \
        cargo run -q -p sophia-cli -- x-smoke-policy-frame
)

echo "root external-wm bridge smoke:"
(
    cd "$ROOT_DIR"
    env DISPLAY="$DISPLAY_NAME" XAUTHORITY="$WORK_DIR/root.xauth" \
        cargo run -q -p sophia-cli -- x-smoke-external-wm --wm="$ROOT_DIR/target/debug/sophia-wm-demo"
)

echo "root routed-input smoke:"
(
    cd "$ROOT_DIR"
    env DISPLAY="$DISPLAY_NAME" XAUTHORITY="$WORK_DIR/root.xauth" \
        cargo run -q -p sophia-cli -- x-smoke-routed-input
)

echo "root routed-input stress:"
(
    cd "$ROOT_DIR"
    env DISPLAY="$DISPLAY_NAME" XAUTHORITY="$WORK_DIR/root.xauth" \
        cargo run -q -p sophia-cli -- x-stress-routed-input --iterations=1000 --threshold-us=500
)

echo "namespace isolation smoke:"
if (
    cd "$ROOT_DIR"
    env DISPLAY="$DISPLAY_NAME" XAUTHORITY="$WORK_DIR/client.xauth" \
        cargo run -q -p sophia-cli -- x-smoke-policy-frame
) >"$WORK_DIR/client-denied.log" 2>&1; then
    echo "expected namespaced bridge attempt to be denied, but it succeeded" >&2
    exit 1
fi

echo "xnamespace-isolation denied client namespace bridge as expected"
cat "$WORK_DIR/client-denied.log"
