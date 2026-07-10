# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Hardware Proof Closeout

Current architecture read:

- Sophia Engine owns visual truth and `SurfaceTransaction` commit readiness.
- Sophia X Authority owns X protocol resources and emits bounded transaction
  batches; it does not own layout, chrome, input devices, rendering, or scanout.
- backend-live and renderer-live own native IO, GBM/EGL, libdrm, page-flip, and
  scanout lifetimes behind reduced evidence records.
- Sophia WM remains blind policy. XLibre remains prototype/reference material,
  not the destination architecture.

Current proof baseline:

- [x] The TTY3 atomic scanout smoke passed both `InitialModeset` and
  `SteadyPageFlip` phases with schema 10 evidence, PRIME-imported rendered GBM
  buffers, `framebuffer=CreatedWithAddFb2`, native page-flip callbacks, and
  clean resource retirement.
- [ ] Run the combined proof on TTY3:
  `tools/atomic_scanout_hardware_proof.sh --slot=1 --output=1 --authority=1
  --page-flip-timeout-ms=8000 --child-timeout-ms=30000`.
- [ ] Verify the combined proof captures passing preflight, destructive
  two-phase atomic scanout evidence, and runtime rendered-scanout
  submit-to-retire evidence.
- [ ] Record only the reduced proof summary here after it passes; keep detailed
  rationale and evidence in `docs/research-log.md`.

---

## Next 3 Milestones

### 1. Hardware Proof Closeout

- [ ] Close the combined TTY3 hardware proof with verifier-accepted reduced
  logs.
- [ ] Treat any proof failure as a backend-live or renderer-live lifetime,
  readiness, or scanout issue before widening protocol work.
- [ ] Keep the operator-facing proof in scripts and reduced logs; do not move
  native paths, fds, object IDs, or driver strings into Engine or WM state.

### 2. External X Client Probe: xclock

- [ ] Add an `x-authority-xclock-smoke` probe that launches `/usr/bin/xclock`
  against the Sophia X Authority socket.
- [ ] Let the first concrete missing opcode, reply, event, or extension from
  `xclock` drive the next X Authority implementation step.
- [ ] Pass only when the probe reaches at least one Engine/Runtime committed
  authority transaction without leaking XIDs, namespace IDs, titles, classes,
  PIDs, or raw property payloads across the WM boundary.

### 3. Live Session Composition

- [ ] Compose Sophia X Authority's bounded transaction queue, runtime intake,
  renderer-live frame targets, and backend-live rendered scanout into one
  operator smoke.
- [ ] Preserve reduced evidence as the public validation surface for authority
  health, runtime transaction intake, rendered scanout submit, page-flip retire,
  and cleanup.
- [ ] Keep Wayland Authority and wgpu deferred until the X Authority plus live
  scanout path is stable.

---

## Later Backlog

- [ ] Continue splitting backend-live by domain where modules still mix
  unrelated authority, renderer, runtime, or scanout ownership.
- [ ] Retire XLibre prototype smokes only after Sophia X Authority has
  equivalent live transaction, namespace, selection, and routed-input coverage.

---

## Done Recently

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
