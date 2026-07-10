#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_ATOMIC_SCANOUT_EVIDENCE:-/tmp/sophia-atomic-scanout-evidence.log}"
PREFLIGHT_FILE="${SOPHIA_ATOMIC_SCANOUT_PREFLIGHT:-/tmp/sophia-atomic-scanout-preflight.log}"
SKIP_PREFLIGHT="${SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT:-0}"
TEST_FILTER="atomic_scanout_hardware_smoke::native_atomic_scanout_smokes_real_primary_card_when_enabled"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
mkdir -p "$(dirname "$PREFLIGHT_FILE")"
: > "$EVIDENCE_FILE"

echo "Sophia atomic scanout hardware smoke"
echo "This test may take DRM master on a primary /dev/dri/card* node."
echo "Preflight: $PREFLIGHT_FILE"
echo "Evidence: $EVIDENCE_FILE"

if [[ "$SKIP_PREFLIGHT" != "1" ]]; then
    "$ROOT_DIR/tools/atomic_scanout_preflight.sh"
else
    echo "Skipping atomic scanout preflight because SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1"
fi

set +e
(
    cd "$ROOT_DIR"
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        cargo test --offline -p sophia-backend-live \
        --features "libdrm-events gbm-probe" \
        "$TEST_FILTER" \
        -- --nocapture
) 2>&1 | tee "$EVIDENCE_FILE"
test_status="${PIPESTATUS[0]}"
set -e

if [[ "$test_status" -eq 0 ]]; then
    "$ROOT_DIR/tools/verify_atomic_scanout_evidence.sh" "$EVIDENCE_FILE"
else
    echo "Atomic scanout smoke failed; evidence left at $EVIDENCE_FILE" >&2
fi

exit "$test_status"
