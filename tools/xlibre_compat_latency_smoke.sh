#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_XLIBRE_LATENCY_DISPLAY:-:179}"
EVIDENCE_FILE="${SOPHIA_XLIBRE_LATENCY_EVIDENCE:-/tmp/sophia-xlibre-latency.log}"
XORG_BIN="${SOPHIA_XLIBRE_XORG:-/usr/libexec/Xorg}"
MODULE_PATH="${SOPHIA_XLIBRE_MODULE_PATH:-/usr/lib/xorg/modules/xlibre-25}"
STATE_DIR="$(mktemp -d /tmp/sophia-xlibre-latency.XXXXXX)"
XORG_PID=""
CLIENT_KIND="${SOPHIA_XLIBRE_LATENCY_CLIENT:-xterm}"
VERIFIER="${SOPHIA_XLIBRE_LATENCY_VERIFIER:-tools/verify_xlibre_compat_latency_evidence.sh}"
EXPECT_VERIFIER_REJECTION="${SOPHIA_XLIBRE_EXPECT_VERIFIER_REJECTION:-0}"

cleanup() {
    if [[ -n "$XORG_PID" ]] && kill -0 "$XORG_PID" 2>/dev/null; then
        kill -TERM "$XORG_PID" 2>/dev/null || true
        wait "$XORG_PID" 2>/dev/null || true
    fi
    rm -rf "$STATE_DIR"
}
trap cleanup EXIT

if [[ ! "$DISPLAY_NAME" =~ ^:[0-9]+$ ]]; then
    echo "SOPHIA_XLIBRE_LATENCY_DISPLAY must have the form :NUMBER" >&2
    exit 1
fi
for command in cargo cvt xdpyinfo "$CLIENT_KIND"; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "missing required command: $command" >&2
        exit 1
    fi
done
if [[ ! -x "$XORG_BIN" || ! -r "$MODULE_PATH/drivers/dummy_drv.so" ]]; then
    echo "XLibre and its dummy video driver are required" >&2
    exit 1
fi

modeline="$(cvt 1280 720 60 | sed -n 's/^Modeline //p')"
if [[ -z "$modeline" ]]; then
    echo "could not generate the dummy XLibre modeline" >&2
    exit 1
fi
cat >"$STATE_DIR/xorg.conf" <<EOF
Section "ServerFlags"
    Option "AutoAddDevices" "false"
    Option "AutoEnableDevices" "false"
    Option "DontVTSwitch" "true"
    Option "DontZap" "true"
EndSection
Section "Device"
    Identifier "SophiaDummy"
    Driver "dummy"
    VideoRam 256000
EndSection
Section "Monitor"
    Identifier "SophiaMonitor"
    HorizSync 5.0-1000.0
    VertRefresh 5.0-200.0
    Modeline $modeline
EndSection
Section "Screen"
    Identifier "SophiaScreen"
    Device "SophiaDummy"
    Monitor "SophiaMonitor"
    DefaultDepth 24
    SubSection "Display"
        Depth 24
        Modes "1280x720_60.00"
        Virtual 1280 720
    EndSubSection
EndSection
EOF

cd "$ROOT_DIR"
cargo build --release --offline -q -p sophia-cli --features atomic-scanout-live

xorg_extension_args=()
if [[ "${SOPHIA_XLIBRE_DISABLE_SHM:-0}" == "1" ]]; then
    xorg_extension_args=(-extension MIT-SHM)
fi
"$XORG_BIN" "$DISPLAY_NAME" \
    -config "$STATE_DIR/xorg.conf" \
    -ac -nolisten tcp -novtswitch -sharevts \
    -modulepath "$MODULE_PATH" \
    "${xorg_extension_args[@]}" \
    -logfile "$STATE_DIR/Xorg.log" >"$STATE_DIR/Xorg.stdout.log" 2>&1 &
XORG_PID=$!
ready=false
for _ in {1..100}; do
    if xdpyinfo -display "$DISPLAY_NAME" >/dev/null 2>&1; then
        ready=true
        break
    fi
    if ! kill -0 "$XORG_PID" 2>/dev/null; then
        break
    fi
    sleep 0.05
done
if [[ "$ready" != true ]]; then
    echo "dummy XLibre did not become ready" >&2
    sed -n '1,160p' "$STATE_DIR/Xorg.log" >&2 || true
    exit 1
fi

mkdir -p "$(dirname "$EVIDENCE_FILE")"
client_args=()
if [[ "$CLIENT_KIND" == "kitty" ]]; then
    client_args=(
        --client-arg=-o
        --client-arg=linux_display_server=x11
        --client-arg=-o
        --client-arg=remember_window_size=no
        --client-arg=-o
        --client-arg=initial_window_width=1280
        --client-arg=-o
        --client-arg=initial_window_height=720
    )
fi
set +e
target/release/sophia sophia-live-session \
    --client-backend=xlibre-compat \
    --compat-display="$DISPLAY_NAME" \
    --client="$CLIENT_KIND" \
    "${client_args[@]}" \
    --max-runtime-ms=6000 \
    --inject-text=sophia \
    --exit-after-input-proof 2>&1 | tee "$EVIDENCE_FILE"
status="${PIPESTATUS[0]}"
set -e
if [[ "$status" -ne 0 ]]; then
    exit "$status"
fi

if [[ "$EXPECT_VERIFIER_REJECTION" == "1" ]]; then
    if "$VERIFIER" "$EVIDENCE_FILE" >/dev/null 2>&1; then
        echo "interactive verifier unexpectedly accepted degraded capture evidence" >&2
        exit 1
    fi
    grep -q '^sophia_xlibre_compat schema=2 status=complete capture_path=get_image_degraded ' "$EVIDENCE_FILE"
    echo "XLibre degraded GetImage fallback remained operational and was rejected for interactive latency"
else
    "$VERIFIER" "$EVIDENCE_FILE"
fi
