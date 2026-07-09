# Sophia Active Roadmap

Sophia is a research prototype. This file tracks the active architecture path
and keeps completed milestones compact. Detailed rationale and historical
evidence belong in `docs/research-log.md`.

---

## Active Focus - Sophia X Authority: X11 Setup Parser

The internal X Authority runtime is executable. The next architecture step is
real X11 connection setup parsing that feeds the existing `XAuthorityRuntime`
reducers instead of creating a second authority path.

**Now**
- [ ] Add an X11 setup parser fixture for byte order, protocol version,
  authorization name/data, resource ID base, and resource ID mask.
- [ ] Model setup success and failure replies as bounded authority artifacts.
- [ ] Add integration tests for little-endian and big-endian setup handshakes.
- [ ] Add malformed setup tests for truncated input, unsupported major version,
  and overlarge auth fields.

**Next**
- [ ] Decode first core X11 request fixtures into existing internal authority
  requests: `CreateWindow`, `MapWindow`, `ChangeProperty`, and selection
  ownership.
- [ ] Add a local Unix socket smoke that completes an X11 setup handshake with a
  tiny synthetic client without running full applications yet.
- [ ] Keep request decoding isolated from runtime reducers: wire parsing should
  produce internal request packets, then `XAuthorityRuntime` executes them.

---

## Next Architecture Milestones

- [ ] Add X11 atom/property tables needed by `ChangeProperty`, ICCCM names, and
  metadata-broker candidates.
- [ ] Add minimal X event/reply emission for setup, errors, map/configure, and
  selection request outcomes.
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
- [x] Future Wayland Authority boundary documented as a later protocol
  authority, not the architectural center.
- [x] Backend skeletons: frame clock, renderer/import abstraction, DRM/KMS
  discovery, libinput polling, physical input routing, and page-flip timing
  seams.
