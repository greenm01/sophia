# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Native Backend Event Intake

- [x] Add a bounded native libdrm page-flip reader contract that feeds the
  existing reduced callback queue without exposing KMS identity.
- [x] Add a feature-gated native libinput event poller shape that implements the
  engine's non-blocking input contract without admitting a concrete libinput
  dependency.
- [x] Validate both native-shaped event intake features:
  `cargo test --offline -p sophia-backend-live --features libdrm-events,libinput-events`.

---

## Next 3 Milestones

### 1. Generic Runtime Backend Pollers

- [ ] Decide whether `LiveBackendRuntimeAssembly` should become generic over
  input poller type or accept boxed poller adapters.
- [ ] Keep `QueuedInputPoller` as the deterministic default.
- [ ] Add one smoke proving native-shaped input polling can drive runtime
  assembly without changing Sophia Engine.

### 2. Real Backend Hardware Gates

- [ ] Define opt-in environment gates for real libdrm and libinput validation.
- [ ] Require native hardware tests to fail closed and return reduced reports.
- [ ] Keep default workspace validation independent of device nodes and seats.

### 3. Authority Probe Selection

- [ ] Choose the next real Sophia X Authority app probe after backend intake
  seams settle.
- [ ] Prefer probes that exercise bounded drawing/upload/present paths over
  broad X11 completeness.
- [ ] Keep Wayland Authority deferred until backend event intake and scanout
  timing are stable.

---

## Later Backlog

- [ ] Expand Sophia X Authority only where real app probes demand it,
  prioritizing bounded drawing, upload, present, selection, and namespace
  behavior over broad X11 completeness.
- [ ] Start Sophia Wayland Authority only after live backend frame targets and
  scanout timing are stable.
- [ ] Keep the XLibre prototype docs and bridge smokes as compatibility lessons
  until Sophia X Authority has equivalent live coverage.
- [ ] Revisit wgpu only after GBM/EGL startup, drawing, presentation,
  frame-target lifecycle, and scanout seams are validated.

---

## Done Recently

- [x] Recorded real GBM/EGL evidence with frame-target allocation status.
- [x] Added reduced frame-target lifecycle observations.
- [x] Added reduced KMS scanout target reports and page-flip readiness
  projection.
- [x] Added a fake live compositor loop smoke before admitting real scanout.
- [x] Added reduced GBM/EGL frame-target readiness to backend startup and runtime
  ticks.
- [x] Added explicit runtime mutation for reduced frame-target size changes.
- [x] Defined renderer-private GBM/EGL frame-target allocation requests and
  reports.
- [x] Threaded fake allocation reports through backend-live without exposing
  renderer-private handles.
- [x] Added native GBM/EGL frame-target allocation skeletons behind existing
  GBM/EGL features.
- [x] Exposed native frame-target allocation through backend-live and runtime
  assembly as explicit caller actions.
