# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Live Session Composition

Current architecture read:

- Sophia Engine owns visual truth and `SurfaceTransaction` commit readiness.
- Sophia X Authority owns X protocol resources and emits bounded transaction
  batches; it does not own layout, chrome, input devices, rendering, or scanout.
- backend-live and renderer-live own native IO, GBM/EGL, libdrm, page-flip, and
  scanout lifetimes behind reduced evidence records.
- Sophia WM remains blind policy. XLibre remains prototype/reference material,
  not the destination architecture.

Current milestone target:

- [ ] Compose Sophia X Authority's bounded transaction queue, runtime intake,
  renderer-live frame targets, and backend-live rendered scanout into one
  operator smoke.
- [ ] Preserve reduced evidence as the public validation surface for authority
  health, runtime transaction intake, rendered scanout submit, page-flip retire,
  and cleanup.
- [ ] Keep Wayland Authority and wgpu deferred until the X Authority plus live
  scanout path is stable.

---

## Next 3 Milestones

### 1. Live Session Composition

- [ ] Compose Sophia X Authority's bounded transaction queue, runtime intake,
  renderer-live frame targets, and backend-live rendered scanout into one
  operator smoke.
- [ ] Preserve reduced evidence as the public validation surface for authority
  health, runtime transaction intake, rendered scanout submit, page-flip retire,
  and cleanup.
- [ ] Keep Wayland Authority and wgpu deferred until the X Authority plus live
  scanout path is stable.

### 2. Authority Coverage From Real Probe Failures

- [ ] Expand Sophia X Authority only where `xclock` and later real probes demand
  it.
- [ ] Prefer bounded drawing, upload, present, selection, event, and namespace
  behavior over broad X11 completeness.
- [ ] Keep XLibre bridge smokes as prototype references until Sophia X Authority
  has equivalent live coverage.

### 3. Wayland Authority Skeleton

- [ ] Define the first minimal Wayland Authority socket/setup boundary without
  committing to wgpu or broad compositor-framework adoption.
- [ ] Preserve the same authority contract: protocol resources in the authority,
  visual truth and commit readiness in Sophia Engine.
- [ ] Start only after live X Authority transaction intake and rendered scanout
  composition have one operator-grade smoke.

---

## Later Backlog

- [ ] Continue splitting backend-live by domain where modules still mix
  unrelated authority, renderer, runtime, or scanout ownership.
- [ ] Retire XLibre prototype smokes only after Sophia X Authority has
  equivalent live transaction, namespace, selection, and routed-input coverage.

---

## Done Recently

- [x] `x-authority-xclock-smoke` launches `/usr/bin/xclock`, reaches mapped
  surface exposure, decodes the xclock-driven font, pixmap, window, and drawing
  requests, and commits observed authority transactions through Engine/Runtime
  counters without X protocol errors.
- [x] Closed the TTY3 combined hardware proof: preflight, destructive two-phase
  atomic scanout, and runtime rendered-scanout submit-to-retire evidence all
  pass their reduced verifiers.
- [x] TTY3 atomic scanout smoke now passes both initial modeset and steady
  page-flip phases with retained rendered GBM/KMS ownership until page-flip
  retirement.
- [x] backend-live imports renderer-exported DMA-BUFs into the KMS submit device
  before framebuffer creation and closes imported GEM handles through the
  existing cleanup path.
- [x] Reduced atomic scanout and runtime rendered-scanout evidence now records
  scanout-buffer layout, primary-plane format-table presence, framebuffer
  creation path, submit status, retire status, and cleanup debt.
- [x] Sophia X Authority already covers bounded X11 setup, atoms/properties,
  x11rb, `xdpyinfo`, C Xlib, `XFillRectangle`, `XPutImage`, and private
  Present-style transaction smokes through Engine/Runtime counters.
