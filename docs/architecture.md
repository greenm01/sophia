# Architecture

This doc maps Sophia's processes and the boundaries between them. The data model
is in `dod.md`; code-level rules live in `style-guide.md`.

Sophia is Engine-centered. Sophia Engine owns physical input, visual state,
atomic surface transactions, rendering, and scanout. Client compatibility lives
behind protocol authorities that terminate a client protocol and translate it
into namespace-checked Sophia surface transactions.

The long-term X path is a Sophia-owned modern X authority, informed by Phoenix,
not a permanent dependency on Xorg or XLibre internals. XLibre remains valuable
as prototype evidence and as a research reference for X11 semantics,
Xnamespace, XComposite/Damage, and routed-input experiments.

## Processes

```text
================================================================================
                         HARDWARE AND KERNEL
================================================================================
 [ physical input devices ]                                  [ display output ]
            │                                                        ▲
            │ raw input via libinput                                 │ DRM/KMS
            ▼                                                        │

================================================================================
                    SOPHIA ENGINE: COMPOSITOR AUTHORITY
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Scene graph | spatial hit-testing | damage tracking | frame scheduling     │
 │ Atomic visual commits | rendering | scanout                                │
 └──────────────┬───────────────────┬────────────────────┬───────────────────┘
        ▲      │                   │                    │      ▲
        │      │ opaque snapshots  │ portal events      │      │ chrome data
        │      ▼                   ▼                    ▼      │
 ┌───────────────┐        ┌────────────────┐       ┌─────────────────────────┐
 │  SOPHIA WM    │        │ SOPHIA PORTALS │       │ METADATA BROKER/CHROME │
 │ blind policy  │        │ allow/deny     │       │ redacted UI only       │
 │ layout/focus  │        │ handoff/revoke │       │ labels/icons/badges    │
 └───────┬───────┘        └────────┬───────┘       └────────────┬────────────┘
         │                         │                            ▲
         │ layout proposals        │ portal commands            │ sanitized
         ▼                         ▼                            │ metadata

================================================================================
                         PROTOCOL AUTHORITY LAYER
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Sophia X Authority | Sophia Wayland Authority | Sophia Native Authority    │
 │ protocol resources | grabs/focus | selections | namespace checks           │
 └────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                                  │ namespace-checked surface transactions
                                  │ routed input / configure / lifecycle
                                  ▲

================================================================================
                         SANDBOXED CLIENT NAMESPACES
================================================================================
 ┌────────────────────────────────────┐     ┌─────────────────────────────────┐
 │ Namespace A: trusted               │     │ Namespace B: untrusted          │
 │ X terminal | Wayland password mgr  │  X  │ X browser | Wayland chat app    │
 └────────────────────────────────────┘     └─────────────────────────────────┘
```

The critical split is that protocol authorities produce reduced, namespace-
checked surface transactions for Sophia Engine, while sanitized chrome
descriptors flow to the metadata broker. The WM receives only opaque policy data
and returns command packets.

## Current Architecture Focus

The internal Sophia X Authority runtime is now executable over a Sophia-owned
IPC frame protocol. That socket protocol is a harness, not the X11 wire
protocol. The first real X11 wire layer now parses connection setup and decodes
early core requests into existing internal authority requests. Wire parsing
feeds `XAuthorityRuntime`; it must not grow a second resource table or a
parallel authority path. The next architecture step is minimal X reply, error,
and event emission for decoded requests.

## Load-Bearing Boundaries

### TEA Boundary Rule

Sophia uses The Elm Architecture as a policy-boundary discipline where it fits:
snapshots and events enter a policy process, that process updates its private
model, and it emits explicit command packets back to the authority that can
execute them.

This applies strongly to Sophia WM and portals. The WM consumes opaque layout
node snapshots and focus/workspace events, updates its layout model, and emits
`LayoutTransaction` commands. Portals consume transfer requests and policy
events, update transfer state, and emit allow, deny, revoke, or handoff
commands.

This does not apply as a universal compositor architecture. Sophia Engine is
performance and security centric: it owns libinput, scene graph state,
hit-testing, damage tracking, frame scheduling, and final scanout. Its hot paths
should be data-oriented systems over owned tables and precomputed snapshots, not
a single app-wide message loop.

### Protocol Authorities

A protocol authority terminates one client protocol and adapts that protocol
into Sophia-owned visual transactions. Authorities may own protocol resources,
client object tables, grabs, selections, configure/ack state, and namespace
checks for their protocol. They must not own workspaces, final layout, global
shortcuts, compositor chrome, portal policy, physical input devices, or scanout.

