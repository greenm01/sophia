# Live Backend Dependency Policy

Sophia keeps device-facing code out of `sophia-engine`. The engine owns the
session state machine, atomic visual commits, routing decisions, and deterministic
tests. It must not own `/dev/dri`, `/dev/input`, GBM, EGL, DMA-BUF, MIT-SHM
mapping, or blocking file-descriptor polling.

Real backend dependencies belong in `sophia-backend-live` or in a later live
backend crate with the same authority boundary. That crate may translate kernel
and graphics APIs into engine records. It may not leak raw file descriptors,
device paths, client metadata, XIDs, Wayland object IDs, or namespace labels into
the window manager, portal reducers, or protocol-neutral runtime state.

## Admission Phases

Phase 0 is the current state. `sophia-backend-live` uses sysfs-style DRM/KMS
fixtures and static input descriptors. This keeps startup, failure, and assembly
tests deterministic.

Phase 1 may introduce `libdrm` and `libinput` for discovery and non-blocking
event intake. These dependencies may enumerate outputs, seats, and input
devices, and may feed reduced records into existing engine traits. They must not
introduce renderer imports, memory mapping, or protocol policy.

Phase 2 may add real KMS page-flip timing and libinput file-descriptor polling.
The output is still reduced data: output readiness, input packets, frame-clock
observations, and fail-closed health reports.

Phase 3 is the renderer import boundary. GBM, EGL, DMA-BUF, and explicit sync
fence handling stay deferred until the `sophia-renderer-live` boundary has
deterministic fake coverage for the same path. Discovery code must not grow into
buffer ownership by accident.

The first native renderer candidate is a feature-gated GBM capability probe. EGL
rendering, DMA-BUF import, and explicit sync remain later steps. Default
workspace tests must continue to run without native renderer dependencies.
The public probe shape is a backend-provided reduced render-device token, not a
device path or borrowed file descriptor. The required default and feature-enabled
local checks are listed in `docs/validation.md`.
Any concrete GBM crate must be isolated behind a renderer-live adapter module;
do not expose third-party GBM handles, errors, paths, or descriptors through
Sophia's public data model.
The selected first candidate is the safe `gbm` crate, with `gbm-sys` kept as a
fallback only if the safe crate cannot support the narrow capability probe.
Render-node discovery stays in `sophia-backend-live` for now, behind a narrow
feature-gated trait. That trait may open backend-owned fd-like authority, but it
reports only path-free discovery state such as not requested, opened, or
unavailable. If the later libdrm implementation needs broader policy, move it
behind a smaller live adapter crate before exposing more surface area.

GBM is Sophia's preferred Linux live renderer path. CPU rendering is a fallback
for absent, unavailable, or degraded GPU startup, and `GpuRequired` sessions fail
closed when the GBM path cannot prove native capability. A degraded native import
must not partially enable the import-capable renderer: Sophia either has a
native-capable startup status or it selects CPU fallback.
Native-capable means the private renderer adapter can open the backend-owned
device, verify the first render format, allocate a tiny private GBM buffer, and
drop it without exporting any GBM object.
Backend startup reports distinguish render-device discovery failure from GBM
device rejection and private allocation failure, but those reports remain
reduced. They do not expose native driver text, device paths, file descriptors,
or GBM handles.

EGL/OpenGL is the first compositor drawing API above the GBM platform boundary.
The `egl-probe` feature models reduced platform/context startup status and
admits `khronos-egl` and `glow` only through the internal
`sophia-renderer-native-egl` adapter. The adapter owns unavoidable unsafe
dynamic EGL calls and GL function loading; backend-live and renderer-live expose
only reduced startup and draw-smoke status.
The first draw smoke proves private pbuffer target readiness, `make_current`
success, GL procedure loading, clear-color execution, and teardown. It does not
admit shaders, exported buffers, presentation, GL object handles, procedure
pointers, native errors, or pbuffer details across the safe renderer/backend
boundary.
`DEFAULT_DISPLAY` is a temporary host smoke only. The production-shaped Linux
path must be GBM-backed EGL: backend-live owns render-device authority,
renderer-live proves GBM capability, and the native EGL adapter creates and
tears down the EGL display from private GBM state while exposing only reduced
platform status. The first native GBM-backed EGL platform smoke now stops at
display initialize/terminate. The first GBM-backed private target smoke creates
a private GBM surface, creates an EGL window surface from it, clears that target,
optionally crosses an offscreen presentation boundary with `eglSwapBuffers`, and
tears everything down. It does not lock front buffers, export buffers, present to
KMS scanout, or replace the `DEFAULT_DISPLAY` clear-color fallback for broad host
compatibility.

