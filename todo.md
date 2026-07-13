# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and the next
major milestones. Completed work lives in `docs/roadmap-history.md`; detailed
evidence lives in `docs/research-log-archive.md`.

Roadmap rule: keep this file short. A completed item leaves this file when the
next milestone becomes active.

---

## Active Milestones: Native Renderer Stability, DMA-BUF Performance, and the Sophia X Server Frontend

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
- DMA-BUF is admitted but remains experimental and currently means direct KMS
  scanout only. It is valid for the output-sized controlled producer, not an
  arbitrary Kitty toplevel: direct scanout requires the physical output size,
  while a Kitty toplevel may be a different size. The guarded Kitty harness
  therefore keeps the proven SHM composition route and does not advertise the
  direct DMA-BUF global. GPU composition (import, scale, blend, then scan out a
  Sophia-owned target) is required before Kitty can use DMA-BUF safely. The
  repaired controlled
  three-frame proof, core-mode 300-frame run, and three preserved normal
  300-frame runs now pass after isolating each imported EGLImage in a transient
  GL texture. One earlier post-repair 300-frame run still aborted after frame 2
  with `free(): invalid pointer`, so retain the normal-stability wrapper as a
  regression gate. SHM remains the production fallback until GPU composition
  and guarded Kitty DMA-BUF evidence pass.

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
- [ ] Prove the guarded native SHM session accepts exact text/navigation and pointer
  input, meets the 100 ms presentation budget, exits cleanly, and restores its
  TTY and prior `keyd` state.
- [x] Make the installed session launch an arbitrary Wayland client without an
  X server, keeping Kitty confined to acceptance tooling.
- [x] Remove XLibre/Xorg launch paths and production dependencies; retain the
  bridge only as frozen source in the non-workspace research archive.
- [x] Add native KMS presentation wiring for the Wayland session while
  preserving the existing independent TTY recovery guard.

## Sophia X Server Frontend

Sophia’s X direction is a Phoenix-style strategic approach: implement a modern
X server that presents the established X11 API directly to applications. Do not
create a separate Sophia-native display protocol. Smithay remains the Wayland
frontend infrastructure; XLibre remains the broad-compatibility provider and
reference while the Sophia X Server Frontend matures.

Terminology: call the component **Sophia X Server Frontend** and call the API it
implements **X11**. The existing `sophia-x-authority` crate is the frontend’s
implementation seed; retain its name until a source-layout migration has an
engineering reason.

### Milestone 1: Server Contract and Long-Running Frontend

- [x] Establish the first X11 setup/socket/resource prototype and reduce
  bounded X lifecycle, property, core-draw, and private-present facts into
  Engine transactions.
- [x] Specify the production X Server Frontend boundary: client connection and
  authentication ownership, X resource lifecycle, Engine transaction intake,
  output/RandR facts, routed-input decisions, and presentation feedback. The
  current implementation and explicit remaining gaps are documented in
  `docs/sophia-x-authority.md`.
- [ ] Turn the temporary socket smoke into a supervised, configurable,
  long-running local X server frontend without giving it DRM/KMS or physical
  input ownership. The first slice now has an explicit
  `XServerFrontendConfig`/`XServerFrontend` listener with owner-only socket
  permissions, safe stale-socket handling, and optional MIT-MAGIC-COOKIE-1
  setup validation; Xauthority-file/credential policy, Engine-backed session
  supervision, and simultaneous clients remain.
- [x] Define the X11 session profiles: classic shared-X behavior for trusted
  sessions, plus explicit confined namespaces/capabilities where requested.
  The confined profile remains gated on client-aware connection routing.
- [x] Establish an application-driven compatibility matrix and make every new
  X11 request, reply, event, or extension earn its implementation through a
  reproducible real-client probe. See `docs/x11-compatibility-matrix.md`.

### Milestone 2: Modern X11 Input and Graphics

- [ ] Complete a real XKB/keymap path and X11 focus, grab, pointer, keyboard,
  and XI delivery semantics using Engine-selected targets and local coordinates.
- [ ] Promote explicit X11 buffer readiness: SHM/software updates first, then
  standard DRI3/Present DMA-BUF handoff with fences, delayed release, and
  Engine-owned atomic presentation.
- [ ] Drive Render, GLX, XFixes, selections, RandR, and extension coverage from
  the compatibility matrix rather than attempting all Xorg behavior.
- [ ] Prove a real X11 client reaches Sophia Engine and KMS through normal
  startup, input, resize, presentation, and teardown—not only a socket smoke.

### Milestone 3: Compatibility Provider and Migration Evidence

- [ ] Define the XLibre provider contract as an optional broad-compatibility
  lane: it may own X11 semantics, but never Sophia DRM/KMS, physical input,
  layout policy, or session control.
- [ ] Keep the XLibre integration thin and evidence-gated; use its behavior and
  real applications to guide the native frontend rather than treating
  XComposite/readback as Sophia’s permanent rendering boundary.
- [ ] Set promotion criteria for moving an application class from XLibre to the
  Sophia X Server Frontend: protocol coverage, input/grab correctness, buffer
  lifetime, latency, recovery, and classic-X behavior where selected.

## DMA-BUF Performance Gates

- [x] Advertise only bounded single-plane XRGB/ARGB linear formats and validate
  dimensions, modifier, plane count, and stride before admitting an opaque
  `DmaBuf` handle.
- [x] Add a controlled, external Wayland DMA-BUF producer that allocates only
  linear XRGB8888 GBM buffers and waits for both frame and buffer-release
  feedback before reuse.
- [x] Pass the controlled three-frame first-frame proof with
  `tools/wayland_dmabuf_first_frame_hardware_proof.sh`: three imports, KMS
  presentation retirement, no cleanup debt, and 16 ms maximum
  submit-to-page-flip latency after the transient-texture repair.
- [x] Pass the controlled 300-frame lifecycle run with the same tool. The
  core-mode run and three separately retained normal logs each prove 300
  imports, KMS presentation retirement, no cleanup debt, and 14–16 ms maximum
  submit-to-page-flip latency. Keep the wrapper as a regression gate because an
  earlier post-repair normal run aborted after frame 2.
- [ ] Pass three independent guarded real-Kitty native-composition runs with
  `tools/wayland_kitty_dmabuf_promotion_gate.sh`: exact text/navigation and
  pointer input, resize, clean normal exit, TTY/`keyd` restoration, and the
  100 ms presentation budget in every log. These runs intentionally use SHM;
  the controlled producer remains the direct-scanout DMA-BUF proof.
- [ ] Add GPU composition for arbitrary DMA-BUF client surfaces: import a
  window-sized buffer, scale/blend it into a Sophia-owned output-sized target,
  then submit that target to KMS. Require buffer release only after the target's
  page flip retires.
- [ ] After GPU composition and its guarded Kitty evidence pass, expose DMA-BUF
  as an explicit non-default `--dmabuf` opt-in. Keep SHM as the default and
  fallback until broader compatibility and recovery evidence exists.

## Deferred

- VRR activation evidence waits for hardware reporting `vrr_capable=1`; the
  current panel cannot prove it.
- Multi-plane/modifier DMA-BUFs, explicit synchronization, and multi-client
  DMA-BUF scheduling wait for single-plane GPU composition and its guarded
  Kitty gate.
- Dedicated-TTY xmonad visual evidence and a second legacy-WM smoke no longer
  block the protocol-authority path.
- Optional Wayland protocols and broader desktop compatibility resume after the
  native terminal path is stable. X11 GTK/XInput2/clipboard/drag-and-drop and
  concurrent-client work now advance through the Sophia X Server Frontend
  compatibility matrix.