The first long-term authority target is **Sophia X Authority**: a modern X
protocol subset capable of running real applications while avoiding the full
Xorg/XLibre object graph. It should learn from Phoenix's clean-room approach and
from Sophia's existing XLibre prototype seams.

A later **Sophia Wayland Authority** can support Wayland-only applications by
terminating `wl_surface`, `xdg_toplevel`, buffer attach, damage, and commit
semantics, then emitting the same internal surface transactions as the X
authority. Its first boundary is documented in
[sophia-wayland-authority.md](sophia-wayland-authority.md). It must not become a
second compositor; Sophia Engine remains the only visual authority.

Every authority must preserve the same namespace model. `NamespaceId`,
`SurfaceId`, portal transfer state, and sanitized metadata are Sophia concepts,
not X-specific or Wayland-specific concepts.

### Atomic Surface Transactions

Sophia follows the macOS/WindowServer lesson: the compositor should commit
geometry and pixels together. A surface may have pending geometry, pending
buffer state, and pending damage, but Sophia Engine should present only a
committed surface state whose geometry and pixels match.

The default slow-client behavior is fail-closed visual integrity:

- keep presenting the last committed good surface state;
- do not stretch stale pixels into new geometry as the default;
- do not expose black borders or half-rendered buffers as normal behavior;
- record slow or failed readiness as transaction outcomes;
- degrade only through explicit timeout policy.

For X clients, the Sophia X Authority should translate `PresentPixmap`, SHM,
Render, and core drawing completion into pending buffer readiness. For Wayland
clients, the Wayland Authority can map the native attach/damage/commit sequence
directly into the same readiness model.

`LayoutEpochState` is not the permanent atomic-commit primitive. It is the
XLibre prototype compatibility mechanism for clients whose buffer readiness must
be inferred from XSync and X Damage. Authority-native paths should emit
`SurfaceTransaction` records with explicit readiness, then let the Engine commit
only ready geometry/buffer pairs into `CommittedSurfaceState`.

### Compositor Strategy

Sophia Engine should follow Smithay-style compositor structure, using niri as a
read-only reference for how a production Rust compositor organizes backends,
renderers, outputs, frame clocks, input devices, and headless tests.

Sophia should not fork niri. Niri combines compositor and window-management
policy in one central state object, while Sophia deliberately splits policy into
Sophia WM. The reusable idea is the compositor machinery, not the process model.

The historical Sophia X Bridge follows picom conceptually for the XLibre
prototype path. Picom imports the X window tree, tracks top-levels and stacking,
redirects windows with XComposite, consumes Damage, builds flat layer snapshots,
and computes damage across buffered layouts. Those lessons remain useful for
understanding X compatibility, but the long-term Sophia X Authority should emit
surface transactions directly instead of requiring an XComposite mirror as the
primary seam.

Do not turn Sophia into a traditional X compositor or a Wayland compositor with
custom policy bolted on. Protocol authorities adapt clients; Sophia Engine owns
final scanout and physical input.

### Engine to WM

The WM protocol is a policy boundary. The WM receives state changes that need
policy decisions: new windows, destroyed windows, output changes, keybindings,
workspace changes, and focus-affecting events.

The WM is blind to X11 identity and namespace identity. It manages opaque layout
nodes keyed by Sophia `SurfaceId`, not XIDs. The protocol must not expose raw
window titles, classes, icons, PIDs, namespace IDs, or X11 resource IDs.

User-facing chrome such as titles, icons, attention indicators, and trust badges
belongs to Sophia Engine or a compositor-shell component fed by a metadata
broker. The broker may consume X metadata from Sophia X Bridge, but it exports
sanitized compositor chrome data separately from WM layout data. Sophia Engine
accepts that broker output as generation-checked sanitized metadata and applies
it to `ChromeDescriptor` state; raw XIDs, namespace IDs, titles, classes, PIDs,
and icon pixels do not cross this boundary.

The WM is not on the per-frame or per-input hot path. Sophia Engine keeps the
last committed policy state if the WM crashes or restarts.

The WM control flow is Engine-only. Sophia Engine mints every transaction ID,
sends the request, applies a strict response timeout, and treats the WM reply as
a proposal. The WM must not initiate layout transactions, push unsolicited
commands, or drive animations frame-by-frame. If a layout should animate, the
WM may provide the target layout; Sophia Engine owns the frame clock,
interpolation, cancellation, and final commit.

