# Data-Oriented Design

Sophia is a set of cooperating processes. The data-oriented rule is simple:
data crosses a boundary as a packet, snapshot, command stream, or typed ID.
Nothing reaches back across the boundary to mutate private state.

This is the same discipline used in Okys, adapted from a graphics library to a
desktop session. Sophia has more processes and stronger security boundaries, so
the rule matters more.

## Core Rule

Separate data from authority.

Protocol authorities own client-protocol authority. Sophia Engine owns visual
and compositor authority. Sophia WM owns policy authority. Portals own
cross-namespace transfer authority.

Each layer exports data about its state, not pointers into its state.

Good:

```text
SurfaceSnapshot {
    surface_id,
    authority_local_id,
    namespace_id,
    geometry,
    damage_region,
    buffer_handle,
    serial,
}
```

Avoid:

```text
Window object shared by Engine, WM, protocol authority, and portal process.
```

A shared object graph would turn process boundaries into fiction. Sophia should
prefer flat records, handles, generations, and immutable snapshots.

## Layers

```text
types      passive records, IDs, enums, flags
state      dense tables owned by one process
protocol   packet definitions and serialization
systems    logic that consumes snapshots and emits new packets
authority  client-protocol translation and protocol resources
bridge     legacy/prototype translation to external servers
portal     namespace-crossing transfer policy
```

Types do not perform work. State owns storage. Systems transform data. Protocols
move data. Authorities terminate client protocols. Bridges translate legacy or
external authority from one domain to another during prototype work.

## TEA Where It Applies

The Elm Architecture is a good fit for policy processes, not for every part of
Sophia.

Use this shape for Sophia WM and portals:

```text
Model + Event/Snapshot -> update -> Command Packet
```

Examples:

- WM model plus `LayoutNodeSnapshot` values produces a `LayoutTransaction`.
- Portal transfer state plus user or policy events produces allow, deny, revoke,
  or handoff packets.

Do not force Sophia Engine into a global TEA loop. The compositor is the
security and performance authority. It should keep explicit state tables,
generation-checked IDs, spatial indexes, damage queues, frame plans, and
renderer/backend systems. Its public boundaries may still speak in snapshots and
commands, but its inner loops should stay allocation-light, cache-conscious, and
auditable.

## Candidate IDs

Typed IDs prevent one domain's integer from masquerading as another.

- `SurfaceId` for compositor surfaces tracked by Sophia Engine.
- `XWindowId` for X11 window XIDs mirrored from the XLibre prototype or owned
  by Sophia X Authority.
- `NamespaceId` for sandbox labels known to Sophia.
- `OutputId` for physical or virtual outputs.
- `SeatId` and `DeviceId` for input routing.
- `TransactionId` for atomic WM updates.
- `PortalTransferId` for cross-namespace transfers.

IDs should carry generations where stale references are plausible. XIDs are
foreign identifiers and should be wrapped, not reused as Sophia-owned IDs.

## Core Packets

### SessionRuntimeState

Session runtime state is the data-only coordinator for the continuous session
loop.

Fields should describe:

- current runtime phase
- total X events polled
- rendered frame count
- drained portal command count
- presented chrome command count
- WM restart request count
- last rendered frame serial

The reducer may emit commands to poll X, request WM policy, schedule or render a
frame, drain portals, present chrome, or restart the WM. It must not poll file
descriptors, render, or mutate X11 state directly.

`SessionRuntimeLoop` may batch `SessionRuntimeEvent` values and collect emitted
commands, but it must remain a pure data shell around the reducer. External
runtime adapters own blocking I/O, socket reads, X event polling, renderer
work, and process waits; they feed observations into the loop rather than
embedding those operations inside runtime policy.

`SessionRuntimeObservation` is the external adapter boundary. It may represent
only reduced, bounded facts: counts, frame serials, restart/layout readiness,
broker health state, broker health generation, and broker status-message
length. It must not become a generic envelope for raw X events, XIDs, namespace
tokens, metadata strings, portal payload bytes, icon bytes, buffers, or file
paths. `SessionRuntimeEventBatch` enforces the maximum observation count before
the runtime loop sees the events.

