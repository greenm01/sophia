#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
xmonad_bin="$(tools/resolve_xmonad_bin.sh)"
SOPHIA_QEMU_IMAGE_PROFILE=m8 SOPHIA_XMONAD_BIN="$xmonad_bin" tools/build_qemu_session_initramfs.sh
SOPHIA_QEMU_SCENARIO=xmonad-m8-soak SOPHIA_QEMU_MEMORY_MIB="${SOPHIA_QEMU_MEMORY_MIB:-4096}" tools/qemu_session_harness.sh
