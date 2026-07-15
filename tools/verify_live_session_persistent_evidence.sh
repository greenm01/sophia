#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-persistent.log}}"
PREFIX="sophia_live_session"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "persistent live-session evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi
if grep -q '^Error:' "$EVIDENCE_FILE"; then
    echo "persistent live-session evidence contains a terminal error" >&2
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
    pointer_pixel_change physical_pointer_events physical_pointer_routed pointer_proof
    native_submissions native_submit_deferred native_submit_failures
    native_retirements native_retire_failures native_max_in_flight_ticks
    native_max_submit_to_page_flip_msec native_callback_accepted
    native_callback_rejected native_callback_queue_saturated
    native_nonzero_exports native_export_attempts native_in_flight
    native_cleanup_pending physical_input
)
if [[ "${observed[schema]:-}" == "8" ]]; then
    expected_keys+=(input_presented_latency_msec)
fi
if [[ "${observed[schema]:-}" == "9" || "${observed[schema]:-}" == "10" || "${observed[schema]:-}" == "11" || "${observed[schema]:-}" == "12" ]]; then
    expected_keys+=(
        cpu_max_compose_msec input_presented_latency_msec input_dispatch_max_gap_msec
        input_queue_max_depth input_queue_dwell_max_msec native_max_upload_msec
        native_target_creations native_target_recreations native_pipeline_creations
        native_frame_uploads
    )
fi
if [[ "${observed[schema]:-}" == "10" || "${observed[schema]:-}" == "11" || "${observed[schema]:-}" == "12" ]]; then
    expected_keys+=(input_events_expected input_events_flushed input_flush_latency_msec)
fi
if [[ "${observed[schema]:-}" == "11" || "${observed[schema]:-}" == "12" ]]; then
    expected_keys+=(input_text_match)
fi
if [[ "${observed[schema]:-}" == "7" || "${observed[schema]:-}" == "8" || "${observed[schema]:-}" == "9" || "${observed[schema]:-}" == "10" || "${observed[schema]:-}" == "11" || "${observed[schema]:-}" == "12" ]]; then
    expected_keys+=(wm_policy wm_requests wm_committed wm_restarts wm_degraded)
fi
if [[ "${observed[schema]:-}" == "12" ]]; then
    expected_keys+=(namespace_profile output_update output_notifications surface_resize)
fi
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

