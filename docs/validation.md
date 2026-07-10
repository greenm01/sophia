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
host state is unknown. That preflight drives the feature-gated
`sophia atomic-scanout-preflight` CLI command instead of a test filter. It does
not request DRM master, does not modeset hardware, and emits exactly one reduced
`sophia_atomic_scanout_preflight` line: schema version, validation target,
readiness status, capped primary card count, capped read/write-openable primary
card count, capped atomic-capability-admitted primary card count, capped KMS
scanout-target primary card count, and capped atomic-property-ready primary card
count. It does not expose device paths, file descriptors, native errors,
permissions, or KMS object identity. The helper verifies the captured log before
exiting and fails unless the host is smoke-ready. Use
`tools/verify_atomic_scanout_preflight.sh` directly when you need to check an
existing capture; the verifier requires
`CandidatePrimaryCardsAtomicReady` and at least one primary card node that
admits the `UniversalPlanes` and `Atomic` DRM client capabilities, exposes a
reduced KMS connector/CRTC/primary-plane target, and has the atomic property
handles needed for the primary-plane request. Under `libdrm-events`, preflight
and `select_real_atomic_scanout_card` use the same reduced readiness probe, so
the non-modesetting gate and the destructive selector do not drift.
Real card-selection and page-flip-session setup failures reduce themselves into
`LibdrmNativeAtomicScanoutSmokeEvidence`, so setup evidence stays consistent
outside the smoke harness as well.

Run `tools/atomic_scanout_smoke.sh` only from a session that may take DRM master
on a primary `/dev/dri/card*` node. The helper verifies preflight first, then
runs the feature-gated `sophia atomic-scanout-smoke` CLI command with
`SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1`. The CLI parent spawns a child process
for the destructive proof and emits reduced `SmokeChildTimeout` evidence if the
child fails to produce page-flip evidence within the bounded deadline. The
helper forwards optional CLI targeting arguments: `--slot`, `--output`,
`--authority`, `--page-flip-timeout-ms`, and `--child-timeout-ms`. The page-flip
timeout bounds native callback waiting inside the smoke child; the child timeout
bounds the parent watchdog around the destructive child process and defaults to
10 seconds.
Backend-live first uses the production `select_real_atomic_scanout_card` seam to
choose an opaque card owner that opens read/write, admits UniversalPlanes and
Atomic client capabilities, exposes a reduced KMS primary-plane scanout target,
and has the atomic property handles needed for primary-plane commit. The
selected card is then promoted into a page-flip session owner that keeps the
submit card, cloned event reader, and routed poller together. With
`libinput-events` enabled, that same owner can drive one backend runtime tick
through the native GBM rendered-primary-plane scanout path and native page-flip
poller, so callers do not have to split fd ownership apart. That same page-flip
session owner also owns the destructive hardware proof phases. The smoke child
creates a persistent backend-live GBM/EGL rendered-scanout exporter, then asks
the session to run `InitialModeset` and `SteadyPageFlip` proof phases. The
session clears a GBM surface, locks the rendered front buffer through the normal
runtime export seam, submits a primary-plane atomic modeset, waits for reduced
page-flip evidence, and retires the submitted framebuffer resources. It then
exports a second rendered front buffer and submits it through the steady-state
page-flip policy, proving the post-modeset path without `ALLOW_MODESET`. Each
submitted phase waits within a bounded deadline for native page-flip evidence
before reducing the final smoke record.
The real card fd is opened nonblocking, so missing callbacks reduce as missing
evidence instead of hanging inside the DRM event read.
Without verified preflight and that environment variable, the destructive path
never opens or modesets hardware.
The stable evidence shape for that run is the
`sophia_atomic_scanout_evidence` line pair: schema version, phase, overall
status, rendered context status, GBM export status, primary-plane property
discovery status, scanout-buffer import status, native resource creation status,
atomic request build status, primary-plane submit status, reduced request scope,
page-flip poll status, reduced commit flags, reduced page-flip wait outcome,
page-flip event status, retirement status, retire-time resource destroy status,
and retire-time cleanup-pending status only. A passing capture must contain both
`InitialModeset` and `SteadyPageFlip`, and both phases must report
`page_flip_wait=Retired`. Failed captures
reduce the stop point without native identity: smoke-child timeout, primary-card
open, DRM client capability setup, KMS target selection, rendered-context
creation, GBM export, retained-resource ownership, scanout-buffer import,
property discovery, resource creation, request build, atomic submit,
request-shape mismatch, page-flip reader setup, page-flip delivery,
page-flip wait state, waiting-retire state, and resource retirement are reported
separately.
Runtime rendered-primary-plane submits can also be captured as reduced
`sophia_runtime_rendered_scanout_submit` lines. Those lines are not a substitute
for the two-phase hardware smoke evidence, but they are useful when inspecting a
running production loop: they include the reduced submit status, scanout target,
frame target, GBM export, scanout-buffer validation, native submit stages,
atomic commit flags, commit submit result, runtime scanout state, and in-flight
age without exposing DRM object IDs or file descriptors.
Runtime retirement and cleanup retries can be captured as
`sophia_runtime_rendered_scanout_retire` and
`sophia_runtime_rendered_scanout_cleanup` lines. They record reduced retirement
status, destroy status, runtime scanout state, in-flight age, and cleanup debt,
so a live loop can distinguish clean retirement, stale callback waits, and
cleanup retry failures.
If the runtime proof producer cannot reach a submit-to-retire observation, it
emits `sophia_runtime_rendered_scanout_failure` with a reduced reason such as
`InitialTickFailed`, `SubmitReportMissing`, `RetireTickFailed`, or
`RetireTimedOut`. Failure lines are useful diagnostics, but they are never valid
clean proof evidence.
Use `tools/verify_runtime_rendered_scanout_evidence.sh` for a narrow clean
runtime proof. It expects exactly one submitted rendered-primary-plane scanout
line and exactly one clean retired line, rejects cleanup retry and failure
lines, and rejects unknown, duplicate, or malformed fields. This verifier proves a
single-frame runtime submit-to-retire observation; the destructive two-phase
hardware proof still comes from `tools/verify_atomic_scanout_evidence.sh`.
To capture that runtime proof on real hardware, run
`tools/runtime_rendered_scanout_evidence.sh` from a session where DRM master and
modeset disruption are acceptable. The helper runs atomic preflight, executes
the feature-gated `sophia atomic-scanout-runtime-evidence` command with
`SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1`, captures the reduced runtime evidence
log, and verifies it with `tools/verify_runtime_rendered_scanout_evidence.sh`.
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

