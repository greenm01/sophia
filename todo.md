# Sophia Active Roadmap

Sophia is a research prototype. This file contains only active work and the
next major milestones. Completed work belongs in `docs/roadmap-history.md`;
detailed evidence belongs in the research logs.

Roadmap rule: keep this file short, keep exit criteria measurable, and move a
completed milestone out when the next milestone becomes active.

---

## Current Direction

Sophia's primary development track is its native **Sophia X Server Frontend**,
which presents the established X11 API directly to applications. The
protocol-neutral Engine remains the sole owner of physical input, scene state,
rendering, and scanout.

Namespace admission, portals, the bounded X11 `CLIPBOARD` plus `PRIMARY`
reference flow, application compatibility, the production Engine loop, the
interactive blind WM API, and the unattended xmonad/Firefox daily-driver gate
are established. The next major milestone has not yet been selected.

X11 is the sole supported application protocol. Engine remains
protocol-neutral, but no alternate frontend or compatibility provider is on the
active roadmap. The retired Wayland and XLibre prototypes remain under
`research/` as architectural evidence.

## Next Milestone Selection

- [ ] Select the next measurable milestone from observed native-X daily-driver
  gaps; do not broaden X11 support without retained client evidence.

## Active Safety Follow-up: Physical TTY3 Recovery

The first native xmonad TTY3 operator attempt reached a blank scanout and had
no usable VT switch or local escape, forcing a reboot. The launcher had omitted
the independent recovery guard and TTY restoration already required by the
Milestone 5 hardware runner.

- [x] Require Ctrl-Alt-Backspace guard arming before xmonad graphics takeover.
- [x] Restore KD mode, termios, and keyd on normal, failed, and emergency exits.
- [x] Diagnose the blank startup as an omitted startup application plus an
  input gate that suppressed global shortcuts when no application was focused;
  the subsequent Kitty launch failure was GLVND selecting the indirect vendor
  because Sophia exposed no Mesa mapping or framebuffer configurations.
- [x] Start Kitty automatically, preserve global WM shortcuts on an empty
  desktop, and implement the bounded direct-Mesa GLX visual/FBConfig/context/
  drawable path needed before DRI3/Present rendering.
- [ ] Retain one passing physical launcher, input-guard, recovery, and Sophia
  session capture with automatic Kitty startup.
- [x] Verify the second Ctrl-Alt-Backspace chord returns to a usable tty3 before
  retrying interactive application input.
- [x] Remove connection-local Present transaction collisions, bound per-client
  Present pressure without dropping feedback, and contain client failures so
  one application cannot terminate the X frontend.
- [x] Start the private xmonad X server with the first Engine-provided layout
  bounds and notify it when those bounds change; production no longer begins
  from the 1280x720 smoke-fixture geometry.
- [x] Stop using CPU-only cursor repaints for DMA-BUF applications; install and
  move a compositor-owned classic hardware cursor independently of Kitty's
  primary-plane content.
- [x] Add a guarded two-output Kitty-only TTY3 profile with no external WM or
  layout-resize transaction dependency.
- [x] Replace launcher path heuristics with production libinput/udev `seat0`
  discovery, capability classification, tap policy, and hotplug accounting;
  retain explicit paths only as a diagnostic override.
- [x] Bound Kitty-only startup at eight seconds, suppress unusable desktop
  service activation, and restore TTY3 automatically when no focused
  application frame is presented.
- [x] Initialize and move the classic hardware cursor before the first
  application surface without routing unfocused keyboard or button events.
- [x] Reconcile the first GPU Present after asynchronous KMS retirement even
  when Kitty sends no later authority batch; apply X11 focus before enabling
  keyboard routing and do not require a second frame for startup readiness.
- [x] Replace best-effort legacy cursor cleanup with checked atomic cursor-plane
  detach/attach/move commits and use the same classic pointer raster in
  hardware and software paths.
- [x] Make the minimal Kitty gate use `--config NONE` rather than the operator's
  normal Kitty configuration.
- [x] Make pending native frames latest-wins per output so a stale CPU frame
  cannot follow and replace a mixed DRI3/Present frame.
