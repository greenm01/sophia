# Style Guide

This guide defines implementation discipline for Sophia. It is intentionally
small because the project is still a research prototype. The rules here defend
the data model in `dod.md`.

## Languages

Sophia user-space components are Rust by default.

- Rust: Sophia Engine, protocol authorities, portals, CLI tools, reference WM,
  and compatibility/prototype bridges.
- C: narrow XLibre prototype patches and protocol extensions.
- Nim: optional experimental WMs or policy prototypes.
- Zig: optional probes or small C-adjacent helpers, not the main architecture.

Do not mix languages inside one component without a concrete boundary reason.

## Rust Layout

Prefer subsystem directories over large files:

```text
src/
  types/
  state/
  protocol/
  systems/
  authority/
  bridge/
  portal/
  wm/
  engine/
```

The same data/logic split applies in Rust:

- `types` contains passive records, IDs, enums, and flags.
- `state` owns tables and lifetimes.
- `protocol` serializes packets and validates wire data.
- `systems` transforms data.
- `authority` terminates a client protocol and owns protocol resources.
- `bridge` talks to legacy or external prototype authorities such as XLibre.
- `portal` owns cross-namespace transfer policy.
- `engine` owns compositor state and hot-path scheduling.
- `wm` owns policy examples.

Avoid placing behavior on data records unless it is a pure helper such as
validation, conversion, or formatting.

## Test Placement

Rust tests live outside production source files.

Use crate-level integration tests:

```text
crates/<crate>/tests/
  behavior.rs
  support/
    mod.rs
```

Rules:

- Do not add `#[cfg(test)] mod tests` to files under `src/`.
- Do not add `#[test]` functions to files under `src/`.
- Put shared fixtures, builders, and mock data under `tests/support/` or inside
  the integration test module that uses them.
- Test through public APIs. Do not make private helpers public only so tests can
  reach them.
- If a private invariant truly cannot be tested through public behavior, record
  the exception in this guide before adding an inline test.

This keeps production modules readable and forces tests to exercise the same
crate boundary that downstream Sophia components use.

## TEA Policy Style

Use TEA-style structure for policy components:

```text
model + event/snapshot -> update -> command
```

Good fits:

- Sophia WM layout, workspace, and focus policy.
- Portal transfer policy.
- Session or launcher policy hints.

Poor fits:

- compositor hit-testing
- frame scheduling
- damage aggregation
- renderer/backend execution
- protocol event mirroring

For TEA-style modules, keep update functions deterministic where practical.
They should consume passive packets and emit command packets. They should not
reach into compositor, XLibre, or portal-owned state through callbacks.

For compositor code, prefer explicit data-oriented systems over a global message
loop. The engine is a security boundary and a hot path; clarity of authority,
bounded allocation, and predictable control flow matter more than architectural
uniformity.

## Naming

Use ordinary Rust naming:

- Types and traits: `PascalCase`
- Functions and variables: `snake_case`
- Modules: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`

IDs should make ownership clear:

- `SurfaceId`, not `WindowId`
- `XWindowId`, not raw `u32`
- `NamespaceId`, not raw string in hot paths
- `TransactionId`, not `Serial` unless it is truly protocol-local

Raw protocol IDs should be wrapped at the boundary where they enter Sophia.

## Ownership

State has one owner. Other components receive snapshots, IDs, or handles.

Prefer:

- dense tables with typed IDs
- generation checks for long-lived references
- immutable snapshots across process boundaries
- explicit handle ownership

Avoid:

- global registries with mutable aliases
- shared object graphs
- callbacks that mutate state hidden behind another component
- stringly typed IDs in hot paths

## Errors

Errors should name the boundary that failed.

Good examples:

- `XBridgeError::BadWindow`
- `InputRouteError::StaleSurface`
- `PortalError::PolicyDenied`
- `TransactionError::TimedOut`

Policy denial is not an internal error. Treat it as an expected outcome with a
clear status.

## Logging

Sophia libraries use `tracing` for structured diagnostics. Binaries and runtime
entrypoints install subscribers; libraries do not.

Default logs must not expose sandbox-sensitive identity or payload data:

- no raw XIDs, namespace IDs, window titles, app classes, PIDs, or icon pixels;
- no clipboard, drag-and-drop, file, URI, notification body, or pixel payloads;
- no raw portal payload handles unless the handle is explicitly opaque and
  already user-approved for that log context.

Prefer opaque Sophia IDs, generations, counts, enum outcomes, and durations.
Engine logs should describe compositor/session decisions, not user data.

Levels:

- `trace`: per-layer, per-command, or hot-path counters.
- `debug`: normal state transitions and accepted reducer outcomes.
- `warn`: rejected, stale, invalid, timed-out, or fallback outcomes that are
  expected but security-relevant.
- `error`: only when a library cannot return the failure to its caller. Most
  engine failures should be returned as typed errors instead.

## Allocation

The compositor hot path should not allocate casually. It may resize capacity at
controlled boundaries, but input processing and frame planning should reuse
buffers where practical.

Allowed edge allocations:

- connecting to a protocol authority or prototype server
- discovering outputs
- creating or destroying surfaces
- resizing dense tables
- capturing test snapshots
- portal transfer setup

Suspicious allocations:

- every input event
- every damage region merge
- every frame for stable surface lists
- every hit-test walk

## Protocol Authorities

Protocol authorities are compatibility boundaries. They own protocol parsing,
client resource tables, protocol-local IDs, focus/grab/selection semantics,
configure/commit state, and namespace enforcement for their clients.

Authority code must not own:

- physical input devices;
- compositor scene graph or scanout;
- workspace/layout policy;
- compositor chrome;
- cross-namespace portal policy.

Authorities emit bounded surface transactions, sanitized metadata candidates,
portal requests, and lifecycle facts. Sophia Engine decides whether a visual
transaction is committed, delayed, rejected, or timed out.

## XLibre Prototype Patches

XLibre changes are C and should stay narrow. They are prototype and research
work, not the long-term center of the architecture.

Patch goals:

- add explicit protocol seams;
- preserve X11 semantics;
- keep access control auditable;
- make changes upstreamable.

Avoid server patches that make Sophia the only possible compositor. XLibre
should gain a useful extension, not a private dependency.

## Verification

Docs-only changes need inspection, not a build.

For code, each component needs a concrete check:

- Rust units for packet validation and table invariants.
- Integration tests for XLibre bridge behavior.
- Headless compositor tests for frame plans.
- Portal tests for allow, deny, revoke, and stale-transfer cases.
- XLibre protocol tests for new extension behavior.

When a test cannot exist yet, document the missing harness in the research log
instead of pretending manual testing is enough.
