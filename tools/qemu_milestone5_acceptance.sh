#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_DIR="${SOPHIA_M5_QEMU_EVIDENCE_DIR:-$ROOT_DIR/.evidence/qemu-milestone5}"
REBUILD="${SOPHIA_M5_QEMU_REBUILD:-1}"

if [[ "$REBUILD" != "0" && "$REBUILD" != "1" ]]; then
    echo "SOPHIA_M5_QEMU_REBUILD must be 0 or 1" >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"

if [[ "$REBUILD" == "1" ]]; then
    "$ROOT_DIR/tools/build_qemu_session_initramfs.sh"
fi

run_scenario() {
    local scenario="$1"
    local evidence="$2"
    shift 2
    env \
        SOPHIA_QEMU_SCENARIO="$scenario" \
        SOPHIA_QEMU_EVIDENCE="$evidence" \
        "$@" \
        "$ROOT_DIR/tools/qemu_session_harness.sh"
}

run_scenario session "$EVIDENCE_DIR/two-xterm.log" SOPHIA_QEMU_TWO_XTERM=1
run_scenario emergency-recovery "$EVIDENCE_DIR/emergency-recovery.log"
run_scenario gtk-classic "$EVIDENCE_DIR/gtk-classic.log"
run_scenario gtk-confined "$EVIDENCE_DIR/gtk-confined.log"

"$ROOT_DIR/tools/verify_live_session_milestone5_gtk_evidence.sh" \
    "$EVIDENCE_DIR/gtk-classic.log" \
    "$EVIDENCE_DIR/gtk-confined.log"

echo "Sophia Milestone 5 unattended QEMU acceptance passed"
echo "Evidence: $EVIDENCE_DIR"
