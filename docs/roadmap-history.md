# Sophia Roadmap History

This file archives completed and prototype roadmap items that previously lived
in `todo.md`. Keep active work in `todo.md`; keep detailed rationale and
validation evidence in `docs/research-log.md`.

---

## 2026-07-14 X11 Session Correctness

- [x] Implemented the XKB keymap/state path and bounded X11 focus, active and
  passive grabs, keyboard, pointer, and required XI2 delivery while preserving
  Engine-selected targets and authority-local coordinates.
- [x] Replaced fixed root/output facts with Engine-sourced topology and RandR
  snapshots, including authenticated update notification and normal
  configure-plus-pixels resize behavior.
- [x] Preserved client-targeted input/control acknowledgements, bounded
  backpressure, deterministic focus/stacking, and complete disconnect cleanup.
- [x] Retained and strictly verified the paired native two-xterm session on the
  X13 under classic shared-X and a fresh zero-capability confined namespace.

Both schema-13 runs used physical evdev keyboard and pointer input, flushed all
routed X11 events, retained two independently interactive CPU layers, applied
the Engine output update with four RandR notifications, committed resized
pixels, presented through Engine-owned KMS, and drained without cleanup debt.
Classic startup readiness was 94 ms and confined readiness was 90 ms; both had
13 ms maximum CPU composition and 0 ms measured input-to-presentation latency.
The retained ignored logs are
`.evidence/remote-target/tmp/sophia-milestone3-{classic,confined}.log`.

## 2026-07-14 Portal Broker And X11 Clipboard

- [x] Split sanitized policy requests from single-use, generation-bound grants
  with bounded pending/active state, deadlines, completion, expiry,
  disconnect/executor revocation, and broker-restart invalidation.
- [x] Added owner-only bounded broker IPC, persistent lifecycle across client
  connections, a default-deny deterministic policy provider, correlated
  payload frames, and fail-closed disconnect handling.
- [x] Implemented ordinary same-namespace `SelectionRequest`, restricted
  `SelectionNotify` SendEvent, owner replacement `SelectionClear`, and
  per-connection sequence/routing behavior.
- [x] Implemented cross-namespace `CLIPBOARD`/`PRIMARY` through an
  authority-private source proxy. The broker sees only sanitized namespace,
  target, size, generation, grant, and payload values; Engine and policy never
  receive XIDs or atoms.
- [x] Implemented `TARGETS`, `UTF8_STRING`, and bounded UTF-8 `text/plain`.
  Denied, stale, expired, disconnected, unsupported, and executor-failure
  paths produce normal `SelectionNotify(property = None)`.
- [x] Deterministic and socket tests prove same-namespace ownership and
  handoff, a complete broker/source-proxy/target-property transfer, stale
  generation, expiry, disconnect, default/capability denial, and executor
  failure without granting general cross-namespace resource visibility.

## 2026-07-13 X11-First Roadmap Reset

- [x] Added a documentation source-of-truth map and a normative
  namespace/admission/portal contract, then reconciled architecture, DOD,
  subsystem status, validation, README, and the native X11 evidence matrix.
- [x] Removed XLibre from the production dependency graph, workspace, launcher,
  and installed session while preserving its source and evidence under
  `research/xlibre`.
- [x] Established a bounded concurrent native X frontend with disjoint creation
  ranges, connection cleanup ledgers, classic same-namespace resource access,
  client-attributed transaction batches, and targeted input/control routes.
- [x] Proved two real xterms concurrently reach Engine-owned CPU composition
  and dedicated KMS presentation. The retained optimized hardware run completed
  in 1,487 ms with 10 ms maximum composition, 23 ms
  input-to-presentation, all 14 events flushed, and no KMS cleanup debt.
- [x] Corrected initial focus, stacking, readiness, and startup sequencing so
  proof input cannot race a uniform startup surface or land behind an
  overlapping secondary client.
- [x] Completed the native Wayland SHM/Kitty foundation and retained controlled
  direct-scanout DMA-BUF first-frame and 300-frame lifecycle evidence.
- [x] Reframed Wayland and DMA-BUF expansion as maintenance/deferred work while
  namespace admission, portals, and the native X session become the active
  critical path.

The two-xterm result remains `hardware`, not `session`, evidence. Engine-derived
output facts, normal resize, XKB/grabs, independently credentialed confined
groups, and standard presentation feedback are still active work.

---

## 2026-07-13 Namespace And X Admission Foundation

