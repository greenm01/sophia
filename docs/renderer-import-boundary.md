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

The initial `gbm-probe` feature is dependency-free scaffolding. It exposes only
fake GBM capability probes, so the feature path can be tested before a real GBM
crate is admitted.

Admission tests for the first real dependency must prove:

- the crate still builds and tests offline without the feature;
- fake degraded capability coverage remains the default test path;
- absence of GBM produces reduced degraded health, not a panic;
- no raw file descriptor, device path, or renderer-private handle crosses into
  `sophia-engine`, WM IPC, portals, or protocol authorities.

## Failure Shape

Unsupported import paths fail closed as reduced decisions. They do not panic, do
not partially start the compositor, and do not cause protocol authorities or the
window manager to see renderer-private state. The session may still render via a
safe fallback when one exists.
