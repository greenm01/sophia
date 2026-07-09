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

Real MIT-SHM mapping remains outside this boundary until Sophia has a bounded
shared-memory upload path with size checks, namespace validation, lifetime
tracking, and fail-closed errors.

## Failure Shape

Unsupported import paths fail closed as reduced decisions. They do not panic, do
not partially start the compositor, and do not cause protocol authorities or the
window manager to see renderer-private state. The session may still render via a
safe fallback when one exists.
