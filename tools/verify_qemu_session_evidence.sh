#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${1:-${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-session.log}}"

"$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$EVIDENCE_FILE"

if [[ "$(grep -c '^sophia_qemu_session schema=1 status=starting isolation=headless display_sink=vnc-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the isolated start marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_guest schema=1 status=complete ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the 300-tick guest completion marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_session schema=1 status=complete qemu_exit=0$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the clean host completion marker" >&2
    exit 1
fi
if grep -q '^sophia_qemu_.* status=failed' "$EVIDENCE_FILE"; then
    echo "QEMU evidence contains a failure marker" >&2
    exit 1
fi

completion_line="$(grep -E '^sophia_live_session .*status=bounded_complete ' "$EVIDENCE_FILE")"
if [[ ! " $completion_line " =~ " session_ticks=300 " ]]; then
    echo "QEMU evidence did not complete exactly 300 session ticks" >&2
    exit 1
fi
if [[ ! " $completion_line " =~ " physical_input=enabled " ]]; then
    echo "QEMU evidence did not open the virtual input path" >&2
    exit 1
fi

echo "QEMU virtio-gpu 300-tick session evidence passed: $EVIDENCE_FILE"
