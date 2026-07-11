#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EVIDENCE_FILE="${SOPHIA_LIVE_SESSION_PERSISTENT_EVIDENCE:-/tmp/sophia-live-session-persistent.log}"
DISPLAY_NAME="${SOPHIA_LIVE_SESSION_DISPLAY:-:181}"
RUNTIME_MSEC="${SOPHIA_LIVE_SESSION_RUNTIME_MSEC:-5000}"
SKIP_PREFLIGHT="${SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT:-0}"

mkdir -p "$(dirname "$EVIDENCE_FILE")"
: > "$EVIDENCE_FILE"

echo "Sophia persistent live-session hardware proof"
echo "This proof requires exclusive DRM/KMS ownership on the active TTY."
echo "Evidence: $EVIDENCE_FILE"

input_proof_args=(--inject-text=sophia)
for arg in "$@"; do
    case "$arg" in
        --inject-text=*|--expect-physical-text=*)
            input_proof_args=()
            ;;
    esac
done

if [[ "$SKIP_PREFLIGHT" != "1" ]]; then
    "$ROOT_DIR/tools/atomic_scanout_preflight.sh"
fi

set +e
(
    cd "$ROOT_DIR"
    SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
        cargo run --quiet --offline -p sophia-cli \
        --features "atomic-scanout-live" \
        -- sophia-live-session --display="$DISPLAY_NAME" --native-scanout \
        --max-runtime-ms="$RUNTIME_MSEC" "${input_proof_args[@]}" "$@"
) 2>&1 | tee "$EVIDENCE_FILE"
proof_status="${PIPESTATUS[0]}"
set -e

if [[ "$proof_status" -eq 0 ]]; then
    "$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$EVIDENCE_FILE"
fi

exit "$proof_status"