Presentation is the next renderer boundary after private drawing. The admitted
public shape is a reduced renderer-live report: ready, unavailable, or degraded.
The fake smoke covers those statuses without native dependencies. The native
offscreen smoke stages a frame, crosses a presentation-like boundary, and
returns the same reduced status without exposing GBM buffers, DRM object IDs,
DMA-BUF fds, EGLImages, fences, pixels, native errors, or GL object names. Real
scanout is deferred until that reduced shape is validated against real render
nodes.

The first scanout-adjacent backend report is still reduced data. Backend-live
may combine output discovery and renderer presentation into
`LiveScanoutReadinessReport`, but the report says only whether scanout is ready,
the output is unavailable, presentation is unavailable, or the path is degraded.
It must not expose connector IDs, CRTC IDs, plane IDs, framebuffer IDs, device
paths, fds, driver errors, or native KMS object identity. Real page flips remain
deferred until this reduced status shape is stable.

The first reduced KMS scanout target report is `LiveKmsScanoutTargetReport`.
It combines reduced output availability, reduced GBM/EGL frame-target status,
and reduced renderer presentation status. It may expose the intended target
`Size`, because size is already part of compositor geometry, but it must not
expose output IDs, connector IDs, CRTC IDs, plane IDs, framebuffer IDs, device
paths, fds, driver errors, or KMS object handles. Missing, invalid, degraded,
and unavailable target states must remain valid while scanout matures.

The first page-flip event shape is also reduced. `LivePageFlipEvent` may report
ready, idle, waiting for output, waiting for transaction readiness, presented,
rejected, output unavailable, frame-target unavailable, invalid frame target,
presentation unavailable, or degraded. Target readiness is derived from
`LiveKmsScanoutTargetReport`, so page-flip readiness cannot bypass frame-target
validation. Terminal events may carry a frame serial. They must not carry
Sophia output IDs, transaction IDs, surface IDs, connector IDs, CRTC IDs, plane
IDs, framebuffer IDs, file descriptors, native driver errors, or KMS object
handles. The future libdrm/KMS adapter must translate native page-flip callbacks
into this shape before runtime code observes them.

`LiveAtomicScanoutCommitReport` is the first reduced report for the atomic
commit seam itself. It accepts the Engine's `PageFlipCommitOutcome`, drops
transaction and surface identity, and reports only whether the commit is idle,
waiting, committed, or rejected plus the derived `LivePageFlipEvent`. A real
KMS backend should call this after page-flip completion or rejection instead of
writing page-flip state directly.
`LiveAtomicScanoutCommitter` is the backend-owned interface for that handoff.
The deterministic fake implementation records only committed count. A native
implementation may own KMS state privately, but the runtime-facing result must
remain `LiveAtomicScanoutCommitReport`.
When page-flip callbacks are available, backend-live must treat them as evidence
for the commit rather than as raw presentation state. The callback must pass the
reduced output route and monotonic frame-serial checks before
`commit_atomic_scanout_after_page_flip` can publish a committed report. Stale,
wrong-output, or frame-serial-mismatched callbacks fail closed and do not advance
committed scanout state.

