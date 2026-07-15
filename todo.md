# Sophia Active Roadmap

Sophia is a research prototype. This file contains only active work and the
next major milestones. Completed work belongs in
`docs/roadmap-history.md`; detailed evidence belongs in the research logs.

Roadmap rule: keep this file short, keep exit criteria measurable, and move a
completed milestone out when the next milestone becomes active.

---

## Current Direction

Sophia's primary development track is its native **Sophia X Server Frontend**,
which presents the established X11 API directly to applications. Sophia is not
creating a separate application-facing display protocol. The protocol-neutral
Engine remains the sole owner of physical input, scene state, rendering, and
scanout.

The namespace, admission, portal lifecycle, bounded X11 `CLIPBOARD` plus
`PRIMARY` reference flow, and native X11 session-correctness milestone are
established. The active track now advances standard X11 buffer and presentation
semantics without weakening those boundaries.

The Smithay-backed Wayland Authority remains supported under a maintenance
lane. XLibre is a retired prototype and documented possible future
compatibility provider; no XLibre integration work is active.

## Milestone 4: X11 Buffer And Presentation Semantics

- [ ] Make SHM/software readiness, immutability, damage, release, and
  presentation feedback explicit rather than inferred from drawing traffic.
- [ ] Implement standard DRI3/Present DMA-BUF handoff with bounded format and
  plane validation, fences, delayed release, and presentation feedback.
- [ ] Keep renderer import, frame scheduling, DRM/KMS, and page-flip retirement
  exclusively in Engine/backend ownership.
- [ ] Prove slow, stale, rejected, and disconnected buffers preserve the last
  committed good geometry-plus-pixels state and release every resource once.

Exit: one software-backed and one GPU-backed real X11 client pass normal
startup, resize, presentation, delayed release, failure recovery, and teardown
through Engine-owned KMS without a private permanent presentation extension.

## Milestone 5: Application Compatibility

- [ ] Advance Render, XFixes, selections/INCR, Xdnd, GLX, and toolkit-specific
  behavior only from captured gaps in `docs/x11-compatibility-matrix.md`.
- [ ] Require a focused wire/authority regression, a reproducible real-client
  probe with `first_error=none`, and the smallest compatible implementation for
  every admitted request or extension.
- [ ] Define application-class promotion using protocol coverage, namespace
  behavior, input/grab correctness, buffer lifetime, latency, recovery, and
  classic shared-X behavior where selected.

Exit: each promoted application class has reproducible `session` evidence and
no undocumented dependency on XLibre, fixed output facts, injected input, or a
Sophia-private presentation path.

## Wayland Maintenance Lane

- [ ] Keep native Wayland SHM/Kitty startup, input, presentation, clean TTY
  recovery, and session teardown as regression gates.
- [ ] Keep the controlled linear DMA-BUF first-frame and retained 300-frame
  lifetime proofs as renderer regressions.
- [ ] Fix security, correctness, recovery, or dependency-boundary regressions
  without adding new Wayland protocols or prioritizing arbitrary DMA-BUF GPU
  composition ahead of the active X11 milestones.

## Deferred

- XLibre provider API and integration remain deferred until native X11 gaps are
  measured, namespace/portal contracts are stable, and the compatibility matrix
  demonstrates that a provider is worth its authority and maintenance cost.
- Wayland protocol expansion and arbitrary client DMA-BUF composition resume
  only after the X11, namespace, and portal foundations are mature or a critical
  regression requires the work.
- VRR activation evidence waits for hardware reporting `vrr_capable=1`.
- Large X11 `INCR` clipboard transfers, full Xdnd execution, prompt UI, file
  descriptor handoff, capture streaming, URI launching, and notification action
  execution follow the bounded clipboard/broker reference flow.
