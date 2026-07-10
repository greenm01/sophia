#!/usr/bin/env bash
set -euo pipefail

PREFLIGHT_FILE="${1:-${SOPHIA_ATOMIC_SCANOUT_PREFLIGHT:-/tmp/sophia-atomic-scanout-preflight.log}}"
PREFLIGHT_PREFIX="sophia_atomic_scanout_preflight"

if [[ ! -s "$PREFLIGHT_FILE" ]]; then
    echo "atomic scanout preflight is missing or empty: $PREFLIGHT_FILE" >&2
    exit 1
fi

preflight="$(grep -F "$PREFLIGHT_PREFIX" "$PREFLIGHT_FILE" | tail -n 1 || true)"

if [[ -z "$preflight" ]]; then
    echo "atomic scanout preflight line not found in: $PREFLIGHT_FILE" >&2
    exit 1
fi

read -r -a parts <<< "$preflight"
prefix="${parts[0]:-}"
fields=("${parts[@]:1}")

if [[ "$prefix" != "$PREFLIGHT_PREFIX" ]]; then
    echo "atomic scanout preflight has wrong prefix: $prefix" >&2
    echo "$preflight" >&2
    exit 1
fi

declare -A observed=()
declare -A expected=(
    ["schema"]="1"
    ["target"]="AtomicScanout"
    ["status"]="CandidatePrimaryCardsPresent"
)

for field in "${fields[@]}"; do
    if [[ "$field" != *=* ]]; then
        echo "atomic scanout preflight has malformed field: $field" >&2
        echo "$preflight" >&2
        exit 1
    fi

    key="${field%%=*}"
    value="${field#*=}"
    if [[ -n "${observed[$key]+set}" ]]; then
        echo "atomic scanout preflight has duplicate field: $key" >&2
        echo "$preflight" >&2
        exit 1
    fi
    case "$key" in
        schema|target|status|primary_card_nodes) ;;
        *)
            echo "atomic scanout preflight has unknown field: $key" >&2
            echo "$preflight" >&2
            exit 1
            ;;
    esac
    observed["$key"]="$value"
done

require_field() {
    local key="$1"
    local expected="$2"
    local actual="${observed[$key]:-}"

    if [[ "$actual" != "$expected" ]]; then
        echo "atomic scanout preflight expected $key=$expected, got ${actual:-missing}" >&2
        echo "$preflight" >&2
        exit 1
    fi
}

for key in "${!expected[@]}"; do
    require_field "$key" "${expected[$key]}"
done

primary_card_nodes="${observed["primary_card_nodes"]:-}"
if [[ ! "$primary_card_nodes" =~ ^[0-9]+$ ]]; then
    echo "atomic scanout preflight expected numeric primary_card_nodes, got ${primary_card_nodes:-missing}" >&2
    echo "$preflight" >&2
    exit 1
fi
if (( primary_card_nodes < 1 || primary_card_nodes > 8 )); then
    echo "atomic scanout preflight expected 1..8 primary_card_nodes, got $primary_card_nodes" >&2
    echo "$preflight" >&2
    exit 1
fi

echo "atomic scanout preflight passed: $PREFLIGHT_FILE"