- [x] Added immutable namespace profiles, directional capabilities, admission
  contexts, and a session-owned generation-safe namespace registry.
- [x] Replaced the production listener-wide namespace shortcut with
  per-connection admission after cookie authentication and kernel peer
  credential checks.
- [x] Added fresh owner-only Xauthority publication, launchable classic and
  confined profiles, disjoint connection XID ranges, and cleanup-attributed
  client identities.
- [x] Proved classic shared-resource access and confined denial for resource
  lookup, properties, selections, event selection, routed input, and metadata.
- [x] Added supervisor-triggered admission revocation that disconnects one
  worker and follows the normal route, resource, surface, selection, and lease
  cleanup path without disrupting a classic peer.

The milestone exit is satisfied: production sessions allocate namespace
identity through the registry; every policy-admitted connection retains an
immutable context; disconnect and targeted supervision converge on the same
fail-closed teardown sequence.

---

## Completed Native Wayland Foundation

- [x] Removed XLibre concepts from Engine input and surface records, replacing
  them with protocol-neutral surface routes and authority-local IDs.
- [x] Added a Smithay-backed private Wayland authority for `wl_surface`,
  `xdg_toplevel`, SHM, bounded linear DMA-BUFs, frame callbacks, buffer release,
  keyboard, and pointer delivery.
- [x] Ran real Kitty over the private Wayland socket with `DISPLAY` removed and
  changing nonzero SHM frames through Engine.
- [x] Made the installed Kitty launcher use the native Wayland authority and
  native KMS path while preserving the independent TTY recovery guard.
- [x] Made the XLibre bridge an opt-in research feature excluded from the
  production dependency graph and default workspace members, then moved the
  crate, bridge-only CLI, patches, scripts, fixtures, and protocol notes into
  the non-workspace `research/xlibre` archive.
- [x] Added direct EGL DMA-BUF import with no CPU readback and delayed Wayland
  presentation feedback until the matching KMS submission is observed.

---

## Completed Visible Interactive X Terminal

- [x] Backed core X drawing with bounded XRGB8888 CPU buffers, including the
  fixed-font xterm path, and composed those pixels into renderer-owned frames.
- [x] Kept X Authority, backend ticks, native scanout ownership, and xterm under
  one persistent session owner with clean submit, page-flip retirement, and
  cleanup evidence.
- [x] Routed QMP virtio-keyboard input through libinput and Engine focus, then
  repeated the exact 14-event `sophia` plus Return proof with an operator on AMD
  TTY hardware. Both paths changed later xterm pixels without internal X event
  injection.
- [x] Selected xterm's core-key event target from authority-private event masks,
  emitted the required focus transition, and withheld readiness until the
  nonzero prompt checksum was page-flip-confirmed.

## Completed xmonad Bridge And Stability Evidence

- [x] Ran real xmonad as metadata-blind layout policy through the isolated
  embedded X11 WM bridge and translated its two-window configure/focus requests
  into bounded Sophia WM packets.
- [x] Recorded bounded session latency, queue, callback, failure, and cleanup
  counters; passed the 30-second TTY stability run and isolated 300-tick QEMU
  run with dual-output keyboard/pointer evidence.

---

## Completed Real Backend Evidence and Runtime Seams

- [x] Added `live-session-composition-smoke`, composing the Sophia X Authority
  Present-pixmap socket path, bounded authority batch intake, runtime commit
  projection, renderer-live frame-target observation, rendered primary-plane
  scanout submit, deterministic page-flip retire, and cleanup-drained reporting
  into one non-destructive reduced evidence line.
- [x] Proved libinput-shaped input polling, native page-flip retirement, and
  rendered scanout submit can share one runtime tick.
- [x] Added a runtime-owned readiness gate so concrete libinput dispatch runs
  only after the session loop observes reduced readiness.
- [x] Kept physical input intake separate from routed-input transformation with
  reduced `PhysicalIntakeOnly` runtime evidence.
- [x] Preserved deterministic queued poller tests as the default validation
  path while keeping native libinput behind optional feature tests.
- [x] Collapsed one-shot helper calls into a reusable session loop owner.
- [x] Fed reduced input, page-flip, and scanout facts through one bounded tick
  budget.
- [x] Kept real file-descriptor readiness outside Sophia Engine state via a
  reduced readiness collector.
- [x] Recorded opt-in real GBM/EGL validation with reduced draw, presentation,
  and frame-target allocation evidence.
- [x] Defined renderer-private GBM/EGL frame-target lifecycle states: created,
  retained, resized, invalidated, and retired.
