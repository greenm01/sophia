# Renderer Import Boundary

The renderer import boundary is separate from backend discovery. Output
discovery answers "what displays exist?" Input discovery answers "what devices
exist?" Renderer import answers "can this already-validated surface buffer become
a renderer-private texture for this frame?"

Sophia keeps those questions apart. A backend may discover `/dev/dri` and input
devices without owning GBM, EGL, DMA-BUF import, or MIT-SHM mapping. The renderer
boundary is admitted only after the engine has a ready `SurfaceTransaction` with
matching geometry and buffer identity.

## Ownership

The engine owns:

- atomic validation of geometry and buffer readiness;
- committed visual state;
- frame planning and render reports;
- protocol-neutral `BufferSource` and `BufferImportPath` values.

The live renderer boundary owns:

- deciding whether a source can use a native import path;
- falling back or deferring when the path is not available;
- keeping renderer-private handles out of WM IPC, portals, and protocol
  authorities;
- reporting the reduced import outcome for tests and runtime observations.

The live renderer boundary does not own:

- output discovery;
- input polling;
- protocol authority parsing;
- X11, Wayland, or namespace policy;
- client metadata.

## Current Admission Rule

CPU-backed uploads are the only always-accepted path. `XPixmap` and `DmaBuf`
sources are reduced records today, not proof that a real GPU import path exists.
They stay deferred unless the live renderer boundary explicitly declares support.
Live backend startup therefore defaults to CPU fallback. Native import-capable
rendering must be selected through startup configuration; it is not implied by
discovering a DRM/KMS output.

Startup reports expose only reduced renderer import health: CPU fallback, native
import capable, or degraded. Per-path status is reduced to disabled, enabled, or
degraded for XPixmap and DMA-BUF. No renderer-private handle, file descriptor,
device path, or client buffer identity belongs in that health report.

`sophia-backend-live` consumes that startup health when it builds its live
runtime assembly wrapper. Each tick report can carry the same reduced renderer
observation beside the engine's protocol-neutral tick report. The engine remains
free of renderer-live dependencies, and runtime consumers still learn whether
the session is using CPU fallback or a native import-capable renderer.

Degraded renderer health has two sources. Startup capability probes can mark a
path degraded before the session starts, and per-frame import failures can
degrade runtime observation after startup. Both are modeled through deterministic
fake paths in `sophia-renderer-live`, which lets Sophia exercise reduced failure
shape before adding GBM, EGL, DMA-BUF, explicit sync, or renderer-private
resource caches.

Real MIT-SHM mapping remains outside this boundary until Sophia has a bounded
shared-memory upload path with size checks, namespace validation, lifetime
tracking, and fail-closed errors.

The first real renderer implementation lives behind the `sophia-renderer-live`
crate boundary. Today that crate has no GBM, EGL, DMA-BUF, MIT-SHM, or explicit
sync dependencies; it only models reduced import admission, startup health, and
runtime observation shape. Future renderer-private resource caches and native
imports should land there, while `sophia-backend-live` remains the session
assembly boundary that wires discovery, input, renderer admission, and startup
health together.

## Native Dependency Admission

The live runtime wrapper stays outside `sophia-engine`. That is a deliberate
boundary: the engine owns protocol-neutral state and deterministic frame
validation, while backend-live and renderer-live own live startup health and
renderer capability facts. Move renderer policy into the engine only if atomic
visual correctness requires it.

The first real native renderer dependency candidate is a GBM capability probe,
not full EGL rendering and not DMA-BUF import. GBM is the smallest useful probe
because it can establish whether the live renderer can speak to a DRM render
device and create renderer-private allocation context. That probe must be gated
behind an optional crate feature and must not be required for the default
workspace test suite.

The initial `gbm-probe` feature admits only the safe `gbm` crate and keeps it
behind an optional feature. It exposes fake probes for deterministic default
coverage and native reduced probes for feature-enabled validation.

The GBM probe API uses a backend-provided reduced render-device token. It does
not accept a public device path, and it does not expose borrowed file descriptors
through Sophia's stable data boundary. Backend-live may own the real device
opening later; renderer-live receives only the reduced token needed to report
capability health. This keeps raw kernel authority out of engine state, WM IPC,
portals, and protocol authorities.

