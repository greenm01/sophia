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
tools/check_atomic_scanout_verifiers.sh
bash -n tools/atomic_scanout_preflight.sh
bash -n tools/atomic_scanout_smoke.sh
bash -n tools/runtime_rendered_scanout_evidence.sh
bash -n tools/atomic_scanout_hardware_proof.sh
bash -n tools/atomic_scanout_hardware_status.sh
bash -n tools/operator_keyboard_hardware_proof.sh
bash -n tools/finish_milestones_1_2.sh
bash -n tools/run_sophia_xmonad_session.sh
bash -n tools/stop_sophia_xmonad_session.sh
bash -n tools/xmonad_live_session_smoke.sh
bash -n tools/run_sophia_kitty_session.sh
bash -n tools/stop_sophia_kitty_session.sh
bash -n tools/wayland_kitty_smoke.sh
bash -n tools/wayland_kitty_hardware_proof.sh
bash -n tools/finish_wayland_kitty_milestones.sh
bash -n tools/build_wayland_dmabuf_producer.sh
bash -n tools/wayland_dmabuf_first_frame_hardware_proof.sh
bash -n tools/wayland_kitty_dmabuf_promotion_gate.sh
bash -n tools/setup_void_dmabuf_promotion.sh
bash -n tools/diagnose_void_dmabuf_heap.sh
bash -n tools/run_void_dmabuf_lifetime_proof.sh
bash -n tools/verify_wayland_kitty_evidence.sh
bash -n tools/install_sophia_session.sh
if ! grep -q 'SOPHIA_KITTY_REQUIRE_DMABUF=0' tools/wayland_kitty_hardware_proof.sh; then
    echo "Interactive Kitty proof must not enable output-sized direct DMA-BUF scanout." >&2
    exit 1
fi
if ! grep -q 'No stable pointer event path was found.' tools/wayland_kitty_hardware_proof.sh; then
    echo "Focused Kitty proof must discover stable input devices when none are supplied." >&2
    exit 1
fi
if ! rg -q 'open_threaded_native_libinput_path_poller' crates/sophia-cli/src/commands/wayland.rs; then
    echo "Native Wayland Kitty must keep input acquisition off the render thread." >&2
    exit 1
fi
dmabuf_producer_check="$(mktemp /tmp/sophia-wayland-dmabuf-producer-check.XXXXXX)"
trap 'rm -f "$dmabuf_producer_check"' EXIT
tools/build_wayland_dmabuf_producer.sh "$dmabuf_producer_check"
python3 -c 'compile(open("tools/sophia_tty_mode.py", encoding="utf-8").read(), "tools/sophia_tty_mode.py", "exec")'

echo "atomic scanout local checks passed"