- [x] Added reduced runtime observations for frame-target lifecycle and
  allocation without implicit native allocation during ticks.
- [x] Added the first reduced KMS scanout target report and derived page-flip
  readiness from it.
- [x] Preserved CPU fallback and degraded GPU paths while scanout matures.
- [x] Added a fake live compositor loop smoke covering input polling, authority
  transaction intake, WM policy, renderer target observation, frame commit, and
  reduced page-flip observation.

---

## Completed GBM/EGL Frame Target Milestones

- [x] Proved a real composed xterm frame, identified by matching requested and
  exported checksums, reaches native GBM/KMS submit and accepted page-flip
  retirement on TTY3 without cleanup debt.

- [x] Defined the opt-in real-device evidence shape for native GBM/EGL
  frame-target allocation.
- [x] Extended real GBM/EGL smoke evidence with reduced frame-target allocation
  status while keeping native handles renderer-private.
- [x] Threaded reduced GBM/EGL frame-target readiness and allocation reports
  through backend-live and runtime observations without making Sophia Engine own
  renderer-private lifecycle.

---

## Completed Atomic Scanout Smoke Milestones

- [x] Added `tools/atomic_scanout_preflight.sh`,
  `tools/atomic_scanout_smoke.sh`, and the strict reduced evidence verifiers for
  preflight, destructive atomic scanout, and runtime rendered-scanout evidence.
- [x] Added `tools/atomic_scanout_hardware_proof.sh` as the combined operator
  proof for preflight, two-phase atomic scanout, and runtime submit-to-retire
  evidence.
- [x] Advanced reduced atomic scanout evidence to schema 10 and runtime
  rendered-scanout submit evidence to schema 6 with reduced scanout-buffer
  format, modifier, plane-count, format-table, framebuffer-creation, submit,
  retire, and cleanup-debt fields.
- [x] Added backend-private primary-plane `IN_FORMATS` discovery and bounded
  modifier parsing so rendered GBM/EGL scanout export can choose usable scanout
  candidates without exposing DRM property blobs or native modifier values.
- [x] Allowed explicit non-linear multi-plane XRGB8888/ARGB8888 buffers to
  reach modifier-aware AddFB2 while keeping unsupported implicit/linear
  multi-plane buffers rejected before native resource creation.
- [x] Made rendered GBM/EGL scanout skip rejected multi-plane export candidates
  and continue searching for a single-plane buffer when the driver rejects the
  modifier-aware framebuffer path.
- [x] Added backend-private PRIME import for renderer-exported DMA-BUF planes:
  backend-live imports them into the KMS submit device, builds AddFB2/AddFB from
  KMS-local handles, and closes imported GEM handles through the existing
  cleanup path.
- [x] Fixed the destructive smoke lifetime rule so the initial rendered GBM/KMS
  owner remains active until the steady page flip presents, then both resource
  bundles retire after accepted page-flip callbacks.
- [x] Captured TTY3 reduced smoke evidence where both `InitialModeset` and
  `SteadyPageFlip` pass with `framebuffer=CreatedWithAddFb2`,
  `page_flip=Presented`, `retire=RetiredAfterPageFlip`, and
  `retire_cleanup_pending=false`.
- [x] Closed the combined TTY3 hardware proof with verifier-accepted preflight,
  destructive two-phase atomic scanout evidence, and runtime rendered-scanout
  submit-to-retire evidence.

---

## Completed X Authority Milestones

- [x] Advanced X window generations after every emitted drawing transaction so
  persistent clients commit contiguous visual updates instead of replaying one
  stale `previous_committed_generation`.
- [x] Added persistent `sophia-live-session` display/xterm ownership with one
  live backend runtime, repeated CPU composition, bounded shutdown, and a real
  injected-input pixel-change regression.
- [x] Added Engine-owned seat focus validation and an explicit libinput-device
  route from reduced physical key packets to X Authority core keyboard events.

- [x] Expand X11 atom/property tables for ICCCM names and metadata-broker
  candidates.
- [x] Add minimal bounded `GetProperty` replies and socket smoke coverage.
- [x] Define and pass the first real-client-library target with `x11rb`: setup,
  atom lookup, create, property write/read, map, and event observation.
- [x] Pass `xdpyinfo` as a broader setup/introspection probe with minimal
  `CreateGC`, `FreeGC`, `GetInputFocus`, `QueryExtension`, `ListExtensions`,
  `QueryBestSize`, full predefined atom, and root-property read support.
