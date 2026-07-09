# Sophia X Authority

Sophia X Authority is the long-term X compatibility target. It replaces the
idea of a permanent XLibre/Xorg dependency with a Sophia-owned X protocol
subset that emits namespace-checked `SurfaceTransaction` values to Sophia
Engine.

The authority is not the compositor. It terminates X protocol, owns X resource
semantics, enforces namespace boundaries, and translates client-visible X state
into Sophia-owned data.

## Authority Boundary

Sophia X Authority owns:

- X client sockets and protocol parsing;
- X resource IDs, atoms, properties, windows, pixmaps, GCs, cursors, and
  colormaps where required for compatibility;
- namespace-aware resource lookup and event subscription;
- X focus, grabs, selections, configure, map/unmap, and lifecycle semantics;
- X drawing completion and buffer readiness;
- reduced metadata candidates for the metadata broker;
- portal request facts for cross-namespace transfers.

Sophia X Authority must not own:

- physical input devices;
- compositor scene graph or hit-testing;
- final layout, workspaces, or global shortcuts;
- compositor chrome;
- portal policy decisions;
- renderer imports, frame scheduling, or scanout.

## Minimum Compatibility Subset

The first practical target is not all of X11. It is enough protocol to run real
toolkit applications while preserving Sophia's authority boundary.

Required baseline:

- core connection setup, errors, requests, replies, and events;
- window lifecycle: create, destroy, map, unmap, reparent, configure, circulate;
- pixmaps, graphics contexts, basic drawing, copy area, clear area, and expose;
- atoms and properties for ICCCM/EWMH enough to support titles, classes,
  protocols, states, selections, and WM hints;
- event masks and event delivery for structure, property, focus, keyboard, and
  pointer events;
- selections for clipboard and primary selection handoff;
- XKB for keyboard compatibility;
- XFixes for selection owner tracking and modern client expectations;
- Sync for compatibility with `_NET_WM_SYNC_REQUEST`;
- Render for modern toolkit drawing paths;
- SHM for software-backed pixmap/image updates;
- DRI3/Present for GPU-backed buffer handoff;
- RandR surface/output facts enough for clients to observe screen size;
- selected GLX compatibility only after DRI3/Present semantics are clear.

Deferred unless a real app requires them:

- network transparency beyond local Unix sockets;
- full legacy font server behavior;
- indirect GLX as a primary rendering path;
- XTEST-style synthetic input as a privileged general API;
- broad extension coverage that does not affect real target clients.

## Namespace Model

Every client connection belongs to a `NamespaceId` before it can create
resources. The authority may learn this from launch tokens, socket routing,
credentials, or a later broker, but namespace identity is authority state and
must not leak to the WM.

Resource rules:

- XIDs are local to the authority and wrapped as `AuthorityLocalId` before
  becoming Sophia data.
- Cross-namespace resource lookup fails closed unless a specific portal flow
  grants a narrow transfer.
- Event subscriptions are namespace-scoped by default.
- Properties may be visible within a namespace, but cross-namespace property
  discovery must not expose titles, classes, PIDs, paths, or atoms that reveal
  another namespace's private clients.
- Grabs and focus are authority semantics. Sophia Engine supplies target
  surfaces and local coordinates; the authority still applies X delivery rules.

## Surface Transactions

Each visible top-level or protocol surface maps to an `AuthoritySurface` owned
by Sophia X Authority and a Sophia `SurfaceId` owned by Sophia Engine.

The authority emits `SurfaceTransaction` records when a surface has new visual
state:

- `Pending` while target geometry or matching pixels are not ready;
- `Ready` when geometry, buffer, damage, and previous committed generation are
  coherent;
- `Failed` when the protocol path cannot produce a valid commit;
- `TimedOut` when timeout policy closes a slow transaction.

Sophia Engine commits only ready transactions whose previous committed
generation matches. Pending, failed, timed-out, stale, or invalid transactions
preserve the last committed visual state.

## Drawing To Buffer Readiness

Authority drawing paths should reduce to one readiness model:

- `PresentPixmap`: preferred explicit handoff; ready when the presented pixmap
  and damage region are known.
- DRI3 DMA-BUF: ready when the buffer handle, dimensions, format, and fence
  state are importable by the renderer boundary.
