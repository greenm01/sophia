# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and the next
major milestones. Completed work lives in `docs/roadmap-history.md`; detailed
evidence lives in `docs/research-log-archive.md`.

Roadmap rule: keep this file short. A completed item leaves this file when the
next milestone becomes active.

---

## Active Milestone: Pointer And Multi-Output Presentation

Current truth:

- The visible interactive X terminal milestone is complete. On AMD TTY
  hardware, operator-entered `sophia` plus Return produced exactly 14 matched
  physical events, changed xterm pixels, and presented/retired the changed frame
  with clean scanout ownership.
- QEMU proves external keyboard and pointer input, two independently presented
  outputs, distinct content, and page-flip-paced fixed refresh without overlap.
- DRM discovery recognizes connector `vrr_capable` and CRTC `VRR_ENABLED`; the
  Engine policy enables only one opaque, unoccluded fullscreen surface and
  falls back to fixed refresh when overlays require composition.
- The current AMD eDP connector exposes the full property contract but reports
  `vrr_capable=0`. It cannot provide activation evidence. Completing this
  milestone requires a different connector/display reporting capability `1`.

Exit criteria:

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
  default; prove both VRR activation and fixed-refresh fallback on hardware
  reporting `vrr_capable=1`. The current panel is not capable, and QEMU is not
  treated as VRR evidence unless its virtual display exposes the real contract.

---

## Next Milestone: Live Generic Legacy-WM Bridge

- [x] Add an optional generic WM socket to `sophia-live-session`. Send only
  opaque live-surface layout snapshots, validate the reply in Engine, and apply
  the committed proposal to composition, hit-testing, and scanout.
- [ ] Prove the existing xterm remains visible and operator input changes its
  presented pixels while xmonad supplies layout through the generic bridge.
  Xmonad remains a proof fixture and must not appear in Engine or live-session
  policy branches. The headless real-xmonad/xterm path passes with a committed
  move, focus, and injected-input pixel change; the dedicated-TTY physical gate
  remains.
- [ ] Remove the first-session fixed client-size constraint after X Authority's
  core-drawing resize path can accept arbitrary xmonad sizes without an xterm
  repaint loop. Keep the bounded configure/focus command and acknowledgement
  seam keyed only by `SurfaceId`.
- [ ] Add a second legacy X11 WM compatibility smoke through the same
  `--wm=PATH --wm-arg=ARG` launcher with no Engine changes.

---

## Following Milestone: Wayland Authority Skeleton

- [ ] Start the deterministic Wayland Authority reducer/socket boundary only
  after the live generic legacy-WM session proof passes.
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
