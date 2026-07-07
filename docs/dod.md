# Data-Oriented Design

Sophia is a set of cooperating processes. The data-oriented rule is simple:
data crosses a boundary as a packet, snapshot, command stream, or typed ID.
Nothing reaches back across the boundary to mutate private state.

This is the same discipline used in Okys, adapted from a graphics library to a
desktop session. Sophia has more processes and stronger security boundaries, so
the rule matters more.

## Core Rule

Separate data from authority.

XLibre owns X11 authority. Sophia Engine owns compositor authority. Sophia WM
owns policy authority. Portals own cross-namespace transfer authority.

Each layer exports data about its state, not pointers into its state.

Good:

```text
SurfaceSnapshot {
    surface_id,
    xid,
    namespace_id,
    geometry,
    damage_region,
    buffer_handle,
    serial,
}
```

Avoid:

```text
Window object shared by Engine, WM, X Bridge, and XLibre.
```

A shared object graph would turn process boundaries into fiction. Sophia should
prefer flat records, handles, generations, and immutable snapshots.

## Layers

```text
types      passive records, IDs, enums, flags
state      dense tables owned by one process
protocol   packet definitions and serialization
systems    logic that consumes snapshots and emits new packets
bridge     XLibre-facing translation and privileged requests
portal     namespace-crossing transfer policy
```

Types do not perform work. State owns storage. Systems transform data. Protocols
move data. Bridges translate authority from one domain to another.

## Candidate IDs

Typed IDs prevent one domain's integer from masquerading as another.

- `SurfaceId` for compositor surfaces tracked by Sophia Engine.
- `XWindowId` for X11 window XIDs mirrored from XLibre.
- `NamespaceId` for Xnamespace labels known to Sophia.
- `OutputId` for physical or virtual outputs.
- `SeatId` and `DeviceId` for input routing.
- `TransactionId` for atomic WM updates.
- `PortalTransferId` for cross-namespace transfers.

IDs should carry generations where stale references are plausible. XIDs are
foreign identifiers and should be wrapped, not reused as Sophia-owned IDs.

## Core Packets

### InputEventPacket

The compositor-side input packet is the value Sophia Engine produces after
reading libinput.

Fields should describe:

- seat and device
- event kind
- time
- global position
- target surface when known
- target XID when routing to XLibre
- local coordinates
- buttons, keycodes, modifiers, valuators

For X11 clients, the packet is not delivered directly to the client. It becomes
input to XLibre's routed-input extension, which still applies X11 semantics.

### LayoutTransaction

The WM emits layout transactions, not one-off mutations.

Fields should describe:

- transaction ID
- affected surfaces
- requested client sizes
- focus changes
- workspace/tag changes
- render positions
- z-order
- decorations
- timeout policy

Sophia Engine commits the transaction as a unit when possible. If clients lag,
the engine may fall back after the transaction timeout.

### SurfaceSnapshot

Sophia X Bridge emits surface snapshots from XLibre state.

Fields should describe:

- Sophia `SurfaceId`
- X11 `XWindowId`
- `NamespaceId`
- window class/title metadata
- current geometry
- mapped/unmapped state
- buffer or pixmap handle
- damage region
- serial/generation

The snapshot is immutable once handed to Sophia Engine.

### FrameSnapshot

A frame is a value. Sophia Engine should be able to capture a frame plan for
tests and replay.

Fields should describe:

- output size and scale
- ordered surface list
- transforms and clips
- damage regions
- buffer handles
- transaction serials

Tests should compare frame snapshots before inspecting live process state.

### PortalTransfer

Namespace crossings are explicit data packets.

Fields should describe:

- source namespace
- target namespace
- transfer kind
- MIME or protocol type
- byte size
- data handle or inline data
- user or policy decision
- lifetime and revocation state

Portals should never grant two namespaces general X11 visibility just to move
one piece of user-approved data.

## Storage

Use dense tables inside a process. Use snapshots between processes.

Sophia Engine owns dense tables for surfaces, outputs, seats, devices, and
active transactions. Sophia X Bridge owns its X11 mirror tables. Sophia WM owns
policy state. Portals own transfer state.

No process should hold a mutable reference into another process's table. Cross a
boundary by serializing a packet or by passing an OS handle with explicit
ownership.

## Hot Paths

The hot paths are:

- physical input to compositor hit test
- compositor frame scheduling
- damage aggregation
- surface ordering and transform evaluation

Keep them allocation-light and branch-obvious. Slow policy work belongs in the
WM or portal processes. X11 protocol complexity belongs in XLibre or Sophia X
Bridge, not inside the compositor's inner frame path.

## Invariants

- XLibre is the source of truth for X11 resources.
- Sophia Engine is the source of truth for visual placement.
- The WM proposes policy; the engine commits renderable state.
- Namespace crossings require portal packets.
- A frame plan is immutable once rendering begins.
- A transaction has a serial and an outcome.
- A stale ID must fail closed.