The socket protocol is a versioned, length-prefixed binary frame. Integers are
little-endian and decoded with explicit fixed-offset parsing, not `repr(C)`
casts or generic serializers. Payloads are bounded before allocation.
Sophia Engine owns the request/response transport: it writes exactly one
`WmRequestPacket`, reads one bounded `WmResponsePacket`, and rejects a response
whose transaction ID does not match the Engine-minted request transaction.
If the response is missing, malformed, oversized, or mismatched, Sophia Engine
preserves the last committed layout and reports the transaction as timed out.
Those IPC failures produce a runtime restart decision for the WM process. A
valid response whose proposed layout is rejected does not restart the WM; it is
a policy proposal failure, not a transport/protocol failure.

A legacy compatibility policy process may sit in this same slot. The
[Sophia X11 WM Bridge](sophia-x11-wm-bridge.md) is a prototype facade that
presents a fake, headless X11 server to existing X11 window managers while
speaking the normal blind Sophia WM IPC to Sophia Engine. It is a stopgap for
reusing legacy layout engines, not a protocol authority and not a path around
the namespace or metadata boundaries.

The protocol should be sequence-oriented:

- **Manage sequence** for state that affects clients: size, focus, fullscreen,
  workspace assignment, activation.
- **Render sequence** for compositor-only state: position, z-order, crop,
  decoration geometry, opacity, transforms.
- **Chrome sequence** for compositor-owned presentation metadata: redacted
  display labels, icon tokens, trust badges, and attention state. This sequence
  is not consumed by the WM. Stale metadata generations are rejected so older
  broker output cannot overwrite newer chrome state.

### Current XLibre Prototype Rendering

XComposite and Damage are the current prototype render seam. XLibre redirects
windows to offscreen pixmaps and reports changed regions. Sophia X Bridge names
or imports those pixmaps, tracks damage, and hands frame packets to Sophia
Engine.

Sophia Engine separates frame validation from renderer/import execution. The
headless path validates `FrameSnapshot` commands, replays them into a
deterministic report, and then hands the validated frame to a `FrameRenderer`.
The conservative default renderer still uses CPU readback, but the renderer
contract now carries explicit imported buffer handles. `ImportCapableRenderer`
can report native `XPixmap` and `DmaBuf` handles when those import paths are
enabled, and falls back to CPU readback when a handle type is unsupported. A
production renderer can replace the skeleton import behavior while preserving
the same command-stream and import-report contract.

XComposite pixmap lifetime is tracked separately from render import. Sophia X
Bridge stores `CompositePixmapRecord` values keyed by client window, with a
generation for each named pixmap. Replacing a pixmap returns a lifetime update
containing both the new current record and the retired record; removing a window
returns the retired record with no current replacement. This gives the later
real renderer an explicit point to release old pixmap/import resources.

This seam exists today in broad shape and remains useful as research evidence.
It should inform the Sophia X Authority design without becoming the permanent
authority boundary.

The XLibre prototype accepts ordinary X11 limitations:

- X11 clients do not have Wayland-style configure/commit acknowledgements.
- Frame-perfect resize needs heuristics at first.
- Slow or non-cooperative clients may force a timeout frame.

Sophia currently models resize synchronization as a tiered X11 compromise. The
X bridge reduces client state to `ResizeSyncCapability`: `ExplicitSync` for
clients that advertise `_NET_WM_SYNC_REQUEST` and have not earned a
bridge-local downgrade, or `ImplicitOnly` for legacy, unknown, or downgraded
clients. The engine only adds explicit-sync surfaces to `LayoutEpochState`;
implicit-only surfaces skip epoch freezing and rely on ordinary X Damage.

Timeouts remain engine-owned. `LayoutEpochState::expire_if_timed_out` closes a
stalled epoch and reports the pending surfaces. The bridge can turn those
timeout reports into class-level reputation strikes keyed by namespace and
bounded `WM_CLASS`, but that class metadata never leaves the bridge in surface
or layer snapshots.

### Current XLibre Prototype Input

This is the hard seam.

Current XLibre still routes pointer events through the legacy flat-window path:
coordinate to window, sprite trace, grabs, focus, then delivery. That cannot
represent compositor-side transforms, scaled scenes, 3D workspaces, or other
visual effects where rendered geometry diverges from XLibre's 2D tree.

The XLibre prototype needs a routed-input path:

```text
Sophia Engine hit-tests the real scene
        |
        v
target XID + local coordinates + device event packet
        |
        v
XLibre routed-input extension
        |
        v
DIX delivery with X11 grabs, focus, XI2, and Xnamespace checks preserved
```

The extension must not become "send arbitrary event directly to client." XLibre
still owns X11 delivery semantics in the prototype. Sophia only supplies the
visual target and local coordinates that XLibre cannot compute by itself.

The smallest useful extension request is:

```text
XLibreRoutedInput {
    serial,
    seat,
    device,
    time_msec,
    target_xid,
    local_x,
    local_y,
    event_kind
}
```

