#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mix="$ROOT_DIR/tools/fixtures/qemu_xmonad_m8_mix_pass.log"
soak="$ROOT_DIR/tools/fixtures/qemu_xmonad_m8_soak_pass.log"
"$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$mix"
"$ROOT_DIR/tools/verify_qemu_xmonad_m8_soak_evidence.sh" "$soak"

tmp="$(mktemp /tmp/sophia-m8-verifier.XXXXXX)"
trap 'rm -f "$tmp"' EXIT
sed '/id=firefox /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing Firefox evidence" >&2
    exit 1
fi
sed '/stage=primary /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing Firefox PRIMARY evidence" >&2
    exit 1
fi
sed '/status=complete stages=6 /d' "$mix" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_mix_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 mix verifier accepted missing Firefox selection evidence" >&2
    exit 1
fi
sed '/cycle=20 /d' "$soak" > "$tmp"
if "$ROOT_DIR/tools/verify_qemu_xmonad_m8_soak_evidence.sh" "$tmp" >/dev/null 2>&1; then
    echo "M8 soak verifier accepted fewer than 20 cycles" >&2
    exit 1
fi
echo "Milestone 8 verifier regressions passed."