`NativeLibdrmAtomicScanoutCommitter` is the first native atomic submit seam
behind `libdrm-events`. It accepts a `LibdrmNativeAtomicCommitRequest`, calls the
real `drm::control::Device::atomic_commit` API when backed by a real DRM device,
and reduces the result to submitted, would-block, or rejected. A submitted ioctl
is not a committed Sophia frame. The runtime may publish committed visual state
only after the matching reduced page-flip evidence is accepted.
If the runtime reaches `SubmitScanout` while a previous rendered primary-plane
submission is still in flight, backend-live reports reduced `Deferred` state.
That state is not a rejection and must not decrement in-flight scanout
accounting; it only records that scanout was intentionally backpressured until
page-flip evidence arrives.
The page-flip callback queue keeps only reduced accepted-callback evidence. The
rendered scanout tick may use that evidence to retire the tracked rendered
primary-plane owner before the runtime observes lifecycle state and requests the
next submit.
`LibdrmNativeAtomicCommitRequest` owns the native atomic request privately and
exposes only reduced flag facts for tests and diagnostics; framebuffer, CRTC,
plane, connector, property, and fd identity stay inside backend-live.
`build_native_primary_plane_atomic_request` is the explicit modeset builder. It
wires a connector, CRTC, primary plane, framebuffer, mode blob, and full-output
rectangle into a `drm::control::atomic::AtomicModeReq` while exposing only
built/invalid-size status. Runtime page flips use the separate plane-only
`build_native_primary_plane_page_flip_atomic_request`, so ordinary scanout does
not smuggle connector, CRTC, mode, or active properties into a non-modeset
commit. This is still not complete hardware proof until real DRM-master evidence
shows the full submit, callback, and retirement chain.
`select_native_primary_plane_target` chooses a connected connector, usable
encoder/CRTC, display mode size, and compatible primary plane through real DRM
resource APIs. It reduces failures to read-failed or missing resource groups;
the selected connector, CRTC, and plane handles remain in a backend-private
bundle until a framebuffer and mode blob are available.
`create_native_primary_plane_resources` is the resource lifecycle seam for that
bundle. It creates a mode blob, registers a scanout framebuffer from a
renderer-owned DRM buffer, validates buffer size against the selected target,
and destroys framebuffer/blob resources on retirement. It reports only reduced
create/destroy status.
Renderer-live may pass buffers to that seam through
`LiveRendererScanoutBufferDescriptor`. The descriptor carries only reduced
scanout facts: size, pitch, XRGB8888 format, and GEM handle. Backend-live
accepts only ready descriptors and wraps them in a private `drm::buffer::Buffer`
adapter before registering a framebuffer.
`NativeGbmScanoutBufferExporter` is the feature-gated owner for actual GBM
buffer objects. It delegates raw handle extraction to `sophia-renderer-native-egl`
and returns an owned buffer object plus the reduced descriptor. The owner must
outlive the backend-live framebuffer resource that was registered from the
descriptor.
For production scanout validation, the rendered export path is preferred over a
raw allocated buffer object. Renderer-native-egl creates a GBM surface with
SCANOUT|RENDERING usage, clears it through EGL, swaps, locks the front buffer,
and returns the locked XRGB8888 buffer behind the same reduced descriptor.
`discover_native_primary_plane_property_handles` resolves the required atomic
property names for that builder through the real `drm::control::Device`
property APIs, but reduces failures to read-failed or missing connector, CRTC,
or plane property groups. Property IDs remain in backend-private handle bundles
and must not appear in runtime reports.
`submit_native_primary_plane_scanout_from_renderer_descriptor` is the first
reduced end-to-end submit chain. It selects a KMS target, validates the reduced
renderer descriptor, creates framebuffer resources, builds the atomic request,
and submits it with explicit modeset permission. A successful return is only
`SubmittedWaitingForPageFlip`; the opaque submission owner must remain alive
until backend-live accepts native page-flip evidence and explicitly retires the
resources. Steady runtime scanout must instead use the policy-aware selection
submit path with page-flip policy, which keeps `ALLOW_MODESET` false and reports
the reduced commit flags.
Runtime code should use the rendered-owner wrapper instead of calling the
descriptor submit seam directly. `LiveRenderedScanoutBufferExporter` abstracts a
rendered front-buffer owner for deterministic tests and native GBM/EGL, while
`LiveRenderedPrimaryPlaneScanoutSubmission` retains that owner together with the
KMS submission owner. If page-flip evidence is stale or rejected, retirement
returns the combined owner to the caller.
The shared session runtime consumes this through a reduced `SubmitScanout`
command. The default deterministic adapter reports submitted state only; a live
backend adapter must translate rendered primary-plane submit, accepted
page-flip retirement, and rejection into reduced scanout observations before
the runtime advances beyond scanout submission.
Backend-live now exposes that first translation for rendered primary-plane
submit results: submitted-waiting-for-page-flip becomes reduced submitted
state, while frame-target, export, or primary-plane submit failure becomes
reduced rejected state. Accepted page-flip retirement maps to reduced retired
state; stale page-flip evidence produces no runtime state change because the
submission remains in flight.
The live runtime tick can now use that submit chain at the actual
`SubmitScanout` command boundary. The adapter delegates polling, layout,
rendering, portals, and chrome to the shared live runtime adapter, but overrides
only scanout submission. Native GBM/KMS work therefore happens only after a frame
has rendered and the runtime has entered `SubmittingScanout`; Engine and runtime
still observe only `RuntimeScanoutState`.
Backend-live also exposes a reusable native GBM rendered-scanout exporter that
owns `RenderDeviceDiscoveryBackend` inside the backend boundary. Runtime ticks
may hold this exporter across frames while it reports only reduced export
attempt count, context-open attempt count, context status, and export status.
The exporter initializes a persistent renderer-live GBM/EGL rendered-scanout
context on first valid use; context startup reports only ready, unavailable, or
degraded. Invalid targets fail before any render-device open attempt.
Render-device or context failure is reduced to scanout export failure and
runtime rejection; raw fds, paths, GBM handles, EGL displays, and native error
text remain private.
The preferred production tick pairs this persistent exporter with native
page-flip event intake. Backend-live reads and reduces native callbacks before
running the `SubmitScanout` phase, then retires any accepted in-flight GBM/KMS
owner before attempting the next rendered GBM export. That ordering keeps
resource lifetime explicit without exposing native object identity to Engine
state.
The tracked rendered primary-plane path keeps the combined rendered buffer owner
and KMS submission owner inside backend-live until page-flip evidence is strong
enough to retire it. This is the resource-lifetime half of atomic rendering:
the runtime can observe submitted/retired/rejected state, but it never sees or
owns GBM buffers, DRM framebuffers, property handles, or KMS submission objects.
Backend-live queues reduced terminal scanout states and drains them through the
next runtime tick. The queue carries only `RuntimeScanoutState` values; native
resource ownership stays in backend-live, and stale page-flip evidence does not
emit a runtime lifecycle state. Runtime reports may expose only reduced
in-flight tick age for a retained scanout owner, which is enough to diagnose a
missing page flip without leaking framebuffer, GBM, or KMS identity.
Backpressure classification is reduced as well: backend-live may report idle,
waiting, or stalled waiting for page-flip evidence against a caller-provided
tick threshold, but that classification must not release resources or publish a
false retirement.
Reusable native GBM rendered-scanout exporters may report only reduced
frame-target lifecycle state across exports. Created, retained, resized, and
invalidated target reports are acceptable; GBM handles, EGL displays, file
descriptors, paths, and native error strings are not.
KMS scanout target readiness must fail closed when the reduced frame-target
size differs from the selected output size. The public report may expose only a
reduced size-mismatch status and the frame-target size; it must not expose
connector, CRTC, plane, or mode object identity.
Rendered primary-plane submit tracking must consume the reduced KMS scanout
target status and reject before renderer export when that status is not ready.
The rejection may expose only the reduced target status, not native KMS object
identity. This reduced target status is mandatory for rendered submit reports;
backend-live must not allow a rendered primary-plane submit path that omits
readiness evidence.
Each tracked rendered scanout owner must carry the last reduced page-flip
sequence observed before submission. Backend-live may retire that owner only
when accepted page-flip evidence is newer than the baseline; accepted-looking
callbacks at or below the baseline must leave the owner in flight.
Accepted page-flip evidence can end the in-flight scanout state, but failed
framebuffer/blob cleanup must remain retryable. Backend-live stores the
remaining native cleanup work with the rendered buffer owner and exposes only
reduced cleanup-pending and cleanup-retry reports. Runtime tick reports and
opt-in atomic smoke evidence may carry the reduced cleanup-pending bit and
reduced retire-time destroy status, but no framebuffer, blob, or GBM identity.
When `run_tick_with_rendered_primary_plane_scanout_with` has a native device, it
may retry one pending cleanup before submitting the next rendered scanout. The
tick report may expose only the reduced cleanup retry status and whether cleanup
is still pending.
If that retry fails, the next rendered primary-plane submit must reduce to
`Deferred`/cleanup-pending rather than allocating another scanout owner.
Backend-live currently retains one cleanup owner; cleanup debt must backpressure
new submission until retry clears it.
`retire_native_primary_plane_scanout_after_page_flip` consumes that owner only
when a reduced callback report is accepted and presented. Rejected or stale
callbacks return the owner to the caller, preserving buffer and framebuffer
lifetime until a real presentation event arrives.
The opt-in atomic hardware smoke ties these seams together in a child process:
primary card open, DRM atomic client capability setup, rendered GBM front-buffer
export from the duplicated fd namespace, primary-plane submit, native page-flip
read, reduced callback validation, and resource retirement. Default validation
never opens or modesets real hardware.
When a KMS target has already been selected to size a rendered frame target,
primary-plane submit must consume that same reduced target snapshot rather than
selecting again. Reselecting after rendering can race connector, mode, or plane
state and break the atomic relation between target geometry and pixels.
Runtime rendered-primary-plane submit must also recheck the native KMS snapshot
before renderer export. A stale reduced readiness report is not enough to open
or render into a scanout target. If the native snapshot is missing or no longer
matches the reduced frame target, the submit report reduces to
scanout-target-not-ready and export remains unattempted.
When the snapshot is valid, runtime submit exports the rendered buffer and
commits the same snapshot using page-flip policy, not modeset policy.
Rendered scanout submit reports carry the reduced request scope from the native
primary-plane submit, so runtime diagnostics prove the steady-state path used a
plane-only page-flip request shape without exposing KMS identity.
`LibdrmNativeAtomicScanoutSmokeEvidence` is the reduced record for that smoke.
It reports only where the chain stopped: no primary card, KMS selection failure,
persistent rendered-context failure, KMS scanout target failure, GBM export
failure, submit failure, missing page-flip evidence, retirement failure, or
passed. The record carries only the reduced KMS scanout target status and
reduced request scope, reduced commit flags, and reduced evidence schema
version, never KMS object identity. The explicit hardware smoke passes only for
a modeset-scoped request; runtime page-flip paths are covered by separate
reduced submit tests. When retirement fails, the record also carries the
reduced resource-destroy status so diagnostics can tell an accepted page flip
from a framebuffer/blob cleanup failure. Native handles and object IDs remain
private. A reduced cleanup-pending bit says whether a retry owner still exists
after the smoke path attempts retirement.