- SHM/software updates: ready when the affected software buffer range is copied
  or otherwise made immutable for the transaction.
- Render/core drawing: ready when drawing commands have updated an authority
  owned backing buffer and damage is bounded.
- XSync resize: compatibility signal only; use it to avoid presenting a resized
  state until matching pixels exist, not as the native transaction model.

The authority should prefer explicit ready buffers over XComposite-style
mirroring. XComposite/Damage remains prototype evidence, not the long-term
surface boundary.

## Portals And Selections

Selections, clipboard, drag-and-drop, URI open, notifications, screenshots, and
file handoff are protocol-specific inputs to Sophia Portals.

The authority owns X requestor context:

- requestor XID;
- selection atom;
- target atom;
- property atom;
- timestamp;
- source and target namespace attribution.

Portals receive reduced transfer facts:

- source namespace;
- target namespace;
- transfer kind;
- MIME or target name;
- byte count or bounded placeholder;
- generation token.

Portal approval is generation-bound and single-use. Denial becomes native X
failure, such as `SelectionNotify` with `property = None`, not synthetic input
or client freezing.

## Lifecycle And Chrome

Sophia X Authority translates compositor lifecycle commands into normal X
semantics:

- polite close uses `WM_DELETE_WINDOW` when advertised;
- force close may become X client termination only after Engine/session policy
  decides the polite path failed;
- map/unmap/destroy events become reduced lifecycle facts for Engine and WM
  relayout;
- raw titles, classes, icons, PIDs, and paths remain metadata-broker inputs,
  never WM layout inputs.

The metadata broker emits sanitized `ChromeDescriptor` data. Sophia Engine owns
the chrome presentation and chrome hit-testing.

## Input Delivery

Sophia Engine reads physical input, owns the scene graph, performs spatial
hit-testing, and sends routed input intent to Sophia X Authority:

- target Sophia `SurfaceId`;
- authority-local object ID when known;
- local coordinates after inverse transform;
- seat, device, time, and event kind.

Sophia X Authority then applies X semantics:

- focus and grabs;
- event masks;
- XI/XKB delivery rules;
- namespace checks;
- sync-frozen device state.

The authority returns reduced accept/reject outcomes. It must not expose raw
client streams or general event injection capability back to Sophia Engine.

## Phoenix Study Targets

Use Phoenix as a reference for clean-room X practicality, not as a direct copy.
The useful study areas are:

- connection setup and request dispatch shape;
- minimal resource tables;
- basic window/pixmap/property behavior;
- extension prioritization based on real toolkit compatibility;
- tests or examples that prove GTK/GL/Vulkan application paths.

Sophia-specific differences must remain intact: namespace enforcement, portal
boundaries, blind WM policy, and Engine-owned atomic visual commits.

## First Implementation Milestones

1. Add a `sophia-x-authority` crate skeleton with passive resource tables and
   no live socket yet.
2. Model namespace-scoped X resource lookup and event subscription in tests.
3. Model `AuthoritySurface` creation from X window lifecycle events.
4. Convert a synthetic Present/SHM/CoreDraw update into a ready
   `SurfaceTransaction`.
5. Convert a synthetic selection request into a portal request and native X
   denial/handoff artifact.
6. Add a local socket parser only after the resource and transaction reducers
   are covered by integration tests.

## v0 Internal Socket Runtime

The first executable authority seam is an internal Sophia frame protocol over a
Unix socket. It is not the X11 wire protocol. It exists to prove that the X
Authority can run across a process boundary while preserving the same reducer
behavior already covered by tests.

The v0 socket path uses the shared 24-byte Sophia IPC header and two message
kinds: `XAuthorityRequest` and `XAuthorityResponse`. Payloads are decoded with
explicit little-endian parsing, bounded counts, and bounded text. No generic
serializer is used.

The internal request surface covers only the reducer-backed behaviors that exist
today:

- create and map a window;
- present a pixmap as a ready `SurfaceTransaction`;
- set a selection owner;
- convert a selection request into a portal prompt or native failure artifact.

The CLI smoke command is:

```sh
cargo run --offline -q -p sophia-cli -- x-authority-runtime-smoke
```

Real X11 connection setup and request parsing starts after this seam stays
green. The next parser should translate X11 setup bytes and a few core request
fixtures into these existing internal request packets rather than bypassing the
authority reducers.