Concrete producer adapters may live beside the data they reduce. For example,
Sophia Engine may map WM transaction updates, frame/render reports, portal
commands, and chrome updates into observations because it already owns those
types. Such adapters must remain pure translation helpers: no socket polling,
no X mutation, no rendering, no process supervision, and no payload retention.

`HeadlessSessionDriver` may execute runtime command sequencing against
deterministic headless adapters. It may own `SessionRuntimeLoop` state and a
last-committed layout cache. It must not become the real compositor event loop:
blocking file-descriptor polling, libinput intake, DRM/KMS commits, X socket
dispatch, broker socket dispatch, and process supervision remain outside this
headless test driver.

CLI smokes that exercise a complete runtime tick should use
`HeadlessSessionDriver`. Direct `SessionRuntimeLoop` use in CLI code should be
reserved for narrow adapter checks where no frame should be planned, such as
broker health packet routing or isolated WM transaction observation.

`RuntimeDriverAdapter` is the command-execution seam between the reducer and
external sources. Implementations must answer runtime commands with reduced
observations or existing frame reports. Live adapters may wrap X bridge, WM
socket, broker, portal, chrome, and renderer sources, but they must not expose
raw X events, namespace launch/auth tokens, metadata strings, portal payload
bytes, icon bytes, or unbounded buffers through the runtime loop.

Live adapter constructors must take non-blocking intake values that were already
reduced by the owning boundary. The runtime executor may receive counts,
transaction outcomes, broker health summaries, portal command counts, chrome
command counts, layer snapshots, and frame reports. It must not become the
place that parses X protocol payloads, blocks on sockets, validates clipboard
payload bytes, reads icon data, or owns renderer import buffers.

Process-supervised portal and metadata broker placeholders are runtime
ownership records. They prove that the runtime can start, poll, and terminate
the intended broker process kinds before the real broker IPC protocols exist.

Broker health is a bounded control packet, not a general metadata channel.
`BrokerHealthPacket` may name only the broker kind, coarse health state,
generation, and an optional short status message. It must not contain client
titles, XIDs, namespace IDs, portal payload bytes, file paths, URIs, or icon
data. The message is capped by
`SOPHIA_BROKER_HEALTH_MAX_MESSAGE_LEN` so a supervised broker cannot force
unbounded allocation in Sophia Engine or runtime.

Broker health uses the same explicit IPC frame header as WM packets. The
`BrokerHealth` message kind stores the health generation in the frame
transaction field, then encodes broker kind, health state, and optional bounded
UTF-8 status text. The portal and metadata placeholder smokes round-trip this
frame before any real broker payload protocol exists.

`SessionRuntimeState` records broker health as reduced state only: health
state, generation, and status-message length. It does not retain the status
message string. Stale health generations are ignored so an older broker report
cannot overwrite a newer readiness/degraded/stopped observation.

### XWindowMirror

Sophia X Bridge keeps a mirror of the XLibre prototype window tree. This is
cache data, not long-term authority.

Fields should describe:

- X window ID and generation
- parent and children relationships
- top-level/client relationship
- map state
- stacking rank
- namespace identity when known
- stale metadata flags

Namespace identity may start from static configuration and later be replaced by
authority-discovered records. X Bridge should treat discovered namespace
ownership as mirror metadata; the active protocol authority remains the
enforcement point.

Picom's window-tree mirror is the reference shape, but Sophia's mirror should
emit snapshots instead of owning render policy.

### LayerSnapshot

A layer snapshot is the reduced frame value that Sophia Engine render planning
consumes. It may come from the XLibre prototype bridge, Sophia X Authority,
Sophia Wayland Authority, or a future native authority.

Fields should describe:

- Sophia surface ID
- protocol-local window/object ID when relevant
- namespace ID
- stack rank
- geometry in compositor coordinates
- source pixmap or buffer handle
- damage region
- opacity, crop, and transform state

The snapshot is flat and immutable for the frame that consumes it.

### AuthoritySurface

An authority surface is a protocol-facing record owned by a protocol authority.
It maps protocol-local objects, such as X windows or Wayland surfaces, to a
Sophia `SurfaceId`.

Fields should describe:

- authority kind
- protocol-local object ID and generation
- Sophia surface ID
- namespace ID
- protocol-visible map/configure state
- protocol constraints and capabilities

