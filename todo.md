# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### External X Client Probe: xclock

Current architecture read:

- Sophia Engine owns visual truth and `SurfaceTransaction` commit readiness.
- Sophia X Authority owns X protocol resources and emits bounded transaction
  batches; it does not own layout, chrome, input devices, rendering, or scanout.
- backend-live and renderer-live own native IO, GBM/EGL, libdrm, page-flip, and
  scanout lifetimes behind reduced evidence records.
- Sophia WM remains blind policy. XLibre remains prototype/reference material,
  not the destination architecture.

Current probe target:

- [ ] Add an `x-authority-xclock-smoke` probe that launches `/usr/bin/xclock`
  against the Sophia X Authority socket.
- [ ] Let the first concrete missing opcode, reply, event, or extension from
  `xclock` drive the next X Authority implementation step.
- [ ] Pass only when the probe reaches at least one Engine/Runtime committed
  authority transaction without leaking XIDs, namespace IDs, titles, classes,
  PIDs, or raw property payloads across the WM boundary.

---

## Next 3 Milestones

### 1. External X Client Probe: xclock

- [ ] Add an `x-authority-xclock-smoke` probe that launches `/usr/bin/xclock`
  against the Sophia X Authority socket.
- [ ] Let the first concrete missing opcode, reply, event, or extension from
  `xclock` drive the next X Authority implementation step.
- [ ] Pass only when the probe reaches at least one Engine/Runtime committed
  authority transaction without leaking XIDs, namespace IDs, titles, classes,
  PIDs, or raw property payloads across the WM boundary.

### 2. Live Session Composition

- [ ] Compose Sophia X Authority's bounded transaction queue, runtime intake,
  renderer-live frame targets, and backend-live rendered scanout into one
  operator smoke.
- [ ] Preserve reduced evidence as the public validation surface for authority
  health, runtime transaction intake, rendered scanout submit, page-flip retire,
  and cleanup.
- [ ] Keep Wayland Authority and wgpu deferred until the X Authority plus live
  scanout path is stable.

### 3. Authority Coverage From Real Probe Failures

- [ ] Expand Sophia X Authority only where `xclock` and later real probes demand
  it.
- [ ] Prefer bounded drawing, upload, present, selection, event, and namespace
  behavior over broad X11 completeness.
- [ ] Keep XLibre bridge smokes as prototype references until Sophia X Authority
  has equivalent live coverage.

---

## Later Backlog

- [ ] Continue splitting backend-live by domain where modules still mix
  unrelated authority, renderer, runtime, or scanout ownership.
- [ ] Retire XLibre prototype smokes only after Sophia X Authority has
  equivalent live transaction, namespace, selection, and routed-input coverage.

---

## Done Recently

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
