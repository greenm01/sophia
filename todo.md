# Sophia Active Roadmap

Sophia is a research prototype. This file tracks the active architecture path
and keeps completed milestones compact. Detailed rationale and historical
evidence belong in `docs/research-log.md`.

---

## Active Focus - Sophia X Authority: SHM Import Decision

**Now**
- [ ] Decide whether the next X Authority step should map real MIT-SHM segments
  or defer memory import until compositor backend work needs it.
- [ ] If SHM import proceeds, document the bounded `shmat`/detach lifetime and
  namespace cleanup rules before code.
- [ ] If SHM import is deferred, move active focus to compositor backend
  assembly with the current core draw, PutImage, and Present seams as inputs.

**Next**
- [ ] Revisit compositor backend work after X Authority can create, map, draw,
  and expose a simple client through the authority transaction model.
- [ ] Convert the bounded X Authority transaction channel into the session
  supervisor path once the long-running authority process exists.

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