The feature-enabled local validation command is documented in
`docs/validation.md`. The GBM crate is admitted only through that path. Real
device probing is available only through backend-owned fd-like authority. It
still collapses native failures into reduced degraded health and does not expose
fds, paths, GBM handles, or native errors through Sophia's public data model.
Backend-live reports render-device discovery separately as reduced path-free
state so startup diagnostics can distinguish "not requested" from "unavailable"
without leaking device identity.

Backend-live also reports a reduced GPU startup status. This status is more
specific than renderer health, but still contains no path, fd, handle, or driver
message. It can distinguish:

- GPU startup was not requested;
- the backend could not open a render device;
- GBM rejected the opened device;
- the opened GBM device could not allocate the first private renderer buffer;
- native GPU startup is capable.

Renderer startup policy is explicit:

- `GpuPreferred` attempts GBM and selects CPU fallback when the probe degrades;
- `CpuOnly` never opens a render device;
- `GpuRequired` fails closed when GBM does not prove native capability.

Degraded GBM does not produce a partial import-capable renderer. This keeps the
atomic visual path honest: native import is either capable, or the session runs
through the fallback renderer with reduced degraded startup health.

Native GBM capability requires more than opening a GBM device or checking a
format flag. The probe must also allocate and immediately drop a tiny
renderer-private buffer. The allocation result is reduced to startup health; the
buffer object, handle, driver error, and fd never cross the adapter boundary.

Admission tests for the first real dependency must prove:

- the crate still builds and tests offline without the feature;
- fake degraded capability coverage remains the default test path;
- absence of GBM produces reduced degraded health, not a panic;
- failed private GBM allocation produces reduced degraded health;
- no raw file descriptor, device path, or renderer-private handle crosses into
  `sophia-engine`, WM IPC, portals, or protocol authorities.

Sophia should not expose a third-party GBM crate directly through its public
renderer-live API. The native binding belongs behind a tiny adapter module that
translates from reduced Sophia tokens into reduced capability health. This keeps
crate-specific handles, error types, lifetime rules, and unsafe requirements
contained inside renderer-live.

The adapter module owns:

- native GBM crate imports behind the `gbm-probe` feature;
- fake GBM capability probes for deterministic default-style coverage;
- reduced native GBM capability probes for feature-enabled validation.
- tiny private allocation probes that are dropped before returning reduced
  health.

The adapter module may later own:

- conversion from backend-owned device authority into renderer-private probe
  context;
- reduced degraded-health mapping for missing devices, unsupported GBM
  operations, or native probe errors.

It must not export:

- raw GBM handles;
- raw file descriptors;
- device paths;
- native error payloads;
- renderer-private allocation objects.

## Candidate Dependency

The first admitted candidate is the safe `gbm` crate, currently documented as `gbm`
0.18.0 on docs.rs. It wraps `libgbm`, documents itself as safe GBM bindings, and
depends on `gbm-sys`. Its optional `drm-support` feature is useful later, but
the first Sophia probe should not require it. The initial probe only needs to
prove that renderer-live can compile the native GBM boundary and translate a
reduced render-device token or backend-owned device authority into reduced GBM
capability health. Capability includes a private allocation check, not just
device construction. Backend-live wires that authority through reduced startup
reports and renderer preference policy.

`gbm-sys` is not the first choice. It exposes raw FFI functions and low-level GBM
types directly. Keep it as a fallback only if the safe `gbm` crate cannot support
the narrow capability probe without pulling in broader rendering or DRM policy.

Admission notes for the admitted `gbm` dependency:

- keep it only as an optional dependency under the existing `gbm-probe` feature;
- keep `default = []`;
- keep all native GBM calls inside a private adapter module;
- map native errors to reduced degraded health;
- keep the fake probe tests as the default path;
- run `cargo test --workspace --offline` without feature flags;
- run `cargo test --offline -p sophia-renderer-live --features gbm-probe`
  separately for native probe changes;
- run `cargo test --offline -p sophia-backend-live --features gbm-probe`
  separately for backend-owned device discovery changes.

## EGL Context Boundary

EGL/OpenGL is Sophia's first compositor drawing API above the GBM platform
boundary. GBM remains responsible for render-device authority and allocation
capability; EGL is only the context/drawing layer that sits on top of a proven
platform.

