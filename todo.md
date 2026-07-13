# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and the next
major milestones. Completed work lives in `docs/roadmap-history.md`; detailed
evidence lives in `docs/research-log-archive.md`.

Roadmap rule: keep this file short. A completed item leaves this file when the
next milestone becomes active.

---

## Active Milestone: Native Renderer Stability and DMA-BUF Performance

Current truth:

- The production CLI and installed launcher no longer link, select, or start the
  XLibre bridge. Its frozen sources and evidence live outside the workspace in
  `research/xlibre`.
- A real Kitty 0.47.4 process connects over native Wayland with `DISPLAY`
  removed and software GL, commits SHM buffers, and produces changing nonzero
  pixels through Sophia's protocol-neutral Engine path.
- The live authority supports ordered pipelined commits, xdg configure/ack,
  frame callbacks, buffer release, keyboard/pointer seat delivery, the proven
  SHM path, and bounded linear DMA-BUF admission.
- The non-modesetting hardware preflight finds one openable atomic-capable card
  with a connected scanout target and the required atomic properties.
- The guarded native SHM session now exits cleanly on hardware with keyboard
  routing, real KMS submissions, page-flip retirement, and no recovery debt.
- The current hardware Kitty result is about 110 ms input-to-presentation, so
  the 100 ms budget is not yet met.
- Keep the native loop at its 2 ms idle cadence and retain the owned CPU-frame
  copy for now: tighter polling or zero-copy handoff reproducibly corrupts the
  native renderer/exporter heap on hardware. Isolate that ownership fault before
  attempting further latency reductions.
- DMA-BUF is admitted but remains experimental. The repaired controlled
  proofs produced several 300-frame passes, but the latest uninstrumented
  promotion preflight aborted on its first DMA-BUF frame with `free(): invalid
  pointer`. The persistent CPU-upload texture is no longer reused as an
  EGLImage sibling; each imported frame now owns a transient texture. SHM
  remains the production fallback until that revised ownership boundary passes
  controlled and guarded Kitty evidence.

Exit criteria:

- [x] Make Engine's committed surface snapshot the single authority: native
  presentation must consume it directly and must not replay Wayland authority
  transactions into a second Engine state. Verified on hardware with a clean
  native Kitty exit after transaction and surface-destruction teardown.
- [x] Add a bounded presentation scheduler: at most one retained pending frame
  per surface, explicit page-flip-to-release ownership, and normal teardown
  ordering. Verified on hardware with 26 Kitty transactions coalesced into 14
  SHM frames, clean native teardown, and no scanout failures.
- [ ] Establish a lifetime-stress baseline for the persistent native renderer:
  no heap corruption at the 2 ms cadence, no ownership debt, and no attempt to
  remove the CPU-frame copy before its fault is isolated.
- [x] Isolate the controlled DMA-BUF first-frame heap corruption with allocator
  evidence and resource-lifetime tracing. The repaired diagnostic proof records
  clean import, KMS submission, page-flip retirement, and client buffer release
  for three full-size frames.
- [x] Add a dedicated-TTY GDB diagnostic mode that records allocator backtraces
  and ordered DMA-BUF import, scanout, page-flip, retirement, and client-release
  stages without changing the production SHM path.
- [x] Prove a real software-rendered native Wayland Kitty toplevel handles a
  compositor configure, keeps its old size live until ack, then commits changing
  nonzero pixels at the requested size.
- [ ] Prove the guarded native session accepts exact text/navigation and pointer
  input, meets the 100 ms presentation budget, exits cleanly, and restores its
  TTY and prior `keyd` state.
- [x] Make the installed session launch an arbitrary Wayland client without an
  X server, keeping Kitty confined to acceptance tooling.
- [x] Remove XLibre/Xorg launch paths and production dependencies; retain the
  bridge only as frozen source in the non-workspace research archive.
- [x] Add native KMS presentation wiring for the Wayland session while
  preserving the existing independent TTY recovery guard.

## DMA-BUF Performance Gates

- [x] Advertise only bounded single-plane XRGB/ARGB linear formats and validate
  dimensions, modifier, plane count, and stride before admitting an opaque
  `DmaBuf` handle.
- [x] Add a controlled, external Wayland DMA-BUF producer that allocates only
  linear XRGB8888 GBM buffers and waits for both frame and buffer-release
  feedback before reuse.
- [ ] Pass the controlled three-frame first-frame proof with
  `tools/wayland_dmabuf_first_frame_hardware_proof.sh`: the latest promotion
  preflight aborted after its first frame with `free(): invalid pointer`.
  Validate the transient EGLImage texture lifetime boundary before counting new
  imports, KMS presentation retirement, cleanup debt, or latency evidence.
- [ ] Pass the controlled 300-frame lifecycle run with the same tool. Earlier
  GDB, traced, and normal runs are diagnostic samples only because the latest
  uninstrumented three-frame preflight still aborts. Require three independent
  normal 300-frame passes after the transient-texture repair.
- [ ] Pass three independent guarded real-Kitty DMA-BUF runs with
  `tools/wayland_kitty_dmabuf_promotion_gate.sh`: exact text/navigation and
  pointer input, resize, clean normal exit, TTY/`keyd` restoration, DMA-BUF
  presentation, and the 100 ms presentation budget in every log.
- [ ] After those gates pass, expose DMA-BUF as an explicit non-default `--dmabuf`
  opt-in. Keep SHM as the default and fallback until broader compatibility and
  recovery evidence exists.

## Deferred

- VRR activation evidence waits for hardware reporting `vrr_capable=1`; the
  current panel cannot prove it.
- Multi-plane/modifier DMA-BUFs, explicit synchronization, and multi-client
  DMA-BUF scheduling wait for the single-plane lifetime and three-run Kitty
  gates.
- Dedicated-TTY xmonad visual evidence and a second legacy-WM smoke no longer
  block the protocol-authority path.
- GTK/XInput2/zenity, clipboard, drag-and-drop, optional Wayland protocols,
  concurrent X clients, and broader desktop compatibility resume after the
  native terminal path is stable.