This request is an alternate target selection path, not a delivery bypass.
XLibre must still reject stale XIDs, namespace violations, sync-frozen devices,
focus policy violations, and unsupported event forms before entering normal DIX
delivery. Ordinary active grabs remain XLibre authority and may redirect
delivery according to normal grab semantics.

Grab/focus edge smokes are represented as closed-route decision reports.
`x-smoke-routed-input-edges` verifies that `RejectedActiveGrab` and
`RejectedFocusPolicy` outcomes do not allow delivery and do not fall back to
direct client injection. Live active-grab redirection remains an XLibre DIX
responsibility; Sophia only records the edge decision and keeps the route
closed when XLibre rejects it.

The flat request path remains as a strict compatibility wrapper, but Sophia X
Bridge also accepts transformed routes when Sophia Engine has already hit-tested
the visual scene and supplied finite target-local coordinates. XLibre still
receives the same target XID plus local-coordinate packet; it is not asked to
understand compositor transforms.

The patch target is tracked in `docs/xlibre-routed-input-extension.md`. This is
historical/prototype work once Sophia owns its X authority: the same routed
target-selection idea should become an internal Engine-to-Authority command
instead of an XLibre extension.

The first implementation optimizes for correctness, not throughput tricks. The
ordinary `RouteEvent` request remains the canonical path until profiling shows
it is the bottleneck. Sophia Engine owns the first optimization through
`RoutedInputCoalescer`: it buffers at most one pure pointer-motion route per
stable target and flushes it at the frame boundary. Later optimizations should
be layered in this order:

- keep coalescing limited to pure pointer motion at frame boundaries when the
  target route is stable
- keep immediate flush barriers for button, key, target-crossing, drag, grab,
  and focus transitions
- use any grab/focus cache only as advisory acceleration; XLibre remains final
  authority in the prototype path
- consider an Engine-to-XLibre shared-memory route ring only after measurement,
  with the X11 request path kept as fallback

The first shared-memory ring, if built, should be unidirectional: Sophia Engine
publishes fixed-size route records and wakes XLibre with a small signal such as
`eventfd`. XLibre rejection and decision reporting can stay on the existing
control path until measurements justify a second status queue. A bidirectional
hot ring would couple the compositor's input loop to XLibre timing and should
not be introduced speculatively.

### Namespace Portals

Namespaces are private by default. Cross-namespace operations go through portal
services, not ad hoc server exceptions.

Initial portal candidates:

- clipboard and selections
- drag-and-drop
- file-open/file-save handoff
- screenshots and screen recording
- URI open requests
- notifications

The portal rule is the same everywhere: data crosses as an explicit packet with
source namespace, target namespace, type, size, policy decision, and lifetime.
Clipboard denial maps to native X11 selection failure, not synthetic input.
Pending approval holds only the transfer request for a bounded timeout; it does
not suspend either application or namespace.

The first portal implementation is the `sophia-portal` clipboard reducer. It
keeps transfers private and pending by default, accepts only text targets,
emits prompt, handoff, and fail-selection commands, and revokes pending
transfers when the source namespace owner generation changes. It does not
monitor protocol selections itself; the relevant authority or prototype bridge
observes protocol-specific ownership and converts it into portal events.

Denied clipboard transfers now have the first concrete X11 failure adapter for
the prototype path. `PortalCommand::FailSelection` maps through Sophia X Bridge
into a normal `SelectionNotify` failure with `property = None`, matching ICCCM
selection conversion failure instead of injecting synthetic input or blocking
clients.

Sophia X Bridge monitors selection ownership through XFixes
`SelectionNotify` events for `PRIMARY`, `SECONDARY`, and `CLIPBOARD`. The bridge
attributes each owner window to a known mirrored namespace when possible and
bumps a per-selection owner generation. Portal approval is bound to that
generation, so a later owner change makes old approval stale.

Sophia X Bridge also has the first requestor-side clipboard execution seam.
Given an X11 `SelectionRequest`, a resolved target atom name, the selection
owner monitor, and the mirrored namespace table, it can construct a
cross-namespace `ClipboardTransferRequest` plus the native failure reply
context. This still does not perform live X dispatch or approved clipboard data
handoff; it makes the request-to-portal boundary explicit and testable.

The runtime-facing dispatcher now accepts the real `x11rb`
`Event::SelectionRequest` variant and calls the clipboard portal reducer. It
fails closed for non-selection events, missing namespace attribution, same-
namespace requests, and unsupported targets. Approved data handoff remains
separate work.

