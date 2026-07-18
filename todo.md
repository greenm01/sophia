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
`PRIMARY` reference flow, native X11 session correctness, and mixed Vulkan
presentation baseline are established. The active track promotes GTK3 software
applications through captured compatibility gaps without weakening those
boundaries.

The Smithay-backed Wayland Authority remains supported under a maintenance
lane. XLibre is a retired prototype and documented possible future
compatibility provider; no XLibre integration work is active.

## Milestone 5: Application Compatibility

- [x] Add a bounded direct X client launcher, reduced protocol-error
  observations, and strict application-session evidence without bypassing the
  Engine transaction path.
- [x] Add fixture-backed paired GTK3 and aggregate three-class evidence
  verifiers covering retained xterm, Vulkan, and GTK3 software sessions.
- [x] Guard GTK hardware takeover with independently armed emergency input,
  bounded process-group termination, verified KD/termios/keyd restoration, and
  durable recovery evidence.
- [x] Resolve the native render-provider Zenity XFixes failure. Raw-minor
  tracing identified request 11 (`SetRegion`), not request 0 (`QueryVersion`);
  authority-owned region lifecycle, Present-region validation, and the raw-zero
  optional-region fix now carry the same X13 sequence through Present with
  `first_error=none`. The non-KMS render smoke reaches the Engine transaction
  but intentionally cannot satisfy a scanout pixel proof.
- [x] Close the deterministic GTK input and protocol gap in isolated QEMU.
  Classic shared-X and confined Zenity entry dialogs now accept exact physical
  `sophia` text and a physical OK click, exit normally with matching stdout,
  present through native virtio-gpu outputs, and tear down with zero X protocol
  errors. The retained regressions cover ChangeGC, core and XI cursor requests,
  opaque non-input SendEvent delivery, XIQueryPointer, and XIUngrabDevice.
- [ ] Capture fresh classic shared-X and confined Zenity entry-dialog sessions
  on target hardware with exact physical text, a physical OK click,
  `first_error=none`, CPU/SHM redraw after resize, native presentation, normal
  exit, and clean teardown. QEMU closes the semantic and unattended-regression
  gap but does not replace the guarded VT/DRM hardware evidence. The prior X13
  attempts exposed a proof-state bug: readiness preceded a presented cursor and
  Return could close Zenity before any pointer selection. The session now centers
  and presents the cursor before readiness, suppresses Return until a physical
  button routes, and fails cleanly if the proof surface disappears; both GTK QEMU
  profiles pass the click-then-submit sequence. Fresh X13 evidence is still required.
- [ ] Advance Render, XFixes, selections/INCR, Xdnd, GLX, or toolkit behavior
  only when that captured Zenity session exposes a focused matrix gap; require
  the smallest compatible change and a wire/authority regression.
- [ ] Run the aggregate three-class verifier and promote GTK3 software from
  `engine` to `session` in `docs/x11-compatibility-matrix.md`.

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
