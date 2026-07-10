# Validation

Sophia's default validation path must not require native renderer libraries,
kernel devices, a display server, or network access. The default suite protects
the data model, protocol authorities, runtime reducers, renderer admission
records, and deterministic backend seams.
Default physical input validation uses `QueuedInputPoller`. Native libinput
coverage is feature-gated and opt-in; ordinary workspace validation must prove
physical input intake with deterministic queued packets and must not open
`/dev/input` devices.

Run before committing ordinary changes:

```sh
cargo fmt --check
cargo test --workspace --offline
```

The optional renderer-native features have extra local checks:

```sh
cargo test --offline -p sophia-renderer-live --features gbm-probe
cargo test --offline -p sophia-backend-live --features gbm-probe
cargo test --offline -p sophia-renderer-live --features egl-probe
cargo test --offline -p sophia-backend-live --features egl-probe
cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
cargo test --offline -p sophia-backend-live --features libdrm-events
cargo test --offline -p sophia-backend-live --features libinput-events
cargo test --offline -p sophia-backend-live --features libdrm-events,libinput-events
```

The `gbm-probe` feature admits the safe `gbm` crate behind an optional feature.
It exercises fake and native GBM capability records while keeping the public
boundary reduced to capability health. This command must remain optional, and
the default workspace suite must continue to pass without native renderer
feature flags.

The `egl-probe` feature admits `khronos-egl` through the internal
`sophia-renderer-native-egl` adapter crate. That crate owns the unavoidable
unsafe dynamic EGL calls. Public renderer-live and backend-live tests assert
only reduced EGL startup and draw-smoke status.

The `libdrm-events` feature admits Smithay's `drm` crate as an optional
backend-live dependency. It checks only the reduced dependency-admission report,
private native adapter skeleton, page-flip event polling adapter shape, and
deterministic fake poller that feeds the runtime-owned bounded callback queue.
Native page-flip values must be reduced before they reach runtime observation.
The native-shaped reader contract is still deterministic: tests feed reduced
native callback facts through a bounded reader before the poller decodes them
through backend-local output routes.
Real libdrm event validation is gated by
`SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE=1`. Without that variable, future hardware
smokes must return a reduced skipped report and avoid opening DRM device nodes.
Until a concrete native page-flip reader exists, the reduced smoke report fails
closed as `BackendUnavailable` when this gate is requested.

The `libinput-events` feature admits the safe Rust `input` wrapper as the
concrete libinput dependency. It defines the reduced live input event reader and
poller shape, proves that the poller implements Sophia Engine's non-blocking
input contract, and smoke-tests an empty path-based libinput context without
opening devices. The reader reduces pointer motion, pointer button, and
keyboard key events through a reduced seat/device map without changing runtime
reports.
Real libinput validation is gated by
`SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE=1`. Without that variable, future
hardware smokes must return a reduced skipped report and avoid opening input
devices or reporting device paths, seat names, file descriptors, or libinput
error strings. Until device-opening hardware smoke is admitted, the reduced
smoke report fails closed as `BackendUnavailable` when this gate is requested.

The backend-live GBM feature suite includes an opt-in real-device smoke. Set
`SOPHIA_RUN_REAL_GBM_SMOKE=1` to let the test look for an openable
`/dev/dri/renderD*` node, route that backend-owned fd-like authority through the
GBM probe, and assert only reduced startup status. Without that environment
variable, the smoke returns early. This keeps CI deterministic and avoids
letting native driver crashes fail ordinary validation.

The combined `gbm-probe,egl-probe` backend suite uses the same environment gate
for the GBM-backed EGL path. When `SOPHIA_RUN_REAL_GBM_SMOKE=1` is set and an
openable render node exists, the test requires the private GBM/EGL draw smoke to
reach `ClearColorReady` and the offscreen presentation smoke to reach `Ready`.
It still exposes no render-node path, fd, GBM object, EGL object, pixel, driver
error, or KMS identity through Sophia's public reports. The real GBM/EGL smoke
runs the native path in a child test process so a driver crash reports as an
opt-in validation failure instead of terminating ordinary deterministic tests.

The combined `libdrm-events,gbm-probe` backend suite also includes an opt-in
atomic scanout smoke. Run `tools/atomic_scanout_preflight.sh` first when the
host state is unknown. That preflight does not request DRM master, does not
modeset hardware, and emits only a reduced
`sophia_atomic_scanout_preflight` line: schema version, validation target,
readiness status, capped primary card count, and capped read/write-openable
primary card count, and capped atomic-capability-admitted primary card count.
It does not expose device paths, file descriptors, native errors, permissions,
or KMS object identity.
Use `tools/verify_atomic_scanout_preflight.sh` on the captured log when the
next step is the real smoke; the verifier requires
`CandidatePrimaryCardsAtomicCapable` and at least one primary card node that
admits the `UniversalPlanes` and `Atomic` DRM client capabilities.