Authority surfaces are not compositor surfaces. They are the authority's private
resource table entries that emit snapshots or surface transactions.

### SurfaceTransaction

A surface transaction is the atomic visual handoff from a protocol authority and
WM policy into Sophia Engine.

Fields should describe:

- transaction ID
- authority kind and surface ID
- namespace ID
- target geometry
- target buffer source
- damage region
- readiness state
- deadline or timeout policy
- previous committed generation

Sophia Engine may accept, reject, wait on, or time out a transaction. It should
not present a transaction's new geometry unless the matching target buffer is
ready.

### CommittedSurfaceState

Committed surface state is the last visually safe state Sophia Engine may
present.

Fields should describe:

- surface ID
- committed generation
- committed geometry
- committed buffer source
- output assignment
- visible state
- damage carried into the next frame

Slow-client fallback presents this state, not partially updated pending state.

### DamageFrame

Damage is a frame artifact. It describes what changed between one committed
layout and another, not a mutable property of a window object.

Fields should describe:

- output ID
- frame serial
- buffer age
- root/background generation
- affected layers
- screen-space damage regions

Picom's buffer-age damage math is the reference idea. Sophia should keep the
calculation over layer snapshots, not live windows.

### FrameClockTick

A frame-clock tick is the scheduling value that starts compositor frame work.

Fields should describe:

- output ID
- frame serial
- target presentation time in the clock's monotonic domain

The headless backend uses a deterministic clock so tests are repeatable. A real
DRM/KMS backend should produce the same value shape from page-flip or vblank
timing rather than pushing backend-specific timing state into frame planning.

### LayoutEpochState

A layout epoch records surfaces that must produce damage before an atomic layout
change is considered visually ready. In the XLibre prototype this models a
tiered XSync/Damage compromise. In the long-term authority architecture,
`SurfaceTransaction` readiness should replace heuristic damage waiting wherever
the authority can prove buffer/geometry pairing directly.

Do not treat epochs as the universal frame scheduler. They are compatibility
state for implicit X11 readiness. Authority-native commits should flow through
`SurfaceVisualStateTable`: pending `SurfaceTransaction` values are staged beside
the last `CommittedSurfaceState`, and only transactions with ready state,
non-empty geometry, concrete buffer source, valid surface, and matching previous
generation may advance committed visual truth.

Fields should describe:

- epoch serial
- pending surface IDs
- start timestamp
- timeout policy

X Damage events retire pending surfaces from the epoch in the prototype path.
Native authority commits should retire pending surfaces by explicit readiness
instead.

Resize behavior samples are derived from the same epoch state. They should
record elapsed time, timeout policy, completion, timeout status, and remaining
pending surfaces so slow clients can be measured without reaching into renderer
or X bridge internals.

For the prototype, epochs should be created only for surfaces marked
`ResizeSyncCapability::ExplicitSync`. Timed-out epochs may be expired by the
engine, which returns the pending surfaces as a bounded timeout report for the
bridge or authority to score.

Slow-client timeout reporting must remain aggregate at runtime boundaries.
Counts for preserved and explicitly degraded timeouts are acceptable. Raw
surface IDs, protocol object IDs, namespaces, classes, titles, and payload data
must stay inside the authority or bridge that owns them.

### DrmKmsOutputDescriptor

A DRM/KMS output descriptor is the backend-facing record for one discovered
output before Sophia has a full device backend.

Fields should describe:

- Sophia output ID
- DRM connector ID
- DRM CRTC ID
- active mode size and refresh
- compositor scale

The descriptor is not authority to scan out by itself. It is the data contract
between future device discovery and existing frame planning.

`DrmKmsSysfsDiscovery` is the first real discovery adapter. It reads connected
connector directories, mode lists, optional connector/CRTC IDs, and optional
scale values from a sysfs-style tree. Missing CRTC IDs must remain explicit as
`0`; only a later libdrm/ioctl backend can authoritatively bind connectors to
CRTCs, planes, and page-flip timing.

### RenderCommand

Render commands are the final planned compositor work for one frame.

Fields should describe:

- operation kind
- source surface or buffer
- destination output
- target region
- clip
- transform
- alpha or effect parameters

The command stream is Sophia Engine authority. Protocol authorities do not own
this data.

### BufferImportReport

