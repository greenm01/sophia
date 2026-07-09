# Validation

Sophia's default validation path must not require native renderer libraries,
kernel devices, a display server, or network access. The default suite protects
the data model, protocol authorities, runtime reducers, renderer admission
records, and deterministic backend seams.

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

When touching renderer-native code, run both paths:

```sh
cargo test --workspace --offline
cargo test --offline -p sophia-renderer-live --features gbm-probe
cargo test --offline -p sophia-backend-live --features gbm-probe
cargo test --offline -p sophia-renderer-live --features egl-probe
cargo test --offline -p sophia-backend-live --features egl-probe
cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
cargo test --offline -p sophia-backend-live --features libdrm-events
```

Run the opt-in local hardware smoke only when you want real render-node
coverage:

```sh
SOPHIA_RUN_REAL_GBM_SMOKE=1 cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe
```

## Retiring `DEFAULT_DISPLAY`

The `DEFAULT_DISPLAY` EGL smoke is temporary, but it is not removable merely
because the GBM-backed path exists. It can be retired only after the opt-in real
render-node validation is repeatably green and the reduced public boundary is
unchanged.

Before removing it, record evidence that:

- `SOPHIA_RUN_REAL_GBM_SMOKE=1` passes after a clean build;
- the same command passes in repeated local runs on the target development
  machine;
- the GBM-backed draw smoke reaches `ClearColorReady`;
- the offscreen presentation smoke reaches `Ready`;
- driver crashes remain isolated to child-process validation failures;
- no public report exposes render-node paths, file descriptors, GBM/EGL objects,
  native errors, pixels, KMS framebuffer IDs, connector IDs, CRTC IDs, or plane
  IDs.

If any condition fails, keep `DEFAULT_DISPLAY` as a host compatibility smoke and
continue treating GBM-backed EGL as the production-shaped path under
development.
