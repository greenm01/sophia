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