A buffer import report describes how the renderer attempted to consume a layer's
buffer source for one validated frame.

Fields should describe:

- surface ID
- original buffer source
- requested import path
- path actually used
- imported buffer handle used by the renderer
- whether a fallback path was used

The default headless renderer always uses CPU readback as the execution path,
including as fallback for `XPixmap` or `DmaBuf` sources. Import-capable
renderers may use native `XPixmap` or `DmaBuf` handles when supported. A GPU
renderer should keep the same report shape so tests can distinguish "requested
import path" from "used path" without inspecting renderer-private state.

### CompositePixmapRecord

A composite pixmap record describes the XLibre prototype bridge-owned lifetime
of one named XComposite pixmap.

Fields should describe:

- client window ID
- named pixmap ID
- pixmap generation

Replacing a named pixmap must produce a lifetime update containing the retired
record. Removing a window must retire the current record. Render import can then
release old resources from explicit lifetime events instead of guessing from raw
pixmap integers.

### CompositorSurface

A compositor surface is Sophia Engine's stable handle for visual placement and
render scheduling.

Fields should describe:

- surface ID
- current layer snapshot generation
- committed geometry
- active buffer handle
- output assignment
- visibility state
- damage accumulator

The surface may refer back to a protocol-local object, but it is not that
protocol object.

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

### LibinputDeviceDescriptor

A libinput device descriptor is the backend-facing record for one discovered
physical input device before Sophia has a real poll loop.

Fields should describe:

- seat ID
- device ID
- broad device kind

The event source accepts packets only when the device is registered and the
packet seat matches the device seat. This keeps physical-input intake explicit
before the compositor starts doing scene hit-tests and routed-input generation.

Physical input polling is represented by `NonBlockingInputPoller` and
`LibinputPhysicalInputAdapter`. Production backends should implement the trait
with non-blocking libinput dispatch over ready file descriptors. Tests should
use deterministic queued pollers rather than opening `/dev/input` devices.

### InputRoute

An input route is the compositor's answer to "what visual surface did this event
hit?"

Fields should describe:

- input event serial
- target surface ID
- target X window ID when the target is X11
- global coordinates
- local coordinates
- transform used for inversion
- route confidence or rejection reason

This packet is Sophia Engine authority, but final X11 delivery remains XLibre
authority.

Transformed scene hit-testing must produce target-local coordinates by applying
the inverse layer transform to the physical pointer position before checking
layer geometry. Hit-test walks should prefer the highest stack rank so a
transformed top layer wins over lower overlapping layers.

### XLibreRoutedInputRequest

The routed-input request is the smallest data packet Sophia should send to an
XLibre routed-input extension.

Fields should describe:

- input serial
- seat and device
- event time
- target XID
- local X/Y coordinates in the target
- event kind

It must not include a client connection, destination socket, or arbitrary
serialized X event. XLibre uses this packet to replace only the visual
hit-test target that legacy X11 cannot compute after compositor transforms.
Grabs, focus policy, XI2 semantics, and Xnamespace checks remain XLibre
authority.

The Engine may generate this request only from a physical `InputEventPacket`
plus an accepted `InputRoute`. Serial mismatches, missing target XIDs, missing
local coordinates, and non-routed outcomes are closed routes.

### XLibreRoutedInputDecision

XLibre's answer is a decision packet.

Fields should describe:

- input serial
- target XID
- accepted or rejected outcome

Expected rejection outcomes include stale target, denied namespace,
sync-frozen device state, focus policy, and unsupported event. Ordinary active
grabs are still XLibre authority; accepted routes may be redirected by normal
grab semantics. Sophia treats every rejection as a closed route and never falls
back to direct client delivery.

Grab/focus edge smoke reports should record the edge kind, XLibre decision, and
whether delivery is allowed. Rejected active-grab and focus-policy outcomes must
always report `delivery_allowed = false`.

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

### WM Request and Response Packets

The external WM boundary uses explicit request and response packets.

Request packets should describe:

- transaction ID
- manage, relayout, or remove event kind
- opaque layout node snapshots
- output and workspace IDs
- workspace bounds

Response packets should describe command intent:

- workspace assignment
- configure size
- focus
- render placement

The response can be reduced into a `LayoutTransaction` for Sophia Engine. The
WM remains outside the compositor's per-frame and per-input hot paths; it only
receives coarse manage/relayout events and returns policy commands.