Approved clipboard handoff is concrete for one bounded text target. When portal
approval returns `HandoffClipboard`, Sophia X Bridge can build a bounded UTF-8
property payload and a successful `SelectionNotify` whose property is the
request property rather than `None`. This is the data artifact a live X path
must later apply with `ChangeProperty` before sending the notify event.

The live X smoke now exercises both branches against an X server. It creates
selection owner/requestor windows, calls `ConvertSelection`, dispatches the real
`SelectionRequest` through the portal reducer, sends denial as
`SelectionNotify(property=None)`, then approves a second request by writing the
bounded text property with `ChangeProperty` and sending a successful
`SelectionNotify`.

The first drag-and-drop portal reducer follows the same rule. It records a
bounded list of offered transfer types, keeps the handoff pending/private until
explicit approval, binds approval to a source generation, and emits abstract
handoff or cancel commands. Xdnd/X11 event translation remains Sophia X Bridge
work; portal policy does not subscribe to X events directly.

File open/save handoff uses the same reducer boundary. It records open versus
save intent, a bounded offered-type list, an optional safe suggested filename,
and generation-bound approval state. Actual file chooser UI, file descriptors,
temporary handles, and namespace filesystem brokering are runtime work, not
portal policy state.

Screenshot and screen-recording requests are also policy-only at this layer.
The reducer records capture mode, redacted capture scope, supported output MIME
type, and generation-bound approval. Actual compositor capture, frame streaming,
redaction, and buffer handoff stay in Sophia Engine/runtime.

URI open requests are represented as explicit portal policy too. The reducer
stores only a bounded URI length, validates a conservative scheme allowlist,
requires generation-bound approval, and emits abstract handoff or cancel
commands. Until the protocol grows a dedicated URI kind, URI requests use a
`uri-open:` type hint on the generic portal transfer path.

Notification requests complete the first portal policy set. The reducer stores
bounded summary/body/action text, urgency, and generation-bound approval state.
Approved `DeliverNotification` commands now pass through an Engine chrome
presenter before becoming compositor-visible notification state. Denied or
revoked `DropNotification` commands dismiss pending or visible chrome state.
Notification history, action dispatch, and rate limiting remain runtime policy
outside the portal reducer.

### Metadata Broker And Chrome Actions

Compositor chrome is Engine/session authority, not WM authority. If the user
clicks a compositor-drawn close button, Sophia Engine hit-tests that chrome and
emits a surface-scoped close request with a generation check. Session/chrome
policy validates the request and asks the owning protocol authority to perform
the polite close path first, such as X11 `WM_DELETE_WINDOW` or a Wayland
xdg-toplevel close. The WM sees only the later consequence through
`SurfaceRemoved` or relayout requests.

The first session seam is a reducer inside Sophia Engine. A
`SessionEvent::ChromeAction` is validated against current layout nodes. Accepted
close requests emit `SessionCommand::RequestPoliteClose`, which the runtime
dispatches to the owning protocol authority or current X bridge prototype.
Rejected chrome actions emit no command. This keeps close intent out of the
blind WM protocol.

Metadata broker output follows the same ownership split. The runtime gives
Sophia Engine only `SanitizedChromeMetadata`: surface identity, optional bounded
display label, redaction bit, compositor icon token, trust level, attention
state, and generation. `ChromeBroker` turns accepted updates into
`ChromeDescriptor` values and removes descriptors only when the removal
generation is not stale.

The WM notification is a separate lifecycle event. Only after the owning
authority reports that the surface was actually removed does Sophia Engine
process `SessionEvent::SurfaceRemoved` and emit a
`WmRequestKind::SurfaceRemoved` command packet. This is the point where the WM
may relayout; a chrome close request itself never wakes the WM.

Process supervision is runtime policy, not compositor policy. Sophia Engine can
emit facts such as "WM IPC failed" or "restart the WM", but a runtime
supervisor reducer decides whether that becomes an immediate start, delayed
restart, or give-up decision based on a bounded restart policy. The supervisor
may manage the WM, portal broker, and metadata broker processes, but it does not
receive raw input, XIDs, namespace tokens, pixmaps, or portal payloads.

The process executor is below that reducer. It consumes supervisor commands,
spawns the configured child process after the bounded delay, polls for process
exit, and terminates owned children during cleanup. It does not mint restart
decisions or inspect compositor/session state.

The long-lived WM path uses the same bounded IPC frames as the in-memory socket
transport. `sophia-wm-demo serve-socket --socket=PATH` accepts repeated
Engine-owned transactions over a private Unix socket. The supervisor smoke
starts that process, commits a socket transaction, kills the child, applies the
restart policy, starts a fresh WM process, and commits a second transaction.

