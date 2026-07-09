# Sophia Active Roadmap

Sophia is a research prototype. This file tracks the active architecture path
and keeps completed milestones compact. Detailed rationale and historical
evidence belong in `docs/research-log.md`.

---

## Active Focus - Real Backend Boundaries

**Now**
- [ ] Keep `DEFAULT_DISPLAY` clear-color smoke as fallback until GBM-backed
  drawing is validated against real render nodes.
- [ ] Define reduced native libdrm page-flip source construction from authority
  without reading events.

**Next**
- [ ] Keep wgpu deferred until GBM/EGL startup, drawing, and presentation seams
  are proven.
- [ ] Record a passing `SOPHIA_RUN_REAL_GBM_SMOKE=1` run before retiring
  `DEFAULT_DISPLAY`.

---

## Next Architecture Milestones

- [x] Expand X11 atom/property tables for ICCCM names and metadata-broker
  candidates.
- [x] Add minimal bounded `GetProperty` replies and socket smoke coverage.
- [x] Define and pass the first real-client-library target with `x11rb`: setup,
  atom lookup, create, property write/read, map, and event observation.
- [x] Pass `xdpyinfo` as a broader setup/introspection probe with minimal
  `CreateGC`, `FreeGC`, `GetInputFocus`, `QueryExtension`, `ListExtensions`,
  `QueryBestSize`, full predefined atom, and root-property read support.
- [x] Pass a tiny compiled C Xlib probe covering `XOpenDisplay`,
  `XInternAtom`, `XCreateSimpleWindow`, `XStoreName`, `XChangeProperty`,
  `XGetWindowProperty`, `XMapWindow`, and `XDestroyWindow`.
- [x] Pass a drawing-oriented C Xlib probe using `XFillRectangle`; opcode 70
  now decodes to a ready `CoreDraw` surface transaction.
- [x] Expose live X11 socket dispatch results through an out-of-band observer,
  preserving no-reply success semantics for core draw requests.
- [x] Add socket-level smoke coverage proving `PolyFillRectangle` creates one
  ready `CoreDraw` transaction outside unit-test-only dispatch.
- [x] Feed observed X Authority drawing transactions into the live runtime
  adapter as reduced authority commit summaries.
- [x] Validate the C Xlib drawing smoke through Engine commit and Runtime
  authority transaction counters without leaking XIDs or namespace metadata.
- [x] Add the first bounded software upload request model with core `PutImage`
  decoding into ready CPU-backed surface transactions.
- [x] Pass a compiled C Xlib `XPutImage` smoke through observed transaction,
  Engine commit, and Runtime authority counters with no direct X reply on
  success.
- [x] Add the first Present-style explicit buffer handoff model as private
  `SOPHIA-PRESENT` minor opcode `0` using XPixmap handles.
- [x] Add socket and CLI smoke coverage proving Present-style handoff reaches
  Engine commit and Runtime counters without adding compositor policy.
- [x] Defer DMA-BUF placeholder modeling until real DRI3/Present semantics are
  ready.
- [x] Move the long-running X Authority transaction side channel to a
  runtime-owned bounded queue while keeping callback observers for focused
  tests.
- [x] Document side-channel backpressure: full or disconnected queues fail
  closed instead of allocating unbounded buffers or dropping visual facts.
- [x] Add minimal `MIT-SHM` `QueryExtension` and `ShmQueryVersion` support with
  unsupported minor opcodes failing closed.
- [x] Model `ShmAttach` as namespace-local metadata without mapping host memory.
- [x] Decode `XShmPutImage` and reject it with bounded native X errors until a
  real SHM import path exists.
- [x] Defer real MIT-SHM memory mapping until a compositor backend can consume
  mapped bytes through a bounded renderer import path.
- [x] Add protocol-neutral `AuthorityTransactionIntake` so runtime can commit
  authority batches without making Sophia Engine depend on Sophia X Authority.
- [x] Define the first runtime-owned backend assembly struct that holds output
  discovery, input polling, frame clock, authority transaction intake, and
  renderer selection without owning protocol policy.
- [x] Add a headless assembly smoke that drains authority batches into committed
  surface state and renders through the existing runtime driver.
- [x] Define the runtime event that reports authority process ready/degraded
  state without leaking X11 resource IDs or namespace metadata.
