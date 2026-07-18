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
presentation baseline are established. The active track finishes GTK3 software
promotion, then consolidates the proven paths behind one production-grade
Engine session loop before adding more X11 compatibility surface.

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
- [ ] Run the aggregate three-class verifier and promote GTK3 software from
  `engine` to `session` in `docs/x11-compatibility-matrix.md`.

Exit: each promoted application class has reproducible `session` evidence and
no undocumented dependency on XLibre, fixed output facts, injected input, or a
Sophia-private presentation path.

## Milestone 6: Production Session Loop

Current state: one session-level production coordinator owns the Engine and committed
snapshot; per-output backend assemblies are projections and cannot become visual
authority. Authority batches commit exactly once through that coordinator. CPU composition now runs after that commit and consumes its immutable
committed snapshot; the CLI no longer owns a second surface, geometry, buffer, or
stacking table. Per-output tick and native submission consume the prepared record.
The production adapter now separates KMS submission from asynchronous retirement
polling and carries the originating cycle into feedback, so an unretired frame
emits no protocol completion. The live native path now uses a backend-owned page-flip tracker for submission
scheduling, monotonic callback validation, exact frame retirement, and reduced
cycle-correlated feedback records; the CLI no longer owns those state tables.
The backend feedback coordinator now retires presentation resources before it
emits paired reduced Complete/Idle outcomes; missing presentations fail closed
with no protocol event. The CLI only translates those outcomes to X wire events.
A retired GPU Present is now prepared and committed once through the session
production coordinator, then its immutable snapshot is projected to the remaining
outputs; it is no longer prepared and applied independently per output. After the
matching page flip, the coordinator applies that prepared state before backend resource
retirement and reduced feedback, and suppresses feedback when the baseline is stale.
CPU authority batches now enter one production cycle that commits, composes, submits, polls retirement, and routes feedback in coordinator order. GPU Present batches now enter one runtime-owned visual entry point; the outer X loop no longer composes their CPU background, builds output frames, initializes scanout, or invokes submission directly. CPU and GPU submission, native idle work, page-flip retirement and cleanup, and
displayed-buffer teardown now cross the coordinator fanout and a protocol-neutral live
output adapter. `LiveProductionNativeScanout` now owns the real atomic sessions, GBM
exporters, callback routing, page-flip correlation, and scanout counters in backend-live;
the CLI no longer defines native DRM/KMS lifecycle state. `LiveProductionCpuScene` now
owns renderer buffers, committed-state layer resolution, focus stacking, cursor composition,
readiness pixel inspection, and per-output frame creation in renderer-live. Reduced Present outcomes now leave the visual runtime through an injected protocol-neutral sink; `XPresentSessionObserver` alone owns X wire routing and completion, idle, fence, and disconnect proof counters. X authority batches are now translated once into backend-owned protocol-neutral transaction, registration, release, and Present records before visual control; `PersistentBackendRuntime` no longer accepts the X observed-batch type. Backend-live now also owns the CPU production adapter that applies renderer updates, composes from the coordinator snapshot, creates per-output frames, and returns reduced composition evidence; the CLI supplies only the output-runtime callback. Backend-live now also owns the Present queue, acquire-delay and fence gates, timeout and controlled rejection policy, submitted frame state, diagnostic trigger, and scheduling counters. Backend-live now owns the concrete per-output runtime set, including construction, projection, native initialization, lookup, and diagnostics. The production X cursor repaint now composes from the coordinator snapshot and submits through the backend-owned output set; it no longer replaces committed state through the legacy snapshot API. Backend-live also owns the prepared-authority and CPU-submission records used internally by that control path. `LiveProductionVisualRuntime` now owns the neutral visual-control implementation in backend-live; `PersistentBackendRuntime` and its CLI implementation are gone. Snapshot replacement is isolated to the Wayland maintenance adapter. Production X now polls one backend service entry point; backend-live owns retirement, queued Present scheduling, and pending native submission order. The remaining production-loop work is to move GPU Present preparation and retirement sequencing into `runtime_driver` so it is the only visual coordinator.

- [ ] Establish `sophia-engine::runtime_driver` as the only production visual
  coordinator. Its ordered phases are bounded authority intake, Engine
  commit/preparation, composition from committed state, KMS submit/retire, and
  protocol feedback after matching retirement.
- [x] Define protocol-neutral authority, renderer, scanout, and feedback
  adapters. Keep X resources in the frontend, imported images/fences in the
  renderer, native DRM/KMS objects in the backend, and process/recovery policy
  in runtime.
- [x] Remove the duplicate CLI scene projection. Composition must consume one
  immutable Engine committed snapshot; `PersistentCpuScene` must not remain a
  second `SurfaceId` table.
- [x] Move `PersistentBackendRuntime` and `PersistentNativeScanout` sequencing
  behind the production coordinator and live backend adapters. The CLI may
  construct dependencies, supervise clients, observe proofs, and request
  shutdown, but it must not commit, compose, submit, or retire frames.
- [x] Preserve fail-closed behavior for stale generations, acquire timeout,
  rejected or missing page flips, authority backpressure/disconnect, surface
  removal, and shutdown. Every path keeps the last committed frame, emits no
  premature Complete/Idle, and drains native resources exactly once.
- [x] Migrate the retained xterm CPU, mixed Vulkan, GTK classic/confined QEMU,
  and guarded X13 gates without weakening their evidence schemas or latency and
  cleanup bounds; then delete the legacy CLI loop state.

Exit: one production state machine owns the five visual phases;
`crates/sophia-cli/src/commands/live_session.rs` owns no scene, Engine commit
loop, native scanout lifecycle, or protocol-feedback timing; all retained
session gates pass with zero cleanup debt.

Only after this exit may Render, broader XFixes, selections/INCR, Xdnd, GLX,
or new toolkit behavior advance from a captured compatibility gap with the
smallest wire/authority regression.

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
