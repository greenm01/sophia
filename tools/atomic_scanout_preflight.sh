#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFLIGHT_FILE="${SOPHIA_ATOMIC_SCANOUT_PREFLIGHT:-/tmp/sophia-atomic-scanout-preflight.log}"

mkdir -p "$(dirname "$PREFLIGHT_FILE")"

echo "Sophia atomic scanout preflight"
echo "This check does not request DRM master and does not modeset hardware."
echo "Preflight: $PREFLIGHT_FILE"

set +e
(
    cd "$ROOT_DIR"
    cargo run --quiet --offline -p sophia-cli \
        --features "atomic-scanout-live" \
        -- atomic-scanout-preflight
) 2>&1 | tee "$PREFLIGHT_FILE"
test_status="${PIPESTATUS[0]}"
set -e

if [[ "$test_status" -ne 0 ]]; then
    echo "Atomic scanout preflight failed; output left at $PREFLIGHT_FILE" >&2
    exit "$test_status"
fi

if ! "$ROOT_DIR/tools/verify_atomic_scanout_preflight.sh" "$PREFLIGHT_FILE"; then
    echo "Atomic scanout preflight did not find a smoke-ready host; output left at $PREFLIGHT_FILE" >&2
    exit 1
fi

exit 0
