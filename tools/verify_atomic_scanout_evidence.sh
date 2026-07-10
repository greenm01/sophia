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

require_pattern() {
    local pattern="$1"

    if [[ "$evidence" != *"$pattern"* ]]; then
        echo "atomic scanout evidence is missing: $pattern" >&2
        echo "$evidence" >&2
        exit 1
    fi
}

require_pattern "status=Passed"
require_pattern "scanout_target=Ready"
require_pattern "rendered_context=Ready"
require_pattern "gbm_export=Exported"
require_pattern "submit=SubmittedWaitingForPageFlip"
require_pattern "commit_page_flip_event=true"
require_pattern "commit_nonblocking=true"
require_pattern "commit_allow_modeset=true"
require_pattern "commit_test_only=false"
require_pattern "page_flip_poll=Emitted"
require_pattern "page_flip=Presented"
require_pattern "retire=RetiredAfterPageFlip"
require_pattern "retire_destroy=Destroyed"
require_pattern "retire_cleanup_pending=false"

echo "atomic scanout evidence passed: $EVIDENCE_FILE"
