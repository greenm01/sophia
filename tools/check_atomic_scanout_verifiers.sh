#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="$ROOT_DIR/tools/fixtures"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

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
expect_pass tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_pass_modifiers.log
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
expect_fail tools/verify_atomic_scanout_evidence.sh atomic_scanout_evidence_smoke_child_timeout.log

expect_pass tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_pass.log
expect_pass tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_pass_modifiers.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_missing_retire.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_cleanup_debt.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_cleanup_retry.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_unknown_field.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_duplicate_field.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_malformed_field.log
expect_fail tools/verify_runtime_rendered_scanout_evidence.sh runtime_rendered_scanout_evidence_failure.log

expect_pass tools/verify_live_session_content_evidence.sh live_session_content_evidence_pass.log
expect_fail tools/verify_live_session_content_evidence.sh live_session_content_evidence_checksum_mismatch.log

expect_pass tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_pass.log
expect_pass tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_physical_pass.log
expect_pass tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_wm_pass.log
expect_pass tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_v8_pass.log
expect_fail tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_cleanup_debt.log
expect_fail tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_physical_mismatch.log
expect_fail tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_physical_missing.log
expect_fail tools/verify_live_session_persistent_evidence.sh live_session_persistent_evidence_post_completion_error.log

expect_pass tools/verify_live_session_two_xterm_evidence.sh live_session_two_xterm_evidence_pass.log
expect_fail tools/verify_live_session_two_xterm_evidence.sh live_session_two_xterm_evidence_slow_startup.log
expect_fail tools/verify_live_session_two_xterm_evidence.sh live_session_two_xterm_evidence_slow_compose.log

cp "$FIXTURE_DIR/live_session_two_xterm_evidence_pass.log" "$TEMP_DIR/classic.log"
sed 's/namespace_profile=classic_shared/namespace_profile=confined/g' \
    "$FIXTURE_DIR/live_session_two_xterm_evidence_pass.log" > "$TEMP_DIR/confined.log"
"$ROOT_DIR/tools/verify_live_session_milestone3_evidence.sh" \
    "$TEMP_DIR/classic.log" "$TEMP_DIR/confined.log" >/dev/null
if "$ROOT_DIR/tools/verify_live_session_milestone3_evidence.sh" \
    "$TEMP_DIR/classic.log" "$TEMP_DIR/classic.log" >/dev/null 2>&1; then
    echo "Milestone 3 verifier accepted classic evidence as confined evidence" >&2
    exit 1
fi
sed 's/namespace_request_capabilities=0/namespace_request_capabilities=1/' \
    "$TEMP_DIR/confined.log" > "$TEMP_DIR/confined-capability.log"
if "$ROOT_DIR/tools/verify_live_session_milestone3_evidence.sh" \
    "$TEMP_DIR/classic.log" "$TEMP_DIR/confined-capability.log" >/dev/null 2>&1; then
    echo "Milestone 3 verifier accepted a capability-bearing confined namespace" >&2
    exit 1
fi

expect_pass tools/verify_qemu_session_evidence.sh qemu_session_evidence_pass.log
sed -e "s/native_target_creations=2/native_target_creations=0/" \
    -e "s/native_pipeline_creations=2/native_pipeline_creations=0/" \
    "$FIXTURE_DIR/qemu_session_evidence_pass.log" > "$TEMP_DIR/qemu-direct-write.log"
"$ROOT_DIR/tools/verify_qemu_session_evidence.sh" \
    "$TEMP_DIR/qemu-direct-write.log" > /dev/null
sed "s/native_target_creations=0/native_target_creations=1/" \
    "$TEMP_DIR/qemu-direct-write.log" > "$TEMP_DIR/qemu-inconsistent-resources.log"
if "$ROOT_DIR/tools/verify_qemu_session_evidence.sh" \
    "$TEMP_DIR/qemu-inconsistent-resources.log" > /dev/null 2>&1; then
    echo "QEMU verifier accepted inconsistent direct-write resource evidence" >&2
    exit 1
fi
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_wrong_ticks.log
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_internal_input.log
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_no_pointer_pixels.log
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_one_connected_output.log
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_missing_second_retire.log
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_duplicate_output_checksum.log
expect_fail tools/verify_qemu_session_evidence.sh qemu_session_evidence_vsync_overlap.log
expect_pass tools/verify_qemu_emergency_recovery_evidence.sh qemu_emergency_recovery_pass.log
expect_fail tools/verify_qemu_emergency_recovery_evidence.sh qemu_emergency_recovery_missing_guard_trigger.log

expect_pass tools/verify_vrr_hardware_evidence.sh vrr_hardware_evidence_pass.log
expect_fail tools/verify_vrr_hardware_evidence.sh vrr_hardware_evidence_missing_fallback.log

echo "atomic scanout verifier fixtures passed"
