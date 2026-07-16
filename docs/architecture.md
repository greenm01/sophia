# Architecture

**Role:** normative system architecture.

This document defines Sophia's authority boundaries and intended process
relationships. [Data-Oriented Design](dod.md) defines the records that cross
those boundaries. [Namespaces and Portals](namespaces-and-portals.md) defines
the trust and transfer model. Subsystem documents record implementation detail
and current coverage.

Sophia is Engine-centered. Sophia Engine owns physical input, the scene graph,
atomic visual commits, rendering, and scanout. Protocol authorities terminate
client protocols and translate them into namespace-checked Sophia facts. The WM
proposes blind layout policy. Portals mediate deliberate namespace crossings.

## System Shape

```text
================================================================================
                         HARDWARE AND KERNEL
================================================================================
 [ physical input devices ]                                  [ display output ]
            │                                                        ▲
            │ libinput                                               │ DRM/KMS
            ▼                                                        │

================================================================================
                    SOPHIA ENGINE: VISUAL AUTHORITY
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Scene graph | hit-testing | atomic commits | rendering | frame scheduling  │
 └───────────────┬────────────────────┬───────────────────────┬────────────────┘
                 │ opaque policy data │ reduced portal facts  │ sanitized chrome
                 ▼                    ▼                       ▼
        ┌────────────────┐   ┌─────────────────┐   ┌──────────────────────────┐
        │   SOPHIA WM    │   │  PORTAL BROKER  │   │ METADATA BROKER / SHELL │
        │ blind policy   │   │ policy + grants │   │ redacted presentation   │
        └───────┬────────┘   └────────┬────────┘   └──────────────────────────┘
                │ layout proposals    │ executor commands
                ▼                     ▼

================================================================================
                         PROTOCOL AUTHORITIES
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Sophia X Server Frontend              | Sophia Wayland Authority           │
 │ X11 resources, selections, grabs      | Wayland objects and protocol state │
 └───────────────────────────────┬────────────────────────────────────────────┘
                                 │ namespace-checked transactions,
                                 │ lifecycle, routed input, configure
                                 ▲

================================================================================
                         ADMITTED CLIENT NAMESPACES
================================================================================
 ┌────────────────────────────────────┐     ┌─────────────────────────────────┐
 │ Namespace A: trusted/shared        │  X  │ Namespace B: confined           │
 │ X terminal | trusted local tools   │     │ browser | chat | untrusted app  │
 └────────────────────────────────────┘     └─────────────────────────────────┘
```

The session supervisor surrounds these components. It owns process lifetime,
namespace allocation, client admission, authorization material, broker health,
and recovery. It does not absorb protocol semantics, portal policy, rendering,
or layout policy.

## Current And Target State

### Implemented

- The native Sophia X Server Frontend accepts bounded concurrent local X11
  clients, owns X resource tables, emits `SurfaceTransaction` batches, routes
  client-targeted input/control, and renders two real xterms through Engine and
  KMS. The retained paired Milestone 3 proof passes under both classic-shared
  and fresh zero-capability confined profiles with physical keyboard and
  pointer input, authenticated RandR delivery, configure-plus-pixels resize,
  and clean teardown. Its X13 runs report 94/90 ms startup readiness and 13 ms
  maximum composition.
- X resources and selection state are namespace-keyed. Protocol values and a
  runtime registry model immutable profiles, directional portal capabilities,
  admission provenance, and revocation. After X setup authentication, the
  frontend asks session policy for an immutable context before allocating X
  client/resource identity. The live classic session admits same-UID Unix peers
  into distinct registry admissions that deliberately share its namespace,
  then revokes each admission after connection cleanup. The supervisor creates
  a fresh owner-only Xauthority file and cookie for every live session, passes
  only its path to clients, and removes it on teardown. Denial uses native X11
  setup failure. `sophia-live-session` explicitly selects classic-shared or a
  confined group with zero ambient portal capabilities. A socket regression
  assigns simultaneous clients distinct confined namespaces and proves a
  cross-namespace window map, property mutation, and selection ownership return
  `BadAccess`; selection conversion fails normally and rejected property writes
  emit no metadata candidate. Foreign event-mask selection also returns
  `BadAccess`, cannot change the receiving worker's authority-local key target,
  and brokered input reaches only the addressed client's private queue. Classic
  same-namespace existing-resource behavior remains available. Session
  supervision can revoke one admission by opaque `ClientAdmissionId`; socket
  shutdown then runs the worker's ordinary route, resource, surface, and lease
  cleanup without terminating its peers.
