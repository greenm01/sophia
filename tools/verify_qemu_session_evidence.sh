#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${1:-${SOPHIA_QEMU_EVIDENCE:-/tmp/sophia-qemu-session.log}}"

"$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$EVIDENCE_FILE"

if [[ "$(grep -c '^sophia_qemu_session schema=3 status=starting isolation=headless display_sink=vnc-unix control=qmp-unix host_drm=none host_vt=none guest_network=none storage=none gpu=virtio-gpu gpu_devices=2 gpu_heads=2 keyboard=virtio mouse=virtio ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the isolated start marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_guest schema=1 status=complete ticks=300$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the 300-tick guest completion marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_session schema=3 status=complete qemu_exit=0$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the clean host completion marker" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_qemu_topology schema=1 status=observed requested_heads=2 connectors=2 connected=2$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing two connected virtual outputs" >&2
    exit 1
fi
if [[ "$(grep -c '^sophia_live_outputs schema=2 status=ready discovered=2 presentation=2 native_owned=2 multi_output_scanout=enabled layout=extended_horizontal$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing complete two-output native ownership" >&2
    exit 1
fi
mapfile -t output_lines < <(grep '^sophia_live_output schema=1 status=complete ' "$EVIDENCE_FILE" || true)
if [[ "${#output_lines[@]}" -ne 2 ]]; then
    echo "QEMU evidence must contain exactly two per-output completion records" >&2
    exit 1
fi
declare -A output_checksums=()
for output_line in "${output_lines[@]}"; do
    for field in submissions retirements callbacks nonzero_exports; do
        value="$(sed -n "s/.* ${field}=\([0-9][0-9]*\).*/\1/p" <<< "$output_line")"
        if [[ -z "$value" ]] || (( value == 0 )); then
            echo "QEMU output evidence has no $field: $output_line" >&2
            exit 1
        fi
    done
    checksum="$(sed -n 's/.* checksum=\([0-9][0-9]*\) .*/\1/p' <<< "$output_line")"
    if [[ -z "$checksum" ]] || [[ -n "${output_checksums[$checksum]:-}" ]]; then
        echo "QEMU output evidence does not contain distinct checksums" >&2
        exit 1
    fi
    output_checksums[$checksum]=1
done
if [[ "$(grep -c '^sophia_live_vsync schema=1 status=complete outputs=2 overlap_rejections=0 phase_rejections=0 policy=page_flip_paced$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
    echo "QEMU evidence is missing the per-output fixed-refresh vsync gate" >&2
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

echo "QEMU dual-output native presentation/QMP-input 300-tick evidence passed: $EVIDENCE_FILE"
