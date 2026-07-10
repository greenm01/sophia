#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_FILE="${1:-${SOPHIA_ATOMIC_SCANOUT_EVIDENCE:-/tmp/sophia-atomic-scanout-evidence.log}}"
EVIDENCE_PREFIX="sophia_atomic_scanout_evidence"

if [[ ! -s "$EVIDENCE_FILE" ]]; then
    echo "atomic scanout evidence is missing or empty: $EVIDENCE_FILE" >&2
    exit 1
fi

evidence="$(grep -F "$EVIDENCE_PREFIX" "$EVIDENCE_FILE" | tail -n 1 || true)"

if [[ -z "$evidence" ]]; then
    echo "atomic scanout evidence line not found in: $EVIDENCE_FILE" >&2
    exit 1
fi

read -r -a parts <<< "$evidence"
prefix="${parts[0]:-}"
fields=("${parts[@]:1}")

if [[ "$prefix" != "$EVIDENCE_PREFIX" ]]; then
    echo "atomic scanout evidence has wrong prefix: $prefix" >&2
    echo "$evidence" >&2
    exit 1
fi

declare -A observed=()
declare -A expected=(
    ["schema"]="1"
    ["status"]="Passed"
    ["scanout_target"]="Ready"
    ["rendered_context"]="Ready"
    ["gbm_export"]="Exported"
    ["submit"]="SubmittedWaitingForPageFlip"
    ["commit_page_flip_event"]="true"
    ["commit_nonblocking"]="true"
    ["commit_allow_modeset"]="true"
    ["commit_test_only"]="false"
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

    key="${field%%=*}"
    value="${field#*=}"
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

require_field() {
    local key="$1"
    local expected="$2"
    local actual="${observed[$key]:-}"

    if [[ "$actual" != "$expected" ]]; then
        echo "atomic scanout evidence expected $key=$expected, got ${actual:-missing}" >&2
        echo "$evidence" >&2
        exit 1
    fi
}

for key in "${!expected[@]}"; do
    require_field "$key" "${expected[$key]}"
done

echo "atomic scanout evidence passed: $EVIDENCE_FILE"
