#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISPLAY_NAME="${SOPHIA_XMONAD_LIVE_DISPLAY:-:277}"
EVIDENCE_FILE="${SOPHIA_XMONAD_LIVE_EVIDENCE:-/tmp/sophia-xmonad-live-session.log}"

xmonad_bin="${SOPHIA_XMONAD_BIN:-}"
if [[ -z "$xmonad_bin" ]] && command -v xmonad >/dev/null 2>&1; then
    xmonad_bin="$(command -v xmonad)"
fi
if [[ -z "$xmonad_bin" ]]; then
    xmonad_source="${SOPHIA_XMONAD_SOURCE:-$HOME/src/xmonad}"
    xmonad_out="${SOPHIA_XMONAD_NIX_OUT:-/tmp/sophia-xmonad}"
    if [[ ! -x "$xmonad_out/bin/xmonad" ]]; then
        nix build "$xmonad_source#defaultPackage.x86_64-linux" --out-link "$xmonad_out"
    fi
    xmonad_bin="$xmonad_out/bin/xmonad"
fi

cd "$ROOT_DIR"
cargo build --quiet --offline -p sophia-cli --features atomic-scanout-live
cargo build --quiet --offline -p sophia-x11-wm-bridge

set +e
target/debug/sophia sophia-live-session \
    --display="$DISPLAY_NAME" \
    --max-runtime-ms=10000 \
    --inject-text=sophia \
    --exit-after-input-proof \
    --wm-process="$ROOT_DIR/target/debug/sophia-x11-wm-bridge" \
    --wm-process-arg="--wm=$xmonad_bin" \
    --wm-process-arg=--profile=xmonad \
    --wm-process-arg=--wm-private-alias=xmonad/xmonad-x86_64-linux \
    >"$EVIDENCE_FILE" 2>&1
status=$?
set -e
cat "$EVIDENCE_FILE"
if (( status != 0 )); then
    exit "$status"
fi

grep -Eq '^sophia_live_wm schema=1 status=layout_committed .* moved_surfaces=1 configure_acks=1 outcome=Committed$' "$EVIDENCE_FILE"
completion="$(grep -E '^sophia_live_session schema=(10|11) status=bounded_complete ' "$EVIDENCE_FILE")"
[[ " $completion " == *" input_pixel_change=true "* ]]
[[ " $completion " == *" wm_policy=external "* ]]
[[ " $completion " == *" wm_requests=1 "* ]]
[[ " $completion " == *" wm_committed=1 "* ]]
[[ " $completion " == *" wm_degraded=false "* ]]

echo "real xmonad live-session smoke passed: $EVIDENCE_FILE"
