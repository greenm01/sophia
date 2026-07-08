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
- [x] Add a resumable session runtime loop that batches reduced events into
  commands without polling file descriptors inside policy code.
- [x] Add a bounded runtime observation adapter for X, broker, WM, portal,
  chrome, and renderer facts.
- [x] Connect concrete X bridge, WM transport, broker IPC, portal execution,
  chrome presenter, and renderer reports to `SessionRuntimeObservation` batches.
- [x] Add a single headless session-driver smoke that executes runtime commands
  through the concrete adapters instead of each smoke owning its own mini-loop.
- [x] Replace remaining per-smoke runtime command execution with the reusable
  headless session driver where the smoke does not need custom setup.
- [x] Add a runtime driver adapter trait so live X, WM, broker, portal, chrome,
  and renderer sources can plug into one command executor.

---

## Synthetic-To-Live Completion Track

Synthetic seams should either become live adapters, remain deterministic test
harnesses, or stay explicitly measurement-gated.

- [x] Keep `HeadlessSessionDriver` as the deterministic session executor.
- [x] Add a `RuntimeDriverAdapter` trait for command execution sources.
- [x] Add a headless adapter implementation for deterministic tests and smokes.
- [x] Add live adapter skeletons for X, WM, broker health, portal, chrome, and
  renderer facts.
- [x] Replace live adapter skeleton inputs with non-blocking X bridge, WM socket,
  broker IPC, portal execution, chrome presenter, and renderer intake.
- [x] Add one live command-executor smoke that runs against Xvfb and the WM
  socket without hand-written command sequencing.
- [ ] Keep SHM routed input deferred until stress measurements exceed the
  documented threshold.

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
- [x] Model explicit XSync versus implicit legacy resize capability with
  bridge-owned timeout reputation.

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
- [x] Wire metadata broker placeholder to a bounded IPC health smoke.
- [x] Route broker health into `SessionRuntimeState`.

## Portal Execution Track

Portal reducers exist. The next work is turning X-derived requestor events into
bounded portal execution without putting raw X authority in portal policy.

- [x] Convert X11 `SelectionRequest` context into a cross-namespace clipboard
  portal import request with native failure reply context.
- [x] Dispatch live X11 `SelectionRequest` events into the clipboard portal
  runtime path.
- [x] Implement approved clipboard handoff for one bounded text target.
- [x] Add a live X smoke for request -> deny and request -> approved handoff.

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