- [x] Move the Present-style smoke to the bounded X Authority transaction
  channel path; callback observers remain for focused tests.
- [x] Add a supervised X Authority process wrapper that emits reduced authority
  health observations.
- [x] Feed bounded authority transaction batches into the compositor backend
  assembly through a protocol-neutral inbox.
- [x] Decide the first real backend dependency boundary for DRM/KMS and
  libinput without changing the Engine/WM/Authority packet contracts.
- [x] Keep real DRM/KMS ioctls, GPU imports, and real MIT-SHM mapping deferred
  behind deterministic backend discovery and assembly seams.
- [x] Sketch the first live compositor backend crate boundary and keep kernel
  IO behind traits that preserve deterministic headless tests.
- [x] Add one smoke that proves backend discovery can fail closed without
  affecting protocol authority or WM IPC contracts.
- [x] Add a live backend dependency policy before adding crates that touch
  `/dev/dri`, `/dev/input`, GBM, EGL, DMA-BUF, or real MIT-SHM mapping.
- [x] Decide that libdrm and libinput may enter through
  `sophia-backend-live`, while GPU imports and real MIT-SHM mapping remain
  deferred until renderer import boundaries exist.
- [x] Define the renderer import boundary separately from backend discovery.
- [x] Sketch reduced renderer import admission records without adding GBM, EGL,
  DMA-BUF, or real MIT-SHM mapping.
- [x] Add deterministic tests for renderer import admission and fail-closed
  unsupported import paths.
- [x] Decide that the first real renderer implementation should live in a
  dedicated `sophia-renderer-live` crate once GBM, EGL, DMA-BUF, or explicit
  sync dependencies are required.
- [x] Thread live renderer import admission into backend startup without adding
  GBM, EGL, DMA-BUF, or real MIT-SHM mapping.
- [x] Add a startup smoke proving native renderer import remains disabled until
  explicitly configured.
- [x] Define the first live renderer health shape for CPU fallback, native
  import-capable, and degraded import capability.
- [x] Add renderer admission status to live backend startup reports without
  leaking renderer-private handles.
- [x] Sketch the `sophia-renderer-live` crate boundary without adding GBM, EGL,
  DMA-BUF, or real MIT-SHM mapping.
- [x] Decide the first runtime observation shape for CPU fallback versus native
  import-capable renderer selection.
- [x] Decide that renderer import startup health is stored in the
  `sophia-backend-live` runtime wrapper, not inside `sophia-engine`.
- [x] Add a reduced runtime observation for renderer import health once startup
  health is consumed by the runtime assembly.
- [x] Keep real GBM/EGL/DMA-BUF dependencies deferred until
  `sophia-renderer-live` has deterministic fake coverage.
- [x] Add fake degraded renderer coverage before modeling any real native import
  failure.
- [x] Decide that degraded renderer health is sourced from both failed startup
  capability probes and per-frame failed imports.
- [x] Add a fake failed-import path in `sophia-renderer-live` for per-frame
  degraded runtime observation.
- [x] Keep the live runtime wrapper outside `sophia-engine` unless engine-local
  renderer policy becomes unavoidable.
- [x] Revisit real GBM/EGL/DMA-BUF admission only after the degraded-health
  source decision is documented.
- [x] Decide that the first real native renderer dependency candidate is a
  feature-gated GBM capability probe.
- [x] Keep the first real renderer dependency behind an optional crate feature
  so default offline deterministic tests remain available.
- [x] Add feature-gate scaffolding for the future GBM capability probe without
  adding the dependency.
- [x] Add a fake feature-enabled GBM probe test that still uses deterministic
  data before introducing a real crate.
- [x] Revisit real GBM dependency admission after the feature-gated fake path
  exists.
- [x] Decide the real GBM probe API shape: backend-provided reduced device token,
  not device-path intake or borrowed fd intake.
- [x] Keep default workspace tests independent of native renderer libraries.
- [x] Add documented local check coverage for
  `cargo test --offline -p sophia-renderer-live --features gbm-probe`.
- [x] Decide to isolate any concrete GBM binding behind a tiny renderer-live
  adapter module rather than exposing it directly.
- [x] Keep real GBM dependency optional until release checks exercise both
  default and feature-enabled paths.
- [x] Revisit real GBM crate admission after selecting the concrete crate and
  checking its offline/system dependency behavior.