The first `egl-probe` feature adds fake reduced records for platform readiness
and context readiness, then projects those records through backend-live as
reduced startup status. It also admits a native dynamic EGL probe behind an
internal adapter crate. The public renderer-live and backend-live APIs still
expose only reduced startup records.

The EGL probe boundary must not expose:

- EGL displays, contexts, configs, or surfaces;
- GBM devices, buffers, handles, paths, or file descriptors;
- native driver error text;
- renderer-private allocation objects.

The first native EGL draw smoke proves a private clear-color path. It creates a
private 1x1 pbuffer target, creates a context, makes that context current
against the pbuffer, loads GL procedures inside the adapter, clears the target,
and tears everything down inside the native adapter. It does not compile
shaders, export buffers, present frames, or hand native handles to
renderer-live.

When both `gbm-probe` and `egl-probe` are enabled, backend-live may project a
reduced GBM startup report into EGL platform status. Degraded GBM startup maps
to degraded EGL platform status; it must not become native drawing capability.
wgpu remains deferred until GBM/EGL startup, drawing, and presentation seams are
proven.

## GBM-Backed EGL Platform Candidate

The current native EGL clear-color smoke uses `DEFAULT_DISPLAY` only as a
temporary platform probe. The first production-shaped Linux path must move EGL
onto backend-owned GBM authority: backend-live discovers and opens the render
device, renderer-live proves GBM capability with a reduced token or owned fd,
and the native EGL adapter creates an EGL display from the private GBM device.

This is a platform handoff, not a public handle handoff. The Engine, WM,
protocol authorities, and safe renderer-live API must still see only reduced
status. They must not see the GBM device, EGL display, driver path, fd, surface,
native visual, or driver error text.

The admitted reduced platform shape is `LiveGbmBackedEglPlatformReport`. It
projects GBM startup into EGL platform readiness without exposing the render
device or EGL objects. The native platform smoke proves that backend-owned
device authority can become a private GBM device, that EGL can create and
initialize a GBM platform display from that private state, and that teardown can
complete without exposing the display.

The GBM-backed private target smoke now reuses that private platform to create a
1x1 GBM surface, create an EGL window surface from the private GBM surface, make
an OpenGL context current, clear the target, optionally cross an offscreen
presentation boundary with `eglSwapBuffers`, and tear everything down. It
returns either the existing reduced `EglDrawSmokeReport` shape or the reduced
`LiveRendererPresentationReport` shape. It does not lock a GBM front buffer,
export a DMA-BUF, create a KMS framebuffer, or present to scanout.
The path is:

1. Backend-live discovers a render node and holds the opened device authority.
2. Renderer-live proves GBM native capability with a private allocation smoke.
3. The native EGL adapter receives only backend-owned authority, creates a GBM
   platform display, initializes it, chooses a window-capable config, creates a
   private GBM/EGL target, clears it, optionally swaps it through the offscreen
   presentation smoke, and tears it down.
4. Buffer export, front-buffer locking, and scanout stay out of scope until the
   next boundary admits them.

Admission rules for the GBM-backed EGL platform:

- keep it behind the combined `gbm-probe,egl-probe` feature path;
- require GBM native capability before attempting GBM-backed EGL;
- return reduced platform status only: ready, unavailable, degraded, or context
  unavailable;
- keep all fd ownership, GBM devices, EGL displays, configs, surfaces, native
  error text, and driver details inside live adapters;
- do not expose DMA-BUF, KMS framebuffer IDs, GBM buffer handles, EGLImages, or
  GL object names;
- keep `DEFAULT_DISPLAY` as a fallback host smoke until GBM-backed drawing is
  exercised against real render nodes in validation.

`DEFAULT_DISPLAY` retirement requires evidence, not intent. Sophia may remove
the fallback host smoke only after all of these are true:

- `SOPHIA_RUN_REAL_GBM_SMOKE=1` passes the GBM capability, GBM-backed EGL draw,
  and offscreen presentation smokes on a real render node;
- the same command remains green for repeated local runs after a clean build;
- native driver crashes stay isolated to the child smoke process and report as
  opt-in validation failures;
- public reports still expose only reduced GBM, EGL, presentation, and scanout
  readiness status;
- the reduced scanout-adjacent readiness report has a successor page-flip event
  shape that does not expose KMS object identity.

