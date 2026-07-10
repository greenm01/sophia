# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and near-term
architecture milestones. Completed milestone history lives in
`docs/roadmap-history.md`; detailed rationale and validation evidence live in
`docs/research-log.md`.

Roadmap rule: keep this file short. Move completed items older than the current
active milestone to `docs/roadmap-history.md`.

---

## Active Milestone

### Real Libinput Adapter

- [ ] Add a concrete libinput reader only behind `libinput-events`.
- [ ] Preserve the existing native-shaped reader and poller report contract.
- [ ] Avoid raw device paths, fd values, seat names, or libinput error strings in
  public runtime reports.

---

## Next 3 Milestones

### 1. Real Libdrm Event Reader

- [x] Add a concrete page-flip reader only behind `libdrm-events`.
- [x] Preserve reduced output-route decoding before runtime observation.
- [x] Keep scanout object identity private to backend-live.

### 2. Authority Probe Selection

- [ ] Choose the next real Sophia X Authority app probe after backend intake
  seams settle.
- [ ] Prefer probes that exercise bounded drawing/upload/present paths over
  broad X11 completeness.
- [ ] Keep Wayland Authority deferred until backend event intake and scanout
  timing are stable.

### 3. Real Input Loop

- [ ] Poll concrete input readers from the live runtime without blocking scanout.
- [ ] Keep physical input and routed-input transformation separate.
- [ ] Preserve deterministic queued poller tests as the default validation path.
---

## Later Backlog

- [ ] Expand Sophia X Authority only where real app probes demand it,
  prioritizing bounded drawing, upload, present, selection, and namespace
  behavior over broad X11 completeness.
- [ ] Start Sophia Wayland Authority only after live backend frame targets and
  scanout timing are stable.
- [x] Build the first real KMS atomic property-set shape from backend-private
  connector, CRTC, plane, framebuffer, and mode state.
- [ ] Run the opt-in atomic hardware smoke on a DRM-master-capable machine and
  capture `LibdrmNativeAtomicScanoutSmokeEvidence` proving rendered GBM
  front-buffer export, primary-plane atomic submit, native page-flip callback,
  and resource retirement.
- [ ] Keep the XLibre prototype docs and bridge smokes as compatibility lessons
  until Sophia X Authority has equivalent live coverage.
- [ ] Revisit wgpu only after GBM/EGL startup, drawing, presentation,
  frame-target lifecycle, and scanout seams are validated.
- [ ] Continue splitting `sophia-backend-live/src/lib.rs` by domain. Rendered
  scanout, page-flip/native DRM event plumbing, and native KMS scanout are
  extracted; next candidates are GBM/EGL probing and runtime assembly wiring.

---

## Done Recently

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
- [x] Added a backend-live runtime tick path that answers active `SubmitScanout`
  commands through rendered GBM/KMS primary-plane scanout.
- [x] Threaded backend-live terminal scanout states into the shared runtime tick
  without exposing GBM or KMS ownership.
- [x] Added backend-live tracked rendered scanout ownership so stale page-flip
  evidence keeps GBM/KMS resources in flight until accepted retirement.
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