The compositor preserves a last-committed layout cache across WM absence. A
successful WM transaction replaces the cache. If the WM socket is missing,
malformed, timed out, or being restarted, Sophia restores that cache before
planning the next frame. Rejected layout proposals do not replace the cache.

The first session runtime step is a headless tick over existing engine data. A
tick consumes either fresh layer snapshots or an explicit restore request for
the last committed layout, then produces a frame snapshot and replay report.
This is not the final event loop; it is the smallest executable coordinator
between X-derived layer state, cached layout state, and frame planning.

The continuous runtime loop now has a data-only reducer. `SessionRuntimeState`
tracks the current phase and counters for X events, rendered frames, drained
portal commands, presented chrome commands, WM restart requests, and the last
frame serial. `update_session_runtime` consumes runtime facts such as
`TickStarted`, `XEventsPolled`, `WmLayoutReady`, `FrameScheduled`, and
`FrameRendered`, then emits explicit commands like `PollXEvents`,
`RequestWmLayout`, `ScheduleFrame`, `RenderFrame`, `DrainPortalCommands`, and
`PresentChrome`. This keeps the event loop assembly testable before wiring it
to real file descriptors.

`SessionRuntimeLoop` is the first reusable shell around that reducer. It accepts
bounded batches of already-observed runtime events, preserves reduced state
between batches, and returns the non-empty command stream for the outer runtime
driver to execute. It still does not own X polling, broker sockets, WM socket
reads, portal execution, renderer commits, or libinput/DRM file descriptors.
Those adapters should translate external facts into `SessionRuntimeEvent`
values, then feed the loop.

`SessionRuntimeObservation` is the bounded intake form for those adapters. It
admits only scalar runtime facts: X event counts, WM layout/restart outcomes,
frame serials, portal/chrome command counts, and broker health state plus
status-message length. `SessionRuntimeEventBatch` rejects batches larger than
`MAX_SESSION_RUNTIME_OBSERVATION_BATCH` and rejects broker status lengths beyond
the broker health packet cap. It does not carry raw X events, XIDs, namespace
tokens, clipboard bytes, labels, icons, or renderer buffers.

Concrete producer wiring now follows that rule. Sophia Engine exposes helper
conversions from `WmTransactionUpdate`, `SessionTickReport`,
`RenderFrameReport`, portal command slices, metadata chrome updates, and
notification chrome updates into `SessionRuntimeObservation` values. The CLI
runtime smokes use the same intake path for X capture counts, broker health
decode results, WM transaction results, portal drain counts, chrome presentation
counts, and rendered frame reports. These helpers translate facts; they do not
poll file descriptors or execute runtime commands themselves.

`HeadlessSessionDriver` is the first reusable command executor around this
loop. It owns a `SessionRuntimeLoop` and last-committed layout cache, starts a
tick with `TickStarted`, then executes emitted runtime commands through
deterministic headless adapters: X event count intake, WM transaction
observation, frame scheduling, session tick rendering, portal command counting,
and chrome command counting. It is still not the production compositor loop: it
does not block on file descriptors, supervise processes, or own real libinput,
DRM/KMS, X sockets, or broker sockets.

The generic runtime tick and damage-epoch CLI smokes now use this driver instead
of hand-rolling command reduction. Broker-health smokes and the external-WM
smoke still call the observation seam directly because they are intentionally
adapter-level checks: health packet decoding and WM transaction observation,
not full session ticks.

`RuntimeDriverAdapter` is the live-source seam for the same executor. The
driver asks an adapter to answer each runtime command with a reduced
observation or concrete frame report: X event count, WM layout/restart result,
frame scheduling/rendering, portal drain count, and chrome presentation count.
`HeadlessRuntimeAdapter` preserves deterministic test behavior.
`LiveRuntimeDriverIntake` is the non-blocking handoff shape for live sources:
the X bridge, WM socket, broker IPC, portal execution, chrome presenter, and
renderer reduce their own data into bounded facts before the runtime executor
sees them. This keeps raw X events, namespace authority tokens, metadata
strings, portal payload bytes, and renderer payload buffers out of the reducer.

The live adapters still do not poll file descriptors themselves. The eventual
session loop owns readiness and process supervision, gathers reduced facts from
each boundary, builds `LiveRuntimeDriverIntake`, then lets the common executor
advance the runtime phases.

The headless runtime tick smoke now drives this reducer around the existing
capture -> session tick -> replay path. It executes the reducer's X polling,
WM policy, frame scheduling, render, portal drain, and chrome presentation
commands in order and reports the resulting runtime counters beside the frame
snapshot/replay counts.

