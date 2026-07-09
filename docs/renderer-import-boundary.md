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

The first native EGL draw smoke stops at private offscreen target readiness. It
creates a private 1x1 pbuffer target, creates a context, makes that context
current against the pbuffer, and tears everything down inside the native adapter.
It does not load GL functions, issue clear calls, compile shaders, export
buffers, or hand native handles to renderer-live.

When both `gbm-probe` and `egl-probe` are enabled, backend-live may project a
reduced GBM startup report into EGL platform status. Degraded GBM startup maps
to degraded EGL platform status; it must not become native drawing capability.
wgpu remains deferred until GBM/EGL startup, drawing, and presentation seams are
proven.

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

The next native rendering dependency must be GL function loading for the first
clear-color smoke. Admit that only after reduced draw-smoke status records exist
and continue to hide GL procedure pointers, contexts, surfaces, and native error
text.

Rejected candidates:

- `egl` 0.2.7: older low-level binding with 0% docs.rs documentation, and the
  `khronos-egl` docs describe it as left unmaintained;
- `glutin` 0.32.3: documented and useful for applications, but it is a broad
  cross-platform OpenGL context abstraction and pulls in policy beyond the
  first narrow EGL probe;
- Smithay EGL helpers: useful reference material for compositor architecture,
  but adopting Smithay would pull in a large compositor framework surface before
  Sophia has proven its own reduced GBM/EGL boundary.

## GL Function Loading Candidate

The first GL function loading candidate is `glow` 0.17.0. It is the narrowest
fit for the next smoke: load GL entry points from the current EGL context,
clear a private 1x1 pbuffer, and return only reduced smoke status to
renderer-live. Do not admit the dependency until the clear-color smoke patch
uses it.

Admission rules for GL function loading:

- keep GL loading optional under the existing `egl-probe` feature path;
- keep it inside `sophia-renderer-native-egl`, after the EGL context is current;
- load procedures only through the adapter's EGL procedure lookup path;
- do not expose `glow::Context`, GL procedure pointers, GL object names,
  shaders, textures, framebuffers, programs, native error strings, or pbuffer
  details outside the adapter;
- limit the first smoke to setting a clear color, clearing the current private
  target, and flushing or finishing as required for a deterministic reduced
  result;
- map every native GL failure to reduced draw-smoke status;
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
