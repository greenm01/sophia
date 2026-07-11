#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_VRR_HARDWARE_EVIDENCE:-/tmp/sophia-vrr-hardware.log}}"
EVIDENCE_PREFIX="sophia_vrr_hardware_evidence"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "VRR hardware evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi

mapfile -t evidence_lines < <(grep -F "$EVIDENCE_PREFIX" "$EVIDENCE_FILE" || true)
if [[ "${#evidence_lines[@]}" -ne 2 ]]; then
    echo "VRR hardware evidence expected exactly 2 phase lines, got ${#evidence_lines[@]}" >&2
    exit 1
fi

verify_phase() {
    local phase="$1"
    local eligibility="$2"
    local decision="$3"
    local property_request="$4"
    local evidence=""
    for line in "${evidence_lines[@]}"; do
        if [[ "$line" == *" phase=$phase "* ]]; then
            evidence="$line"
        fi
    done
    if [[ -z "$evidence" ]]; then
        echo "VRR hardware evidence missing phase=$phase" >&2
        exit 1
    fi

    read -r -a parts <<< "$evidence"
    if [[ "${parts[0]:-}" != "$EVIDENCE_PREFIX" ]]; then
        echo "VRR hardware evidence has wrong prefix" >&2
        exit 1
    fi
    declare -A observed=()
    declare -A expected=(
        [schema]="1"
        [phase]="$phase"
        [status]="Passed"
        [discovery]="Discovered"
        [capability]="true"
        [eligibility]="$eligibility"
        [decision]="$decision"
        [property_request]="$property_request"
        [atomic_commit]="Presented"
        [retire]="RetiredAfterPageFlip"
    )
    for field in "${parts[@]:1}"; do
        if [[ "$field" != *=* ]]; then
            echo "VRR hardware evidence has malformed field: $field" >&2
            exit 1
        fi
        local key="${field%%=*}"
        local value="${field#*=}"
        if [[ -n "${observed[$key]+set}" ]]; then
            echo "VRR hardware evidence has duplicate field: $key" >&2
            exit 1
        fi
        if [[ -z "${expected[$key]+set}" ]]; then
            echo "VRR hardware evidence has unknown field: $key" >&2
            exit 1
        fi
        observed[$key]="$value"
    done
    for key in "${!expected[@]}"; do
        if [[ "${observed[$key]:-}" != "${expected[$key]}" ]]; then
            echo "VRR hardware evidence expected $key=${expected[$key]}, got ${observed[$key]:-missing}" >&2
            exit 1
        fi
    done
}

verify_phase Activation Fullscreen Enabled true
verify_phase FixedFallback OverlayPresent Ineligible false
echo "VRR hardware evidence passed: $EVIDENCE_FILE"
