# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Atomic Hardware Evidence

- [ ] Run the non-modesetting atomic scanout preflight with
  `tools/atomic_scanout_preflight.sh`.
- [ ] Run the opt-in atomic hardware smoke on a DRM-master-capable machine.
- [ ] Capture `LibdrmNativeAtomicScanoutSmokeEvidence` with
  `tools/atomic_scanout_smoke.sh`.
- [ ] Verify rendered GBM front-buffer export, primary-plane atomic submit,
  native page-flip callback, resource retirement, and steady-state page-flip
  submit appear in reduced evidence.

Support now exists for that hardware proof:

- `tools/atomic_scanout_smoke.sh` runs verified preflight before the opt-in
  modesetting smoke, records the smoke log, and verifies passing evidence
  automatically.
- `tools/atomic_scanout_preflight.sh` records reduced host readiness without
  requesting DRM master or modesetting hardware, and proves at least one primary
  card node can be opened read/write and admit the required DRM atomic client
  capabilities, with a reduced KMS scanout target and atomic property handles,
  before the destructive smoke runs.
- `tools/verify_atomic_scanout_preflight.sh` can verify that a captured
  preflight log is ready for the DRM-master smoke.
- `tools/verify_atomic_scanout_evidence.sh` can verify a captured log offline
  against the reduced atomic scanout evidence contract.

---

## Next 3 Milestones

### 1. Authority Probe Selection

- [ ] Choose the next real Sophia X Authority app probe after backend intake
  seams settle.
- [ ] Prefer probes that exercise bounded drawing/upload/present paths over
  broad X11 completeness.
- [ ] Keep Wayland Authority deferred until backend event intake and scanout
  timing are stable.

### 2. Sophia X Authority Coverage

- [ ] Expand Sophia X Authority only where real app probes demand it.
- [ ] Prioritize bounded drawing, upload, present, selection, and namespace
  behavior over broad X11 completeness.
- [ ] Keep XLibre prototype docs and bridge smokes as compatibility lessons
  until Sophia X Authority has equivalent live coverage.

### 3. Future Authority Substrate

- [ ] Start Sophia Wayland Authority only after live backend frame targets and
  scanout timing are stable.
- [ ] Revisit wgpu only after GBM/EGL startup, drawing, presentation,
  frame-target lifecycle, and scanout seams are validated.
---

## Later Backlog

- [x] Build the first real KMS atomic property-set shape from backend-private
  connector, CRTC, plane, framebuffer, and mode state.
- [ ] Continue splitting backend-live by domain where modules still mix
  unrelated authority, renderer, runtime, or scanout ownership.

---

## Done Recently

- [x] Strengthened atomic scanout preflight to schema 5 with a capped
  atomic-property-ready primary card count, so the smoke gate catches
  unsupported, permission-limited, target-less, or property-incomplete cards
  before the modesetting test.
- [x] Split backend-live native scanout into commit, evidence, and submit
  modules while keeping the public DRM façade stable.
- [x] Opened the opt-in atomic hardware smoke card fd with `O_NONBLOCK` so the
  bounded page-flip evidence wait cannot hang inside DRM event reads.
- [x] Made the opt-in atomic hardware smoke wait within a bounded deadline for
  native page-flip evidence instead of sampling the nonblocking fd once.
- [x] Extended atomic scanout evidence to schema 3 so a passing hardware capture
  must prove both initial modeset presentation and steady-state page-flip
  presentation.
- [x] Gated the modesetting atomic scanout smoke helper behind verified
  non-modesetting preflight so unsupported hosts fail before requesting DRM
  master.
- [x] Added strict verification fixtures for atomic scanout preflight logs so
  hardware readiness can be checked before attempting the modesetting smoke.
- [x] Added a non-modesetting atomic scanout preflight report and tool so
  hardware validation can distinguish missing primary card nodes before the
  DRM-master smoke.
- [x] Carried reduced request scope through rendered primary-plane runtime
  submit reports so steady-state scanout diagnostics prove page-flip request
  shape, not only commit flags.