`x-smoke-live-runtime-wm-socket` is the first combined live command-executor
smoke. Under the XLibre/Xvfb smoke script it captures real XComposite layers,
requests a relayout from the long-lived WM socket server, commits that
transaction into layer snapshots, and then lets `HeadlessSessionDriver` execute
the runtime commands through `LiveRuntimeDriverAdapter`. This proves the live
X bridge and WM socket can feed the shared executor without a per-smoke
runtime mini-loop.

`runtime-damage-epoch-smoke` exercises the next runtime seam without requiring a
live slow-resize client. It creates an X-shaped `DamageFrame`, completes a
layout epoch through `schedule_frame_from_damage`, then drives the runtime
reducer through frame scheduling, rendering, portal drain, and chrome
presentation. The full XLibre smoke script runs this check alongside the live X
capture smokes.

Portal and metadata brokers now have process-supervised placeholders.
`RuntimeBrokerSupervisors` owns one `ProcessSupervisor` for `PortalBroker` and
one for `MetadataBroker`; `runtime-brokers-smoke` starts both placeholder
processes and observes their exits. This proves restart/supervision ownership
without claiming the broker IPC protocols or UI surfaces are implemented.

The first broker IPC contract is health/control only.
`BrokerHealthPacket` is intentionally small: broker kind, coarse health state,
generation, and an optional bounded status message. It cannot carry raw client
metadata, namespace labels, XIDs, portal payloads, URIs, file paths, or icon
bytes. Runtime may later consume this packet to mark a broker ready, degraded,
or stopped, but real portal execution and sanitized chrome metadata remain
separate protocols with their own validation. The portal broker placeholder now
has a bounded health-frame smoke that encodes and decodes the packet over the
shared Sophia IPC frame header; the metadata broker placeholder uses the same
control frame. The session runtime reducer consumes decoded health as reduced
state only: broker health state, generation, and status-message length.

Frame scheduling now has an explicit seam. `FrameClock` produces output-scoped
frame ticks, and the deterministic headless implementation advances serials
without depending on wall-clock time. A real DRM/KMS backend should implement
the same boundary from vblank/page-flip timing while preserving the session-tick
contract: clock tick in, committed surface state selected, frame snapshot and
replay/commit report out.

Authority transaction commits are now tied to that same presentation boundary.
`PageFlipCommitGate` stages a batch of ready authority `SurfaceTransaction`
records for a specific output and transaction ID. On a matching
`FrameClockTick`, the gate validates the staged transactions against
`SurfaceVisualStateTable` and commits them as the new visual truth only if they
are ready. If the tick belongs to another output, or any transaction is still
pending/timed out, the gate preserves the last committed surface state and keeps
the batch staged. This gives the DRM/KMS backend a concrete page-flip seam
without making tests depend on real scanout hardware.

The XLibre prototype scheduler may still consume X Damage. In that path,
`schedule_frame_from_damage` combines a frame-clock tick, an optional X-derived
`DamageFrame`, and an optional layout epoch. If no damage exists, the scheduler
waits. If a layout epoch is pending, damage from affected surfaces retires
pending surface IDs; rendering waits until the epoch completes. When damage is
present and the epoch is complete, the scheduler emits a render decision with
the tick's frame serial.

Authority-native scheduling should instead treat protocol damage as one input
to `SurfaceTransaction` readiness. The Engine's commit gate is the transaction
validator: valid surface, non-empty target geometry, concrete buffer source,
`Ready` state, and matching previous committed generation. Pending, timed-out,
failed, malformed, or stale transactions keep the last committed visual state
unless an explicit timeout policy produces a degraded artifact.

Resize behavior measurement is tied to the same epoch state. `LayoutEpochState`
records start time and timeout policy, and `measure_resize_behavior` reports
elapsed time, pending surfaces, completion, and timeout status. Slow or
non-cooperative clients therefore become explicit samples instead of implicit
black frames or hidden scheduler stalls.

Only XLibre prototype layers marked `ResizeSyncCapability::ExplicitSync`
participate in a layout epoch. Mixed resize transactions therefore wait for
cooperative clients without letting legacy clients hold the whole frame hostage.
If the epoch times out, Sophia clears the pending set, renders the fallback
frame from committed state, and leaves the bridge to decide whether the client
class should be downgraded for future snapshots.

The DRM/KMS output backend now has a discovery boundary. `DrmKmsMode`,
`DrmKmsOutputDescriptor`, and `DrmKmsOutputRegistry` preserve connector ID,
CRTC ID, mode, scale, and Sophia `OutputId`. `DrmKmsSysfsDiscovery` can discover
connected outputs and active modes from a sysfs-style `/sys/class/drm` tree and
seed engine output state from that data. Sysfs cannot fully replace a libdrm
ioctl backend: when CRTC IDs are not available, `crtc_id = 0` means the output
is discovered but not yet scanout-bound.

