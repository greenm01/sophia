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

- [x] Make SHM/software buffer ownership, damage, and release explicit rather
  than inferred from drawing traffic; retain the established real-client SHM
  resize and teardown evidence as the software baseline.
- [x] Decode standard DRI3 `PixmapFromBuffer`/`FenceFromFD` and Present
  `Pixmap`/`SelectInput`/`QueryCapabilities`, validate bounded descriptors and
  options, transfer duplicated DMA-BUF and xshmfence FDs into Engine-facing
  batches, and encode standard Present XGE complete/idle events.
- [x] Add a safe dynamically loaded xshmfence adapter plus a renderer-private
  DMA-BUF registry that owns plane FDs and polls acquire fences without
  blocking; prove pending fences hold submission in external tests.
- [x] Connect the observed DMA-BUF/fence registrations to the persistent live
  renderer path. Hold a Present transaction until its acquire fence signals,
  preserve the last committed frame on rejection, and submit mixed CPU/GPU
  scene layers without exposing native objects to the Engine or X authority.
- [x] Route real backend page-flip completion through a cloneable frontend
  protocol router, emit Present Complete before Idle, trigger the idle fence,
  and retire each imported buffer exactly once.
- [x] Implement FD-bearing server replies and standard DRI3 `Open` (then the
  smallest modifier/multi-plane requests proven necessary by the client
  trace). Retain the bounded X13 `vkcube` trace as the transport proof; do not
  invent a permanent private presentation path.
- [x] Keep renderer import, frame scheduling, DRM/KMS, and page-flip retirement
  exclusively in Engine/backend ownership.
- [x] Prove slow, stale, rejected, and disconnected buffers preserve the last
  committed good geometry-plus-pixels state and release every resource once.
- [ ] Automate and retain one software-backed real-client run and one
  GPU-backed `vkcube` run on the X13 native KMS target, including startup,
  resize, delayed-fence, failure-recovery, and clean-teardown evidence.

Implementation checkpoint: the persistent session transfers typed source and
fence handles into `LivePresentationResourceSession`, quarantines each Present
by exact `TransactionId`, and submits a mixed CPU/DMA-BUF frame through the
persistent native EGL/GBM exporter. Engine prepared commits become authoritative
only after the matching KMS page flip; rejection preserves prior state, while
unrelated surfaces may continue committing. Page-flip retirement routes
Complete before Idle, triggers the idle fence, and releases deferred resources
exactly once. The schema-14 session summary fails closed on cleanup debt, and
the full offline all-feature workspace suite passes.

`tools/live_session_milestone4_hardware_proof.sh` automates the remaining X13
exit: an established software xterm/resize pass followed by a mixed `vkcube`
plus CPU-xterm pass with a controlled first acquire delay and one forced
rejection/recovery. The 2026-07-15 hardware checkpoint proves the software
half and an isolated DMA-BUF-only mixed export, including 86 Flip completions,
one Skip, 87 matching Idle events, 87 idle-fence triggers, and zero live
resources. The real CPU-plus-Vulkan mixed draw still aborts in Radeon
`glFinish` with `amdgpu: The CS has been rejected ... (-2)` before KMS submit.

Next continuation: capture the privileged kernel validator record immediately
after the failure with `sudo dmesg -T`, reduce the failing two-layer draw to a
native-EGL hardware regression, and correct the rejected resource/state
transition without weakening the required mixed-pixel gate. DCC disablement,
CPU/import submission splits, frame-local CPU textures, frame-local VBOs, and
reversed layer order did not clear the rejection and should not be repeated as
blind hardware experiments. Retain a normal paired run only after the strict
schema-14 verifier passes, then promote the final unchecked item and archive
Milestone 4.

Exit: one software-backed and one GPU-backed real X11 client pass normal
startup, resize, presentation, delayed release, failure recovery, and teardown
through Engine-owned KMS without a private permanent presentation extension.

## Milestone 5: Application Compatibility

Milestone 5 remains blocked until the Milestone 4 software and mixed GPU
session exit passes through Engine-owned KMS.

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