Until then, `DEFAULT_DISPLAY` remains a compatibility smoke. It is not the
production compositor platform, but it proves the dynamic EGL and GL loading
path on machines where backend-owned GBM/EGL cannot yet create a stable private
target.

Decision after the first repeated real GBM/EGL validation pass: keep
`DEFAULT_DISPLAY` as a compatibility smoke. The production-shaped renderer path
is GBM-backed EGL, but `DEFAULT_DISPLAY` still catches host EGL loader issues on
machines where backend-owned render-node validation is unavailable or not yet
trusted across a broader device matrix.

Retirement requires at least Intel, AMD, degraded-driver, and restricted/headless
host evidence. Each result must use the reduced real-GBM evidence shape; device
paths, handles, driver errors, pixels, and KMS identity stay out of project
records.

Rejected shortcuts:

- using `DEFAULT_DISPLAY` as the production compositor platform: it does not
  prove Sophia controls the backend-owned render device;
- exporting a GBM handle to renderer-live so safe code can assemble EGL state:
  that would leak renderer-private authority across the boundary;
- moving directly to DMA-BUF or scanout before the reduced presentation status
  exists.

## Presentation Smoke Boundary

Presentation is the next boundary after private drawing. A presentation smoke
must prove that renderer-live can advance a frame to a reduced presentation
status without exposing the GPU objects that made it happen.

The first native presentation smoke is not real KMS scanout. It is a
renderer-owned, offscreen GBM/EGL boundary that models the final
PageFlipCommitGate shape: a frame is staged, `eglSwapBuffers` reaches the
presentation boundary, and the public result says only whether presentation was
ready, unavailable, or degraded.

The presentation smoke must not expose:

- GBM buffer handles or buffer object pointers;
- DRM framebuffer IDs, connector IDs, CRTC IDs, plane IDs, or device paths;
- DMA-BUF fds, modifiers, fences, or EGLImages;
- native driver errors or GL object names;
- frame pixels or readback data.

The admitted public shape is `LiveRendererPresentationReport` with only three
statuses: ready, unavailable, or degraded. The fake presentation smoke exercises
those statuses without native dependencies. The native offscreen smoke preserves
the same shape while keeping GBM surfaces, EGL surfaces, driver errors, pixels,
and GL object names private. Only after this reduced shape holds under real
render-node validation should Sophia admit scanout-facing code. This keeps the
macOS-style invariant intact: the Engine can learn that a presentation boundary
happened, but it cannot accidentally present partially assembled native state
through a leaky renderer API.

The backend may project presentation into a scanout-adjacent readiness report,
but that report stays outside renderer-private state. `LiveScanoutReadinessReport`
is reduced to ready, output unavailable, presentation unavailable, or degraded.
It is not a KMS page-flip result and must not contain connector IDs, CRTC IDs,
plane IDs, framebuffer IDs, fds, paths, driver errors, or framebuffer handles.

Real GBM-backed validation evidence is reduced through
`LiveRealGbmSmokeEvidence`. It records only whether the opt-in smoke passed, the
reduced EGL draw-smoke status, and the reduced presentation status. It does not
store the render-node path, fd, GBM/EGL objects, driver errors, pixels, or KMS
identity.

The next production-shaped step after offscreen presentation evidence is a
reduced GBM/EGL frame-target record. `LiveGbmEglFrameTargetRecord` records only
the intended target size and whether that size is valid. It is deliberately not
a GBM surface, EGL surface, framebuffer, DMA-BUF, or scanout object. Future
renderer integration can use it to agree on target dimensions before native
allocation code exists.
Backend-live may project the selected startup output into this record because
the projection contains no renderer-private authority. That projection is a
startup readiness hint, not a native allocation request.
Backend-live runtime may also update the record from an output-size change. The
update accepts only `Size`, derives the reduced status again, and does not own
or borrow native renderer resources.