- Live native-X setup and populated RandR resource replies now derive their
  bounded root/output/mode facts from an Engine output-topology snapshot. Engine
  hit-testing also crosses the boundary as a `RoutedInputRequest`; the frontend
  resolves the owning client and applies deterministic authority-local XKB
  modifier state before emitting core and selected XI2 events. Live topology
  updates produce mask-selected RandR notifications, and surface resize keeps
  new geometry quarantined until matching pixels commit.
- `sophia-portal` has deterministic reducers for clipboard, drag-and-drop, file
  handoff, screen capture, URI open, and notifications. Owner-only bounded
  broker IPC, policy-provider IPC, expiry/revocation lifecycle, and the first
  native-X `CLIPBOARD`/`PRIMARY` source-proxy executor are complete. The other
  portal kinds do not yet have complete native executors.
- Standard DRI3 1.2 carries FD-bearing `Open`, modifier-bearing multi-plane
  pixmaps, xshmfences, and Present submissions through bounded frontend batches.
  A Mesa RADV `vkcube` trace reaches an Engine transaction without an X11 error.
  The reusable renderer-private DMA-BUF registry and cloneable Present feedback
  router now feed the persistent mixed CPU/DMA-BUF renderer and page-flip
  retirement path. The guarded X13 GPU-to-KMS evidence run remains to be
  retained before the milestone is complete.
- The Smithay-backed Wayland Authority runs real Kitty with SHM, routed input,
  frame callbacks, buffer release, and native KMS. Controlled DMA-BUF
  direct-scanout evidence exists, but arbitrary client GPU composition does not.
- XLibre is absent from the production workspace and launcher. Its frozen
  prototype remains under `research/xlibre` as historical evidence.

### Active Milestone 4 Integration Seam

The live executable still contains transitional compositor assembly.
`PersistentNativeScanout` owns persistent native output state, while
`PersistentCpuScene` retains a second CPU-only `SurfaceId` projection and
drives composition from `crates/sophia-cli/src/commands/live_session.rs`. This
is current implementation location, not normative ownership: the CLI must not
become the lasting scene, renderer-import, frame-scheduling, or scanout owner.

Milestone 4 uses a narrow hybrid extraction. The Engine/backend-owned
live-presentation seam contains DMA-BUF import, acquire-fence polling, mixed
CPU/GPU composition, KMS submission correlation, and page-flip retirement.
`sophia-renderer-live` retains imported sources and performs
renderer-private composition; `sophia-backend-live` retains native KMS
submission and retirement; Engine remains the sole committed scene truth. The
CLI is limited to launch, supervision, and bounded coordination. Broader
session-loop extraction waits until the GPU presentation exit is proven.

An asynchronous GPU Present uses `PreparedSurfaceCommit`. Preparation validates
and snapshots only protocol-neutral visual state without mutating the committed
scene. A matching page flip revalidates every touched surface and merges the
candidate while preserving unrelated newer commits. Timeout, rejection,
disconnect, or a changed touched baseline discards the candidate and retains
the last committed geometry-plus-pixels state.

### Target

- Standard X11 DMA-BUF registrations and Present submissions enter the
  renderer-private live path without placing native objects in Engine scene
  records or the X authority runtime.
- Acquire fences quarantine unready frames while Engine preserves the last
  committed geometry-plus-pixels state.
- Real KMS page-flip feedback drives Present Complete before Idle, idle-fence
  triggering, and exact-once imported-buffer retirement.
- One software client and GPU-backed `vkcube` pass startup, resize, delayed
  readiness, recovery, and teardown through the same Engine-owned KMS session.
- Wayland remains a supported frontend through Smithay, but it is a maintenance
  lane while X11 presentation semantics are completed.

## Load-Bearing Ownership Rules