- [x] Pass a tiny compiled C Xlib probe covering `XOpenDisplay`,
  `XInternAtom`, `XCreateSimpleWindow`, `XStoreName`, `XChangeProperty`,
  `XGetWindowProperty`, `XMapWindow`, and `XDestroyWindow`.
- [x] Pass a drawing-oriented C Xlib probe using `XFillRectangle`; opcode 70
  now decodes to a ready `CoreDraw` surface transaction.
- [x] Expose live X11 socket dispatch results through an out-of-band observer,
  preserving no-reply success semantics for core draw requests.
- [x] Add socket-level smoke coverage proving `PolyFillRectangle` creates one
  ready `CoreDraw` transaction outside unit-test-only dispatch.
- [x] Feed observed X Authority drawing transactions into the live runtime
  adapter as reduced authority commit summaries.
- [x] Validate the C Xlib drawing smoke through Engine commit and Runtime
  authority transaction counters without leaking XIDs or namespace metadata.
- [x] Add the first bounded software upload request model with core `PutImage`
  decoding into ready CPU-backed surface transactions.
- [x] Pass a compiled C Xlib `XPutImage` smoke through observed transaction,
  Engine commit, and Runtime authority counters with no direct X reply on
  success.
- [x] Add the first Present-style explicit buffer handoff model as private
  `SOPHIA-PRESENT` minor opcode `0` using XPixmap handles.
- [x] Add socket and CLI smoke coverage proving Present-style handoff reaches
  Engine commit and Runtime counters without adding compositor policy.
- [x] Defer DMA-BUF placeholder modeling until real DRI3/Present semantics are
  ready.
- [x] Move the long-running X Authority transaction side channel to a
  runtime-owned bounded queue while keeping callback observers for focused
  tests.
- [x] Document side-channel backpressure: full or disconnected queues fail
  closed instead of allocating unbounded buffers or dropping visual facts.
- [x] Add minimal `MIT-SHM` `QueryExtension` and `ShmQueryVersion` support with
  unsupported minor opcodes failing closed.
- [x] Model `ShmAttach` as namespace-local metadata without mapping host memory.
- [x] Decode `XShmPutImage` and reject it with bounded native X errors until a
  real SHM import path exists.
- [x] Defer real MIT-SHM memory mapping until a compositor backend can consume
  mapped bytes through a bounded renderer import path.
- [x] Add protocol-neutral `AuthorityTransactionIntake` so runtime can commit
  authority batches without making Sophia Engine depend on Sophia X Authority.
- [x] Define the first runtime-owned backend assembly struct that holds output
  discovery, input polling, frame clock, authority transaction intake, and
  renderer selection without owning protocol policy.
- [x] Add a headless assembly smoke that drains authority batches into committed
  surface state and renders through the existing runtime driver.
- [x] Define the runtime event that reports authority process ready/degraded
  state without leaking X11 resource IDs or namespace metadata.
- [x] Move the Present-style smoke to the bounded X Authority transaction
  channel path; callback observers remain for focused tests.
- [x] Add a supervised X Authority process wrapper that emits reduced authority
  health observations.
- [x] Feed bounded authority transaction batches into the compositor backend
  assembly through a protocol-neutral inbox.
- [x] Decide the first real backend dependency boundary for DRM/KMS and
  libinput without changing the Engine/WM/Authority packet contracts.
- [x] Keep real DRM/KMS ioctls, GPU imports, and real MIT-SHM mapping deferred
  behind deterministic backend discovery and assembly seams.
- [x] Sketch the first live compositor backend crate boundary and keep kernel
  IO behind traits that preserve deterministic headless tests.
- [x] Pass `/usr/bin/xclock` against the Sophia X Authority socket through
  mapped exposure and observed Engine/Runtime authority transactions.
- [x] Add one smoke that proves backend discovery can fail closed without
  affecting protocol authority or WM IPC contracts.
- [x] Add a live backend dependency policy before adding crates that touch
  `/dev/dri`, `/dev/input`, GBM, EGL, DMA-BUF, or real MIT-SHM mapping.
- [x] Decide that libdrm and libinput may enter through
  `sophia-backend-live`, while GPU imports and real MIT-SHM mapping remain
  deferred until renderer import boundaries exist.
- [x] Define the renderer import boundary separately from backend discovery.
- [x] Sketch reduced renderer import admission records without adding GBM, EGL,
  DMA-BUF, or real MIT-SHM mapping.