Backend-live runtime ticks carry the current reduced scanout readiness report,
KMS scanout target report, and page-flip event beside renderer health. This
keeps the runtime-facing diagnostics useful without introducing KMS dependencies
or leaking native object identity. Native presentation and future page-flip
callbacks should update those fields through reduced reports before the next
runtime tick observes them.
The deterministic fake compositor loop smoke runs through the same runtime-owned
tick shape: poll queued input, commit protocol-neutral authority transactions,
request policy through the runtime adapter, update reduced frame-target and
scanout observations, render a CPU-backed frame, and report reduced page-flip
readiness. This is the required proof before admitting real DRM/KMS scanout into
the loop.
Runtime ticks also carry the reduced GBM/EGL frame-target observation when a
startup output is selected. That observation is only a size/status record; it is
not a GBM surface, EGL surface, framebuffer, DMA-BUF, file descriptor, or native
allocation request.
Runtime output-size changes update that observation through a size-only method.
The method may report an invalid target size, but it still must not allocate or
retain native renderer resources.
Renderer-private GBM/EGL frame-target allocation is admitted only through a
reduced allocator seam. Public reports may say ready, invalid target,
unavailable, or degraded, and may echo the reduced target record. They must not
contain GBM surfaces, EGL surfaces, framebuffers, DMA-BUFs, file descriptors, or
driver error payloads.
Backend-live may retain that reduced allocation report for the next runtime
tick, but it must clear the report whenever the frame-target size changes.
Native GBM/EGL frame-target allocation remains behind the renderer adapter. The
native skeleton accepts backend-owned device results and emits the same reduced
report shape; it must not return handles, native errors, or driver identity.
Backend-live may call that skeleton through feature-gated helpers, but the
runtime-facing observation remains the same reduced allocation report.
Runtime assembly keeps native frame-target allocation as an explicit action.
Ticks report the most recent reduced allocation result, but they do not trigger
native allocation by themselves.
Frame-target lifecycle observations follow the same rule. Runtime assembly may
report created, retained, resized, invalidated, and retired target states, but
those observations contain only reduced target size/status and lifecycle status.
Retaining a target may retain the reduced allocation report; resizing,
invalidating, or retiring clears it. A runtime tick must observe this state only.

