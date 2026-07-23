#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"

cargo fmt --check
cargo check --offline -p sophia-cli --features atomic-scanout-smoke-live --quiet
cargo test --offline -p sophia-cli --features atomic-scanout-smoke-live --test backend_evidence --quiet
cargo test --offline -p sophia-cli --features atomic-scanout-smoke-live --test input_proof --quiet
cargo test --offline -p sophia-renderer-native-egl --features gbm-platform --quiet
cargo test --offline -p sophia-renderer-live --features "gbm-probe egl-probe" --quiet
cargo test --offline -p sophia-backend-live --features "libdrm-events libinput-events gbm-probe egl-probe" --quiet
bash -n tools/qemu_xmonad_m7_acceptance.sh
bash -n tools/check_qemu_xmonad_m7_verifier.sh
tools/check_qemu_xmonad_m7_verifier.sh
bash -n tools/qemu_xmonad_m8_mix_acceptance.sh
bash -n tools/qemu_xmonad_m8_soak_acceptance.sh
bash -n tools/verify_qemu_xmonad_m8_mix_evidence.sh
bash -n tools/verify_qemu_xmonad_m8_soak_evidence.sh
tools/check_qemu_xmonad_m8_verifiers.sh
tools/check_atomic_scanout_verifiers.sh
bash -n tools/audit_xcentric_runtime.sh
tools/audit_xcentric_runtime.sh
bash -n tools/atomic_scanout_preflight.sh
bash -n tools/atomic_scanout_smoke.sh
bash -n tools/runtime_rendered_scanout_evidence.sh
bash -n tools/atomic_scanout_hardware_proof.sh
bash -n tools/atomic_scanout_hardware_status.sh
bash -n tools/operator_keyboard_hardware_proof.sh
bash -n tools/finish_milestones_1_2.sh
bash -n tools/run_sophia_xmonad_session.sh
bash -n tools/start_sophia_kitty_tty3.sh tools/run_sophia_kitty_session.sh tools/stop_sophia_kitty_session.sh
grep -Fq 'sophia-session-input-guard' tools/run_sophia_xmonad_session.sh
grep -Fq 'sophia_tty_mode.py" "$kd_mode"' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--session-start=terminal' tools/run_sophia_xmonad_session.sh
grep -Fq 'SOPHIA_TTY_PROFILE=kitty' tools/run_sophia_kitty_session.sh
grep -Fq 'dbus-run-session --' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--startup-ready-timeout-ms=8000' tools/run_sophia_xmonad_session.sh
grep -Fq 'tools/run_sophia_kitty_session.sh' tools/start_sophia_kitty_tty3.sh
grep -Fq -- '--exit-when-startup-exits' tools/run_sophia_xmonad_session.sh
grep -Fq -- '--input-seat=$input_seat' tools/run_sophia_xmonad_session.sh
if grep -Fq -- '*-event-kbd' tools/run_sophia_xmonad_session.sh; then
    echo "TTY launcher must use production libinput seat discovery, not path heuristics" >&2
    exit 1
fi
bash -n tools/stop_sophia_xmonad_session.sh
bash -n tools/xmonad_live_session_smoke.sh
bash -n tools/live_session_persistent_hardware_proof.sh
bash -n tools/live_session_milestone5_gtk_hardware_proof.sh
if ! grep -Fq -- '--namespace-profile="$profile" --software-client-rendering' tools/live_session_milestone5_gtk_hardware_proof.sh; then
    echo "Milestone 5 GTK proof must retain the selected namespace profile in software-rendering mode." >&2
    exit 1
fi
bash -n tools/verify_live_session_milestone5_tty_recovery.sh
bash -n tools/check_live_session_milestone5_verifier.sh
tools/check_live_session_milestone5_verifier.sh
python3 -c 'compile(open("tools/sophia_tty_mode.py", encoding="utf-8").read(), "tools/sophia_tty_mode.py", "exec")'

echo "atomic scanout local checks passed"