For the current atomic scanout backend work, the local non-hardware gate is:

```sh
tools/check_atomic_scanout_local.sh
```

It runs formatting, the GBM/EGL renderer checks, the backend-live
libdrm/libinput scanout feature checks, and the reduced verifier fixture
checks. It does not request DRM master or modeset hardware.

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
runs only the opt-in atomic scanout CLI smoke:

```sh
tools/atomic_scanout_smoke.sh
SOPHIA_ATOMIC_SCANOUT_EVIDENCE=/tmp/sophia-atomic-smoke.log tools/atomic_scanout_smoke.sh
tools/atomic_scanout_smoke.sh --slot=1 --output=1 --authority=1 --page-flip-timeout-ms=2000 --child-timeout-ms=10000
```

The helper runs the verified preflight before the smoke and
`tools/verify_atomic_scanout_evidence.sh` after a successful smoke. Set
`SOPHIA_ATOMIC_SCANOUT_SKIP_PREFLIGHT=1` only when preflight is known to be
wrong for the host and a modesetting smoke is still intentional.

For the full hardware proof, prefer `tools/atomic_scanout_hardware_proof.sh`.
It runs preflight once, captures the destructive two-phase atomic evidence,
captures the runtime rendered-scanout submit-to-retire evidence, and verifies
all three reduced logs:

```sh
tools/atomic_scanout_hardware_proof.sh --slot=1 --output=1 --authority=1 --page-flip-timeout-ms=2000 --child-timeout-ms=10000
```

To verify captured logs without rerunning hardware:

```sh
tools/verify_atomic_scanout_evidence.sh /tmp/sophia-atomic-smoke.log
tools/verify_atomic_scanout_preflight.sh /tmp/sophia-atomic-scanout-preflight.log
tools/verify_runtime_rendered_scanout_evidence.sh /tmp/sophia-runtime-rendered-scanout.log
```

The verifier accepts only reduced evidence that proves a rendered GBM
front-buffer export, primary-plane property discovery, native resource
creation, atomic request build, primary-plane atomic submit, nonblocking
page-flip commit flags, native page-flip delivery, and explicit resource
retirement for both the initial modeset and steady-state page-flip phases. It
also requires the current evidence schema and rejects duplicate or unknown
fields, so a passing capture cannot smuggle native object identity into the
reduced log.

The verifier fixtures can be checked without hardware:

```sh
tools/check_atomic_scanout_verifiers.sh
```

That script proves the preflight verifier accepts only an atomic-ready reduced
host record, rejects impossible count relationships, and rejects native host
identity fields. It also proves the scanout evidence verifier rejects missing
steady-state page-flip evidence, the wrong steady-state request scope, and
native identity fields.

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
