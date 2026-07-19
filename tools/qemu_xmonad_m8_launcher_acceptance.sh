#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
xmonad_bin="${SOPHIA_XMONAD_BIN:-$(command -v xmonad || true)}"
if [[ -z "$xmonad_bin" || ! -x "$xmonad_bin" ]]; then
    echo "Set SOPHIA_XMONAD_BIN or install xmonad in PATH." >&2
    exit 1
fi

cd "$ROOT_DIR"
SOPHIA_XMONAD_BIN="$xmonad_bin" tools/build_qemu_session_initramfs.sh
SOPHIA_QEMU_SCENARIO=xmonad-m8-launcher tools/qemu_session_harness.sh
tools/verify_qemu_xmonad_m8_launcher_evidence.sh "${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-xmonad-m8-launcher.log}"

echo "Milestone 8 normal xmonad launcher acceptance passed."