The durable WM IPC format is a bounded binary frame:

```text
u32 magic              "SOPH"
u16 protocol_version
u16 message_kind
u64 transaction_id
u32 payload_len
u32 reserved
[payload bytes]
```

All integers are little-endian. Decoders must parse fixed offsets explicitly
with `from_le_bytes`; do not cast socket bytes into structs. A frame with an
unknown message kind, unsupported version, non-zero reserved field, truncated
payload, trailing bytes, oversized payload, or excessive vector count fails
closed. The first implementation lives in `sophia-protocol` and covers
`WmRequestPacket` and `WmResponsePacket`.

### TransactionCommit

A committed or rejected transaction is reported as data.

Fields should describe:

- transaction ID
- committed, rejected, stale, or timed-out outcome
- surfaces actually applied

Rejected transactions must preserve the last committed compositor state.

### LayoutNodeSnapshot

The WM receives opaque layout nodes, not X11 windows.

Fields should describe:

- Sophia `SurfaceId`
- workspace/tag identity
- broad surface kind
- move/resize/focus/close/fullscreen capabilities
- focus, urgency, fullscreen, floating, and visibility state
- size constraints
- current compositor geometry
- serial/generation

Fields must not include `XWindowId`, `NamespaceId`, raw title, app class, PID,
or icon pixels. App-specific behavior should come later from launch/session
policy hints, not WM sniffing of client metadata.

### ChromeDescriptor

Chrome descriptors are compositor-owned presentation metadata. They are separate
from WM layout state.

Fields should describe:

- Sophia `SurfaceId`
- optional redacted display label
- compositor-owned icon token
- trust level
- attention state
- serial/generation

The compositor may use this data to draw title bars, top bars, tab strips, and
security badges. The external WM should not need this packet to tile or focus
surfaces.

Sanitized metadata broker output enters Sophia Engine as a bounded metadata
packet, not raw X properties. It may contain only `SurfaceId`, optional bounded
display label, redaction bit, icon token, trust level, attention state, and
generation. `ChromeBroker` maps accepted packets into `ChromeDescriptor`
entries and rejects invalid labels, invalid surfaces, and stale generations.
Descriptor removal follows the same generation rule.

Chrome actions are not WM commands. A compositor close button produces a
`ChromeActionRequest` owned by Engine/session policy, validated against surface
generation and capabilities, then translated by Sophia X Bridge into normal X11
close semantics. The WM receives only later layout consequences.

Session events are compositor/session inputs that may produce privileged
commands. For chrome close, `SessionEvent::ChromeAction` can produce
`SessionCommand::RequestPoliteClose`; it must not produce a WM command.
When a surface is actually removed, `SessionEvent::SurfaceRemoved` can produce
a `WmRequestKind::SurfaceRemoved` packet. This keeps WM relayout tied to X11
lifecycle consequences, not compositor chrome intent.

### SurfaceSnapshot

Sophia X Bridge emits surface snapshots from XLibre state.

Fields should describe:

- Sophia `SurfaceId`
- X11 `XWindowId`
- `NamespaceId`
- current geometry
- mapped/unmapped state
- buffer or pixmap handle
- damage region
- serial/generation
- resize sync capability

Window titles, app classes, and sync reputation keys are bridge-local metadata.
They must not be copied into `SurfaceSnapshot` or `LayerSnapshot`; the snapshot
may carry only the reduced `ResizeSyncCapability`.

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

Clipboard transfers are asynchronous. Denial becomes normal X11 selection
failure, such as a failed conversion, rather than synthetic input. Pending
approval holds only the specific transfer for a bounded timeout. Approval is
single-use and generation-bound; if the source owner changes, the pending
transfer becomes stale and must be revoked or restarted.

The `sophia-portal` crate implements this policy as a reducer over
`PortalTransfer` values. Its commands are intentionally abstract:
prompt user/policy, hand off clipboard data, or fail the X11 selection. X Bridge
code later translates those commands into concrete ICCCM/XFixes behavior.

