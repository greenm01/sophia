#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLASSIC_EVIDENCE="${1:-${SOPHIA_MILESTONE3_CLASSIC_EVIDENCE:-/tmp/sophia-milestone3-classic.log}}"
CONFINED_EVIDENCE="${2:-${SOPHIA_MILESTONE3_CONFINED_EVIDENCE:-/tmp/sophia-milestone3-confined.log}}"

verify_profile() {
    local evidence="$1"
    local profile="$2"

    "$ROOT_DIR/tools/verify_live_session_two_xterm_evidence.sh" "$evidence" >/dev/null
    local completion
    completion="$(grep -E '^sophia_live_session schema=13 status=bounded_complete ' "$evidence")"
    if [[ " $completion " != *" namespace_profile=$profile "* ]]; then
        echo "Milestone 3 evidence expected namespace_profile=$profile: $evidence" >&2
        exit 1
    fi
    if [[ " $completion " != *" output_update=applied "* ]]; then
        echo "Milestone 3 evidence requires the deterministic Engine output update: $evidence" >&2
        exit 1
    fi
    if [[ ! " $completion " =~ [[:space:]]output_notifications=[1-9][0-9]*[[:space:]] ]]; then
        echo "Milestone 3 evidence requires delivered RandR update records: $evidence" >&2
        exit 1
    fi
    if [[ " $completion " != *" surface_resize=committed "* ]]; then
        echo "Milestone 3 evidence requires a committed configure-plus-pixels resize: $evidence" >&2
        exit 1
    fi
    for required in \
        "injected_input=false" \
        "physical_input=enabled" \
        "pointer_proof=enabled" \
        "input_text_match=true" \
        "pointer_pixel_change=true"; do
        if [[ " $completion " != *" $required "* ]]; then
            echo "Milestone 3 evidence is missing required physical proof field $required: $evidence" >&2
            exit 1
        fi
    done
}

verify_profile "$CLASSIC_EVIDENCE" classic_shared
verify_profile "$CONFINED_EVIDENCE" confined

if [[ "$(grep -Ec '^sophia_live_session schema=7 status=running .* namespace_profile=confined namespace_request_capabilities=0 namespace_publish_capabilities=0$' "$CONFINED_EVIDENCE" || true)" -ne 1 ]]; then
    echo "Milestone 3 confined evidence is not a fresh zero-capability namespace" >&2
    exit 1
fi
if [[ "$(grep -Ec '^sophia_live_session schema=7 status=running .* namespace_profile=classic_shared ' "$CLASSIC_EVIDENCE" || true)" -ne 1 ]]; then
    echo "Milestone 3 classic evidence is missing its shared-X admission profile" >&2
    exit 1
fi

echo "sophia_milestone3_evidence status=passed classic=$CLASSIC_EVIDENCE confined=$CONFINED_EVIDENCE"