The deterministic page-flip callback intake seam accepts only backend-local
facts: the Sophia output selected by startup and a frame serial. It rejects
callbacks for unexpected outputs and non-monotonic frame serials before updating
runtime observation. The callback report exposes only a reduced decision and
`LivePageFlipEvent`; it must not expose connector IDs, CRTC IDs, plane IDs,
framebuffer IDs, native timestamps, fds, driver errors, or KMS callback payloads.

Native page-flip callbacks enter runtime through a bounded queue. Each tick
drains at most the configured callback count, reports drained/accepted/rejected
counts, and reports queue disconnection or drain-limit pressure explicitly. It
does not allocate unbounded callback history, and queue observations retain the
same reduced shape as direct callback intake.

`FakePageFlipCallbackSource` is the deterministic stand-in for future libdrm
event polling. It emits queued callback facts through the same bounded sender,
retains unsent callbacks on backpressure or disconnection, and reports only
counts plus queue state. Real libdrm event handling should replace the source,
not the intake, queue, or reduced runtime observation contracts.
The native-shaped libdrm reader contract sits before that poller. It reads a
bounded batch of backend-local page-flip callback facts, reports only
`LibdrmNativeReadLoopReport`, and hands those facts to the existing output-route
decoder. Read failure reports reduced failure and must not drop already-pending
callbacks.

