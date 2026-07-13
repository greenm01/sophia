#!/bin/bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_WAYLAND_KITTY_EVIDENCE:-/tmp/sophia-wayland-kitty.log}}"
MAX_LATENCY_MSEC="${SOPHIA_WAYLAND_MAX_LATENCY_MSEC:-100}"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "Wayland Kitty evidence is missing: $EVIDENCE_FILE" >&2
    exit 1
fi
if grep -qE 'XLibre|xlibre|Xorg|x_server=enabled|ProtocolError|status=failed' "$EVIDENCE_FILE"; then
    echo "Wayland Kitty evidence contains an X server dependency or failure" >&2
    exit 1
fi
start_count="$(grep -c 'sophia_wayland_session schema=1 status=running .*x_server=disabled' "$EVIDENCE_FILE" || true)"
complete="$(grep 'sophia_wayland_session schema=1 status=complete ' "$EVIDENCE_FILE" | tail -n 1 || true)"
frame_count="$(grep -c 'sophia_wayland_frame schema=1 ' "$EVIDENCE_FILE" || true)"
if [[ "$start_count" != 1 || -z "$complete" || "$frame_count" -lt 1 ]]; then
    echo "Wayland Kitty evidence is missing its bounded session or frame records" >&2
    exit 1
fi
if ! grep -Eq 'sophia_wayland_frame schema=1 .*buffer=(shm|dmabuf) ' "$EVIDENCE_FILE"; then
    echo "Wayland Kitty evidence has no admitted client buffers" >&2
    exit 1
fi
if [[ "${SOPHIA_WAYLAND_REQUIRE_DMABUF:-0}" == 1 ]] \
    && ! grep -Eq 'sophia_wayland_frame schema=1 .*buffer=dmabuf ' "$EVIDENCE_FILE"; then
    echo "Wayland Kitty hardware evidence did not exercise DMA-BUF import" >&2
    exit 1
fi
if [[ "${SOPHIA_WAYLAND_REQUIRE_DMABUF:-0}" == 1 ]]; then
    native="$(grep '^sophia_wayland_native schema=1 status=complete ' "$EVIDENCE_FILE" || true)"
    if [[ "$(grep -c '^sophia_wayland_native schema=1 status=complete ' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
        echo "Wayland Kitty evidence requires one native completion record" >&2
        exit 1
    fi
    native_outputs="$(sed -n 's/.*outputs=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    native_submissions="$(sed -n 's/.*submissions=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    native_retirements="$(sed -n 's/.*retirements=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    native_callbacks="$(sed -n 's/.*callbacks=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    import_attempts="$(sed -n 's/.*dmabuf_import_attempts=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    imports="$(sed -n 's/.*dmabuf_imports=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    submit_latency="$(sed -n 's/.*max_submit_to_page_flip_msec=\([0-9][0-9]*\).*/\1/p' <<<"$native")"
    if [[ "${native_outputs:-0}" -eq 0 || "${native_submissions:-0}" -lt 2 \
        || "${native_retirements:-0}" -eq 0 || "${native_callbacks:-0}" -eq 0 \
        || "${import_attempts:-0}" -eq 0 || "$import_attempts" != "$imports" \
        || -z "$submit_latency" || "$submit_latency" -gt "$MAX_LATENCY_MSEC" \
        || "$native" != *"submit_failures=0"* || "$native" != *"retire_failures=0"* \
        || "$native" != *"callback_rejected=0"* || "$native" != *"in_flight=false"* \
        || "$native" != *"cleanup_pending=false"* ]]; then
        echo "Wayland Kitty native DMA/KMS completion evidence is not clean" >&2
        exit 1
    fi
fi
resize_commits="$(sed -n 's/.*resize_commits=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
if [[ "${SOPHIA_WAYLAND_REQUIRE_RESIZE:-0}" == 1 && "${resize_commits:-0}" -eq 0 ]]; then
    echo "Wayland Kitty evidence did not commit the requested resize" >&2
    exit 1
fi
if ! grep -Eq 'sophia_wayland_frame schema=1 .*buffer=dmabuf |sophia_wayland_frame schema=1 .*buffer=shm .*nonzero_pixel_bytes=[1-9][0-9]*' "$EVIDENCE_FILE"; then
    echo "Wayland Kitty evidence has neither DMA-BUF frames nor nonzero SHM pixels" >&2
    exit 1
fi
routed_input="$(sed -n 's/.*routed_input=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
input_presentations="$(sed -n 's/.*input_presentations=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
if [[ "${routed_input:-0}" -gt 0 && "${input_presentations:-0}" -eq 0 ]]; then
    echo "Wayland Kitty routed input did not reach a presented frame" >&2
    exit 1
fi
if [[ "${SOPHIA_WAYLAND_REQUIRE_INPUT:-0}" == 1 ]]; then
    routed_pointer="$(sed -n 's/.*routed_pointer=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
    pointer_presentations="$(sed -n 's/.*pointer_presentations=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
    keycodes_matched="$(sed -n 's/.*expected_keycodes_matched=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
    keycodes_total="$(sed -n 's/.*expected_keycodes_total=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
    if [[ "${routed_pointer:-0}" -eq 0 || "${pointer_presentations:-0}" -eq 0 \
        || "${keycodes_total:-0}" -eq 0 \
        || "$keycodes_matched" != "$keycodes_total" || "${input_presentations:-0}" -eq 0 ]]; then
        echo "Wayland Kitty input evidence is incomplete" >&2
        exit 1
    fi
fi
if [[ "${SOPHIA_WAYLAND_REQUIRE_RECOVERY:-0}" == 1 ]] \
    && ! grep -q '^sophia_wayland_recovery schema=1 status=complete .*termios_restored=1 keyd_restored=1 processes=0$' "$EVIDENCE_FILE"; then
    echo "Wayland Kitty TTY recovery evidence is missing" >&2
    exit 1
fi
latency="$(sed -n 's/.*max_input_latency_msec=\([0-9][0-9]*\).*/\1/p' <<<"$complete")"
if [[ -z "$latency" || "$latency" -gt "$MAX_LATENCY_MSEC" ]]; then
    echo "Wayland Kitty latency ${latency:-missing}ms exceeds ${MAX_LATENCY_MSEC}ms" >&2
    exit 1
fi

echo "Native Wayland Kitty evidence passed: frames=$frame_count latency=${latency}ms evidence=$EVIDENCE_FILE"