X Bridge owns requestor-side clipboard context. A `SelectionRequest` is reduced
with the selection owner monitor, mirrored namespace table, resolved target atom
name, and runtime-minted transfer ID into a `ClipboardTransferRequest` plus
`ClipboardSelectionFailureRequest`. The portal receives only namespaces, target
name, byte-size placeholder, and owner generation; the bridge keeps X requestor,
selection, target atom, property, and timestamp context for concrete X11
replies.

The dispatcher boundary consumes `Event::SelectionRequest` from x11rb and calls
the clipboard portal reducer. It must fail closed before portal mutation when
the event is not a selection request, when namespace attribution is missing, or
when source and target are in the same namespace.

Approved clipboard text handoff is represented as data before it touches X. The
bridge validates a `HandoffClipboard` command against the original request,
requires a non-`None` request property, rejects non-text targets, caps UTF-8
payload bytes with `MAX_CLIPBOARD_TEXT_HANDOFF_BYTES`, and emits a property
payload plus successful `SelectionNotify` artifact.

The live clipboard portal smoke applies those artifacts to X: denial sends only
the failure notify, while approval writes the bounded text property before
sending the success notify. The smoke must read the requestor property back to
prove the bytes reached X.

Drag-and-drop follows the same reducer shape. Offered MIME/protocol targets are
bounded before storage, approval is generation-bound, denial or stale ownership
becomes an abstract cancel command, and Xdnd-specific protocol mechanics stay
in X Bridge.

File handoff also stays metadata-only at the reducer level. The policy model
stores open/save intent, bounded offered types, and a sanitized suggested
filename. It emits abstract handoff or cancel commands; concrete file handles,
temporary storage, and chooser UI are runtime responsibilities.

Screen capture policy records only capture intent: screenshot versus recording,
redacted scope, supported MIME type, size hint, decision, and generation. It
must not expose raw surface IDs, pixels, or buffers to policy code.

URI-open policy records bounded URI metadata only. It validates syntax and a
small scheme allowlist before creating pending policy state; the runtime owns
the actual launcher/browser handoff.

Notification policy stores bounded text/action metadata and urgency only. It
emits abstract deliver/drop commands. Sophia Engine maps those commands into
bounded compositor chrome notification state: deliver presents a staged
notification, while drop dismisses pending or visible state. The compositor
shell still owns drawing, notification action execution, history, and rate
limits.

X Bridge owns selection monitoring data. It should reduce XFixes owner-change
events into records keyed by selection atom and namespace, then pass only the
selection, namespace, owner generation, and owner-change fact to portal policy.
The portal reducer should not subscribe to X events or hold raw X authority.

## Storage

Use dense tables inside a process. Use snapshots between processes.

Sophia Engine owns dense tables for surfaces, outputs, seats, devices,
committed visual state, and active transactions. Protocol authorities own their
client resource tables. Sophia X Bridge owns X11 mirror tables only for the
XLibre prototype path. Sophia WM owns policy state. Portals own transfer state.

No process should hold a mutable reference into another process's table. Cross a
boundary by serializing a packet or by passing an OS handle with explicit
ownership.

## Observability

Logs are another boundary. Engine diagnostics may carry opaque Sophia IDs,
generations, counts, outcomes, and timing data, but default logs must not carry
raw XIDs, namespace IDs, titles, classes, PIDs, icon pixels, portal payloads, or
buffer contents. Structured `tracing` spans and events should explain decisions
without weakening namespace isolation.

## Hot Paths

The hot paths are:

- physical input to compositor hit test
- compositor frame scheduling
- damage aggregation
- surface ordering and transform evaluation

Keep them allocation-light and branch-obvious. Slow policy work belongs in the
WM or portal processes. X11 protocol complexity belongs in XLibre or Sophia X
Bridge, not inside the compositor's inner frame path.

Policy can be TEA-style. Compositor hot paths should be table/system style.

## Invariants

- Protocol authorities are the source of truth for their client resources.
- Sophia X Bridge mirrors XLibre prototype state; it does not become the target
  X authority.
- Sophia Engine is the source of truth for committed visual placement.
- Sophia must not present new geometry without matching committed pixels.
- Layer snapshots are frame values, not mutable windows.
- The WM proposes policy; the engine commits renderable state.
- Namespace crossings require portal packets.
- A frame plan is immutable once rendering begins.
- A transaction has a serial and an outcome.
- A stale ID must fail closed.