- [x] Evaluate concrete GBM crate options and choose the safe `gbm` crate as the
  first candidate, keeping `gbm-sys` as a fallback only.
- [x] Add dependency-admission notes for the selected GBM crate before adding it.
- [x] Confirm local `libgbm` development visibility through `pkg-config`.
- [x] Add the optional `gbm` dependency under `gbm-probe` with default features
  disabled.
- [x] Add the private adapter module with fake/native split under `gbm-probe`.
- [x] Teach the private native GBM probe to consume backend-owned device
  authority without exposing raw fds through public Sophia data.
- [x] Map real native GBM probe failures to reduced degraded renderer health.
- [x] Wire backend-live render-device discovery into the reduced GBM probe path.
- [x] Keep CPU fallback startup as the default when GBM probing is absent,
  unavailable, or degraded.
- [x] Add feature-enabled backend-live coverage for GBM degraded startup health.
- [x] Add a live render-device discovery abstraction that can later choose a DRM
  render node without leaking paths into engine state.
- [x] Decide render-node discovery stays in backend-live for now, behind a
  narrower feature-gated trait.
- [x] Add feature-enabled startup assembly that selects CPU fallback when GBM
  probing degrades a requested DMA-BUF path.
- [x] Add a runtime observation for degraded GBM startup that remains count-only
  and path-free.
- [x] Add renderer preference policy: `GpuPreferred`, `CpuOnly`, and
  `GpuRequired`.
- [x] Decide the first real render-node discovery source is explicit
  backend-owned fd injection.
- [x] Add docs for why degraded native import must not partially enable the
  import-capable renderer.
- [x] Add a real GBM allocation probe that creates and drops a tiny private
  buffer without exporting GBM handles.
- [x] Add a reduced GPU startup report that distinguishes render-device
  discovery failure from GBM device rejection and private allocation failure
  without leaking native error text.
- [x] Add opt-in backend-owned fd injection smoke coverage around a real render
  device when the host test environment exposes one.
- [x] Decide EGL/OpenGL is the first compositor drawing API above GBM; raw
  Vulkan and wgpu remain deferred.
- [x] Add `egl-probe` feature scaffolding without admitting a native EGL crate.
- [x] Add fake reduced EGL capability records and backend startup projection.
- [x] Evaluate concrete EGL binding options and choose `khronos-egl` as the
  candidate for the native context probe.
- [x] Add EGL dependency-admission notes before admitting a real EGL crate.
- [x] Add the optional `khronos-egl` dependency under `egl-probe` with dynamic
  loading only.
- [x] Add a real EGL context probe behind the `egl-probe` feature, isolated in
  a tiny native adapter crate that returns only reduced status.
- [x] Add reduced EGL draw-smoke status records.
- [x] Add the first native EGL offscreen target smoke: create a private pbuffer,
  make a context current against it, and return only reduced status.
- [x] Evaluate concrete GL function loading options and choose `glow` as the
  candidate for the first clear-color smoke.
- [x] Add GL function-loading dependency-admission notes before admitting
  `glow`.
- [x] Add optional `glow` dependency for clear-color smoke inside the native EGL
  adapter.
- [x] Extend the EGL draw smoke from private pbuffer readiness to a clear-color
  smoke without exporting handles.
- [x] Add reduced clear-color smoke status and validation docs for GL failure
  modes.
- [x] Add a GBM-backed EGL platform candidate doc before moving beyond
  `DEFAULT_DISPLAY`.
- [x] Define the first presentation smoke boundary without exposing GPU handles.
- [x] Add renderer-live reduced presentation status before admitting scanout or
  exported buffer paths.
- [x] Add fake presentation smoke coverage for ready, unavailable, and degraded
  statuses.
- [x] Add a GBM-backed EGL platform status model behind `gbm-probe,egl-probe`.
- [x] Add fake GBM-backed EGL platform projection coverage before native code.
- [x] Add a native GBM-backed EGL platform smoke that preserves
  `LiveGbmBackedEglPlatformReport`.
- [x] Keep `DEFAULT_DISPLAY` as fallback smoke until the GBM-backed platform
  smoke passes.
- [x] Add a GBM-backed EGL private target smoke without exporting buffers.
- [x] Keep `DEFAULT_DISPLAY` clear-color smoke as fallback until GBM-backed
  drawing passes.
