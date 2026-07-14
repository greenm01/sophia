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

Namespaces and portals precede broader X11 compatibility because client
identity, isolation, admission, and explicit transfer grants must be correct
before more protocol surface depends on them. The selected first portal is
X11 `CLIPBOARD` plus `PRIMARY`.

The Smithay-backed Wayland Authority remains supported under a maintenance
lane. XLibre is a retired prototype and documented possible future
compatibility provider; no XLibre integration work is active.

## Milestone 2: Portal Broker And X11 Clipboard

- [ ] Reconcile portal kinds so clipboard, drag-and-drop, file handoff, screen
  capture, screen recording, URI open, and notification are explicit protocol
  values.
- [ ] Split request decisions from grant lifecycle, including deadlines,
  completion, expiry, disconnect revocation, broker-restart revocation, and
  generation checks.
- [ ] Add bounded broker IPC, an I/O-free policy reducer, a deterministic
  headless policy provider, and runtime executors that keep payloads and file
  descriptors outside Engine and policy state.
- [ ] Connect native X selection ownership/request context to the broker while
  keeping XIDs and atoms authority-private.
- [ ] Implement same-namespace native selection behavior and cross-namespace
  `CLIPBOARD`/`PRIMARY` mediation for `TARGETS`, `UTF8_STRING`, and bounded UTF-8
  `text/plain` data.
- [ ] Map denial, stale owner generation, timeout, disconnect, unsupported
  target, and executor failure to normal `SelectionNotify(property = None)`.

Exit: deterministic and socket-level tests prove allowed, denied, stale,
expired, disconnected, same-namespace, and cross-namespace transfers without
freezing either client or granting general resource visibility.

## Milestone 3: X11 Session Correctness

- [ ] Implement a real XKB keymap/state path and complete X11 focus, active and
  passive grabs, keyboard, pointer, and required XI2 delivery using
  Engine-selected targets and authority-local coordinates.
- [ ] Replace fixed root/output facts with Engine-sourced output and RandR
  snapshots, including normal resize/configure behavior.
- [ ] Preserve client-targeted input/control acknowledgements, bounded
  backpressure, deterministic focus/stacking, and complete disconnect cleanup.
- [ ] Run normal startup, physical input, resize, presentation, and teardown
  through the native frontend under classic shared-X and the applicable
  confined profile.

Exit: promote the guarded two-xterm path from `hardware` to `session` evidence
only when all X11 events flush, both clients remain independently interactive,
output facts come from Engine, resize completes, KMS cleanup is clean, startup
is at most 2,000 ms, maximum composition is at most 25 ms, and
input-to-presentation is at most 100 ms.

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