- [x] Added reduced request scope to atomic scanout evidence, so hardware smoke
  captures prove whether they submitted a modeset or page-flip request shape.
- [x] Split primary-plane atomic request building so runtime page-flip policy
  emits plane-only requests while explicit modeset smokes keep connector,
  CRTC, mode, and active properties.
- [x] Made reduced atomic scanout evidence require explicit destroyed-resource
  status and no cleanup debt before reporting `Passed`.
- [x] Versioned the reduced atomic scanout evidence schema and hardened the
  verifier to parse fields exactly, rejecting duplicate or unknown fields.
- [x] Extracted backend-live runtime page-flip observation, atomic-commit
  reporting, and callback queue draining into a dedicated runtime submodule.
- [x] Extracted backend-live runtime rendered-primary-plane ownership,
  backpressure, retirement, and cleanup tracking into a dedicated runtime
  submodule.
- [x] Extracted backend-live runtime frame-target and reduced KMS scanout
  target lifecycle/allocation into a dedicated runtime submodule.
- [x] Split native page-flip plumbing into domain modules for reduced reports,
  output routing/decode, reader implementations, and poller state.
- [x] Split rendered primary-plane scanout into domain modules for reduced
  report types, submit tracking, command-time runtime adaptation, and
  page-flip retirement/cleanup.
- [x] Preserved reduced native destroy status in tracked rendered scanout
  retire reports so runtime diagnostics can distinguish clean retirement from
  retryable cleanup debt without native identity.
- [x] Made native page-flip read-and-poll drain retained callbacks before
  reading more fd events, bounding pending callback growth under queue
  backpressure.
- [x] Preserved reduced native page-flip `WouldBlock` diagnostics through empty
  read-and-poll cycles so production scanout can distinguish an idle queue from
  a nonblocking fd read.
- [x] Added reduced physical-input intake evidence to Engine backend ticks so
  runtime reports prove physical packets are queued without doing scene
  hit-testing or routed-input request generation.
- [x] Extended the default queued-poller backend-live smoke to assert
  `PhysicalIntakeOnly`, keeping native libinput behind optional feature tests.
- [x] Added a reduced session-loop owner that observes input readiness, drains
  native page-flip events, and drives rendered primary-plane scanout through one
  bounded runtime tick without passing fds into Sophia Engine.
- [x] Added a reduced readiness collector so input and page-flip readiness are
  one-shot selector facts, and native page-flip reads occur only after reduced
  page-flip readiness is observed.
- [x] Added a one-shot live input readiness gate so runtime ticks continue when
  input is idle and concrete libinput dispatch only runs after the outer loop
  observes readiness.
- [x] Proved libinput-shaped input polling can run in the same live runtime tick
  as native page-flip retirement and rendered primary-plane scanout submit.
- [x] Added a concrete safe-wrapper libinput reader behind `libinput-events`
  that reduces pointer/key events into Sophia input packets without exposing
  native paths, fds, seat names, or libinput error strings.
- [x] Combined native page-flip intake with the persistent native GBM rendered
  scanout exporter so runtime ticks retire accepted GBM/KMS owners before the
  next reusable export attempt.
- [x] Moved backend-live startup discovery and renderer-selection helpers out
  of `src/lib.rs`, leaving the root module as wiring and public exports.
- [x] Extracted rendered primary-plane scanout types, helpers, and the
  command-time runtime adapter from backend-live `lib.rs` into a domain module.
- [x] Extracted page-flip callback intake, queueing, fake source, reduced poll
  reports, and fake poller types from backend-live `lib.rs`.
- [x] Extracted native libdrm page-flip source, reader, decode, and poller state
  from backend-live `lib.rs`.
- [x] Extracted native KMS/atomic primary-plane scanout selection, property
  discovery, resource ownership, submit, and retirement from backend-live
  `lib.rs`.
