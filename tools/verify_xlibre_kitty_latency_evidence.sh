#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_XLIBRE_KITTY_EVIDENCE:-/tmp/sophia-xlibre-kitty-latency.log}}"
MAX_LATENCY_MSEC="${SOPHIA_XLIBRE_MAX_LATENCY_MSEC:-100}"
MAX_READBACK_BYTES=$((1280 * 720 * 4))

if [[ ! "$MAX_LATENCY_MSEC" =~ ^[1-9][0-9]*$ ]]; then
    echo "SOPHIA_XLIBRE_MAX_LATENCY_MSEC must be a positive integer" >&2
    exit 1
fi
if [[ ! -s "$EVIDENCE_FILE" ]] || grep -q '^Error:' "$EVIDENCE_FILE"; then
    echo "Kitty latency evidence is missing or contains an error" >&2
    exit 1
fi
if grep -q 'libinput error: .*event processing lagging' "$EVIDENCE_FILE"; then
    echo "Kitty latency evidence contains a libinput processing-lag warning" >&2
    exit 1
fi

mapfile -t completion_lines < <(grep '^sophia_live_session schema=8 status=bounded_complete ' "$EVIDENCE_FILE" || true)
mapfile -t compat_lines < <(grep '^sophia_xlibre_compat schema=2 status=complete ' "$EVIDENCE_FILE" || true)
if [[ "${#completion_lines[@]}" -ne 1 || "${#compat_lines[@]}" -ne 1 ]]; then
    echo "Kitty latency evidence requires one session and one compatibility completion line" >&2
    exit 1
fi

declare -A session=()
read -r -a parts <<< "${completion_lines[0]}"
for field in "${parts[@]:1}"; do
    [[ "$field" == *=* ]] || continue
    key="${field%%=*}"
    [[ -z "${session[$key]+set}" ]] || { echo "duplicate session field: $key" >&2; exit 1; }
    session["$key"]="${field#*=}"
done
latency="${session[input_presented_latency_msec]:-}"
if [[ "${session[schema]:-}" != "8" || "${session[status]:-}" != "bounded_complete" \
    || "${session[input_pixel_change]:-}" != "true" || ! "$latency" =~ ^[0-9]+$ ]]; then
    echo "Kitty latency evidence is missing a presented input pixel change" >&2
    exit 1
fi
if (( latency > MAX_LATENCY_MSEC )); then
    echo "Kitty presented input latency ${latency}ms exceeds ${MAX_LATENCY_MSEC}ms" >&2
    exit 1
fi

declare -A compat=()
read -r -a parts <<< "${compat_lines[0]}"
for field in "${parts[@]:1}"; do
    [[ "$field" == *=* ]] || continue
    key="${field%%=*}"
    [[ -z "${compat[$key]+set}" ]] || { echo "duplicate compatibility field: $key" >&2; exit 1; }
    compat["$key"]="${field#*=}"
done
numeric=(shm_fallbacks full_readbacks patch_readbacks bytes_read max_readback_bytes max_capture_msec keys_injected max_inject_msec)
for key in "${numeric[@]}"; do
    [[ "${compat[$key]:-}" =~ ^[0-9]+$ ]] || { echo "invalid compatibility field: $key" >&2; exit 1; }
done
if [[ "${compat[schema]:-}" != "2" || "${compat[status]:-}" != "complete" \
    || "${compat[capture_path]:-}" != "mit_shm" || "${compat[shm_fallbacks]}" != "0" \
    || "${#compat[@]}" -ne 11 ]]; then
    echo "Kitty interactive evidence requires an unfallbacked MIT-SHM capture path" >&2
    exit 1
fi
if (( compat[full_readbacks] == 0 || compat[bytes_read] == 0 || compat[keys_injected] == 0 )); then
    echo "Kitty latency evidence did not exercise replacement, bytes, and input paths" >&2
    exit 1
fi
if (( compat[max_readback_bytes] > MAX_READBACK_BYTES )); then
    echo "Kitty readback ${compat[max_readback_bytes]} bytes exceeds the 1280x720 XRGB budget" >&2
    exit 1
fi

if [[ "${session[native_presentation]:-disabled}" == "enabled" ]]; then
    native_numeric=(native_submissions native_submit_failures native_retirements native_retire_failures native_callback_rejected native_callback_queue_saturated)
    for key in "${native_numeric[@]}"; do
        [[ "${session[$key]:-}" =~ ^[0-9]+$ ]] || { echo "invalid native field: $key" >&2; exit 1; }
    done
    if (( session[native_submissions] == 0 || session[native_retirements] == 0 \
        || session[native_submit_failures] != 0 || session[native_retire_failures] != 0 \
        || session[native_callback_rejected] != 0 || session[native_callback_queue_saturated] != 0 )) \
        || [[ "${session[native_in_flight]:-}" != "false" \
            || "${session[native_cleanup_pending]:-}" != "false" ]]; then
        echo "Kitty native scanout evidence did not finish cleanly" >&2
        exit 1
    fi
fi

echo "XLibre Kitty input latency passed: ${latency}ms readback=${compat[max_readback_bytes]}B evidence=$EVIDENCE_FILE"