The `libdrm-events` feature defines the first libdrm page-flip polling adapter
shape without admitting a native libdrm crate. `LibdrmPageFlipEventPoller`
accepts a bounded callback sender and returns `LibdrmPageFlipEventPollReport`,
which reduces native event-loop state to idle, emitted, backpressure,
disconnected, or emit-limit-reached. The fake feature poller exercises this
shape deterministically; a later native poller must preserve the same public
report and callback queue contracts.

The first concrete libdrm candidate is Smithay's `drm` crate. It is admitted as
the preferred candidate because it is a safe, low-level DRM/KMS interface built
around caller-owned file descriptors, `Device`/`control::Device` traits, typed
KMS resources, and typed page-flip events. That matches Sophia's boundary:
backend-live owns the card fd, private adapter code handles native event
iteration, and public runtime state receives only `LivePageFlipCallback` and
reduced poll reports. Do not expose `drm::control::PageFlipEvent`, CRTC handles,
durations, device paths, fds, raw ioctls, or native errors beyond the adapter.

`drm-ffi` and `drm-sys` are not the first public dependency candidates. They may
remain transitive implementation details of `drm`, but Sophia should not depend
on them directly unless the safe crate cannot support the narrow page-flip poll
adapter. A direct FFI dependency would enlarge the unsafe audit surface before
the reduced callback path needs it.

`drm = "0.15"` is admitted as an optional `sophia-backend-live` dependency
behind `libdrm-events`. The first code path is only an admission probe that
checks the typed page-flip event is available and returns a reduced
`LibdrmDependencyAdmissionReport`. It does not open a card, poll a file
descriptor, read native events, or expose `drm` types publicly. Native polling
must still land in a small private adapter module and preserve the existing
callback queue contracts.

The first private native adapter module exists as a skeleton only:
`native_libdrm_events`. Its public surface is reduced to
`LibdrmNativeEventAdapterReport`, currently `SkeletonReady`. The module may
reference `drm::control::PageFlipEvent` privately, but it does not own a device,
open a card, register callbacks, or poll file descriptors. The next native step
must introduce backend-owned fd authority explicitly and keep all `drm` handles
inside the module.

The backend-owned fd authority shape is `LibdrmBackendFdAuthority`. It is a
generation-checked token, not a file descriptor wrapper and not a path. It may
be minted only with a nonzero generation and currently reduces to
`LibdrmBackendFdAuthorityReport { BackendOwned }`. The token gives the native
adapter a future place to receive private fd ownership without letting runtime,
WM IPC, docs, or tests depend on raw descriptors.

`native_libdrm_event_adapter_report_for_authority` proves that the private
adapter can accept backend-owned authority while remaining a non-polling
readiness seam. It consumes the token only to reduce authority into
`LibdrmNativeEventAdapterReport { SkeletonReady }`. It still does not open a
card, register callbacks, poll a file descriptor, expose a descriptor, or emit a
native event shape.

`LibdrmNativePageFlipSource::from_authority` is the first reduced source
construction seam. It is created from backend-owned authority and reports
`ConstructedWithoutPolling`. It does not implement `LibdrmPageFlipEventPoller`
yet, and it must not be wired into runtime page-flip intake until a bounded
native read loop can preserve the existing callback queue and reduced poll
report contracts.

