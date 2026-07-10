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
  automatically. The parent process has an explicit `--child-timeout-ms`
  watchdog, separate from the child process `--page-flip-timeout-ms` native
  callback wait.
- `tools/atomic_scanout_preflight.sh` records reduced host readiness without
  requesting DRM master or modesetting hardware, and proves at least one primary
  card node can be opened read/write and admit the required DRM atomic client
  capabilities, with a reduced KMS scanout target and atomic property handles,
  before the destructive smoke runs.
- `tools/verify_atomic_scanout_preflight.sh` can verify that a captured
  preflight log is ready for the DRM-master smoke.
- `tools/verify_atomic_scanout_evidence.sh` can verify a captured log offline
  against the reduced atomic scanout evidence contract.

Current local host note: `tools/atomic_scanout_preflight.sh` records exactly one
reduced line and exits nonzero with `DeviceDirectoryUnavailable` and zero
primary card counts. The local non-hardware gate passes, but the modesetting
smoke still needs a DRM-master-capable machine.

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

- [x] Made the opt-in atomic hardware smoke retain the renderer-owned GBM
  front-buffer owner with the primary-plane submission until page-flip
  retirement, matching the production runtime lifetime rule.
- [x] Threaded reduced native submit-stage diagnostics through runtime rendered
  primary-plane submit reports, including property discovery, resource creation,
  request build, and atomic commit submit status.
- [x] Added a schema-versioned
  `sophia_runtime_rendered_scanout_submit` reduced log line for runtime
  rendered-primary-plane submit reports.
- [x] Added schema-versioned runtime rendered-scanout retirement and cleanup
  reduced log lines, covering stale waits, clean retire, cleanup debt, and
  cleanup retry outcomes.
- [x] Added `tools/verify_runtime_rendered_scanout_evidence.sh` and fixtures for
  a strict clean runtime submit-to-retire evidence proof.
- [x] Added session-loop coverage proving pending decoded native page-flip
  callbacks drain under the bounded emit budget without requiring another
  reduced page-flip readiness token.
- [x] Queued reduced `Deferred` scanout lifecycle states for direct rendered
  primary-plane handoff paths, so in-flight and cleanup-blocked scanout attempts
  are delivered to the next engine tick instead of living only in submit reports.
- [x] Extended atomic scanout evidence to schema 6 with a reduced
  `page_flip_wait` field, so real hardware captures distinguish clean
  retirement from missing callbacks, callback rejection, poll failure, and
  retire failure without exposing native IDs.
- [x] Moved real atomic card/session setup failure mapping into production
  reduced evidence helpers, removing duplicated status matching from the
  hardware smoke.
- [x] Unified atomic scanout preflight and real card selection around the same
  reduced readiness probe so the non-modesetting gate cannot drift from the
  destructive selector.
- [x] Added a production runtime-tick seam on the real atomic page-flip session
  owner so selected DRM card, event reader, and routed poller ownership stays
  bundled while driving native GBM rendered-primary-plane scanout.
- [x] Switched the opt-in atomic hardware smoke from direct renderer-live
  context calls to the backend-live rendered scanout exporter seam.
- [x] Added a render-device discovery owner for selected real atomic scanout
  cards, so persistent GBM/EGL export can be built from an opaque cloned fd.
- [x] Added a production page-flip session owner that promotes a selected real
  atomic scanout card into submit-card, event-reader, and routed-poller
  ownership.
- [x] Added a production `select_real_atomic_scanout_card` seam that returns an
  opaque nonblocking DRM card owner only after reduced preflight-equivalent
  atomic scanout readiness is proven.
- [x] Switched the opt-in atomic hardware smoke away from test-local DRM card
  scanning and onto the production atomic scanout card selector.
- [x] Split native libdrm page-flip read, poll, decode, diagnostics, and fake
  poller runtime tests into their own `libdrm_events_feature` module.
- [x] Split atomic scanout evidence contract tests into their own
  `libdrm_events_feature` test-domain module.
- [x] Split the opt-in real atomic scanout hardware smoke out of the large
  `libdrm_events_feature` integration test into its own test-domain module.
- [x] Aligned the opt-in atomic hardware smoke with preflight readiness by
  choosing an atomic-scanout-ready primary card instead of the first merely
  openable card.
- [x] Added verifier coverage proving reduced `SmokeChildTimeout` evidence is
  rejected as an incomplete atomic hardware proof.
- [x] Made the opt-in atomic hardware-smoke parent emit reduced
  `SmokeChildTimeout` evidence before killing a hung child process.
- [x] Added verifier coverage proving forged passing atomic evidence from a
  blocking commit or missing page-flip-event commit flag is rejected offline.
- [x] Added verifier coverage proving forged passing atomic evidence from a
  test-only commit is rejected offline.
- [x] Added verifier coverage proving forged passing atomic evidence with
  retire-time cleanup debt is rejected offline.
