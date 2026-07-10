#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_RUNTIME_RENDERED_SCANOUT_EVIDENCE:-/tmp/sophia-runtime-rendered-scanout.log}}"
SUBMIT_PREFIX="sophia_runtime_rendered_scanout_submit"
RETIRE_PREFIX="sophia_runtime_rendered_scanout_retire"
CLEANUP_PREFIX="sophia_runtime_rendered_scanout_cleanup"
FAILURE_PREFIX="sophia_runtime_rendered_scanout_failure"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "runtime rendered scanout evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi

mapfile -t evidence_lines < <(grep -E "^($SUBMIT_PREFIX|$RETIRE_PREFIX|$CLEANUP_PREFIX|$FAILURE_PREFIX) " "$EVIDENCE_FILE" || true)
mapfile -t submit_lines < <(grep -F "$SUBMIT_PREFIX" "$EVIDENCE_FILE" || true)
mapfile -t retire_lines < <(grep -F "$RETIRE_PREFIX" "$EVIDENCE_FILE" || true)
mapfile -t cleanup_lines < <(grep -F "$CLEANUP_PREFIX" "$EVIDENCE_FILE" || true)
mapfile -t failure_lines < <(grep -F "$FAILURE_PREFIX" "$EVIDENCE_FILE" || true)

if [[ "${#evidence_lines[@]}" -eq 0 ]]; then
    echo "runtime rendered scanout evidence lines not found in: $EVIDENCE_FILE" >&2
    exit 1
fi
if [[ "${#failure_lines[@]}" -ne 0 ]]; then
    echo "runtime rendered scanout evidence reported failure lines" >&2
    printf '%s\n' "${evidence_lines[@]}" >&2
    exit 1
fi
if [[ "${#submit_lines[@]}" -ne 1 ]]; then
    echo "runtime rendered scanout evidence expected exactly 1 submit line, got ${#submit_lines[@]}" >&2
    printf '%s\n' "${evidence_lines[@]}" >&2
    exit 1
fi
if [[ "${#retire_lines[@]}" -ne 1 ]]; then
    echo "runtime rendered scanout evidence expected exactly 1 retire line, got ${#retire_lines[@]}" >&2
    printf '%s\n' "${evidence_lines[@]}" >&2
    exit 1
fi
if [[ "${#cleanup_lines[@]}" -ne 0 ]]; then
    echo "runtime rendered scanout evidence expected no cleanup retry lines for a clean proof" >&2
    printf '%s\n' "${evidence_lines[@]}" >&2
    exit 1
fi

verify_line() {
    local evidence="$1"
    local prefix="$2"
    local -n expected_ref="$3"

    read -r -a parts <<< "$evidence"
    local observed_prefix="${parts[0]:-}"
    local fields=("${parts[@]:1}")

    if [[ "$observed_prefix" != "$prefix" ]]; then
        echo "runtime rendered scanout evidence has wrong prefix: $observed_prefix" >&2
        echo "$evidence" >&2
        exit 1
    fi

    declare -A observed=()
    for field in "${fields[@]}"; do
        if [[ "$field" != *=* ]]; then
            echo "runtime rendered scanout evidence has malformed field: $field" >&2
            echo "$evidence" >&2
            exit 1
        fi

        local key="${field%%=*}"
        local value="${field#*=}"
        if [[ -n "${observed[$key]+set}" ]]; then
            echo "runtime rendered scanout evidence has duplicate field: $key" >&2
            echo "$evidence" >&2
            exit 1
        fi
        if [[ -z "${expected_ref[$key]+set}" ]]; then
            echo "runtime rendered scanout evidence has unknown field: $key" >&2
            echo "$evidence" >&2
            exit 1
        fi
        observed["$key"]="$value"
    done

    for key in "${!expected_ref[@]}"; do
        local actual="${observed[$key]:-}"
        if [[ "$key" == "framebuffer" ]]; then
            case "$actual" in
                CreatedWithAddFb2|CreatedWithAddFb2Modifiers|CreatedWithLegacyAddFb)
                    continue
                    ;;
            esac
        fi
        if [[ "$key" == "buffer_format" ]]; then
            case "$actual" in
                Xrgb8888|Argb8888)
                    continue
                    ;;
            esac
        fi
        if [[ "$key" == "buffer_modifier" ]]; then
            case "$actual" in
                Implicit|Linear|NonLinear)
                    continue
                    ;;
            esac
        fi
        if [[ "$key" == "buffer_planes" ]]; then
            case "$actual" in
                Single|Multiple)
                    continue
                    ;;
            esac
        fi
        if [[ "$key" == "format_table" ]]; then
            case "$actual" in
                Present|Missing)
                    continue
                    ;;
            esac
        fi
        if [[ "$key" == "output_size" || "$key" == "target_size" ]]; then
            if [[ "$actual" =~ ^[1-9][0-9]*x[1-9][0-9]*$ ]]; then
                continue
            fi
        fi
        if [[ "$actual" != "${expected_ref[$key]}" ]]; then
            echo "runtime rendered scanout evidence expected $key=${expected_ref[$key]}, got ${actual:-missing}" >&2
            echo "$evidence" >&2
            exit 1
        fi
    done

    if [[ -n "${observed[output_size]+set}" && -n "${observed[target_size]+set}" \
        && "${observed[output_size]}" != "${observed[target_size]}" ]]; then
        echo "runtime rendered scanout evidence expected output_size to match target_size" >&2
        echo "$evidence" >&2
        exit 1
    fi
}

declare -A expected_submit=(
    ["schema"]="6"
    ["status"]="SubmittedWaitingForPageFlip"
    ["scanout_target"]="Ready"
    ["output_size"]="MatchingReducedSize"
    ["target"]="Ready"
    ["target_size"]="MatchingReducedSize"
    ["export"]="Exported"
    ["scanout_buffer"]="Ready"
    ["buffer_format"]="SupportedBufferFormat"
    ["buffer_modifier"]="SupportedBufferModifier"
    ["buffer_planes"]="SupportedBufferPlanes"
    ["properties"]="Discovered"
    ["format_table"]="KnownFormatTableState"
    ["resources"]="Created"
    ["framebuffer"]="CreatedFramebuffer"
    ["request"]="Built"
    ["submit"]="SubmittedWaitingForPageFlip"
    ["request_scope"]="PageFlip"
    ["commit_page_flip_event"]="true"
    ["commit_nonblocking"]="true"
    ["commit_allow_modeset"]="false"
    ["commit_test_only"]="false"
    ["commit_submit"]="Submitted"
    ["runtime_scanout_state"]="Submitted"
    ["in_flight"]="true"
    ["in_flight_ticks"]="0"
    ["cleanup_pending"]="false"
)

declare -A expected_retire=(
    ["schema"]="1"
    ["status"]="RetiredAfterPageFlip"
    ["destroy"]="Destroyed"
    ["runtime_scanout_state"]="Retired"
    ["in_flight"]="false"
    ["in_flight_ticks"]="0"
    ["cleanup_pending"]="false"
)

verify_line "${submit_lines[0]}" "$SUBMIT_PREFIX" expected_submit
verify_line "${retire_lines[0]}" "$RETIRE_PREFIX" expected_retire

echo "runtime rendered scanout evidence passed: $EVIDENCE_FILE"