- [x] Add deterministic tests for renderer import admission and fail-closed
  unsupported import paths.
- [x] Decide that the first real renderer implementation should live in a
  dedicated `sophia-renderer-live` crate once GBM, EGL, DMA-BUF, or explicit
  sync dependencies are required.
- [x] Thread live renderer import admission into backend startup without adding
  GBM, EGL, DMA-BUF, or real MIT-SHM mapping.
- [x] Add a startup smoke proving native renderer import remains disabled until
  explicitly configured.
- [x] Define the first live renderer health shape for CPU fallback, native
  import-capable, and degraded import capability.
- [x] Add renderer admission status to live backend startup reports without
  leaking renderer-private handles.
- [x] Sketch the `sophia-renderer-live` crate boundary without adding GBM, EGL,
  DMA-BUF, or real MIT-SHM mapping.
- [x] Decide the first runtime observation shape for CPU fallback versus native
  import-capable renderer selection.
- [x] Decide that renderer import startup health is stored in the
  `sophia-backend-live` runtime wrapper, not inside `sophia-engine`.
- [x] Add a reduced runtime observation for renderer import health once startup
  health is consumed by the runtime assembly.
- [x] Keep real GBM/EGL/DMA-BUF dependencies deferred until
  `sophia-renderer-live` has deterministic fake coverage.
- [x] Add fake degraded renderer coverage before modeling any real native import
  failure.
- [x] Decide that degraded renderer health is sourced from both failed startup
  capability probes and per-frame failed imports.
- [x] Add a fake failed-import path in `sophia-renderer-live` for per-frame
  degraded runtime observation.
- [x] Keep the live runtime wrapper outside `sophia-engine` unless engine-local
  renderer policy becomes unavoidable.
- [x] Revisit real GBM/EGL/DMA-BUF admission only after the degraded-health
  source decision is documented.
- [x] Decide that the first real native renderer dependency candidate is a
  feature-gated GBM capability probe.
- [x] Keep the first real renderer dependency behind an optional crate feature
  so default offline deterministic tests remain available.
- [x] Add feature-gate scaffolding for the future GBM capability probe without
  adding the dependency.
- [x] Add a fake feature-enabled GBM probe test that still uses deterministic
  data before introducing a real crate.
- [x] Revisit real GBM dependency admission after the feature-gated fake path
  exists.
- [x] Decide the real GBM probe API shape: backend-provided reduced device token,
  not device-path intake or borrowed fd intake.
- [x] Keep default workspace tests independent of native renderer libraries.
- [x] Add documented local check coverage for
  `cargo test --offline -p sophia-renderer-live --features gbm-probe`.
- [x] Decide to isolate any concrete GBM binding behind a tiny renderer-live
  adapter module rather than exposing it directly.
- [x] Keep real GBM dependency optional until release checks exercise both
  default and feature-enabled paths.
- [x] Revisit real GBM crate admission after selecting the concrete crate and
  checking its offline/system dependency behavior.
- [x] Evaluate concrete GBM crate options and choose the safe `gbm` crate as the
  first candidate, keeping `gbm-sys` as a fallback only.
- [x] Add dependency-admission notes for the selected GBM crate before adding it.
- [x] Confirm local `libgbm` development visibility through `pkg-config`.
- [x] Add the optional `gbm` dependency under `gbm-probe` with default features
  disabled.
- [x] Add the private adapter module with fake/native split under `gbm-probe`.
- [x] Teach the private native GBM probe to consume backend-owned device
  authority without exposing raw fds through public Sophia data.
- [x] Map real native GBM probe failures to reduced degraded renderer health.
- [x] Wire backend-live render-device discovery into the reduced GBM probe path.
- [x] Keep CPU fallback startup as the default when GBM probing is absent,
  unavailable, or degraded.
- [x] Add feature-enabled backend-live coverage for GBM degraded startup health.
- [x] Add a live render-device discovery abstraction that can later choose a DRM
  render node without leaking paths into engine state.
- [x] Decide render-node discovery stays in backend-live for now, behind a
  narrower feature-gated trait.
- [x] Add feature-enabled startup assembly that selects CPU fallback when GBM
  probing degrades a requested DMA-BUF path.
- [x] Add a runtime observation for degraded GBM startup that remains count-only
  and path-free.
- [x] Add renderer preference policy: `GpuPreferred`, `CpuOnly`, and
  `GpuRequired`.
- [x] Decide the first real render-node discovery source is explicit
  backend-owned fd injection.
- [x] Add docs for why degraded native import must not partially enable the
  import-capable renderer.
