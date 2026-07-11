#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-persistent.log}}"
PREFIX="sophia_live_session"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "persistent live-session evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi

mapfile -t lines < <(grep -E "^${PREFIX} .*status=bounded_complete " "$EVIDENCE_FILE" || true)
if [[ "${#lines[@]}" -ne 1 ]]; then
    echo "persistent live-session evidence expected exactly 1 completion line, got ${#lines[@]}" >&2
    exit 1
fi

read -r -a parts <<< "${lines[0]}"
declare -A observed=()
for field in "${parts[@]:1}"; do
    if [[ "$field" != *=* ]]; then
        echo "persistent live-session evidence has malformed field: $field" >&2
        exit 1
    fi
    key="${field%%=*}"
    value="${field#*=}"
    if [[ -n "${observed[$key]+set}" ]]; then
        echo "persistent live-session evidence has duplicate field: $key" >&2
        exit 1
    fi
    observed["$key"]="$value"
done

expected_keys=(
    schema status display elapsed_msec session_ticks authority_batches authority_transactions
    authority_queue_capacity authority_batches_dropped backend_ticks
    runtime_committed runtime_surfaces cpu_layers cpu_nonzero_pixel_bytes
    cpu_max_nonzero_pixel_bytes cpu_nonzero_frames cpu_checksum injected_input
    input_pixel_change physical_events physical_keys_routed native_presentation
    native_submissions native_submit_deferred native_submit_failures
    native_retirements native_retire_failures native_max_in_flight_ticks
    native_max_submit_to_page_flip_msec native_callback_accepted
    native_callback_rejected native_callback_queue_saturated
    native_nonzero_exports native_export_attempts native_in_flight
    native_cleanup_pending physical_input
)
if [[ "${#observed[@]}" -ne "${#expected_keys[@]}" ]]; then
    echo "persistent live-session evidence has an unknown or missing field" >&2
    exit 1
fi
for key in "${expected_keys[@]}"; do
    if [[ -z "${observed[$key]+set}" ]]; then
        echo "persistent live-session evidence is missing field: $key" >&2
        exit 1
    fi
done

[[ "${observed[schema]}" == "5" ]]
[[ "${observed[status]}" == "bounded_complete" ]]
[[ "${observed[injected_input]}" == "true" || "${observed[injected_input]}" == "false" ]]
[[ "${observed[input_pixel_change]}" == "true" ]]
[[ "${observed[native_presentation]}" == "enabled" ]]
[[ "${observed[native_in_flight]}" == "false" ]]
[[ "${observed[native_cleanup_pending]}" == "false" ]]

numeric_keys=(
    elapsed_msec session_ticks authority_batches authority_transactions authority_queue_capacity
    authority_batches_dropped backend_ticks runtime_committed runtime_surfaces
    cpu_layers cpu_nonzero_pixel_bytes cpu_max_nonzero_pixel_bytes
    cpu_nonzero_frames cpu_checksum physical_events physical_keys_routed
    native_submissions native_submit_deferred native_submit_failures
    native_retirements native_retire_failures native_max_in_flight_ticks
    native_max_submit_to_page_flip_msec native_callback_accepted
    native_callback_rejected native_callback_queue_saturated
    native_nonzero_exports native_export_attempts
)
for key in "${numeric_keys[@]}"; do
    if [[ ! "${observed[$key]}" =~ ^[0-9]+$ ]]; then
        echo "persistent live-session evidence expected numeric $key" >&2
        exit 1
    fi
done

if [[ "${observed[injected_input]}" == "false" ]]; then
    if [[ "${observed[physical_input]}" != "enabled" ]] || (( observed[physical_keys_routed] == 0 )); then
        echo "persistent live-session physical proof has no routed physical keys" >&2
        exit 1
    fi
fi

positive_keys=(
    elapsed_msec session_ticks authority_batches authority_transactions authority_queue_capacity
    backend_ticks runtime_committed runtime_surfaces cpu_layers
    cpu_max_nonzero_pixel_bytes cpu_nonzero_frames cpu_checksum
    native_submissions native_retirements native_callback_accepted
    native_nonzero_exports native_export_attempts
)
for key in "${positive_keys[@]}"; do
    if (( observed[$key] == 0 )); then
        echo "persistent live-session evidence expected positive $key" >&2
        exit 1
    fi
done

zero_keys=(
    authority_batches_dropped native_submit_failures native_retire_failures
    native_callback_rejected native_callback_queue_saturated
)
for key in "${zero_keys[@]}"; do
    if (( observed[$key] != 0 )); then
        echo "persistent live-session evidence expected zero $key" >&2
        exit 1
    fi
done

if (( observed[backend_ticks] < observed[authority_batches] )); then
    echo "persistent live-session evidence has fewer backend ticks than authority batches" >&2
    exit 1
fi
if (( observed[runtime_committed] != observed[authority_transactions] )); then
    echo "persistent live-session evidence runtime/authority commit mismatch" >&2
    exit 1
fi
if (( observed[native_retirements] > observed[native_submissions] )); then
    echo "persistent live-session evidence retired more frames than it submitted" >&2
    exit 1
fi
if (( observed[native_nonzero_exports] > observed[native_export_attempts] )); then
    echo "persistent live-session evidence has impossible nonzero export count" >&2
    exit 1
fi

echo "persistent live-session evidence passed: $EVIDENCE_FILE"
