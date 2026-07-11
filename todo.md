# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and the next
major milestones. Completed work lives in `docs/roadmap-history.md`; detailed
evidence lives in `docs/research-log-archive.md`.

Roadmap rule: keep this file short. A completed item leaves this file when the
next milestone becomes active.

---

## Active Milestone: Visible Interactive X Terminal

Current truth:

- Sophia Engine owns visual truth, frame commits, input routing, and scanout.
- Sophia X Authority owns X protocol state and emits bounded transaction
  batches. The persistent sequential listener and one-shot xterm drawing probe
  pass.
- Core X drawing now updates bounded XRGB8888 software buffers. The real xterm
  proof produces inspectable text pixels, and the CPU compositor produces a
  deterministic terminal frame.
- A bounded core-keyboard channel, US keymap, and real xterm smoke prove that
  injected `sophia` plus Return changes later terminal pixels. Persistent mode
  can now open explicit libinput device paths, route keys through Engine-owned
  seat focus, and translate evdev modifiers into core X events. An operator
  typed-text pixel proof is still required.
- TTY3 native GBM/KMS submit, page-flip retirement, and cleanup evidence pass.
- The native renderer can upload a composed XRGB8888 frame into its GL/GBM
  front buffer, but terminal-content scanout has not yet been hardware-proven.
- `sophia-live-session --proof` remains a one-shot composition/input proof.
- Default `sophia-live-session` now binds an explicit display, owns one xterm
  and X Authority server, and drains repeated authority batches through one
  live backend runtime and CPU scene until bounded or externally stopped.

Exit criteria:

- [x] Back core X drawing with bounded XRGB8888 CPU buffers, including the
  fixed-font text path demanded by xterm.
- [ ] Compose those pixels into a renderer-owned frame and prove the scanned-out
  frame contains terminal content rather than an allocated blank buffer.
- [ ] Keep X Authority, live backend ticks, scanout ownership, and xterm alive
  under one persistent session owner until the outside control plane stops it.
  Authority, backend ticks, and xterm are persistent; native scanout ownership
  is the remaining part of this item.
- [ ] Route focused keyboard press/release events from physical libinput into
  xterm and prove typed text changes composed terminal pixels. The route and
  real device open pass; the injected X11-event proof passes; TTY typing
  evidence remains.

---

## Next Milestones

### 1. xmonad X11 WM Bridge

- [ ] Add the isolated `sophia-x11-wm-bridge` binary with an embedded minimal
  X server and synthetic windows only.
- [x] Add bounded synthetic XID/lifecycle state and translate configure/focus
  requests into metadata-blind Sophia WM commands.
- [ ] Run xmonad as blind layout policy: no physical input, real metadata,
  namespaces, client buffers, rendering, or scanout.
- [ ] Translate xmonad configure/focus requests into bounded Sophia WM response
  packets and pass a real two-window tiling smoke.

### 2. Live Session Stability Evidence

- [ ] Record submit-to-page-flip latency, maximum in-flight frame age, authority
  queue pressure, rejected/dropped batches, and cleanup debt over repeated ticks.
- [ ] Pass 300 deterministic ticks and a 30-second TTY hardware session with no
  dropped authority batches or unresolved cleanup debt.

### 3. Wayland Authority Skeleton

- [ ] Start the deterministic Wayland Authority reducer/socket boundary only
  after visible xterm pixels, keyboard input, xmonad layout translation, and
  repeated-tick X session evidence pass.
- [ ] Preserve the same authority contract: Wayland protocol resources remain
  in the authority; visual truth and commit readiness remain in Sophia Engine.

---

## Deferred

- GTK/XInput2/zenity rendered-dialog work resumes after the terminal and xmonad
  control paths are usable.
- Concurrent X clients and per-client X resource-ID allocation remain later X
  Authority milestones; the first operator session stays one client/namespace.
- XLibre remains a prototype/reference until equivalent live transaction,
  namespace, selection, and routed-input coverage exists in Sophia-owned paths.
