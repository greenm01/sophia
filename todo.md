# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Real Backend Evidence

- [x] Decide the opt-in real-device evidence shape for native GBM/EGL
  frame-target allocation.
- [x] Extend `LiveRealGbmSmokeEvidence` with reduced frame-target allocation
  status.
- [x] Update validation docs so the real GBM/EGL smoke records draw,
  presentation, and allocation status without exposing render-node paths, file
  descriptors, GBM/EGL handles, pixels, driver errors, or KMS object identity.
- [x] Run and record the opt-in real-device validation:
  `SOPHIA_RUN_REAL_GBM_SMOKE=1 cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe`.

---

## Next 3 Milestones

### 1. Frame-Target Lifecycle

- [x] Define renderer-private frame-target lifecycle states: create, retain,
  resize, invalidate, and retire.
- [x] Keep runtime observations reduced to target size, allocation status, and
  lifecycle status.
- [x] Preserve the current rule that runtime ticks do not allocate native frame
  targets implicitly.

### 2. Scanout Path

- [x] Define the first reduced KMS scanout target report.
- [x] Connect renderer presentation readiness to page-flip readiness without
  exposing connector, CRTC, plane, framebuffer, fd, or driver identity.
- [x] Keep CPU fallback and degraded GPU paths valid while scanout matures.

### 3. Live Compositor Runtime Loop

- [x] Assemble a runtime-owned loop that sequences input polling, authority
  transaction intake, WM policy, renderer target updates, frame commit, and
  reduced page-flip observation.
- [x] Keep Sophia Engine independent of protocol authority policy and
  renderer-private resource ownership.
- [x] Add one smoke proving the loop can run with fake backend components before
  real DRM/KMS scanout is admitted.

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