[[ "${observed[schema]}" == "6" || "${observed[schema]}" == "7" || "${observed[schema]}" == "8" || "${observed[schema]}" == "9" || "${observed[schema]}" == "10" || "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]
[[ "${observed[status]}" == "bounded_complete" ]]
[[ "${observed[injected_input]}" == "true" || "${observed[injected_input]}" == "false" ]]
[[ "${observed[input_pixel_change]}" == "true" ]]
if [[ "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]; then
    [[ "${observed[input_text_match]}" == "true" ]]
fi
if [[ "${observed[schema]}" == "12" ]]; then
    [[ "${observed[namespace_profile]}" == "classic_shared" || "${observed[namespace_profile]}" == "confined" ]]
    [[ "${observed[output_update]}" == "applied" || "${observed[output_update]}" == "disabled" ]]
    [[ "${observed[surface_resize]}" == "committed" || "${observed[surface_resize]}" == "disabled" ]]
    if [[ "${observed[output_update]}" == "applied" ]] \
        && [[ "$(grep -Ec '^sophia_live_output_update schema=2 status=applied width=[1-9][0-9]* height=[1-9][0-9]* notifications=[1-9][0-9]*$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
        echo "persistent live-session evidence is missing its applied output update" >&2
        exit 1
    fi
    if [[ "${observed[surface_resize]}" == "committed" ]] \
        && [[ "$(grep -Ec '^sophia_live_resize schema=1 status=committed transaction=[1-9][0-9]* surface=[1-9][0-9]* width=[1-9][0-9]* height=[1-9][0-9]* configure_ack=true pixels=true$' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
        echo "persistent live-session evidence is missing committed resize pixels" >&2
        exit 1
    fi
fi
[[ "${observed[native_presentation]}" == "enabled" ]]
[[ "${observed[native_in_flight]}" == "false" ]]
[[ "${observed[native_cleanup_pending]}" == "false" ]]
[[ "${observed[pointer_proof]}" == "enabled" || "${observed[pointer_proof]}" == "disabled" ]]
[[ "${observed[pointer_pixel_change]}" == "true" || "${observed[pointer_pixel_change]}" == "false" ]]

numeric_keys=(
    elapsed_msec session_ticks authority_batches authority_transactions authority_queue_capacity
    authority_batches_dropped backend_ticks runtime_committed runtime_surfaces
    cpu_layers cpu_nonzero_pixel_bytes cpu_max_nonzero_pixel_bytes
    cpu_nonzero_frames cpu_checksum physical_events physical_keys_routed
    physical_pointer_events physical_pointer_routed
    native_submissions native_submit_deferred native_submit_failures
    native_retirements native_retire_failures native_max_in_flight_ticks
    native_max_submit_to_page_flip_msec native_callback_accepted
    native_callback_rejected native_callback_queue_saturated
    native_nonzero_exports native_export_attempts
)
if [[ "${observed[schema]}" == "12" ]]; then
    numeric_keys+=(output_notifications)
fi
if [[ "${observed[schema]}" == "8" ]]; then
    numeric_keys+=(input_presented_latency_msec)
fi
if [[ "${observed[schema]}" == "9" || "${observed[schema]}" == "10" || "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]; then
    numeric_keys+=(
        cpu_max_compose_msec input_presented_latency_msec input_dispatch_max_gap_msec
        input_queue_max_depth input_queue_dwell_max_msec native_max_upload_msec
        native_target_creations native_target_recreations native_pipeline_creations
        native_frame_uploads
    )
fi
if [[ "${observed[schema]}" == "10" || "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]; then
    numeric_keys+=(input_events_expected input_events_flushed input_flush_latency_msec)
fi
if [[ "${observed[schema]}" == "7" || "${observed[schema]}" == "8" || "${observed[schema]}" == "9" || "${observed[schema]}" == "10" || "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]; then
    numeric_keys+=(wm_requests wm_committed wm_restarts)
    [[ "${observed[wm_policy]}" == "disabled" || "${observed[wm_policy]}" == "external" ]]
    [[ "${observed[wm_degraded]}" == "true" || "${observed[wm_degraded]}" == "false" ]]
    if [[ "${observed[wm_policy]}" == "disabled" ]]; then
        (( observed[wm_requests] == 0 && observed[wm_committed] == 0 && observed[wm_restarts] == 0 ))
        [[ "${observed[wm_degraded]}" == "false" ]]
    fi
fi

if [[ "${observed[schema]}" == "10" || "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]; then
    if (( observed[input_events_flushed] != observed[input_events_expected] )) \
        || { [[ "${observed[injected_input]}" == "true" ]] \
            && (( observed[input_events_expected] == 0 )); }; then
        echo "persistent live-session evidence has incomplete X11 input delivery" >&2
        exit 1
    fi
fi
if [[ "${observed[schema]}" == "10" || "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]] && [[ "${observed[input_events_expected]}" != "0" ]]; then
    if [[ "$(grep -Fxc 'sophia_live_session_input_pipeline schema=1 status=terminal_content_ready' "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
        echo "persistent live-session evidence is missing terminal-content readiness" >&2
        exit 1
    fi
    mapfile -t flushed_lines < <(grep -E '^sophia_live_session_input_pipeline schema=2 status=key_flushed ' "$EVIDENCE_FILE" || true)
    if [[ "${#flushed_lines[@]}" -ne 1 ]]; then
        echo "persistent live-session evidence is missing matching flushed X11 input proof" >&2
        exit 1
    fi
    marker_expected="$(sed -n 's/.* expected=\([0-9][0-9]*\) .*/\1/p' <<< "${flushed_lines[0]}")"
    marker_flushed="$(sed -n 's/.* flushed=\([0-9][0-9]*\)$/\1/p' <<< "${flushed_lines[0]}")"
    if [[ -z "$marker_expected" || -z "$marker_flushed" ]] \
        || (( marker_expected == 0 || marker_flushed != marker_expected )); then
        echo "persistent live-session evidence has an incomplete flushed X11 input marker" >&2
        exit 1
    fi
fi
if [[ "${observed[schema]}" == "11" || "${observed[schema]}" == "12" ]]; then
    mapfile -t semantic_lines < <(grep -E '^sophia_live_session_input schema=3 status=semantic_complete source=(synthetic|physical) text_match=true bytes=[0-9]+$' "$EVIDENCE_FILE" || true)
    if [[ "${#semantic_lines[@]}" -ne 1 ]]; then
        echo "persistent live-session evidence is missing exact terminal text confirmation" >&2
        exit 1
    fi
fi
for key in "${numeric_keys[@]}"; do
    if [[ ! "${observed[$key]}" =~ ^[0-9]+$ ]]; then
        echo "persistent live-session evidence expected numeric $key" >&2
        exit 1
    fi
done
if [[ "${observed[schema]}" == "12" ]]; then
    if [[ "${observed[output_update]}" == "applied" ]] && (( observed[output_notifications] == 0 )); then
        echo "persistent live-session output update reached no subscribed X11 client" >&2
        exit 1
    fi
    if [[ "${observed[output_update]}" == "disabled" ]] && (( observed[output_notifications] != 0 )); then
        echo "persistent live-session reported output notifications without an update" >&2
        exit 1
    fi
fi

if [[ "${observed[injected_input]}" == "false" ]]; then
    if [[ "${observed[physical_input]}" != "enabled" ]] || (( observed[physical_keys_routed] == 0 )); then
        echo "persistent live-session physical proof has no routed physical keys" >&2
        exit 1
    fi
    mapfile -t input_lines < <(grep -E '^sophia_live_session_input schema=2 status=complete ' "$EVIDENCE_FILE" || true)
    if [[ "${#input_lines[@]}" -ne 1 ]]; then
        echo "persistent live-session evidence expected exactly 1 physical input completion line, got ${#input_lines[@]}" >&2
        exit 1
    fi
    read -r -a input_parts <<< "${input_lines[0]}"
    declare -A input_observed=()
    for field in "${input_parts[@]:1}"; do
        if [[ "$field" != *=* ]]; then
            echo "persistent live-session physical input evidence has malformed field: $field" >&2
            exit 1
        fi
        key="${field%%=*}"
        value="${field#*=}"
        if [[ -n "${input_observed[$key]+set}" ]]; then
            echo "persistent live-session physical input evidence has duplicate field: $key" >&2
            exit 1
        fi
        input_observed["$key"]="$value"
    done
    input_expected_keys=(schema status source text expected_events matched_events pixel_change)
    if [[ "${#input_observed[@]}" -ne "${#input_expected_keys[@]}" ]]; then
        echo "persistent live-session physical input evidence has an unknown or missing field" >&2
        exit 1
    fi
    for key in "${input_expected_keys[@]}"; do
        if [[ -z "${input_observed[$key]+set}" ]]; then
            echo "persistent live-session physical input evidence is missing field: $key" >&2
            exit 1
        fi
    done
    if [[ "${input_observed[schema]}" != "2" \
        || "${input_observed[status]}" != "complete" \
        || "${input_observed[source]}" != "physical" \
        || "${input_observed[pixel_change]}" != "true" \
        || ! "${input_observed[text]}" =~ ^[a-z]{1,24}$ \
        || ! "${input_observed[expected_events]}" =~ ^[0-9]+$ \
        || ! "${input_observed[matched_events]}" =~ ^[0-9]+$ ]]; then
        echo "persistent live-session physical input evidence is invalid" >&2
        exit 1
    fi
    expected_events=$(( (${#input_observed[text]} + 1) * 2 ))
    if [[ "$(grep -Fxc "sophia_live_session_input schema=1 status=ready source=physical text=${input_observed[text]}" "$EVIDENCE_FILE" || true)" -ne 1 ]]; then
        echo "persistent live-session physical input evidence is missing matching readiness" >&2
        exit 1
    fi
    if (( input_observed[expected_events] != expected_events \
        || input_observed[matched_events] != expected_events \
        || observed[physical_keys_routed] != expected_events )); then
        echo "persistent live-session physical input evidence did not match the exact sequence" >&2
        exit 1
    fi
elif grep -q '^sophia_live_session_input schema=2 status=complete ' "$EVIDENCE_FILE"; then
    echo "persistent live-session injected proof contains physical input completion evidence" >&2
    exit 1
fi
if [[ "${observed[pointer_proof]}" == "enabled" ]]; then
    if [[ "${observed[pointer_pixel_change]}" != "true" ]] || (( observed[physical_pointer_routed] == 0 )); then
        echo "persistent live-session pointer proof has no routed pixel change" >&2
        exit 1
    fi
elif [[ "${observed[pointer_pixel_change]}" != "false" ]]; then
    echo "persistent live-session evidence claims pointer pixels without a pointer proof" >&2
    exit 1
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
if (( observed[physical_pointer_routed] > observed[physical_pointer_events] )); then
    echo "persistent live-session evidence routed more pointer events than it observed" >&2
    exit 1
fi

echo "persistent live-session evidence passed: $EVIDENCE_FILE"