`LibdrmNativeReadLoopReport` defines that reduced read-loop vocabulary before
real fd polling exists. Native idle and would-block states collapse to an idle
poll report, decoded callbacks become an emitted poll report with only a count,
and read failure becomes a disconnected poll report. The mapping carries no
native errno, fd, CRTC, connector, or raw event identity.
Poller diagnostics must still preserve the reduced read-loop status. A native
fd read that returns would-block may yield an idle poll report because no
callback entered runtime, but diagnostics must report `WouldBlock` so the
session loop can distinguish an empty callback queue from a real nonblocking
fd read.
The report may count rejected native callbacks separately from decoded
callbacks. Rejection counts stay inside backend-live diagnostics and do not emit
runtime callbacks.

`NativeLibdrmPageFlipEventPoller` owns the backend-local page-flip source and
the reduced callback queue handoff. It can drain injected callback batches for
deterministic tests, and it can consume a `LibdrmNativePageFlipReader` when a
native reader is available. Real polling replaces only the private read-loop
implementation, not the public queue, report, or runtime observation contracts.

Native page-flip callback decoding uses `LibdrmNativeOutputSlot` and
`LibdrmNativeOutputRoute`. The slot is backend-local routing data, not a KMS
connector id, CRTC id, fd, device path, or native event handle. A decoded native
callback may only produce `LivePageFlipCallback { output, frame_serial }`.
Unknown slots and zero frame serials fail closed before they can enter the
runtime callback queue.

`decode_native_page_flip_batch` is the bounded bridge between already
materialized native callback facts and the runtime callback queue. It decodes at
most the caller-provided limit, reports decoded and rejected counts through
`LibdrmNativeReadLoopReport`, and reports queue backpressure, disconnection, or
emit-limit state through `LibdrmPageFlipEventPollReport`. It still does not read
from a file descriptor or expose native DRM/KMS resource identity.

`NativeLibdrmPageFlipEventPoller` may drain injected callback batches through
that helper, or receive already-reduced callback facts from
`NativeLibdrmPageFlipEventReader`. This preserves queue backpressure and
retains undelivered callbacks while keeping the native fd read loop behind the
feature-gated reader.
When callbacks are already pending, `read_and_poll_page_flip_events` must try
to hand off that retained backlog before reading more native fd events. This
keeps sustained runtime-queue backpressure from growing an unbounded backend
pending list and preserves page-flip ordering.
Backend-live exposes a combined rendered-primary-plane tick helper that reads
native page-flip events, polls them into the bounded callback queue, records
reduced diagnostics, and then runs the scanout tick. This is the preferred live
path because accepted page-flip evidence is observed before retirement and
before any next scanout submit.
Decoded callback counts describe native callback facts that passed reduction
before queue handoff; emitted callback counts describe facts that actually
entered the runtime queue. If the queue is disconnected, pending injected
callbacks remain owned by the poller.
Route replacement is explicit through `NativeLibdrmPageFlipEventPoller`; hotplug
or modeset-shaped changes replace backend-local slots without exposing native
KMS identifiers. Pending callbacks are decoded against the current route table
at poll time.
Native poller diagnostics stay reduced to aggregate counts and the reduced
read-loop report. They may report route count and pending callback count, but
must not expose route slots, connector IDs, CRTC IDs, file descriptors, or other
native KMS identity.
Live runtime assemblies may carry those diagnostics through startup and tick
reports as reduced status plus counts. Engine state still receives no native
poller handles, routes, or output identity.
Native libdrm poller construction derives backend-local output slots from the
already-discovered output registry. Those slots are process-local routing
indices; they must not mirror connector IDs, CRTC IDs, fd numbers, or any other
native DRM/KMS identifier.
Native libdrm poller startup status is likewise reduced to ready/no-output or
backend-not-ready state plus route count. It is a startup health record, not a
native resource inventory.

`NativeLibdrmPageFlipEventReader` is the first concrete page-flip reader behind
`libdrm-events`. It wraps a value implementing `drm::control::Device`, calls
`receive_events()` only when the caller has chosen to read, reduces page-flip
events through a private CRTC-to-slot table, and emits only
`LibdrmNativePageFlipCallback` values. CRTC handles remain inside backend-live;
the runtime still sees only reduced output routes, frame serials, counts, and
fail-closed read status.

