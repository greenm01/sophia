# Sophia Active Roadmap

Sophia is a research prototype. This file tracks active architecture work and
keeps completed milestones compact. Detailed rationale and historical evidence
belong in `docs/research-log.md`.

---

## Active Focus - Sophia X Authority Design

Define the long-term X protocol authority that replaces XLibre/Xorg as the
target dependency while preserving Engine-owned atomic visual commits and
namespace boundaries.

**Now**
- [x] Start `docs/sophia-x-authority.md` with authority ownership boundaries,
  minimum protocol subset, namespace resource model, drawing-to-buffer
  readiness, selections/portals, lifecycle, input delivery, Phoenix study
  targets, and first implementation milestones.

**Next**
- [x] Add a `sophia-x-authority` crate skeleton with passive resource tables
  and no live socket yet.
- [x] Model namespace-scoped X resource lookup and event subscription in tests.
- [x] Model `AuthoritySurface` creation from synthetic X window lifecycle
  events.
- [x] Convert a synthetic Present/SHM/CoreDraw update into a ready
  `SurfaceTransaction`.
- [x] Convert a synthetic selection request into a portal request and native X
  denial/handoff artifact.

---

## Completed Focus - Executable Authority Transactions

- [x] Add protocol-neutral `AuthorityKind`, `AuthorityLocalId`,
  `AuthoritySurface`, `SurfaceTransaction`, `SurfaceTransactionReadiness`, and
  `CommittedSurfaceState` data records.
- [x] Map existing `SurfaceSnapshot`, `LayerSnapshot`, `DamageFrame`, and
  `LayoutEpochState` concepts into `SurfaceTransaction` records and readiness
  states for the XLibre prototype path.
- [x] Add engine helpers that commit ready surface transactions atomically while
  preserving the previous committed state for pending, failed, timed-out,
  stale, or invalid transactions.
- [x] Project committed surface state back into renderable `LayerSnapshot`
  values so render planning can consume committed visual truth rather than raw
  authority snapshots.
- [x] Thread authority transaction outcomes into headless session runtime
  observations as reduced outcome/count data without exposing protocol-local
  IDs, namespace metadata, or surface IDs to the WM.
- [x] Add a headless/live session adapter path that projects committed state
  before frame planning when authority transactions are present.
- [x] Keep an XLibre bridge regression that marks the old snapshot path as
  `AuthorityKind::XLibrePrototype`.
- [x] Define which XLibre bridge smokes remain prototype references and which
  should retire once Sophia X Authority has equivalent coverage. See
  `docs/xlibre-prototype-regression-map.md`.

---

## Sophia X Authority Track

Replace the long-term dependency on XLibre/Xorg with a Sophia-owned modern X
protocol subset that can run real applications without carrying the full legacy
server object graph.

- [x] Define the minimum X protocol subset for real app compatibility:
  core windows/pixmaps/atoms/properties/events, ICCCM/EWMH, XKB, XFixes, Sync,
  Render, SHM, DRI3/Present, RandR, and selected GLX compatibility.
- [x] Define namespace-aware X resource ownership, lookup, event subscription,
  selection, focus, grab, and property access rules.
- [x] Define how X drawing paths become Sophia pending buffers:
  PresentPixmap, DRI3 DMA-BUF, SHM/software updates, Render, and core drawing.
- [x] Define X selection, clipboard, drag-and-drop, URI, notification, and
  screen-capture requests as protocol-specific inputs to Sophia Portals.
- [x] Define X lifecycle and polite close semantics as authority commands that
  preserve the blind WM boundary.
- [x] Identify Phoenix components and tests worth studying before implementation.

---

## Atomic Transaction Track

Make macOS-style transaction integrity a first-class Sophia invariant.

- [x] Define pending versus committed surface state in the engine data model.
- [ ] Define buffer/geometry readiness and the conditions required to commit a
  visual transaction.
- [ ] Define fail-closed slow-client behavior: keep the last committed visual
  state unless timeout policy explicitly degrades.
- [ ] Define timeout/degrade reporting so chronic offenders can be measured
  without leaking protocol metadata to the WM.
- [ ] Update frame scheduling docs so layout epochs become a prototype-specific
  compatibility mechanism, while authority-native commits use explicit
  readiness.

---

## Future Wayland Authority Track

Document the later path for Wayland-only applications without turning Sophia
into a Wayland compositor as the architectural center.

- [ ] Map `wl_surface` attach/damage/commit into Sophia `SurfaceTransaction`
  readiness.
- [ ] Map `xdg_toplevel` configure/ack/lifecycle into authority-owned protocol
  semantics and Engine-owned visual commits.
- [ ] Define Wayland input delivery as Engine-routed, authority-delivered, and
  namespace-checked.
- [ ] Define Wayland clipboard/data-device/screencopy-style requests as portal
  inputs instead of compositor-wide privileges.
- [ ] Document that Wayland Authority must not own workspaces, global shortcuts,
  compositor chrome, or scanout.

---

## Backend Track - Real Compositor Work

Do this after the authority transaction model has a clear shape.

- [x] Add frame-clock abstraction while preserving headless determinism.
- [x] Add renderer/import abstraction with CPU readback kept as fallback.
- [x] Add DRM/KMS output skeleton.
- [x] Add libinput event source skeleton.
- [x] Integrate physical input with routed-input request generation.
- [ ] Replace skeleton DRM/KMS descriptors with a real device/output discovery
  adapter.
- [ ] Replace libinput descriptor intake with a real non-blocking physical input
  poller.
- [ ] Connect real frame-clock/page-flip timing to transaction commits.

---

## Deferred / Prototype Reference

These items are useful evidence from the XLibre-centered prototype, but they are
not the long-term target architecture.

- [x] Keep SHM routed input deferred unless repeated routed-input stress
  measurements exceed the documented threshold.
- [x] Keep XLibre routed-input extension docs as a compatibility lesson.
- [x] Keep XComposite/Damage bridge smokes as reference tests until Sophia X
  Authority has equivalent transaction tests.
- [x] Keep XLibre namespace smoke as isolation evidence until the Sophia X
  Authority namespace model has live coverage.

---

## Completed Milestones

- [x] Engine-centered authority reframe: README, architecture docs, atomic
  rendering invariant, and XLibre prototype/reference status.
- [x] Phase 0-2: repository shape, Rust skeleton, protocol/data model, headless
  engine.
- [x] Phase 3-4: XLibre mirror probe, XComposite/Damage capture, CPU readback,
  first X11 surface in headless frames.
- [x] Phase 5-6.5: blind WM protocol, bounded IPC codec, external WM demo,
  routed-input XLibre patch and smoke/stress coverage.
- [x] Phase 7-8: portal reducers, compositor chrome action reducer, polite X11
  close helper.
- [x] Phase 9 supervisor work: restart policy, process supervisor, WM restart
  adapter, last committed layout cache.
- [x] Session runtime assembly: data-only runtime reducer, bounded observation
  intake, headless session driver, broker health/control packets, and live X/WM
  socket smoke.
- [x] Portal execution prototype: X11 `SelectionRequest` conversion, denial as
  native selection failure, approved bounded text handoff, and live X smoke.