- [x] Add a native offscreen presentation smoke that preserves reduced
  presentation status.
- [x] Add opt-in real render-node validation for GBM-backed EGL drawing and
  offscreen presentation.
- [x] Define the first scanout-adjacent status without exposing KMS object
  identity.
- [x] Decide when `SOPHIA_RUN_REAL_GBM_SMOKE=1` results are strong enough to
  retire `DEFAULT_DISPLAY` as a fallback smoke.
- [x] Add the first reduced page-flip event shape without KMS object identity.
- [x] Thread reduced scanout and page-flip events into runtime observation.
- [x] Define the first deterministic page-flip callback intake seam.
- [x] Wire page-flip callbacks into a runtime-owned bounded queue.
- [x] Add a fake page-flip callback source before real libdrm event polling.
- [x] Define the feature-gated libdrm page-flip event polling adapter shape.
- [x] Evaluate concrete libdrm crate admission for native page-flip polling.
- [x] Add optional `drm` dependency under `libdrm-events` without wiring native
  polling yet.
- [x] Add a private native libdrm event adapter module skeleton without opening
  devices.
- [x] Define backend-owned libdrm fd authority shape without exposing fds.
- [x] Let the private libdrm adapter accept backend-owned authority without
  polling.

---

## Deferred / Prototype Reference

These items are useful evidence from the XLibre-centered prototype, but they are
not the long-term target architecture.

- [x] Keep SHM routed input deferred unless repeated routed-input stress
  measurements exceed the documented threshold.
- [x] Keep XLibre routed-input extension docs as a compatibility lesson.
- [x] Keep XComposite/Damage bridge smokes as reference tests until Sophia X
  Authority has equivalent transaction tests.
- [x] Keep XLibre namespace smoke as isolation evidence until Sophia X
  Authority namespace enforcement has live coverage.
- [x] Keep the documented Sophia X11 WM Bridge as a stopgap for reusing legacy
  tiling WMs without weakening the native blind WM IPC boundary.

---

## Completed Milestones

- [x] Engine-centered authority reframe: README, architecture docs, atomic
  rendering invariant, and XLibre prototype/reference status.
- [x] Data-oriented design and style rules, including domain-first file
  cohesion guidance.
- [x] Phase 0-2: repository shape, Rust skeleton, protocol/data model, and
  headless engine.
- [x] Phase 3-4: XLibre mirror probe, XComposite/Damage capture, CPU readback,
  and first X11 surface in headless frames.
- [x] Phase 5-6.5: blind WM protocol, bounded IPC codec, external WM demo,
  routed-input XLibre patch, and smoke/stress coverage.
- [x] Phase 7-8: portal reducers, compositor chrome action reducer, and polite
  X11 close helper.
- [x] Phase 9: process supervisor, restart policy, WM restart adapter, and last
  committed layout cache.
- [x] Session runtime assembly: runtime reducer, bounded observation intake,
  headless session driver, broker health/control packets, and live X/WM socket
  smoke.
- [x] Portal execution prototype: X11 `SelectionRequest` conversion, native
  denial, approved bounded text handoff, and live X smoke.
- [x] Protocol-neutral authority transactions: `AuthoritySurface`,
  `SurfaceTransaction`, readiness states, and committed surface projection into
  renderable layers.
- [x] Sophia X Authority design: namespace-scoped resources, event
  subscriptions, synthetic lifecycle, drawing updates, and selection portal
  conversion.
- [x] Sophia X Authority v0 runtime: internal request/response packets, bounded
  codec, reducer-backed runtime, Unix socket helper, and
  `x-authority-runtime-smoke`.
- [x] Sophia X Authority X11 wire start: setup parser, setup success/failure
  encoders, first core request decoder, minimal property table, and setup
  socket smoke.
- [x] Sophia X Authority client-visible output: bounded X error/event records,
  `ConfigureNotify`, `MapNotify`, `PropertyNotify`, `SelectionNotify`, and
  setup/create/map socket smoke.
- [x] Future Wayland Authority boundary documented as a later protocol
  authority, not the architectural center.
- [x] Backend skeletons: frame clock, renderer/import abstraction, DRM/KMS
  discovery, libinput polling, physical input routing, and page-flip timing
  seams.
