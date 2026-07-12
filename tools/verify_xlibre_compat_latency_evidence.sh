#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_XLIBRE_LATENCY_EVIDENCE:-/tmp/sophia-xlibre-latency.log}}"
MAX_LATENCY_MSEC="${SOPHIA_XLIBRE_MAX_LATENCY_MSEC:-100}"

if [[ ! "$MAX_LATENCY_MSEC" =~ ^[1-9][0-9]*$ ]]; then
    echo "SOPHIA_XLIBRE_MAX_LATENCY_MSEC must be a positive integer" >&2
    exit 1
fi
if [[ ! -s "$EVIDENCE_FILE" ]] || grep -q '^Error:' "$EVIDENCE_FILE"; then
    echo "XLibre latency evidence is missing or contains an error" >&2
    exit 1
fi
if grep -q 'libinput error: .*event processing lagging' "$EVIDENCE_FILE"; then
    echo "XLibre latency evidence contains a libinput processing-lag warning" >&2
    exit 1
fi

mapfile -t completion_lines < <(grep '^sophia_live_session schema=8 status=bounded_complete ' "$EVIDENCE_FILE" || true)
mapfile -t compat_lines < <(grep '^sophia_xlibre_compat schema=1 status=complete ' "$EVIDENCE_FILE" || true)
if [[ "${#completion_lines[@]}" -ne 1 || "${#compat_lines[@]}" -ne 1 ]]; then
    echo "XLibre latency evidence requires one session and one compatibility completion line" >&2
    exit 1
fi

declare -A session=()
read -r -a parts <<< "${completion_lines[0]}"
for field in "${parts[@]:1}"; do
    [[ "$field" == *=* ]] || continue
    key="${field%%=*}"
    if [[ -n "${session[$key]+set}" ]]; then
        echo "XLibre latency evidence has duplicate session field: $key" >&2
        exit 1
    fi
    session["$key"]="${field#*=}"
done
latency="${session[input_presented_latency_msec]:-}"
if [[ "${session[schema]:-}" != "8" || "${session[status]:-}" != "bounded_complete" \
    || "${session[input_pixel_change]:-}" != "true" || ! "$latency" =~ ^[0-9]+$ ]]; then
    echo "XLibre latency evidence is missing a presented input pixel change" >&2
    exit 1
fi
if (( latency > MAX_LATENCY_MSEC )); then
    echo "XLibre presented input latency ${latency}ms exceeds ${MAX_LATENCY_MSEC}ms" >&2
    exit 1
fi

declare -A compat=()
read -r -a parts <<< "${compat_lines[0]}"
for field in "${parts[@]:1}"; do
    [[ "$field" == *=* ]] || continue
    key="${field%%=*}"
    if [[ -n "${compat[$key]+set}" ]]; then
        echo "XLibre latency evidence has duplicate compatibility field: $key" >&2
        exit 1
    fi
    compat["$key"]="${field#*=}"
done
numeric=(full_readbacks patch_readbacks bytes_read max_capture_msec keys_injected max_inject_msec)
for key in "${numeric[@]}"; do
    if [[ ! "${compat[$key]:-}" =~ ^[0-9]+$ ]]; then
        echo "XLibre latency evidence has invalid $key" >&2
        exit 1
    fi
done
if [[ "${compat[schema]:-}" != "1" || "${compat[status]:-}" != "complete" \
    || "${#compat[@]}" -ne 8 ]]; then
    echo "XLibre latency evidence has unknown or missing compatibility fields" >&2
    exit 1
fi
if (( compat[full_readbacks] == 0 || compat[patch_readbacks] == 0 \
    || compat[bytes_read] == 0 || compat[keys_injected] == 0 )); then
    echo "XLibre latency evidence did not exercise replacement, patch, bytes, and input paths" >&2
    exit 1
fi

echo "XLibre compatibility input latency passed: ${latency}ms evidence=$EVIDENCE_FILE"
