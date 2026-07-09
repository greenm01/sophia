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

Renderer feature scaffolding has one extra local check:

```sh
cargo test --offline -p sophia-renderer-live --features gbm-probe
```

The `gbm-probe` feature is currently dependency-free. It exercises fake GBM
capability records and reduced degraded-health behavior before a real GBM crate
is admitted. Once real native renderer dependencies are introduced, this command
must remain optional and the default workspace suite must continue to pass
without feature flags.

Before admitting a real native renderer dependency, run both paths:

```sh
cargo test --workspace --offline
cargo test --offline -p sophia-renderer-live --features gbm-probe
```
