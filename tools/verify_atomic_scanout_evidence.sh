#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_ATOMIC_SCANOUT_EVIDENCE:-/tmp/sophia-atomic-scanout-evidence.log}}"
EVIDENCE_PREFIX="sophia_atomic_scanout_evidence"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "atomic scanout evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi

mapfile -t evidence_lines < <(grep -F "$EVIDENCE_PREFIX" "$EVIDENCE_FILE" || true)

if [[ "${#evidence_lines[@]}" -eq 0 ]]; then
    echo "atomic scanout evidence line not found in: $EVIDENCE_FILE" >&2
    exit 1
fi
if [[ "${#evidence_lines[@]}" -ne 2 ]]; then
    echo "atomic scanout evidence expected exactly 2 phase lines, got ${#evidence_lines[@]}" >&2
    printf '%s\n' "${evidence_lines[@]}" >&2
    exit 1
fi

verify_phase() {
    local phase="$1"
    local request_scope="$2"
    local commit_allow_modeset="$3"
    local evidence=""

    for line in "${evidence_lines[@]}"; do
        if [[ "$line" == *" phase=$phase "* ]]; then
            evidence="$line"
        fi
    done

    if [[ -z "$evidence" ]]; then
        echo "atomic scanout evidence missing phase=$phase" >&2
        printf '%s\n' "${evidence_lines[@]}" >&2
        exit 1
    fi

    read -r -a parts <<< "$evidence"
    local prefix="${parts[0]:-}"
    local fields=("${parts[@]:1}")

    if [[ "$prefix" != "$EVIDENCE_PREFIX" ]]; then
        echo "atomic scanout evidence has wrong prefix: $prefix" >&2
        echo "$evidence" >&2
        exit 1
    fi

    declare -A observed=()
    declare -A expected=(
        ["schema"]="10"
        ["phase"]="$phase"
        ["status"]="Passed"
        ["scanout_target"]="Ready"
        ["rendered_context"]="Ready"
        ["gbm_export"]="Exported"
        ["gbm_export_detail"]="Exported"
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
        ["request_scope"]="$request_scope"
        ["commit_page_flip_event"]="true"
        ["commit_nonblocking"]="true"
        ["commit_allow_modeset"]="$commit_allow_modeset"
        ["commit_test_only"]="false"
        ["page_flip_wait"]="Retired"
        ["page_flip_poll"]="Emitted"
        ["page_flip"]="Presented"
        ["retire"]="RetiredAfterPageFlip"
        ["retire_destroy"]="Destroyed"
        ["retire_cleanup_pending"]="false"
    )

    for field in "${fields[@]}"; do
        if [[ "$field" != *=* ]]; then
            echo "atomic scanout evidence has malformed field: $field" >&2
            echo "$evidence" >&2
            exit 1
        fi

        local key="${field%%=*}"
        local value="${field#*=}"
        if [[ -n "${observed[$key]+set}" ]]; then
            echo "atomic scanout evidence has duplicate field: $key" >&2
            echo "$evidence" >&2
            exit 1
        fi
        if [[ -z "${expected[$key]+set}" ]]; then
            echo "atomic scanout evidence has unknown field: $key" >&2
            echo "$evidence" >&2
            exit 1
        fi
        observed["$key"]="$value"
    done

    for key in "${!expected[@]}"; do
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
        if [[ "$actual" != "${expected[$key]}" ]]; then
            echo "atomic scanout evidence expected $key=${expected[$key]}, got ${actual:-missing}" >&2
            echo "$evidence" >&2
            exit 1
        fi
    done
}

verify_phase "InitialModeset" "Modeset" "true"
verify_phase "SteadyPageFlip" "PageFlip" "false"

echo "atomic scanout evidence passed: $EVIDENCE_FILE"