- [x] Add a real GBM allocation probe that creates and drops a tiny private
  buffer without exporting GBM handles.
- [x] Add a reduced GPU startup report that distinguishes render-device
  discovery failure from GBM device rejection and private allocation failure
  without leaking native error text.
- [x] Add opt-in backend-owned fd injection smoke coverage around a real render
  device when the host test environment exposes one.
- [x] Decide EGL/OpenGL is the first compositor drawing API above GBM; raw
  Vulkan and wgpu remain deferred.
- [x] Add `egl-probe` feature scaffolding without admitting a native EGL crate.
- [x] Add fake reduced EGL capability records and backend startup projection.
- [x] Evaluate concrete EGL binding options and choose `khronos-egl` as the
  candidate for the native context probe.
- [x] Add EGL dependency-admission notes before admitting a real EGL crate.
- [x] Add the optional `khronos-egl` dependency under `egl-probe` with dynamic
  loading only.
- [x] Add a real EGL context probe behind the `egl-probe` feature, isolated in
  a tiny native adapter crate that returns only reduced status.
- [x] Add reduced EGL draw-smoke status records.
- [x] Add the first native EGL offscreen target smoke: create a private pbuffer,
  make a context current against it, and return only reduced status.
- [x] Evaluate concrete GL function loading options and choose `glow` as the
  candidate for the first clear-color smoke.
- [x] Add GL function-loading dependency-admission notes before admitting
  `glow`.
- [x] Add optional `glow` dependency for clear-color smoke inside the native EGL
  adapter.
- [x] Extend the EGL draw smoke from private pbuffer readiness to a clear-color
  smoke without exporting handles.
- [x] Add reduced clear-color smoke status and validation docs for GL failure
  modes.
- [x] Add a GBM-backed EGL platform candidate doc before moving beyond
  `DEFAULT_DISPLAY`.
- [x] Define the first presentation smoke boundary without exposing GPU handles.
- [x] Add renderer-live reduced presentation status before admitting scanout or
  exported buffer paths.
- [x] Add fake presentation smoke coverage for ready, unavailable, and degraded
  statuses.
- [x] Add a GBM-backed EGL platform status model behind `gbm-probe,egl-probe`.
- [x] Add fake GBM-backed EGL platform projection coverage before native code.
- [x] Add a native GBM-backed EGL platform smoke that preserves
  `LiveGbmBackedEglPlatformReport`.
- [x] Keep `DEFAULT_DISPLAY` as fallback smoke until the GBM-backed platform
  smoke passes.
- [x] Add a GBM-backed EGL private target smoke without exporting buffers.
- [x] Keep `DEFAULT_DISPLAY` clear-color smoke as fallback until GBM-backed
  drawing passes.
- [x] Add a native offscreen presentation smoke that preserves reduced
  presentation status.
- [x] Add opt-in real render-node validation for GBM-backed EGL drawing and
  offscreen presentation.
- [x] Define the first scanout-adjacent status without exposing KMS object
  identity.
- [x] Decide when `SOPHIA_RUN_REAL_GBM_SMOKE=1` results are strong enough to
  retire `DEFAULT_DISPLAY` as a fallback smoke.
- [x] Add the first reduced page-flip event shape without KMS object identity.
- [x] Thread reduced scanout and page-flip events into runtime observation.
- [x] Define the first deterministic page-flip callback intake seam.
- [x] Wire page-flip callbacks into a runtime-owned bounded queue.
- [x] Add a fake page-flip callback source before real libdrm event polling.
- [x] Define the feature-gated libdrm page-flip event polling adapter shape.
- [x] Evaluate concrete libdrm crate admission for native page-flip polling.
- [x] Add optional `drm` dependency under `libdrm-events` without wiring native
  polling yet.
- [x] Add a private native libdrm event adapter module skeleton without opening
  devices.
- [x] Define backend-owned libdrm fd authority shape without exposing fds.
- [x] Let the private libdrm adapter accept backend-owned authority without
  polling.
- [x] Define reduced native libdrm page-flip source construction from authority
  without reading events.
- [x] Define native libdrm read-loop result mapping before real fd polling.
- [x] Add a non-polling native libdrm poller skeleton that preserves reduced
  report contracts.
- [x] Define reduced native page-flip callback decoding without exposing KMS
  resource identity.
- [x] Thread reduced native page-flip callback decode counts into read-loop
  reports.