- [x] Added verifier coverage proving forged passing atomic evidence with a
  `WaitingForAcceptedPageFlip` retire state is rejected offline.
- [x] Made page-flip timeout evidence preserve a reduced
  `WaitingForAcceptedPageFlip` retirement state so missing callbacks do not look
  like invisible resource drops.
- [x] Reduced additional atomic hardware-smoke setup failures, including primary
  card open, DRM client capability setup, retained-resource ownership, and
  page-flip reader setup, instead of losing evidence to raw panics.
- [x] Made the standalone atomic scanout preflight helper verify its reduced log
  and exit nonzero unless the host is ready for the DRM-master smoke.
- [x] Tightened atomic scanout preflight verification so hardware-smoke readiness
  logs must contain exactly one reduced preflight record.
- [x] Added negative atomic scanout preflight fixtures proving duplicate and
  malformed fields are rejected before the hardware smoke gate trusts host
  readiness.
- [x] Added negative atomic scanout evidence fixtures proving duplicate and
  malformed fields are rejected by the local verifier gate.
- [x] Normalized backend-live rendered scanout exports before submit so
  non-exported reports cannot carry forged descriptors or retained owners into
  the DRM path.
- [x] Hardened renderer-live native GBM scanout export reports so only reports
  with exported status can retain native buffer ownership.
- [x] Added a negative atomic scanout evidence fixture proving the shell verifier
  rejects missing rendered-context readiness.
- [x] Aligned Rust atomic scanout smoke evidence with the shell verifier so
  missing rendered-context readiness fails before GBM export can pass.
- [x] Normalized renderer-live native GBM scanout export reports so an exported
  native result without a retained valid reduced buffer degrades before
  backend-live sees it.
- [x] Hardened native primary-plane resource creation so generic DRM buffers
  with unsupported format or undersized pitch fail before mode-blob or
  framebuffer allocation.
- [x] Reused native primary-plane scanout size validation during resource
  creation so oversized selected modes fail before mode-blob or framebuffer
  allocation.
- [x] Hardened native primary-plane atomic request building so oversized source
  dimensions fail before KMS 16.16 plane properties are emitted.
- [x] Extended backend-live scanout tests so forged undersized-pitch descriptors
  fail before DRM framebuffer resource creation or atomic submit.
- [x] Hardened renderer-live scanout descriptors so XRGB8888 buffers must report
  a row pitch large enough for the target width before backend-live can import
  them for primary-plane scanout.
- [x] Made atomic scanout preflight inspect each primary DRM node through one
  ordered readiness probe, so reduced counts come from a consistent live fd
  observation instead of separate repeated opens.
- [x] Split atomic scanout preflight report, count normalization, device-node
  filtering, and live host probing into separate hardware-validation modules.
- [x] Carried reduced scanout-buffer import status through rendered primary-plane
  submit reports so runtime diagnostics match the reduced hardware evidence.
- [x] Centralized native primary-plane scanout submit result construction so
  reduced evidence fields default consistently across failure branches.
- [x] Extended atomic scanout evidence to schema 5 so passing hardware captures
  must prove the reduced scanout-buffer import status is ready before submit.
- [x] Split page-flip callback intake, callback queue draining, poller
  diagnostics, and fake callback emission into separate scanout modules.
- [x] Hardened rendered GBM scanout target validation so forged ready frame
  targets are rejected before native render-device discovery.
- [x] Centralized backend runtime tick report construction so plain and
  rendered-scanout ticks expose the same reduced evidence shape.
- [x] Split rendered primary-plane scanout target reduction and tracked runtime
  submit bookkeeping out of the one-shot submit translator.
- [x] Normalized reduced native scanout submit reports so forged ready
  renderer descriptors are reported as invalid before native DRM allocation.
- [x] Centralized renderer scanout descriptor validation so fake exports,
  native GBM/EGL exports, and backend imports share the same fail-closed
  readiness predicate.
- [x] Hardened renderer-live GBM/EGL scanout target validation so malformed
  ready frame targets with non-positive dimensions reduce to `InvalidTarget`
  before reaching native GBM/EGL allocation or export.
- [x] Split reduced atomic scanout smoke failure status by submit stage, so
  hardware captures can distinguish property discovery, resource creation,
  request build, atomic submit, and request-shape failures without native
  identity.
- [x] Split native primary-plane resource device, bundle, creation, and cleanup
  code so framebuffer and mode-blob lifetime handling is isolated from request
  construction and easier to audit before hardware smoke capture.
- [x] Split native primary-plane scanout policy, submission ownership,
  submit/retire reports, and page-flip retirement into separate modules so the
  syscall-facing submit path is easier to audit before hardware smoke capture.
- [x] Added runtime assembly coverage proving a timed-out atomic scanout commit
  remains visible after page-flip callback intake instead of becoming a generic
  rejection or committed scanout.
