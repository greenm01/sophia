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

## Deferred

- XLibre provider integration remains deferred until measured native-X gaps
  demonstrate that a provider is worth its authority and maintenance cost.
- Future compatibility frontends require a specification amendment backed by
  named product evidence; they are not deferred roadmap work.
- VRR activation evidence waits for hardware reporting `vrr_capable=1`.
- Large X11 `INCR`, full Xdnd, prompt UI, FD handoff, capture streaming, URI
  launching, and notification actions follow the bounded portal reference flow.