The libinput backend starts with the same adapter discipline.
`LibinputDeviceDescriptor` records the seat, device, and broad device kind that
future libinput discovery will produce. `LibinputEventSource` accepts
`InputEventPacket` values only from registered device/seat pairs and drains them
in order for the routing pipeline. `NonBlockingInputPoller` and
`LibinputPhysicalInputAdapter` define the runtime seam: a real libinput backend
will dispatch ready file-descriptor events without blocking the engine loop,
while the deterministic `QueuedInputPoller` keeps tests independent of kernel
devices.

Physical input now has a request-generation seam. After Sophia Engine produces
an `InputRoute`, `routed_input_request_from_physical_event` combines the
physical `InputEventPacket` with the accepted route and emits an
`XLibreRoutedInputRequest`. The adapter rejects serial mismatches, denied or
unrouted outcomes, missing target windows, and missing local coordinates. A
coalescer flush can be converted into a bounded batch of routed-input requests
without involving WM policy.

Scene hit-testing now handles transformed layer geometry. Sophia Engine walks
renderable layers from highest stack rank to lowest, inverts each layer's
transform against the physical pointer position, checks the untransformed layer
geometry, and emits an `InputRoute` with target-local coordinates. That route
feeds the same routed-input request generator, so compositor-side transforms are
resolved before XLibre receives the target XID and local position.

XFixes selection owner updates are the first portal execution input. Sophia X
Bridge converts owner-generation changes into source-namespace clipboard events;
the clipboard portal reducer uses those events to revoke stale pending
transfers. X11 `SelectionRequest` context can now become a bounded clipboard
portal import request and native failure reply context, and the X bridge can
dispatch the real `Event::SelectionRequest` into the clipboard portal reducer.
Approved text handoff now produces a bounded property payload and success
notify artifact. The live smoke verifies those artifacts against real X
property writes and event delivery for one text target. General target
negotiation and full clipboard ownership brokering remain future work.

## Protocol Authority Responsibilities

Each protocol authority is responsible for:

- parsing and replying to its client protocol
- client resource ownership and protocol-local object IDs
- namespace checks for every resource, event subscription, and transfer request
- protocol-specific selections, focus, grabs, configure/ack, and lifecycle
  semantics
- reducing client buffers, damage, constraints, and readiness into Sophia
  surface transactions
- converting protocol-specific metadata into sanitized metadata candidates

Authorities must not duplicate Sophia Engine's visual object graph. They emit
surface transactions and lifecycle facts; Sophia Engine remains the source of
truth for committed visual placement and scanout.

The XLibre prototype remains responsible for the same X11 concepts while that
prototype is in use. The long-term Sophia X Authority should bring those
responsibilities into a smaller, namespace-aware X subset owned by Sophia.

## Sophia Responsibilities

Sophia owns:

- physical input devices
- output configuration
- scene graph and transforms
- damage aggregation and frame scheduling
- final composition
- global shortcuts
- compositor-to-WM policy protocol
- portal UI hooks
- atomic geometry-plus-buffer commits

Sophia Engine can cache authority state, but protocol authorities remain the
source of truth for protocol resources. Sophia Engine is the source of truth for
visual state.

## Next Research Thread

The next useful proof is not a full desktop. It is a design-to-code transition
from XLibre prototype seams into a Sophia-owned authority:

1. Define the minimum X protocol subset for real applications.
2. Define the namespace-aware X resource model.
3. Define `SurfaceTransaction` and `CommittedSurfaceState` semantics.
4. Translate X Present/DRI3/SHM/Render paths into pending buffer readiness.
5. Translate X selections and drag-and-drop through protocol-neutral portals.
6. Keep the blind WM protocol unchanged.
7. Verify that slow clients preserve the last committed visual state.

The XLibre/Xvfb smokes remain valuable regression evidence for compatibility
ideas, but they are no longer the destination architecture.

## Reference Boundaries

Use each reference at the boundary where it is strongest:

- niri: Rust/Smithay backend patterns, frame scheduling, renderer integration,
  headless test scaffolding.
- picom: XComposite/Damage flow, X window mirror, layer snapshots, render
  command planning, damage over buffer age.
- river: external WM protocol shape, manage/render sequence thinking, crash
  isolation for policy.
- Phoenix: clean-room modern X server feasibility and real-app compatibility
  lessons.
- XLibre: namespace enforcement, X11 delivery semantics, and routed-input
  prototype lessons.
- macOS WindowServer/Core Animation: transaction-first rendering and fail-closed
  visual integrity.
