#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_VRR_HARDWARE_EVIDENCE:-/tmp/sophia-vrr-hardware.log}"
SKIP_PREFLIGHT="${SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT:-0}"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
: > "$EVIDENCE_FILE"

echo "Sophia AMD VRR activation/fixed-fallback hardware proof"
echo "This proof requires a VRR-capable connector and exclusive DRM/KMS ownership."
echo "Evidence: $EVIDENCE_FILE"

if [[ "$SKIP_PREFLIGHT" != "1" ]]; then
    "$ROOT_DIR/tools/atomic_scanout_preflight.sh"
fi

set +e
(
    cd "$ROOT_DIR"
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        cargo run --quiet --offline -p sophia-cli \
        --features "atomic-scanout-smoke-live" \
        -- atomic-vrr-smoke "$@"
) 2>&1 | tee "$EVIDENCE_FILE"
proof_status="${PIPESTATUS[0]}"
set -e

if [[ "$proof_status" -eq 0 ]]; then
    "$ROOT_DIR/tools/verify_atomic_scanout_evidence.sh" "$EVIDENCE_FILE"
    "$ROOT_DIR/tools/verify_vrr_hardware_evidence.sh" "$EVIDENCE_FILE"
fi

exit "$proof_status"
