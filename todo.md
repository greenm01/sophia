# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Authority Coverage From Real Probe Failures

Current architecture read:

- Sophia Engine owns visual truth and `SurfaceTransaction` commit readiness.
- Sophia X Authority owns X protocol resources and emits bounded transaction
  batches; it does not own layout, chrome, input devices, rendering, or scanout.
- backend-live and renderer-live own native IO, GBM/EGL, libdrm, page-flip, and
  scanout lifetimes behind reduced evidence records.
- Sophia WM remains blind policy. XLibre remains prototype/reference material,
  not the destination architecture.

Current milestone target:

- [ ] Expand Sophia X Authority only where real probes demand
  it.
- [ ] Prefer bounded drawing, upload, present, selection, event, and namespace
  behavior over broad X11 completeness.
- [ ] Keep XLibre bridge smokes as prototype references until Sophia X Authority
  has equivalent live coverage.

---

## Next 3 Milestones

### 1. Authority Coverage From Real Probe Failures

- [ ] Expand Sophia X Authority only where real probes demand
  it.
- [ ] Prefer bounded drawing, upload, present, selection, event, and namespace
  behavior over broad X11 completeness.
- [ ] Keep XLibre bridge smokes as prototype references until Sophia X Authority
  has equivalent live coverage.

### 2. Wayland Authority Skeleton

- [ ] Define the first minimal Wayland Authority socket/setup boundary without
  committing to wgpu or broad compositor-framework adoption.
- [ ] Preserve the same authority contract: protocol resources in the authority,
  visual truth and commit readiness in Sophia Engine.
- [ ] Start only after live X Authority transaction intake and rendered scanout
  composition have one operator-grade smoke.

### 3. Live Session Throughput Instrumentation

- [ ] Track submit-to-page-flip latency, in-flight frame age, cleanup debt, and
  backpressure over repeated non-destructive composition ticks.
- [ ] Keep optimization decisions behind measured reduced evidence rather than
  speculative buffering or batching changes.

---

## Later Backlog

- [ ] Continue splitting backend-live by domain where modules still mix
  unrelated authority, renderer, runtime, or scanout ownership.
- [ ] Retire XLibre prototype smokes only after Sophia X Authority has
  equivalent live transaction, namespace, selection, and routed-input coverage.

---

## Done Recently

- [x] `x-authority-xrandr-query-smoke` launches `/usr/bin/xrandr --query`,
  keeps `first_error=none`, and adds only the demanded minimal `RANDR`
  extension advertisement, fixed root screen-size range, and empty screen
  resource replies.
- [x] `x-authority-xmessage-smoke` launches `/usr/bin/xmessage Sophia`, keeps
  `first_error=none`, and adds only the demanded bounded `CreateGlyphCursor`,
  `FreeCursor`, `SetClipRectangles`, and `PolyText8` paths with reduced
  Engine/Runtime transaction evidence.
- [x] `x-authority-xlogo-smoke` launches `/usr/bin/xlogo`, keeps
  `first_error=none`, and reaches committed drawing transactions through the
  existing polygon/rectangle paths without new protocol expansion.
- [x] `x-authority-xsetroot-name-smoke` launches `/usr/bin/xsetroot -name`,
  keeps `first_error=none`, and proves root property mutation through existing
  bounded property paths.
- [x] `x-authority-xprop-root-smoke` launches `/usr/bin/xprop -root`, exits
  successfully with `first_error=none`, and adds only the demanded bounded
  `ListProperties` root/window property atom reply path.
- [x] `x-authority-xwininfo-root-smoke` launches `/usr/bin/xwininfo -root`,
  exits successfully with `first_error=none`, and adds only the demanded
  `GetWindowAttributes`, `GetGeometry`, `QueryTree`, and
  `TranslateCoordinates` root/window introspection replies.
- [x] `x-authority-xeyes-smoke` launches `/usr/bin/xeyes`, keeps
  compatibility expansion probe-driven, and adds only the demanded
  `QueryColors`, `ClearArea`, and `PolyFillArc` paths with reduced
  Engine/Runtime transaction evidence.
- [x] `live-session-composition-smoke` now reuses the X Authority
  Present-pixmap socket path, drains the bounded authority queue into runtime
  intake, commits one authority transaction, submits a rendered primary plane
  scanout, retires it after a deterministic accepted page flip, and reports
  cleanup drained with reduced evidence.
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
