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

The Smithay-backed Wayland Authority remains supported under a maintenance
lane. XLibre is a retired prototype and documented possible future
compatibility provider; no XLibre integration work is active.

## Next Milestone Selection

- [ ] Select the next measurable milestone from observed daily-driver gaps;
  do not broaden X11 or Wayland support without retained client evidence.

## Wayland Maintenance Lane

- Keep native Wayland SHM/Kitty startup, input, presentation, clean TTY
  recovery, and session teardown as regression gates.
- Keep the controlled linear DMA-BUF first-frame and retained 300-frame
  lifetime proofs as renderer regressions.
- Fix security, correctness, recovery, or dependency-boundary regressions
  without adding new Wayland protocols ahead of the active X11 milestone.

## Deferred

- XLibre provider integration remains deferred until measured native-X gaps
  demonstrate that a provider is worth its authority and maintenance cost.
- Wayland protocol expansion and arbitrary client DMA-BUF composition resume
  only after the X11, namespace, and portal foundations mature or a critical
  regression requires the work.
- VRR activation evidence waits for hardware reporting `vrr_capable=1`.
- Large X11 `INCR`, full Xdnd, prompt UI, FD handoff, capture streaming, URI
  launching, and notification actions follow the bounded portal reference flow.
