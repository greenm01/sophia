#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
verifier="$ROOT_DIR/tools/verify_qemu_xmonad_m7_evidence.sh"
fixture="$ROOT_DIR/tools/fixtures/qemu_xmonad_m7_pass.log"
tmp="$(mktemp /tmp/sophia-xmonad-m7-verifier.XXXXXX)"
trap 'rm -f "$tmp"' EXIT

"$verifier" "$fixture"

expect_failure() {
    local label=$1
    if "$verifier" "$tmp" >/dev/null 2>&1; then
        echo "xmonad M7 verifier accepted invalid evidence: $label" >&2
        exit 1
    fi
}

sed '/action=LaunchTerminal$/d' "$fixture" >"$tmp"
expect_failure missing_launch_action

sed '/status=layout_committed .*surfaces=3 /d' "$fixture" >"$tmp"
expect_failure missing_three_surface_layout

sed 's/wm_degraded=false/wm_degraded=true/' "$fixture" >"$tmp"
expect_failure degraded_wm

sed '/status=restarted /d' "$fixture" >"$tmp"
expect_failure missing_restart

cp "$fixture" "$tmp"
printf '%s\n' 'sophia_qemu_guest schema=1 status=failed reason=test' >>"$tmp"
expect_failure guest_failure

echo "Milestone 7 xmonad verifier regressions passed."