The `libinput-events` feature defines the native-shaped live input intake
contract and admits the safe Rust `input` wrapper as the only concrete libinput
dependency. `LiveLibinputEventReader` returns bounded `InputEventPacket`
batches plus `LibinputNativeEventReadReport`; `NativeLibinputEventPoller`
implements Sophia Engine's `NonBlockingInputPoller` contract. The public report
exposes only read status, event count, and remaining queued count.
`NativeLibinputEventReader` owns a backend-created `input::Libinput` context,
dispatches it, and reduces pointer motion, pointer button, and keyboard key
events through a caller-provided `NativeLibinputDeviceMap`. It must not expose
device paths, fd values, libinput seat names, raw device identity, or native
error strings. New libinput event kinds should extend that reduced mapper, not
the engine input adapter or runtime loop contract.
Combined runtime coverage now proves a libinput-shaped input poller can run in
the same tick as native page-flip intake, rendered scanout retirement, and the
next rendered primary-plane submit. The next production rule is readiness
ownership: the session loop may observe fd readiness and decide whether to call
the concrete reader, but Sophia Engine may see only reduced input-poll reports
and input packets.
`LiveInputReadinessGatedPoller` is the first enforcement point for that rule.
It wraps any `NonBlockingInputPoller`, exposes a one-shot readiness token, and
returns an empty batch when the token has not been observed. Polling consumes
the token and records only reduced gate status: idle, ready, polled, or
read-failed. The wrapper must not expose the fd, readiness source, seat name,
device path, or native error string.
`LiveBackendSessionLoop` is the next enforcement point. It owns the native
page-flip poller and bounded read/emit budgets, while the outer session selector
passes only reduced per-tick readiness into the live runtime.
`LiveBackendReadinessCollector` records input-ready and page-flip-ready as
one-shot booleans, then drains them into the session loop. The session-loop tick
observes the input token, calls the native page-flip reader only after reduced
page-flip readiness is present, retires rendered scanout ownership, and answers
active scanout commands through the rendered primary-plane path. It still does
not carry fds, native handles, device paths, connector IDs, seat names, or
selector backend state into Sophia Engine.

Real hardware validation for libdrm and libinput is opt-in only. The gates are
`SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE` and
`SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE`; both collapse to
`LiveHardwareValidationGateReport` with a target and skipped/requested status.
The gate report must not expose env values, device paths, file descriptors,
connector identity, seat names, or native error strings. Default validation must
remain independent of DRM device nodes and `/dev/input` devices.
`LiveHardwareValidationSmokeReport` is the next reduced layer. Until
device-opening hardware smoke is admitted, a requested real-hardware smoke
reports `BackendUnavailable` rather than opening devices. Future hardware
validation may change that status to passed or failed, but it must keep the same
reduced shape.

WebGPU/wgpu is a future compositor drawing API candidate above the Linux
platform boundary, not a replacement for GBM, DRM/KMS, or explicit scanout
authority. On Linux, wgpu will usually target Vulkan, but Sophia must first prove
GBM/EGL startup, drawing, presentation, and buffer import before admitting that
higher-level renderer dependency.
`LiveBackendDependencyKind::Wgpu` therefore fails closed for every use case until
that evidence exists. It must not enter through the generic renderer-import
admission path.

Phase 4 is the shared-memory import boundary. Real MIT-SHM mapping stays
deferred until mapped bytes can pass through a bounded renderer upload path with
namespace validation, size checks, lifetime tracking, and fail-closed errors.

## Rules

- `sophia-engine` remains dependency-neutral for kernel, GPU, and protocol IO.
- Every new live dependency must have a deterministic fixture or fake backend.
- Native renderer dependencies must have deterministic fake degraded coverage in
  `sophia-renderer-live` before real GBM, EGL, DMA-BUF, or explicit sync code is
  admitted.
- Every live failure must return a reduced status report instead of panicking or
  partially starting the session.
- Discovery, input polling, renderer import, and shared-memory import stay
  separate domains.
- No raw authority identity crosses the backend boundary. The engine receives
  Sophia IDs and reduced descriptors only.
- A dependency added only to satisfy a smoke test is rejected. The boundary must
  be useful to the session runtime.

## Required Tests

Before adding a real device or graphics dependency, Sophia needs tests proving:

- startup fails closed when the device or feature is absent;
- deterministic tests can run without `/dev/dri`, `/dev/input`, or a display;
- reduced records do not expose raw descriptors to WM IPC or portal state;
- backpressure and malformed data produce explicit degraded status;
- the new code does not change protocol authority or WM packet contracts.
