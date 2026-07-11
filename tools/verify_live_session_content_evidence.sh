#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_LIVE_SESSION_CONTENT_EVIDENCE:-/tmp/sophia-live-session-content-scanout.log}}"
PREFIX="sophia_live_session_content_scanout"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "live-session content evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi

mapfile -t lines < <(grep -E "^${PREFIX} " "$EVIDENCE_FILE" || true)
if [[ "${#lines[@]}" -ne 1 ]]; then
    echo "live-session content evidence expected exactly 1 line, got ${#lines[@]}" >&2
    exit 1
fi

read -r -a parts <<< "${lines[0]}"
declare -A observed=()
for field in "${parts[@]:1}"; do
    if [[ "$field" != *=* ]]; then
        echo "live-session content evidence has malformed field: $field" >&2
        exit 1
    fi
    key="${field%%=*}"
    value="${field#*=}"
    if [[ -n "${observed[$key]+set}" ]]; then
        echo "live-session content evidence has duplicate field: $key" >&2
        exit 1
    fi
    observed["$key"]="$value"
done

expected_keys=(
    schema status width height layers nonzero_pixel_bytes requested_checksum
    exported_checksum export_attempts export_status frame_pending scanout_clean
)
if [[ "${#observed[@]}" -ne "${#expected_keys[@]}" ]]; then
    echo "live-session content evidence has an unknown or missing field" >&2
    exit 1
fi
for key in "${expected_keys[@]}"; do
    if [[ -z "${observed[$key]+set}" ]]; then
        echo "live-session content evidence is missing field: $key" >&2
        exit 1
    fi
done

[[ "${observed[schema]}" == "1" ]]
[[ "${observed[status]}" == "Passed" ]]
[[ "${observed[export_attempts]}" == "1" ]]
[[ "${observed[export_status]}" == "Some(Exported)" ]]
[[ "${observed[frame_pending]}" == "false" ]]
[[ "${observed[scanout_clean]}" == "true" ]]
for key in width height layers nonzero_pixel_bytes requested_checksum exported_checksum; do
    if [[ ! "${observed[$key]}" =~ ^[1-9][0-9]*$ ]]; then
        echo "live-session content evidence expected positive numeric $key" >&2
        exit 1
    fi
done
if [[ "${observed[requested_checksum]}" != "${observed[exported_checksum]}" ]]; then
    echo "live-session content evidence checksum mismatch" >&2
    exit 1
fi

echo "live-session content evidence passed: $EVIDENCE_FILE"