- [x] Added reduced `Deferred` scanout state so rendered primary-plane
  backpressure does not masquerade as rejection or corrupt in-flight accounting.
- [x] Threaded accepted reduced page-flip evidence into the rendered scanout
  runtime tick so tracked GBM/KMS owners retire before the next submit.
- [x] Added a reusable native GBM rendered-scanout exporter for runtime ticks;
  render-device discovery stays inside backend-live and failures reduce to
  runtime scanout rejection.
- [x] Added a persistent renderer-live GBM/EGL rendered-scanout context so the
  reusable backend exporter no longer reinitializes GBM/EGL on every valid
  export.
- [x] Added reduced persistent-context status to atomic scanout smoke evidence
  so context startup failure is distinct from GBM front-buffer export failure.
- [x] Added reduced context-open attempt counts to the reusable native GBM
  rendered-scanout exporter so context reuse/failure can be observed without
  leaking device identity.
- [x] Added a backend-live runtime tick path that answers active `SubmitScanout`
  commands through rendered GBM/KMS primary-plane scanout.
- [x] Threaded backend-live terminal scanout states into the shared runtime tick
  without exposing GBM or KMS ownership.
- [x] Added backend-live tracked rendered scanout ownership so stale page-flip
  evidence keeps GBM/KMS resources in flight until accepted retirement.
- [x] Added reduced in-flight tick age for tracked rendered scanout ownership so
  missing page flips are observable without exposing GBM/KMS identity.
- [x] Added reduced rendered-scanout backpressure classification so callers can
  distinguish idle, waiting, and stalled page-flip states without retiring early.
- [x] Bound tracked rendered scanout submissions to the last observed page-flip
  sequence so replayed accepted callbacks cannot retire a newer owner.
- [x] Made rendered scanout cleanup retryable after accepted page-flip cleanup
  failure without exposing framebuffer/blob identity.
- [x] Threaded reduced cleanup-pending diagnostics through runtime ticks and
  atomic scanout smoke evidence.
- [x] Added device-backed runtime tick cleanup retry with reduced retry status
  before the next rendered scanout submit.
- [x] Backpressured rendered primary-plane submit while cleanup remains pending
  so native cleanup debt stays bounded to one retained owner.
- [x] Added reduced frame-target lifecycle tracking to the reusable native GBM
  rendered-scanout exporter so resize/reuse behavior is observable without
  native identity.
- [x] Made KMS scanout readiness fail closed when the reduced frame target size
  does not match the selected output size.
- [x] Threaded reduced KMS scanout readiness into rendered primary-plane submit
  so not-ready targets reject before renderer export or native KMS work.
- [x] Made reduced KMS scanout readiness mandatory in rendered primary-plane
  submit reports so the submit path cannot omit readiness evidence.
- [x] Added reduced KMS scanout target status to atomic scanout smoke evidence
  so opt-in hardware proof cannot pass without target-readiness evidence.
- [x] Added preselected KMS target primary-plane submit so rendered target
  sizing and atomic submit can share one coherent selection snapshot.
- [x] Rechecked the native KMS target snapshot before runtime rendered scanout
  export so stale readiness cannot render into the wrong target.
- [x] Split atomic submit policy so runtime rendered scanout uses page-flip
  commits without `ALLOW_MODESET`, while modeset permission stays explicit.
- [x] Added reduced commit flags to atomic scanout smoke evidence so hardware
  captures prove the submit policy used by the page-flip.
- [x] Added a native page-flip intake runtime tick so rendered primary-plane
  scanout reads libdrm events before retirement and next-submit sequencing.
- [x] Added reduced live scanout submit intake so backend-live rendered
  primary-plane submit results can drive runtime `SubmitScanout`.
- [x] Added shared runtime scanout lifecycle state so rendered frames progress
  through `SubmitScanout` before portal/chrome phases.
- [x] Added a reduced `LiveAtomicScanoutCommitReport` that maps Engine
  `PageFlipCommitOutcome` into runtime-safe scanout commit state.