| Component | Owns | Must not own |
| --- | --- | --- |
| Sophia Engine | physical input, scene graph, hit-testing, committed visual state, damage, frame scheduling, rendering, DRM/KMS presentation | client protocol parsing, X/Wayland resources, portal decisions, WM policy |
| X Server Frontend | X11 sockets, authentication adapter, XIDs, atoms, properties, selections, focus, grabs, events, drawing readiness | physical devices, workspaces, final layout, portal policy, renderer imports, scanout |
| Wayland Authority | Wayland sockets and objects, configure/ack, attach/damage/commit, protocol delivery | a second scene graph, physical input, final layout, scanout, portal policy |
| Session runtime | process supervision, namespace registry, admission, authorization publication, bounded I/O coordination, recovery | protocol meaning, visual policy, portal allow/deny decisions |
| Sophia WM | layout, workspace, focus and launch policy through opaque handles | XIDs, protocol objects, namespaces, titles, PIDs, payloads, rendering |
| Portal broker | bounded transfer policy, decision and grant lifecycle | protocol object tables, payload rendering, DRM/KMS, client-global visibility |
| Portal executor | concrete payload/handle transfer and native protocol completion | policy decisions or ambient namespace authority |
| Metadata broker/shell | sanitization and compositor-owned chrome | WM identity disclosure or client-protocol authority |

No component may acquire another row's authority merely because it currently
runs in the same process. Crate and process boundaries may evolve; ownership
does not.

## Protocol Authorities

A protocol authority parses one client protocol, owns its object/resource
tables, enforces the admitted namespace context, and reduces client-visible
state into Sophia records. It may own protocol focus, grabs, selections,
configure/ack state, buffer readiness, and lifecycle behavior.

An authority emits:

- `AuthoritySurface` and `SurfaceTransaction` values;
- surface removal and lifecycle facts;
- sanitized metadata candidates;
- bounded portal request facts;
- accept/reject acknowledgements for Engine-routed input and control.

An authority must not duplicate Engine's scene graph or infer presentation from
client traffic. Engine output snapshots and presentation retirement flow back to
the authority so it can produce native protocol replies and events.

### Sophia X Server Frontend

Use **Sophia X Server Frontend** for the component and **X11** for its
client-facing API and wire protocol. “X Authority” is acceptable shorthand for
the protocol role and current crate name.

The frontend follows the Phoenix strategy, not Phoenix code: implement a modern
X server cleanly, retain the established X11 API, and expand only from real
application evidence. Sophia is not designing an “X12” or a separate native
application protocol.

Modernization happens beneath X11 through Sophia-owned atomic commits,
Engine-owned presentation, explicit buffer readiness, session-selected
namespaces, and narrowly justified extensions. Classic shared-X is a supported
profile rather than a defect to be silently removed.

The detailed contract is in
[sophia-x-authority.md](sophia-x-authority.md). Real-client admission is tracked
in [x11-compatibility-matrix.md](x11-compatibility-matrix.md).

### Sophia Wayland Authority

The Wayland Authority uses Smithay as protocol infrastructure. Smithay's
dependency does not make it the compositor authority: the frontend terminates
Wayland objects and emits Sophia transactions; Engine owns composition and
scanout.

No new Wayland protocol work is currently prioritized beyond correctness,
security, recovery, and regression maintenance. The implemented and deferred
surface is recorded in
[sophia-wayland-authority.md](sophia-wayland-authority.md).

### XLibre Boundary

XLibre is not a production dependency or an active integration track. Its
XComposite mirror, namespace experiment, and routed-input extension are
historical lessons under `research/xlibre`.

A future XLibre provider may be reconsidered only when measured native-X gaps
justify its authority and maintenance cost. Such a provider could own broad X11
semantics, but it could never own Sophia DRM/KMS, physical input, layout,
namespace admission, portal policy, or session control.

## Atomic Visual Transactions

Sophia commits geometry and matching pixels together. An authority may retain
pending geometry, buffers, damage, and protocol synchronization, but Engine
presents only a coherent `CommittedSurfaceState`.

The default slow-client behavior is:

- preserve the last committed good surface;
- do not stretch stale pixels into new geometry;
- do not expose half-rendered buffers or black resize borders;
- reject stale generations and invalid readiness;
- degrade only through explicit bounded timeout policy.

X core drawing, SHM, Render, and Present/DRI3 must reduce to the same readiness
model. Wayland attach/damage/commit maps into that model through its authority.
Prototype `LayoutEpochState` and XComposite readiness inference are historical
compatibility mechanisms, not the permanent native-X seam.

Renderer imports and KMS handles remain backend-private. An authority may pass
an opaque buffer handle with explicit ownership, dimensions, format, damage,
and fence/readiness facts; it never submits scanout.

## Input

Sophia Engine reads physical devices, applies global shortcuts and chrome
hit-testing, walks the actual transformed scene, and selects a `SurfaceId` plus
target-local coordinates. It sends that route to the owning authority.

The authority then applies protocol delivery rules:

