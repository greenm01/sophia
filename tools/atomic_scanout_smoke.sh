#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_ATOMIC_SCANOUT_EVIDENCE:-/tmp/sophia-atomic-scanout-evidence.log}"
TEST_FILTER="atomic_scanout_hardware_smoke::native_atomic_scanout_smokes_real_primary_card_when_enabled"

mkdir -p "$(dirname "$EVIDENCE_FILE")"

echo "Sophia atomic scanout hardware smoke"
echo "This test may take DRM master on a primary /dev/dri/card* node."
echo "Evidence: $EVIDENCE_FILE"

(
    cd "$ROOT_DIR"
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        cargo test --offline -p sophia-backend-live \
        --features "libdrm-events gbm-probe" \
        "$TEST_FILTER" \
        -- --nocapture
) 2>&1 | tee "$EVIDENCE_FILE"

exit "${PIPESTATUS[0]}"
