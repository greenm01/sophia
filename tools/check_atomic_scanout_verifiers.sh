#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$ROOT_DIR/tools/fixtures"

expect_pass() {
    local verifier="$1"
    local fixture="$2"

    "$ROOT_DIR/$verifier" "$FIXTURE_DIR/$fixture" >/dev/null
}

expect_fail() {
    local verifier="$1"
    local fixture="$2"

    if "$ROOT_DIR/$verifier" "$FIXTURE_DIR/$fixture" >/dev/null 2>&1; then
        echo "verifier unexpectedly accepted fixture: $fixture" >&2
        exit 1
    fi
}

expect_pass tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_pass.log
expect_fail tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_unavailable.log
expect_fail tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_impossible_counts.log
expect_fail tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_unknown_native_field.log
expect_fail tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_duplicate_field.log
expect_fail tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_malformed_field.log
expect_fail tools/verify_atomic_scanout_preflight.sh atomic_scanout_preflight_multiple_lines.log

expect_pass tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_pass.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_missing_rendered_context.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_missing_scanout_buffer.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_missing_steady_phase.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_wrong_steady_scope.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_unknown_native_field.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_duplicate_field.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_malformed_field.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_waiting_retire.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_cleanup_pending.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_test_only_commit.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_blocking_commit.log
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_missing_page_flip_event_flag.log

echo "atomic scanout verifier fixtures passed"
