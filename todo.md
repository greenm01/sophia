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
`PRIMARY` reference flow, application compatibility, and the production Engine
session loop are established. The active track now makes the blind Sophia WM API
interactive, carries legacy X11 window managers through the compatibility bridge,
and proves xmonad plus Firefox as an unattended daily-driver session.

The Smithay-backed Wayland Authority remains supported under a maintenance
lane. XLibre is a retired prototype and documented possible future
compatibility provider; no XLibre integration work is active.

## Milestone 7: Interactive WM API And Legacy Bridge

Current implementation: API-v2 negotiation, fail-closed binding validation,
per-seat physical shortcut consumption, nine-slot Engine workspace policy, live
`ActionActivated` dispatch, and bounded xmonad-profile workspace/session-token
translation are implemented. Engine now installs workspace and session-action
effects only with the accepted layout transaction; approved named actions,
polite `WM_DELETE_WINDOW`, and bounded synthetic xmonad focus/layout keys are
wired. The diskless QEMU gate now passes with a real bundled xmonad binary,
physical virtio input, bridge recovery, and approved session actions.

- [x] Negotiate WM API version 2 over the existing bounded IPC frame without
  changing unrelated broker protocols. A WM must register capabilities and
  bindings before Engine sends layout or action requests; mismatch, duplicate
  chords, oversized registration, and reserved emergency bindings fail closed.
- [x] Add Engine-owned shortcut routing. WMs register bounded evdev-keycode plus
  modifier chords mapped to opaque action IDs; Engine consumes matched
  press/release sequences and sends action activations without exposing raw input.
- [x] Add nine configured workspace slots with one active workspace per output.
  Workspace activation, visible-workspace swaps, surface movement, focus, and
  layout commit atomically and survive WM restart.
- [x] Add advertised named session actions for approved launch, close, Firefox,
  launcher, and logout behavior. WMs may request tokens but never executable
  paths, argument vectors, or arbitrary commands.
- [x] Update the native demo WM to exercise the version-2 API directly, including
  bindings, focus, workspace switching, surface movement, and session actions.
- [x] Add a generic legacy-WM profile boundary and a bundled metadata-blind Sophia
  xmonad profile. The bridge alone translates action activations into bounded
  synthetic events and private action messages; xmonad sees no real XIDs, raw
  metadata, namespaces, client sockets, pixels, or physical input.
- [x] Pass an unattended QEMU xmonad gate with three real windows, focus and
  layout changes, workspace switch/move, approved terminal launch, polite close,
  two-output workspace ownership, bridge restart, and preserved committed state.

Exit: Sophia-native and profiled legacy WMs use the same versioned, blind WM API;
Engine remains the only input, workspace, transaction, rendering, and scanout
authority; the complete xmonad interaction gate passes without operator input.

## Milestone 8: xmonad Daily Driver

- [ ] Replace the proof-only xmonad wrapper with a normal supervised session
  launcher supporting WM selection, approved startup applications, logout, clean
  recovery, and a generic multi-application process model.
- [ ] Capture native-X11 Firefox request traces and close compatibility gaps with
  focused wire regressions. Retain an offline local-page proof for visible pixels,
  keyboard and pointer input, resize, dialogs, clipboard, normal exit, and cleanup.
- [ ] Pass a two-output application-mix gate containing xterm, GTK, Vulkan,
  Firefox, `CLIPBOARD`/`PRIMARY`, dialogs, multiple workspaces, launch/close, and
  a restarted WM bridge.
- [ ] Pass a 30-minute unattended QEMU soak with repeated focus, layout, launch,
  close, workspace movement, bridge restart, and application restart, with zero
  protocol errors, stuck transactions, input leaks, or native cleanup debt.

Exit: the normal xmonad-backed Sophia session boots, runs the retained application
mix including Firefox, recovers its WM policy process, and completes the bounded
soak. Machine-specific runs remain optional compatibility diagnostics.



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
- [x] Run the unattended local QEMU Milestone 5 acceptance gate for classic
  shared-X and confined Zenity entry-dialog sessions with exact virtio text and
  an OK click, `first_error=none`, CPU/SHM redraw after resize, native
  presentation, normal exit, clean teardown, emergency recovery, and the strict
  two-xterm regression. `tools/qemu_milestone5_acceptance.sh` rebuilds the
  diskless guest and runs the complete gate without operator input. Real-hardware
  runs remain available as optional compatibility diagnostics rather than
  milestone gates.
- [x] Run the aggregate three-class verifier and promote GTK3 software from
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
readiness pixel inspection, and per-output frame creation in renderer-live. Reduced Present outcomes now leave the visual runtime through an injected protocol-neutral sink; `XPresentSessionObserver` alone owns X wire routing and completion, idle, fence, and disconnect proof counters. X authority batches are now translated once into backend-owned protocol-neutral transaction, registration, release, and Present records before visual control; `PersistentBackendRuntime` no longer accepts the X observed-batch type. Backend-live now also owns the CPU production adapter that applies renderer updates, composes from the coordinator snapshot, creates per-output frames, and returns reduced composition evidence; the CLI supplies only the output-runtime callback. Backend-live now also owns the Present queue, acquire-delay and fence gates, timeout and controlled rejection policy, submitted frame state, diagnostic trigger, and scheduling counters. Backend-live now owns the concrete per-output runtime set, including construction, projection, native initialization, lookup, and diagnostics. The production X cursor repaint now composes from the coordinator snapshot and submits through the backend-owned output set; it no longer replaces committed state through the legacy snapshot API. Backend-live also owns the prepared-authority and CPU-submission records used internally by that control path. `LiveProductionVisualRuntime` now owns the neutral visual-control implementation in backend-live; `PersistentBackendRuntime` and its CLI implementation are gone. Snapshot replacement is isolated to the Wayland maintenance adapter. Production X now polls one backend service entry point; backend-live owns retirement, queued Present scheduling, and pending native submission order. `runtime_driver` now owns CPU cycle order, full-state Present rebasing and preparation, prepared retirement commit-before-feedback, and dynamic asynchronous KMS phase order. Backend-live executes requested retire, Present-schedule, and pending-submit phases and returns reduced observations; it is no longer a second phase coordinator. All Milestone 6 architecture items are complete. The retained paired physical X13 GTK gate remains shared with Milestone 5 before the exit can be promoted as daily-driver evidence.

- [x] Establish `sophia-engine::runtime_driver` as the only production visual
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
- [x] Migrate the retained xterm CPU, mixed Vulkan, and GTK classic/confined
  QEMU gates without weakening their evidence schemas or latency and cleanup
  bounds; then delete the legacy CLI loop state.

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
