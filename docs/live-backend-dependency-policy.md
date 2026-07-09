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

The first page-flip event shape is also reduced. `LivePageFlipEvent` may report
ready, idle, waiting for output, waiting for transaction readiness, presented,
rejected, output unavailable, presentation unavailable, or degraded. Terminal
events may carry a frame serial. They must not carry Sophia output IDs,
transaction IDs, surface IDs, connector IDs, CRTC IDs, plane IDs, framebuffer
IDs, file descriptors, native driver errors, or KMS object handles. The future
libdrm/KMS adapter must translate native page-flip callbacks into this shape
before runtime code observes them.

Backend-live runtime ticks carry the current reduced scanout readiness report
and page-flip event beside renderer health. This keeps the runtime-facing
diagnostics useful without introducing KMS dependencies or leaking native object
identity. Native presentation and future page-flip callbacks should update those
fields through reduced reports before the next runtime tick observes them.

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

`NativeLibdrmPageFlipEventPoller` is a non-polling skeleton over
`LibdrmNativePageFlipSource`. It implements the same `LibdrmPageFlipEventPoller`
trait as the deterministic fake, but currently returns the idle reduced report
and emits no callbacks. Real polling must replace only the private read-loop
implementation, not the public queue, report, or runtime observation contracts.

Native page-flip callback decoding uses `LibdrmNativeOutputSlot` and
`LibdrmNativeOutputRoute`. The slot is backend-local routing data, not a KMS
connector id, CRTC id, fd, device path, or native event handle. A decoded native
callback may only produce `LivePageFlipCallback { output, frame_serial }`.
Unknown slots and zero frame serials fail closed before they can enter the
runtime callback queue.

WebGPU/wgpu is a future compositor drawing API candidate above the Linux
platform boundary, not a replacement for GBM, DRM/KMS, or explicit scanout
authority. On Linux, wgpu will usually target Vulkan, but Sophia must first prove
GBM/EGL startup, drawing, presentation, and buffer import before admitting that
higher-level renderer dependency.

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