- [x] Add a bounded native page-flip decode batch helper before real fd polling.
- [x] Let the non-polling native libdrm poller drain an injected callback batch.
- [x] Add reduced native libdrm poller disconnected-queue retention coverage.
- [x] Add native libdrm poller route replacement coverage for hotplug-shaped
  changes.
- [x] Add reduced native libdrm poller route-count diagnostics without native
  identities.
- [x] Add startup wiring for reduced native libdrm poller diagnostics.
- [x] Add native libdrm poller construction from discovered output routes.
- [x] Add a reduced page-flip poller startup status for ready/no-output cases.
- [x] Keep wgpu deferred until GBM/EGL startup, drawing, and presentation seams
  are proven.
- [x] Add a reduced real-GBM-smoke evidence record shape for validation output.
- [x] Record a passing `SOPHIA_RUN_REAL_GBM_SMOKE=1` run before retiring
  `DEFAULT_DISPLAY`.
- [x] Record repeated passing `SOPHIA_RUN_REAL_GBM_SMOKE=1` runs before retiring
  `DEFAULT_DISPLAY`.
- [x] Keep `DEFAULT_DISPLAY` clear-color smoke as fallback until GBM-backed
  drawing is validated against real render nodes.
- [x] Decide whether `DEFAULT_DISPLAY` should retire now or remain as a host
  compatibility smoke.
- [x] Define the broader host/device matrix required before retiring
  `DEFAULT_DISPLAY`.
- [x] Decide the next production-shaped GBM/EGL step after offscreen
  presentation smoke evidence.
- [x] Define the first reduced GBM/EGL frame target record for future renderer
  integration.
- [x] Decide whether the frame-target record should be threaded into
  backend-live startup reports or stay renderer-local for one more step.
- [x] Add backend-live projection for reduced GBM/EGL frame target readiness if
  the renderer-local record proves stable.
- [x] Decide the first runtime observation shape for reduced GBM/EGL frame
  target readiness.
- [x] Add runtime tick projection for reduced GBM/EGL frame target readiness if
  startup projection remains stable.
- [x] Decide the first runtime mutation path for GBM/EGL frame-target readiness
  when output size changes.
- [x] Add a reduced frame-target update method on backend-live runtime assembly
  if output-size mutation belongs outside startup.
- [x] Define the first renderer-private allocation seam for GBM/EGL frame
  targets.
- [x] Add a fake allocation smoke that proves native frame-target handles stay
  renderer-private while runtime observes only reduced status.
- [x] Decide how backend-live should observe reduced GBM/EGL frame-target
  allocation reports.
- [x] Thread fake frame-target allocation reports into backend-live without
  exposing renderer-private handles.
- [x] Decide the first native GBM/EGL frame-target allocator skeleton shape.
- [x] Add a native allocator skeleton behind existing GBM/EGL features that
  returns reduced allocation status without exporting handles.
- [x] Decide how backend-live should call the native GBM/EGL frame-target
  allocator skeleton.
- [x] Add backend-live feature-gated coverage for native frame-target allocation
  using invalid and missing render devices.
- [x] Decide whether native frame-target allocation should feed
  `LiveBackendRuntimeAssembly` directly or remain an explicit caller action.
- [x] Add runtime assembly helper for native frame-target allocation if the
  explicit caller action remains stable.

---

## Deferred / Prototype Reference

These items are useful evidence from the XLibre-centered prototype, but they are
not the long-term target architecture.

- [x] Kept SHM routed input deferred because repeated routed-input stress did
  not exceed the documented optimization threshold.
- [x] Keep XLibre routed-input extension docs as a compatibility lesson.
- [x] Keep XComposite/Damage bridge smokes as reference tests until Sophia X
  Authority has equivalent transaction tests.
- [x] Keep XLibre namespace smoke as isolation evidence until Sophia X
  Authority namespace enforcement has live coverage.
- [x] Keep the documented Sophia X11 WM Bridge as a stopgap for reusing legacy
  tiling WMs without weakening the native blind WM IPC boundary.

---

## Completed Milestones

- [x] Protocol-neutral authority boundary: Engine routed input now targets
  `SurfaceId`, visual layers carry `AuthorityLocalId`, and Engine source has no
  XLibre, X-window, Wayland, Smithay, or Kitty types. XLibre wire encoding is
  confined to its opt-in historical adapter.
- [x] Sophia Wayland Authority foundation: a frontend-only Smithay authority
  owns private sockets, client namespaces, compositor/xdg resources, ordered
  surface reducers, SHM snapshots, output advertisement, seat delivery, frame
  callbacks, buffer releases, and bounded DMA-BUF negotiation.