The first allocation seam is `LiveGbmEglFrameTargetAllocator`. It accepts a
`LiveGbmEglFrameTargetAllocationRequest` and returns only
`LiveGbmEglFrameTargetAllocationReport`: reduced allocation status plus the
reduced target record. Implementations may allocate native GBM/EGL resources
privately, but those handles must not appear in renderer-live, backend-live, or
runtime observations.
Backend-live observes this seam by invoking the allocator with its current
reduced target and storing only the returned reduced report. A target-size
change invalidates the retained report.
The first native skeleton is `NativeGbmBackedEglFrameTargetAllocator`, gated
behind the existing GBM/EGL probe features. It accepts a backend-owned render
device result and a reduced allocation request, performs any native work inside
the renderer adapter, and returns only the reduced allocation report.
Backend-live exposes this through feature-gated helpers so startup and runtime
code can test the native seam without importing renderer-private handles.
`LiveBackendRuntimeAssembly` may invoke the native seam explicitly and retain
only the reduced allocation report. A runtime tick must not allocate native
frame targets implicitly.

The next reduced boundary is `LivePageFlipEvent`. It can be derived from
scanout readiness or from the engine's `PageFlipCommitOutcome`, but it drops the
output ID, transaction ID, surface IDs, commit payload, and all native KMS
identity. Presented and rejected events retain only the frame serial needed to
relate the event to the compositor timeline.

## EGL Candidate Dependency

The admitted native EGL dependency is `khronos-egl` 6.0.0. It binds the Khronos
EGL 1.5 API, exposes an explicit `Instance` API, and supports dynamic loading
through its `dynamic` feature. Sophia admits it only inside
`sophia-renderer-native-egl`, a tiny internal adapter crate that owns the
unavoidable unsafe FFI calls. `sophia-renderer-live` depends on that adapter
only when `egl-probe` is enabled.

Admission rules for `khronos-egl`:

- keep it optional under the existing `egl-probe` feature path;
- use dynamic loading so missing `libEGL.so.1` becomes reduced startup
  failure instead of a hard link/load failure;
- keep all EGL displays, contexts, configs, surfaces, errors, and loaded
  library handles private to the native adapter;
- project native failures only to reduced EGL startup status;
- keep fake EGL tests as the default feature-enabled coverage path;
- run `cargo test --workspace --offline` without feature flags;
- run `cargo test --offline -p sophia-renderer-live --features egl-probe`;
- run `cargo test --offline -p sophia-backend-live --features egl-probe`;
- run `cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe`.

Rejected candidates:

- `egl` 0.2.7: older low-level binding with 0% docs.rs documentation, and the
  `khronos-egl` docs describe it as left unmaintained;
- `glutin` 0.32.3: documented and useful for applications, but it is a broad
  cross-platform OpenGL context abstraction and pulls in policy beyond the
  first narrow EGL probe;
- Smithay EGL helpers: useful reference material for compositor architecture,
  but adopting Smithay would pull in a large compositor framework surface before
  Sophia has proven its own reduced GBM/EGL boundary.

## GL Function Loading Dependency

The admitted GL function loading dependency is `glow` 0.17.0. Sophia admits it
only inside `sophia-renderer-native-egl` for the first clear-color smoke: load
GL entry points from the current EGL context, clear a private 1x1 pbuffer, and
return only reduced smoke status to renderer-live.

Admission rules for `glow`:

- keep GL loading optional under the existing `egl-probe` feature path;
- keep it inside `sophia-renderer-native-egl`, after the EGL context is current;
- load procedures only through the adapter's EGL procedure lookup path;
- do not expose `glow::Context`, GL procedure pointers, GL object names,
  shaders, textures, framebuffers, programs, native error strings, or pbuffer
  details outside the adapter;
- limit the first smoke to setting a clear color, clearing the current private
  target, and flushing or finishing as required for a deterministic reduced
  result;
- map every native GL failure to reduced `GlUnavailable` draw-smoke status;
- keep wgpu deferred until GBM/EGL startup, drawing, and presentation seams are
  proven.

Rejected GL-loading candidates for the first smoke:

- `gl` 0.14.0: its global `load_with` model and generated unsafe function
  surface are broader than the current adapter needs;
- `gl_generator` 0.14.0: generated bindings may be useful later, but they add a
  build-time binding surface before Sophia needs it;
- raw manual `eglGetProcAddress` calls: smallest in dependency count, but they
  would hand-roll procedure pointer safety and duplicate a tested loader
  boundary.

## Failure Shape

Unsupported import paths fail closed as reduced decisions. They do not panic, do
not partially start the compositor, and do not cause protocol authorities or the
window manager to see renderer-private state. The session may still render via a
safe fallback when one exists.
