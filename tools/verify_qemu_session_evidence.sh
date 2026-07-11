#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${1:-${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-session.log}}"

"$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$EVIDENCE_FILE"

if [[ "$(grep -c '^sophia_qemu_session schema=2 status=starting isolation=headless display_sink=vnc-unix control=qmp-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu keyboard=virtio mouse=virtio ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the isolated start marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_guest schema=1 status=complete ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the 300-tick guest completion marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_session schema=2 status=complete qemu_exit=0$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the clean host completion marker" >&2
    exit 1
fi
if grep -q '^sophia_qemu_.* status=failed' "$EVIDENCE_FILE"; then
    echo "QEMU evidence contains a failure marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_session_input schema=1 status=ready source=physical text=sophia$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing physical-input readiness" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_input schema=1 status=sent source=qmp device=virtio-keyboard text=sophia events=14$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing QMP keyboard delivery" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_session_pointer schema=1 status=ready source=physical action=select$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing physical-pointer readiness" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_pointer schema=1 status=sent source=qmp device=virtio-mouse action=select commands=5$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing QMP pointer delivery" >&2
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
if [[ ! " $completion_line " =~ " injected_input=false " ]]; then
    echo "QEMU evidence used the internal X11 injection path" >&2
    exit 1
fi
physical_keys="$(sed -n 's/.* physical_keys_routed=\([0-9][0-9]*\) .*/\1/p' <<< "$completion_line")"
if [[ -z "$physical_keys" ]] || (( physical_keys == 0 )); then
    echo "QEMU evidence has no routed virtio keyboard events" >&2
    exit 1
fi
if [[ ! " $completion_line " =~ " pointer_proof=enabled " ]] || [[ ! " $completion_line " =~ " pointer_pixel_change=true " ]]; then
    echo "QEMU evidence did not prove visible pointer input" >&2
    exit 1
fi
physical_pointer="$(sed -n 's/.* physical_pointer_routed=\([0-9][0-9]*\) .*/\1/p' <<< "$completion_line")"
if [[ -z "$physical_pointer" ]] || (( physical_pointer == 0 )); then
    echo "QEMU evidence has no routed virtio mouse events" >&2
    exit 1
fi

echo "QEMU virtio-gpu/QMP keyboard+pointer 300-tick session evidence passed: $EVIDENCE_FILE"
