# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Live Session Terminal Bootstrap

Current architecture read:

- Sophia Engine owns visual truth and `SurfaceTransaction` commit readiness.
- Sophia X Authority owns X protocol resources and emits bounded transaction
  batches; it does not own layout, chrome, input devices, rendering, or scanout.
- backend-live and renderer-live own native IO, GBM/EGL, libdrm, page-flip, and
  scanout lifetimes behind reduced evidence records.
- Sophia WM remains blind policy. XLibre remains prototype/reference material,
  not the destination architecture.

Current milestone target:

- [ ] Keep Codex on an outside TTY/SSH control plane while Sophia owns a
  separate live-session TTY.
- [x] Turn xterm from setup/lifecycle coverage into a rendered terminal
  transaction proof.
- [x] Bootstrap the first `sophia-live-session` launcher slice from the passing
  terminal render proof and report reduced health.
- [ ] Extend `sophia-live-session` from single-client proof mode into a
  persistent X Authority/live backend loop.

Next logical steps:

- [ ] Split the X Authority socket server into reusable single-client and
  persistent-loop entry points.
- [ ] Teach `sophia-live-session` to keep the X Authority loop and terminal
  process alive until the outside control plane stops it.
- [ ] Route keyboard input to the focused xterm surface, then test launching
  Codex inside that terminal.
- [ ] Return to GTK/XInput2/zenity rendered dialog proof after the terminal
  control loop is usable.

---

## Next 3 Milestones

### 1. Live Session Terminal Bootstrap

- [ ] Keep the operator control plane outside Sophia until xterm rendering and
  keyboard routing are proven.
- [x] Convert xterm into the first rendered terminal proof.
- [x] Add the first `sophia-live-session` bootstrap command around the xterm
  render proof, runtime intake, and deterministic live scanout composition.
- [ ] Upgrade `sophia-live-session` to a persistent X Authority/live backend
  loop.

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
- [ ] Add the next probe-driven GTK slice for XInput2 startup, then turn zenity
  from startup protocol coverage into a mapped/rendered dialog proof under a
  host session with working DBus/portal state.
- [ ] Retire XLibre prototype smokes only after Sophia X Authority has
  equivalent live transaction, namespace, selection, and routed-input coverage.

---

## Done Recently

- [x] Documented the live-session bootstrap path in
  `docs/live-session-bootstrap.md`: keep Codex on an outside TTY/SSH control
  plane, make xterm the first rendered terminal proof, then add keyboard routing
  before trying to run Codex inside Sophia.
- [x] Added the strict `x-authority-xterm-render-smoke` milestone probe. It
  now reaches xterm text drawing with `first_error=none`, commits four terminal
  `SurfaceTransaction` values, and uses `-cm -dc` to avoid spending the proof
  window on 256-color palette setup.
- [x] Added `sophia-live-session --terminal=xterm` as the first bootstrap
  launcher slice. It binds a generated display for one xterm render proof,
  drains authority transactions through deterministic live composition, reports
  `status=bootstrap_ready_keyboard_pending`, and keeps explicit display binding,
  persistence, and keyboard routing as pending work.
- [x] `x-authority-zenity-smoke` launches `zenity` through PATH/env-resolved
  probe configuration, keeps `first_error=none` under `dbus-run-session`, and
  adds only the demanded bounded `GetSelectionOwner`, `GrabServer`,
  `UngrabServer`, `CreateColormap`, reduced `MIT-SHM`, additional `RANDR`,
  `XKEYBOARD` `UseExtension`, and `BIG-REQUESTS` `Enable` startup paths. Current
  TTY evidence is GTK startup protocol coverage, not a rendered dialog proof;
  the remaining blocker is XInput2 plus working host portal/display state.
- [x] External X Authority probe binaries now resolve through `PATH` with
  `SOPHIA_XAUTHORITY_<LABEL>` overrides instead of hard-coded `/usr/bin` paths.
- [x] `x-authority-xterm-smoke` launches `xterm`, keeps
  `first_error=none`, and adds only the demanded bounded `ConfigureWindow`
  decode/dispatch path. This is a terminal setup/lifecycle regression with no
  rendered transaction proof yet.
- [x] `x-authority-xcalc-smoke` launches `xcalc`, keeps
  `first_error=none`, and adds only the demanded bounded `AllocNamedColor`,
  `UnmapWindow`, one-character padded `PolyText8`, and client-disconnect
  handling needed by this Athena widget probe.
- [x] `x-authority-xrandr-query-smoke` launches `xrandr --query`,
  keeps `first_error=none`, and adds only the demanded minimal `RANDR`
  extension advertisement, fixed root screen-size range, and empty screen
  resource replies.
- [x] `x-authority-xmessage-smoke` launches `xmessage Sophia`, keeps
  `first_error=none`, and adds only the demanded bounded `CreateGlyphCursor`,
  `FreeCursor`, `SetClipRectangles`, and `PolyText8` paths with reduced
  Engine/Runtime transaction evidence.
- [x] `x-authority-xlogo-smoke` launches `xlogo`, keeps
  `first_error=none`, and reaches committed drawing transactions through the
  existing polygon/rectangle paths without new protocol expansion.
- [x] `x-authority-xsetroot-name-smoke` launches `xsetroot -name`,
  keeps `first_error=none`, and proves root property mutation through existing
  bounded property paths.
- [x] `x-authority-xprop-root-smoke` launches `xprop -root`, exits
  successfully with `first_error=none`, and adds only the demanded bounded
  `ListProperties` root/window property atom reply path.
- [x] `x-authority-xwininfo-root-smoke` launches `xwininfo -root`,
  exits successfully with `first_error=none`, and adds only the demanded
  `GetWindowAttributes`, `GetGeometry`, `QueryTree`, and
  `TranslateCoordinates` root/window introspection replies.
- [x] `x-authority-xeyes-smoke` launches `xeyes`, keeps
  compatibility expansion probe-driven, and adds only the demanded
  `QueryColors`, `ClearArea`, and `PolyFillArc` paths with reduced
  Engine/Runtime transaction evidence.
- [x] `live-session-composition-smoke` now reuses the X Authority
  Present-pixmap socket path, drains the bounded authority queue into runtime
  intake, commits one authority transaction, submits a rendered primary plane
  scanout, retires it after a deterministic accepted page flip, and reports
  cleanup drained with reduced evidence.
- [x] `x-authority-xclock-smoke` launches `xclock`, reaches mapped
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