- [x] Added `LiveAtomicScanoutCommitter` so runtime assembly commits through a
  backend-owned scanout boundary.
- [x] Required accepted page-flip callback evidence before callback-driven
  atomic scanout commits can publish committed state.
- [x] Added a concrete `NativeLibdrmPageFlipEventReader` behind
  `libdrm-events` without opening devices during default validation.
- [x] Added a feature-gated native atomic submit committer that calls the DRM
  atomic commit API but waits for page-flip evidence before visual commit.
- [x] Added a feature-gated primary-plane atomic request builder for the
  full-output scanout case.
- [x] Added feature-gated KMS connector/CRTC/primary-plane target selection for
  the native atomic request path.
- [x] Added feature-gated primary-plane mode-blob/framebuffer resource lifecycle
  for the native atomic request path.
- [x] Added renderer-live scanout-buffer descriptor export and backend-live DRM
  buffer adapter for framebuffer registration.
- [x] Added feature-gated native GBM scanout buffer exporter that owns the
  buffer object behind the reduced descriptor.
- [x] Added a reduced primary-plane scanout submit chain that selects KMS
  target resources, validates a renderer scanout descriptor, creates
  framebuffer resources, submits an atomic commit, and retains an opaque
  submission owner until page-flip retirement.
- [x] Added page-flip-gated retirement for submitted primary-plane scanout
  owners so stale or rejected callbacks keep resources alive.
- [x] Added an opt-in atomic hardware smoke child that opens real DRM/GBM
  devices, allocates an owned GBM scanout buffer, submits primary-plane scanout,
  polls native page-flip evidence, and retires the submitted resources.
- [x] Added reduced `LibdrmNativeAtomicScanoutSmokeEvidence` so the opt-in
  hardware smoke reports where the GBM/submit/page-flip/retire chain stopped
  without exposing native handles or KMS object IDs.
- [x] Added reduced retire-destroy status to atomic scanout smoke evidence so
  accepted page flips and framebuffer/blob cleanup failures remain distinct.
- [x] Added a rendered GBM scanout export path that clears an EGL-backed GBM
  surface, swaps, locks the XRGB8888 front buffer, and feeds the same reduced
  scanout descriptor into the atomic submit chain.
- [x] Added a runtime-facing rendered primary-plane scanout submit seam that
  retains the rendered buffer owner with the KMS submission owner until
  accepted page-flip retirement.
- [x] Added feature-gated DRM atomic property discovery for the primary-plane
  request builder.
- [x] Defined opt-in environment gates for real libdrm and libinput validation.
- [x] Kept default workspace validation independent of device nodes and seats.
- [x] Added reduced real-hardware smoke reports that fail closed before concrete
  native readers exist.
- [x] Made compositor backend assemblies generic over `NonBlockingInputPoller`
  instead of boxing the hot path.
- [x] Kept `QueuedInputPoller` as the default deterministic backend assembly
  poller.
- [x] Added a native-shaped libinput runtime smoke proving live assembly can run
  a tick without changing Sophia Engine.
- [x] Added bounded native libdrm and libinput event intake seams.
- [x] Validated combined native-shaped event intake features.
- [x] Recorded real GBM/EGL evidence with frame-target allocation status.
- [x] Added reduced frame-target lifecycle observations.
- [x] Added reduced KMS scanout target reports and page-flip readiness
  projection.
- [x] Added a fake live compositor loop smoke before admitting real scanout.
- [x] Added reduced GBM/EGL frame-target readiness to backend startup and runtime
  ticks.
- [x] Added explicit runtime mutation for reduced frame-target size changes.
- [x] Defined renderer-private GBM/EGL frame-target allocation requests and
  reports.
- [x] Threaded fake allocation reports through backend-live without exposing
  renderer-private handles.
- [x] Added native GBM/EGL frame-target allocation skeletons behind existing
  GBM/EGL features.
- [x] Exposed native frame-target allocation through backend-live and runtime
  assembly as explicit caller actions.