- [x] First native Kitty transaction proof: Kitty 0.47.4 ran with `DISPLAY`
  removed and software GL, submitted 16 changing nonzero SHM frames through the
  Sophia Wayland Authority, and completed without an X server process.
- [x] XLibre runtime retirement: release builds and the installed launcher use
  native Wayland; the XLibre bridge has no live feature and is isolated under
  `research/xlibre`, outside the workspace and production dependency graph.

- [x] Pointer and multi-output presentation: QEMU proved physical keyboard and
  pointer routing, independent content on two KMS outputs, per-output
  page-flip pacing, and clean retirement. DRM VRR discovery and fullscreen
  policy are implemented; activation proof remains deferred for capable
  hardware.
- [x] XLibre Kitty compatibility proof: a real Kitty X11/GLX client produced
  readable pixels through software GL and XComposite/MIT-SHM capture, accepted
  physical keyboard input including terminal navigation keys, met the bounded
  presentation-latency gate, and recovered successfully through the independent
  Ctrl-Alt-Backspace guard.
- [x] Generic legacy-WM bridge core: opaque layout snapshots, validated Engine
  commits, resizable Xterm transactions, configure acknowledgement, focus, and
  injected-input pixel change passed headless coverage. Remaining dedicated-TTY
  and second-WM demonstrations are deferred rather than architecture blockers.

- [x] Added the first `sophia-live-session --terminal=xterm` one-shot bootstrap
  around xterm authority transactions and deterministic composition lifecycle.
- [x] Split X11 Authority socket binding and serving into reusable one-client
  and persistent sequential entry points with authority state shared across
  accepted connections.
- [x] Proved the xterm request stream reaches committed drawing transactions;
  this evidence is now classified accurately as a transaction proof rather
  than an inspectable-pixel proof.

- [x] Engine-centered authority reframe: README, architecture docs, atomic
  rendering invariant, and XLibre prototype/reference status.
- [x] Data-oriented design and style rules, including domain-first file
  cohesion guidance.
- [x] Phase 0-2: repository shape, Rust skeleton, protocol/data model, and
  headless engine.
- [x] Phase 3-4: XLibre mirror probe, XComposite/Damage capture, CPU readback,
  and first X11 surface in headless frames.
- [x] Phase 5-6.5: blind WM protocol, bounded IPC codec, external WM demo,
  routed-input XLibre patch, and smoke/stress coverage.
- [x] Phase 7-8: portal reducers, compositor chrome action reducer, and polite
  X11 close helper.
- [x] Phase 9: process supervisor, restart policy, WM restart adapter, and last
  committed layout cache.
- [x] Session runtime assembly: runtime reducer, bounded observation intake,
  headless session driver, broker health/control packets, and live X/WM socket
  smoke.
- [x] Portal execution prototype: X11 `SelectionRequest` conversion, native
  denial, approved bounded text handoff, and live X smoke.
- [x] Portal request/grant lifecycle: bounded pending and active state,
  deadlines, completion, expiry, disconnect and executor revocation,
  source-generation validation, and broker-restart invalidation.
- [x] Native same-namespace selection handshake: per-client routing for core
  `SelectionRequest` and restricted `SelectionNotify` SendEvent, with a
  two-client socket proof and connection-local event sequences.
- [x] Protocol-neutral authority transactions: `AuthoritySurface`,
  `SurfaceTransaction`, readiness states, and committed surface projection into
  renderable layers.
- [x] Sophia X Authority design: namespace-scoped resources, event
  subscriptions, synthetic lifecycle, drawing updates, and selection portal
  conversion.
- [x] Sophia X Authority v0 runtime: internal request/response packets, bounded
  codec, reducer-backed runtime, Unix socket helper, and
  `x-authority-runtime-smoke`.
- [x] Sophia X Authority X11 wire start: setup parser, setup success/failure
  encoders, first core request decoder, minimal property table, and setup
  socket smoke.
- [x] Sophia X Authority client-visible output: bounded X error/event records,
  `ConfigureNotify`, `MapNotify`, `PropertyNotify`, `SelectionNotify`, and
  setup/create/map socket smoke.
- [x] Future Wayland Authority boundary documented as a later protocol
  authority, not the architectural center.
- [x] Backend skeletons: frame clock, renderer/import abstraction, DRM/KMS
  discovery, libinput polling, physical input routing, and page-flip timing
  seams.