Set `SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1` only from a session that may take
DRM master on a primary `/dev/dri/card*` node. The child test opens the card
read/write, enables UniversalPlanes and Atomic client capabilities, duplicates
the same fd namespace into a persistent GBM/EGL rendered-scanout context,
clears a GBM surface, locks the rendered front buffer, submits a primary-plane
atomic modeset, waits for reduced page-flip evidence, and retires the submitted
framebuffer resources. It then exports a second rendered front buffer and
submits it through the steady-state page-flip policy, proving the post-modeset
path without `ALLOW_MODESET`. Each submitted phase waits within a bounded
deadline for native page-flip evidence before reducing the final smoke record.
The real card fd is opened nonblocking, so missing callbacks reduce as missing
evidence instead of hanging inside the DRM event read.
Without that environment variable, the test returns early and never opens or
modesets hardware.
The stable evidence shape for that run is the
`sophia_atomic_scanout_evidence` line pair: schema version, phase, overall
status, rendered context status, GBM export status, primary-plane submit
status, reduced request scope, page-flip poll status, reduced commit flags,
page-flip event status, retirement status, retire-time resource destroy status,
and retire-time cleanup-pending status only. A passing capture must contain
both `InitialModeset` and `SteadyPageFlip`.
The stable evidence shape for the GBM/EGL renderer smoke is
`LiveRealGbmSmokeEvidence`: status, draw status, presentation status, and
frame-target allocation status only.

When touching renderer-native code, run both paths:

```sh
cargo test --workspace --offline
cargo test --offline -p sophia-renderer-live --features gbm-probe
cargo test --offline -p sophia-backend-live --features gbm-probe
cargo test --offline -p sophia-renderer-live --features egl-probe
cargo test --offline -p sophia-backend-live --features egl-probe
cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
cargo test --offline -p sophia-backend-live --features libdrm-events
cargo test --offline -p sophia-backend-live --features libinput-events
cargo test --offline -p sophia-backend-live --features libdrm-events,libinput-events
```

Run the opt-in local hardware smoke only when you want real render-node
coverage:

```sh
SOPHIA_RUN_REAL_GBM_SMOKE=1 cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
```

The libdrm and libinput real-hardware gates are defined before their concrete
native readers are admitted:

```sh
SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE=1 cargo test --offline -p sophia-backend-live --features libdrm-events
SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE=1 cargo test --offline -p sophia-backend-live --features libinput-events
```

Until those readers exist, these variables only document the future opt-in
shape. The deterministic feature tests must continue to pass without them.

Run the atomic scanout hardware smoke only from a local session where modeset
and DRM master disruption are acceptable. The helper captures the reduced
preflight log, verifies host readiness, captures the reduced evidence log, and
runs only the opt-in atomic scanout test:

```sh
tools/atomic_scanout_smoke.sh
SOPHIA_ATOMIC_SCANOUT_EVIDENCE=/tmp/sophia-atomic-smoke.log tools/atomic_scanout_smoke.sh
```

The helper runs `tools/verify_atomic_scanout_preflight.sh` before the smoke and
`tools/verify_atomic_scanout_evidence.sh` after a successful smoke. Set
`SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1` only when preflight is known to be
wrong for the host and a modesetting smoke is still intentional. To verify a
captured log without rerunning the hardware smoke:

```sh
tools/verify_atomic_scanout_evidence.sh /tmp/sophia-atomic-smoke.log
tools/verify_atomic_scanout_preflight.sh /tmp/sophia-atomic-scanout-preflight.log
```

The verifier accepts only reduced evidence that proves a rendered GBM
front-buffer export, primary-plane atomic submit, nonblocking page-flip commit
flags, native page-flip delivery, and explicit resource retirement for both the
initial modeset and steady-state page-flip phases. It also requires the current
evidence schema and rejects duplicate or unknown fields, so a passing capture
cannot smuggle native object identity into the reduced log.

## Retiring `DEFAULT_DISPLAY`

The `DEFAULT_DISPLAY` EGL smoke is temporary, but it is not removable merely
because the GBM-backed path exists. It can be retired only after the opt-in real
render-node validation is repeatably green and the reduced public boundary is
unchanged.

Current decision: keep `DEFAULT_DISPLAY` for now as a host compatibility smoke.
The real GBM/EGL path has passed repeated local validation on the current
machine, but one host is not enough evidence to remove a broad compatibility
check. `DEFAULT_DISPLAY` remains non-production-shaped; it must not be used as
the compositor platform boundary.

Before removing it, record evidence that:

- `SOPHIA_RUN_REAL_GBM_SMOKE=1` passes after a clean build;
- the same command passes in repeated local runs on the target development
  machine;
- the GBM-backed draw smoke reaches `ClearColorReady`;
- the offscreen presentation smoke reaches `Ready`;
- the reduced frame-target allocation smoke reaches `Ready`;
- `LiveRealGbmSmokeEvidence` records `Passed` without exposing native identity;
- driver crashes remain isolated to child-process validation failures;
- no public report exposes render-node paths, file descriptors, GBM/EGL objects,
  native errors, pixels, KMS framebuffer IDs, connector IDs, CRTC IDs, or plane
  IDs.

If any condition fails, keep `DEFAULT_DISPLAY` as a host compatibility smoke and
continue treating GBM-backed EGL as the production-shaped path under
development.

Minimum host/device matrix before retirement:

- one Intel integrated GPU machine;
- one AMD integrated or discrete GPU machine;
- one machine where `/dev/dri/renderD*` exists but GBM/EGL degrades cleanly;
- one headless or restricted environment where the real smoke is skipped or
  unavailable without failing default validation;
- repeated clean-build runs on the primary development machine.

Each matrix entry must record only reduced evidence: command, pass/fail status,
draw status, presentation status, and whether a child-process crash was
contained. Do not record render-node paths, fd numbers, GBM/EGL handles, driver
error strings, pixels, or KMS object identity.
