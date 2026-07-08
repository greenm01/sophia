# Sophia Active Roadmap

Sophia is a research prototype. This file tracks active architecture work and
keeps completed milestones compact. Detailed rationale and historical evidence
belong in `docs/research-log.md`.

---

## Active Focus - Engine-Centered Authorities

Reframe Sophia around Sophia Engine as the permanent visual authority and
protocol authorities as client-protocol adapters.

**Now**
- [x] Reframe README around Engine-owned input, atomic rendering, and protocol
  authorities.
- [x] Update architecture docs with Sophia X Authority, future Wayland
  Authority, future Native Authority, and protocol-neutral namespace boundaries.
- [x] Add docs-level atomic rendering invariant: never present new geometry
  without matching committed pixels.
- [x] Move XLibre bridge/routed-input work into prototype/reference status
  instead of long-term architecture status.

**Next**
- [ ] Define authority-facing docs-level `AuthoritySurface`,
  `SurfaceTransaction`, and `CommittedSurfaceState` acceptance criteria in more
  implementation-ready terms.
- [ ] Map existing `SurfaceSnapshot`, `LayerSnapshot`, `DamageFrame`, and
  `LayoutEpochState` concepts onto the new transaction model.
- [ ] Decide which existing XLibre bridge tests remain regression references
  and which should be retired once Sophia X Authority starts.

---

## Sophia X Authority Track

Replace the long-term dependency on XLibre/Xorg with a Sophia-owned modern X
protocol subset that can run real applications without carrying the full legacy
server object graph.

- [ ] Define the minimum X protocol subset for real app compatibility:
  core windows/pixmaps/atoms/properties/events, ICCCM/EWMH, XKB, XFixes, Sync,
  Render, SHM, DRI3/Present, RandR, and selected GLX compatibility.
- [ ] Define namespace-aware X resource ownership, lookup, event subscription,
  selection, focus, grab, and property access rules.
- [ ] Define how X drawing paths become Sophia pending buffers:
  PresentPixmap, DRI3 DMA-BUF, SHM/software updates, Render, and core drawing.
- [ ] Define X selection, clipboard, drag-and-drop, URI, notification, and
  screen-capture requests as protocol-specific inputs to Sophia Portals.
- [ ] Define X lifecycle and polite close semantics as authority commands that
  preserve the blind WM boundary.
- [ ] Identify Phoenix components and tests worth studying before implementation.

---

## Atomic Transaction Track

Make macOS-style transaction integrity a first-class Sophia invariant.

- [ ] Define pending versus committed surface state in the engine data model.
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

- [ ] Keep SHM routed input deferred unless repeated routed-input stress
  measurements exceed the documented threshold.
- [ ] Keep XLibre routed-input extension docs as a compatibility lesson.
- [ ] Keep XComposite/Damage bridge smokes as reference tests until Sophia X
  Authority has equivalent transaction tests.
- [ ] Keep XLibre namespace smoke as isolation evidence until the Sophia X
  Authority namespace model has live coverage.

---

## Completed Milestones

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
