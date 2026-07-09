# Sophia Active Roadmap

Sophia is a research prototype. This file tracks the active architecture path
and keeps completed milestones compact. Detailed rationale and historical
evidence belong in `docs/research-log.md`.

---

## Active Focus - Real Backend Boundaries

**Now**
- [ ] Keep default workspace tests independent of native renderer libraries.
- [ ] Revisit real GBM dependency admission only after the feature-gated fake
  path exists.

**Next**
- [ ] Decide the real GBM probe API shape: device-path intake, borrowed fd
  intake, or backend-provided reduced device token.
- [ ] Keep real GBM dependency optional until CI exercises both default and
  feature-enabled paths.

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