- [x] Split rendered primary-plane scanout ownership, submit/retire reports,
  tracked diagnostics, and backpressure types into separate modules so buffer
  lifetime code stays isolated from reduced runtime reporting.
- [x] Added a reduced `TimedOut` atomic scanout commit status so slow-client
  fail-closed commits are visible without being flattened into generic
  rejection.
- [x] Made the page-flip commit gate clear timed-out surface transactions
  without changing committed visuals, preventing a slow client from blocking
  future atomic commits indefinitely.
- [x] Replaced the page-flip commit gate's staged-batch panic with an explicit
  idle fallback so the atomic visual authority fails closed.
- [x] Removed panic-prone rendered-scanout owner retention from the page-flip
  retirement path; resource ownership now moves through explicit reduced
  branches.
- [x] Added tick-level rendered primary-plane scanout backpressure reporting
  so the production runtime can observe waiting versus stalled page-flip state.
- [x] Made deferred rendered-scanout submits update the backend's latest reduced
  scanout diagnostic state without queueing a terminal lifecycle event.
- [x] Added negative preflight verifier fixtures for impossible reduced count
  relationships and native host identity leakage.
- [x] Made opt-in atomic hardware smoke early failures print reduced evidence
  and fail with the actual reduced status instead of asserting `Passed`.
- [x] Made reduced atomic scanout smoke evidence require phase-correct commit
  flags before reporting `Passed`, matching the shell verifier contract.
- [x] Made native primary-plane scanout policy expose its required reduced
  request scope and fail closed if a built atomic request does not match it.
- [x] Added a local non-hardware atomic scanout gate that runs formatting,
  GBM/EGL scanout feature tests, backend-live scanout intake tests, and reduced
  verifier fixture checks.
- [x] Added atomic scanout verifier fixture checks so reduced preflight and
  evidence logs reject unavailable hosts, missing steady-state evidence, wrong
  steady-state request scope, and native identity leakage.
- [x] Split backend-live runtime assembly, tick reports, and tick orchestration
  into domain modules while preserving the runtime facade.
- [x] Split backend-live session-loop readiness, page-flip budget/reporting,
  loop owner, and runtime adapter code into domain modules.
- [x] Split renderer-native GBM platform into config, smoke/probe, and retained
  scanout-owner modules with explicit front-buffer lifetime notes.
- [x] Split renderer-native EGL into status, default-display, shared GL, and
  GBM-platform modules so rendered scanout export can be hardened separately.
- [x] Split renderer-live scanout data into presentation, frame-target,
  scanout-buffer, native GBM ownership, and import-boundary modules.
- [x] Split native atomic request flags/status from primary-plane property
  discovery while keeping the public DRM facade stable.
- [x] Split native KMS target selection into snapshot, device, and selection
  modules while keeping the public DRM facade stable.
- [x] Split the libinput event adapter into report, gate, device-map, native
  reader, poller, and fake-reader modules.
- [x] Made modeset resource-creation cleanup retryable when framebuffer
  registration fails after mode-blob creation.
- [x] Made submit-time framebuffer/blob cleanup failures retryable instead of
  dropping cleanup debt after atomic request-build or submit failure.
- [x] Made steady-state primary-plane page flips create framebuffer-only
  resources instead of requiring a modeset mode blob.
- [x] Split native primary-plane scanout into buffer, resource-lifetime,
  object-handle, and atomic-request domains while keeping the DRM facade stable.
- [x] Split `hardware_validation` into gate and atomic preflight domains while
  keeping the public backend-live API stable.
- [x] Extended atomic scanout evidence to schema 4 so passing hardware captures
  must prove primary-plane property discovery, native resource creation, and
  atomic request build before submit.
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
- [x] Split the real atomic scanout card owner into card, readiness, selection,
  render-device discovery, page-flip wait, and session modules so the live
  hardware-validation path is organized by domain rather than by crate facade.
- [x] Moved submitted page-flip wait and retirement out of the hardware smoke
  and into the real atomic scanout page-flip session owner.
- [x] Moved initial-modeset and steady-page-flip GBM rendered scanout proof
  phases into the real atomic scanout page-flip session owner, leaving the
  opt-in hardware smoke as evidence capture rather than pipeline ownership.
- [x] Added a feature-gated `sophia atomic-scanout-preflight` CLI command and
  moved the non-modesetting preflight helper off the cargo-test runner.
- [x] Added a feature-gated `sophia atomic-scanout-smoke` CLI parent/child
  command and moved the destructive hardware smoke helper off the cargo-test
  runner.
- [x] Added a reduced atomic scanout smoke config seam so slot, output,
  authority generation, and page-flip wait policy are explicit instead of
  hard-coded inside the proof runner.
- [x] Threaded atomic scanout smoke CLI targeting flags through the shell
  helper and child process: `--slot`, `--output`, `--authority`, and
  `--page-flip-timeout-ms`.
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