- X11 focus, grabs, event masks, XKB/XI state, and namespace checks;
- Wayland focus, serials, implicit grabs, and namespace checks.

The authority returns a reduced delivery acknowledgement. Engine never writes
arbitrary client events or receives a client connection handle. Route failure
does not fall back to synthetic input.

Input delivery stays off the WM path. The WM may choose focus policy in response
to reduced Engine facts, but it does not receive every motion/key event or
protocol identity.

## Window Manager And Chrome

The WM consumes immutable snapshots keyed by opaque `SurfaceId` values and
emits `LayoutTransaction` proposals. It never sees XIDs, Wayland object IDs,
namespace IDs, titles, classes, PIDs, paths, icons, credentials, or portal
payloads.

Engine mints transaction IDs, validates every proposal, and keeps the last
committed layout when the WM is absent, malformed, timed out, or restarting. A
valid but rejected layout is a policy failure, not a transport failure.

Manage state such as size, focus, fullscreen, and workspace assignment is
separate from compositor-only render state such as position, z-order, crop,
opacity, and transforms. Engine owns frame-clock interpolation and cancellation;
the WM does not drive animation frame by frame.

Titles, icons, attention, trust badges, and close actions belong to the metadata
broker and compositor shell. Chrome output is sanitized and generation-checked.
Polite close is routed to the owning authority as `WM_DELETE_WINDOW` or
`xdg_toplevel.close`; process termination remains later session policy.

The optional [X11 WM Bridge](sophia-x11-wm-bridge.md) may present a synthetic
X11 facade to a legacy WM such as xmonad while speaking the normal blind Sophia
WM protocol. It is policy compatibility, not an application X server or a path
around namespace boundaries.

## Namespaces And Portals

Namespace identity is session policy, not protocol metadata. The supervisor
assigns an immutable `ClientAdmissionContext`; authorities enforce it; Engine
and WM do not reinterpret it.

Classic trusted X clients may deliberately share one namespace. Confined
clients receive separate namespaces and explicit capabilities. Cross-namespace
lookup, delivery, and transfer fail closed unless a live portal grant authorizes
one bounded operation.

Portal policy receives only source/target namespaces, transfer kind,
generation, bounded metadata, and lifetime. Runtime executors retain protocol
request context, data, handles, and UI effects. Denial maps to native protocol
failure and never freezes the session or fabricates input.

The complete admission, capability, grant-lifecycle, and first X11
clipboard/PRIMARY contract is in
[namespaces-and-portals.md](namespaces-and-portals.md).

## Runtime And Process Failure

Runtime adapters translate external readiness and completion into bounded
`SessionRuntimeObservation` values. The data-only session reducer may request
polling, WM policy, rendering, scanout, portal drain, chrome presentation, or
restart. It does not parse X11, read portal payloads, render, or own native file
descriptors.

Supervised components report bounded health state and generations. Runtime
policy owns restart delay and give-up decisions. Engine preserves committed
visual state while a WM or broker restarts. A broker restart revokes grants
whose executor continuity cannot be proven.

Logs are a boundary. Default diagnostics may contain opaque IDs, generations,
counts, outcomes, and timings. They must not contain raw XIDs, namespace IDs,
credentials, titles, PIDs, paths, payloads, icons, or buffer contents.

## Development Order

The active critical path is:

1. completed: freeze the namespace, admission, capability, and portal
   contracts;
2. completed: make classic and confined X admission launchable;
3. completed: complete the broker and X11 `CLIPBOARD`/`PRIMARY` reference flow;
4. completed: complete XKB, input/grab, Engine-output, RandR, resize, and
   session behavior;
5. active: complete explicit SHM and DRI3/Present lifetime and native
   presentation semantics through the hybrid Engine/backend seam; and
6. next: expand application compatibility from the evidence matrix after the
   Milestone 4 hardware exit.

Wayland remains under maintenance gates during this work. XLibre remains
documented and deferred.

## Reference Boundaries

- Phoenix: clean-room modern X server strategy and evidence-driven coverage.
- XLibre: historical X11 delivery, namespace, and compatibility lessons.
- Smithay/niri: Wayland protocol, backend, renderer, and test patterns without
  adopting their compositor-policy architecture.
- picom: historical XComposite/Damage and buffer-lifetime lessons.
- river: external policy-protocol and crash-isolation lessons.
- macOS WindowServer/Core Animation: transaction-first presentation and
  fail-closed visual integrity.