- [x] Enforce one cursor owner in native sessions: hardware-cursor scanout never
  includes software-cursor pixels, and CPU-only frames cannot replace retained
  DMA-BUF application content.
- [x] Bind mixed presentation and readiness to the explicit primary output and
  exact displayed Present transaction rather than generic retirement.
- [x] Remove multi-output Present head-of-line blocking: retire all outputs,
  gate Present only on its primary output, and submit pending frames only on
  individually idle outputs.
- [x] Remove the duplicate global in-flight veto inside the Present driver and
  put tty3 into KD graphics plus raw/no-echo mode for graphical ownership.
- [x] Reserve the primary output for a queued Present transaction so a
  transiently deferred mixed submit cannot be starved by recurring CPU frames.
- [x] Add one-shot aggregate pixel evidence at the native mixed-composition
  boundary and correct ARGB layers to premultiplied source-over blending.
- [x] Preserve the server-global transaction identity from standard X Present
  decode through Engine retirement and frontend Complete/Idle routing; do not
  replace it with a connection-local X sequence number.
- [x] Retain Present selections independently by client and event ID so
  clearing a toolkit bootstrap window cannot delete the active window's
  Complete/Idle subscription.
- [x] Reactivate the launcher’s originating VT after restoring greetd instead
  of leaving the display manager’s VT active.
- [x] Treat `EBUSY` from a nonblocking atomic cursor-plane commit as deferred
  work instead of terminating an otherwise healthy interactive session.
- [ ] Retain a physical Kitty-only capture proving visible cursor motion within
  two 60 Hz refresh intervals, click-drag selection, typed input, clean exit,
  and emergency recovery.
- [ ] Re-enable xmonad only after the Kitty input gate passes and the xmonad
  resize handshake has its own retained regression.

## Active Follow-up: Interactive QEMU Operator Session

The unattended M8 QEMU gate remains complete and unaffected. A separate manual
Void guest launcher now exists at `tools/start_sophia_xmonad_vm.sh`, but its
host input path is not yet usable enough to call interactive support complete.

Current state (2026-07-20):

- The launcher resolves its repository path through the `~/start-sophia-vm`
  symlink and boots the current kernel-matched initramfs.
- VNC, native-Wayland GTK, Xwayland GTK, virtio mouse, and USB HID mouse paths
  were tried. Guest logs prove initial key and button events are observed and
  routed, but sustained keyboard/mouse control was not visible to the operator.
- The current launcher uses Xwayland GTK, one `virtio-vga` output, a virtio
  keyboard, and a USB HID mouse. The one-output change is not yet verified.
- The reused `xmonad-m8-soak` scenario is acceptance-oriented: it injects WM
  bridge restarts, carries Firefox proof requirements, and eventually exits.
  It must not become the permanent interactive-session contract.

- [ ] Reproduce the current one-output launcher once and retain the host QEMU
  invocation plus `/tmp/sophia-interactive.log`.
- [ ] Add bounded input diagnostics that distinguish device discovery, key
  transitions, button transitions, and relative pointer-motion counts without
  logging sensitive input content.
- [ ] Identify whether the remaining failure is host grab delivery, guest
  evdev/libinput device selection, Engine pointer motion, or display/focus
  targeting; fix only the evidenced boundary.
- [ ] Add a dedicated `xmonad-interactive` guest scenario with manual shutdown,
  no acceptance proof watchdog, and no scheduled bridge-restart injection.
- [ ] Gate the operator session with visible pointer movement, terminal launch,
  typed text, focus change, application close, and clean manual shutdown.
- [ ] Document the supported local display/input backend and update the home
  launcher instructions after the gate passes.

## Deferred

- XLibre provider integration remains deferred until measured native-X gaps
  demonstrate that a provider is worth its authority and maintenance cost.
- Future compatibility frontends require a specification amendment backed by
  named product evidence; they are not deferred roadmap work.
- VRR activation evidence waits for hardware reporting `vrr_capable=1`.
- Large X11 `INCR`, full Xdnd, prompt UI, FD handoff, capture streaming, URI
  launching, and notification actions follow the bounded portal reference flow.
- Full client-selected classic X11 cursor images remain an X-frontend follow-up;
  Engine cursor presentation stays protocol-neutral and contains no
  application-specific policy.
