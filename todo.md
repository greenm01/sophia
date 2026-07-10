# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Real Libinput Adapter

- [ ] Add a concrete libinput reader only behind `libinput-events`.
- [ ] Preserve the existing native-shaped reader and poller report contract.
- [ ] Avoid raw device paths, fd values, seat names, or libinput error strings in
  public runtime reports.

---

## Next 3 Milestones

### 1. Real Libdrm Event Reader

- [ ] Add a concrete page-flip reader only behind `libdrm-events`.
- [ ] Preserve reduced output-route decoding before runtime observation.
- [ ] Keep scanout object identity private to backend-live.

### 2. Authority Probe Selection

- [ ] Choose the next real Sophia X Authority app probe after backend intake
  seams settle.
- [ ] Prefer probes that exercise bounded drawing/upload/present paths over
  broad X11 completeness.
- [ ] Keep Wayland Authority deferred until backend event intake and scanout
  timing are stable.

### 3. Real Input Loop

- [ ] Poll concrete input readers from the live runtime without blocking scanout.
- [ ] Keep physical input and routed-input transformation separate.
- [ ] Preserve deterministic queued poller tests as the default validation path.
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

- [x] Added a reduced `LiveAtomicScanoutCommitReport` that maps Engine
  `PageFlipCommitOutcome` into runtime-safe scanout commit state.
- [x] Added `LiveAtomicScanoutCommitter` so runtime assembly commits through a
  backend-owned scanout boundary.
- [x] Defined opt-in environment gates for real libdrm and libinput validation.
- [x] Kept default workspace validation independent of device nodes and seats.
- [x] Added reduced real-hardware smoke reports that fail closed before concrete
  native readers exist.
- [x] Made compositor backend assemblies generic over `NonBlockingInputPoller`
  instead of boxing the hot path.
- [x] Kept `QueuedInputPoller` as the default deterministic backend assembly
  poller.
- [x] Added a native-shaped libinput runtime smoke proving live assembly can run
  a tick without changing Sophia Engine.
- [x] Added bounded native libdrm and libinput event intake seams.
- [x] Validated combined native-shaped event intake features.
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
