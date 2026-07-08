# Sophia Active Roadmap

Sophia is a research prototype. This file tracks active architecture work and
keeps completed milestones compact. Detailed rationale and historical evidence
belong in `docs/research-log.md`.

---

## Active Focus - Session Runtime Assembly

Close the gap between existing reducers/protocols and a running session loop.

**Now**
- [x] Add a headless session tick that turns layer snapshots into frame/replay
  reports while preserving the last committed layout.
- [x] Convert XFixes selection owner updates into clipboard portal owner-change
  events.
- [x] Route clipboard owner-change events into `ClipboardPortal` revocation
  commands.
- [x] Add a headless runtime smoke that performs: X capture -> session tick ->
  frame replay.

**Next**
- [x] Route notification delivery commands to compositor chrome presentation.
- [x] Route sanitized metadata broker output into compositor chrome descriptors.
- [x] Add a supervised long-lived WM socket smoke with kill/restart behavior.
- [x] Add a portal smoke proving denied cross-namespace clipboard transfer
  becomes normal X11 selection failure.

---

## Backend Track - Real Compositor Work

Do this after the runtime loop has a clear shape.

- [x] Add frame-clock abstraction while preserving headless determinism.
- [x] Add renderer/import abstraction with CPU readback kept as fallback.
- [ ] Add DRM/KMS output skeleton.
- [ ] Add libinput event source skeleton.
- [ ] Integrate physical input with routed-input request generation.

---

## Rendering Track - Move Beyond Proof Grade

- [ ] Replace CPU-readback-only rendering with import-capable buffer handles.
- [ ] Track buffer lifetime explicitly across XComposite pixmap updates.
- [ ] Add frame scheduling around X Damage and layout epochs.
- [ ] Measure resize behavior under slow or non-cooperative X11 clients.

---

## Routed Input Track

The current X11 `RouteEvent` request path is the correctness baseline.

- [x] Measure routed-input dispatch cost before replacing the X11 request path.
- [x] Keep the X11 request path as fallback for any future SHM work.
- [ ] Add routed-input grab/focus edge smokes.
- [ ] Add transformed scene hit-test integration once physical input exists.
- [ ] Prototype a unidirectional Engine-to-XLibre SHM route ring only if
  repeated measurements justify it.

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
