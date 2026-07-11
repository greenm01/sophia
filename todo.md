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
  opens explicit libinput device paths, routes keys through Engine-owned seat
  focus, and translates evdev modifiers into core X events. The QEMU proof now
  types through QMP, virtio-keyboard, and libinput; all 14 press/release events
  route and change later xterm pixels without internal X event injection. An
  operator typed-text proof on the AMD TTY remains required.
- TTY3 native GBM/KMS submit, page-flip retirement, and cleanup evidence pass.
- The native renderer can upload a composed XRGB8888 frame into its GL/GBM
  front buffer. The TTY3 content proof exports the exact composed xterm
  checksum, submits it to KMS, observes page-flip retirement, and drains cleanup.
- `sophia-live-session --proof` remains a one-shot composition/input proof.
- The isolated headless QEMU harness boots a direct-kernel initramfs with
  virtio-gpu, virtual keyboard and mouse devices, QMP/serial control, no guest
  network or storage, and no host DRM/VT access. Its strict 300-tick native
  session and external keyboard/pointer proof passes. QMP word selection sends
  five virtio-mouse motion/button events through libinput, Engine surface-only
  hit-testing, and X Authority; all route and change later xterm pixels without
  exposing X window identity to Engine.
- The isolated guest has two separate virtio GPU devices and two connected KMS
  connectors. Engine bounds discovery to 16 outputs, lays them out as an
  extended horizontal desktop, and tracks independent damage, frame clocks,
  scanout ownership, callbacks, retirement, and cleanup. The strict 300-tick
  proof owns and retires both outputs with distinguishable content and no
  overlapping page-flip-paced submission.
- Default `sophia-live-session` now binds an explicit display, owns one xterm
  and X Authority server, and drains repeated authority batches through one
  live backend runtime and CPU scene until bounded or externally stopped. Its
  gated `--native-scanout` mode owns GBM/KMS export, submit, page-flip intake,
  retirement, and cleanup in that same loop. The strict persistent hardware
  proof and a 30-second TTY3 run pass without dropped batches, callback
  rejection, submit/retire failure, or cleanup debt.

Exit criteria:

- [x] Back core X drawing with bounded XRGB8888 CPU buffers, including the
  fixed-font text path demanded by xterm.
- [x] Compose those pixels into a renderer-owned frame and prove the scanned-out
  frame contains terminal content rather than an allocated blank buffer.
- [x] Keep X Authority, live backend ticks, scanout ownership, and xterm alive
  under one persistent session owner until the outside control plane stops it.
- [x] Route QMP-driven virtio keyboard press/release events from libinput into
  xterm and prove typed text changes composed terminal pixels without using
  Sophia's internal X event injector.
- [ ] Repeat the external keyboard pixel proof with operator input on the AMD
  TTY hardware path.

---

## Next Milestones

### 1. Pointer And Multi-Output Presentation

- [x] Drive virtio-mouse motion and buttons through QMP, reduce them through
  libinput, apply Engine-owned hit-testing/focus, deliver core X pointer events,
  and prove a real xterm changes pixels through word selection.
- [x] Bound Engine output discovery and add independent per-output clocks,
  pending damage, in-flight ownership, and exact retirement validation. Pass a
  QEMU topology gate with two connected virtio KMS outputs.
- [x] Replace the persistent native session's single selected output with a
  bounded output table whose scanout owner, damage, in-flight frame, retirement,
  and frame clock are tracked independently per output.
- [x] Present independent content/damage and observe clean native retirement on
  both connected QEMU outputs, then retain an AMD multi-connector run as the
  physical-driver gate.
- [x] Pace fixed-refresh presentation from each output's vblank/page-flip
  timeline and prove no unsynchronized or overlapping submission is accepted;
  this is the per-output vsync gate.
- [ ] Complete the hardware gate for the implemented DRM VRR capability/property
  discovery and Engine fullscreen-eligibility policy. VRR remains disabled by
  default; prove both VRR activation and fixed-refresh fallback on capable AMD
  hardware. QEMU is not treated as VRR evidence unless its virtual display
  exposes the real property contract.

### 2. xmonad X11 WM Bridge

- [x] Add the isolated `sophia-x11-wm-bridge` binary with an embedded minimal
  X server and synthetic windows only.
- [x] Add bounded synthetic XID/lifecycle state and translate configure/focus
  requests into metadata-blind Sophia WM commands.
- [x] Run xmonad as blind layout policy: no physical input, real metadata,
  namespaces, client buffers, rendering, or scanout.
- [x] Translate xmonad configure/focus requests into bounded Sophia WM response
  packets and pass a real two-window tiling smoke.

### 3. Live Session Stability Evidence

- [x] Record submit-to-page-flip latency, maximum in-flight frame age, authority
  queue capacity, rejected/dropped batches, page-flip callback pressure, and
  cleanup debt over repeated ticks.
- [x] Pass a 30-second TTY hardware session with no dropped authority batches,
  rejected callbacks, failed scanout transitions, or unresolved cleanup debt.
- [x] Add an isolated QEMU `virtio-gpu` session harness and pass 300
  deterministic ticks without depending on host DRM or VT ownership.
- [x] Replace the QEMU proof's internal X key injection with QMP-driven
  virtio-keyboard input and require nonzero routed libinput keys plus changed
  terminal pixels.

### 4. Wayland Authority Skeleton

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
