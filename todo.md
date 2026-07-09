# Sophia Active Roadmap

Sophia is a research prototype. This file tracks the active architecture path
and keeps completed milestones compact. Detailed rationale and historical
evidence belong in `docs/research-log.md`.

---

## Active Focus - Sophia X Authority: Tiny Real Xlib Smoke

**Now**
- [ ] Define the smallest real Xlib client target: connect, intern atoms,
  create a window, set/read a title, map, and observe events.
- [ ] Add a CLI smoke wrapper that starts the Sophia X Authority socket and
  runs the tiny client against it.
- [ ] Record the next missing X11 opcode or reply shape found by the real
  client instead of guessing ahead.

**Next**
- [ ] Add minimal request coverage for the first real-client failure.
- [ ] Re-run the Xlib smoke until the connect/create/map/property path is
  stable.

---

## Next Architecture Milestones

- [x] Expand X11 atom/property tables for ICCCM names and metadata-broker
  candidates.
- [x] Add minimal bounded `GetProperty` replies and socket smoke coverage.
- [ ] Define the first real-client target after synthetic setup succeeds:
  likely a tiny Xlib window before GTK/Qt/browser paths.
- [ ] Revisit compositor backend work after X Authority can create, map, draw,
  and expose a simple client through the authority transaction model.

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
