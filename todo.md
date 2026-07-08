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
- [x] Add DRM/KMS output skeleton.
- [x] Add libinput event source skeleton.
- [x] Integrate physical input with routed-input request generation.

---

## Rendering Track - Move Beyond Proof Grade

- [x] Replace CPU-readback-only rendering with import-capable buffer handles.
- [x] Track buffer lifetime explicitly across XComposite pixmap updates.
- [x] Add frame scheduling around X Damage and layout epochs.
- [x] Measure resize behavior under slow or non-cooperative X11 clients.

---

## Routed Input Track

The current X11 `RouteEvent` request path is the correctness baseline.

- [x] Measure routed-input dispatch cost before replacing the X11 request path.
- [x] Keep the X11 request path as fallback for any future SHM work.
- [x] Add routed-input grab/focus edge smokes.
- [x] Add transformed scene hit-test integration once physical input exists.

## Continuous Runtime Track

The one-shot smoke paths have proven the seams. The next work is assembling a
repeatable runtime loop without putting policy or X11 authority on compositor
hot paths.

- [x] Add a data-only session runtime reducer for X polling, WM policy, frame
  scheduling, portal drain, and chrome presentation phases.
- [x] Connect the reducer to the headless session tick smoke.
- [x] Add a runtime smoke that schedules from X Damage and layout epochs.
- [x] Add process-supervised portal and metadata broker placeholders.

## Broker IPC Track

The broker processes are supervised placeholders. The next work is turning them
into bounded, data-only peers without exposing client metadata or payload bytes
to runtime policy.

- [x] Add a bounded broker health/control packet contract.
- [x] Wire portal broker placeholder to a bounded IPC health smoke.
- [ ] Wire metadata broker placeholder to a bounded IPC health smoke.
- [ ] Route broker health into `SessionRuntimeState`.

## Deferred / Measurement-Gated

- [ ] Prototype a unidirectional Engine-to-XLibre SHM route ring only if
  repeated routed-input stress measurements exceed the documented threshold.

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
