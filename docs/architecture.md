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
 └───────────────┬───────────────────┬────────────────────┬───────────────────┘
          ▲      │                   │                    │      ▲
          │      │ opaque snapshots  │ portal events      │      │ chrome data
          │      ▼                   ▼                    ▼      │
 ┌───────────────┐        ┌────────────────┐       ┌─────────────────────────┐
 │  SOPHIA WM    │        │ SOPHIA PORTALS │       │ METADATA BROKER/CHROME  │
 │ blind policy  │        │ allow/deny     │       │ redacted UI only        │
 │ layout/focus  │        │ handoff/revoke │       │ labels/icons/badges     │
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

Sophia now has a bounded native Wayland authority built on Smithay. A real
Kitty client connects to its private socket with no X server, commits SHM
buffers, and receives Engine-routed keyboard and pointer events. The same
protocol-neutral `SurfaceTransaction` and `RoutedInputRequest` records serve
the Wayland and modern-X authorities; Engine contains no XLibre window or wire
identity.

The native session admits a narrow linear XRGB/ARGB DMA-BUF subset. Its repaired
three-frame controlled hardware proof, GDB-backed 300-frame diagnostic,
release-timing trace, and three retained normal 300-frame lifetime proofs pass.
The route remains experimental until the real-Kitty hardware evidence passes.
SHM is the current verified path. When the experimental route is enabled,
KMS submission remains backend-owned and Wayland frame/buffer feedback waits
for the matching observed presentation. The production CLI and installed Kitty
launcher neither link nor start XLibre; its bridge remains an opt-in research
fixture. The active gates are native renderer lifetime safety, a controlled
DMA-BUF first-frame and 300-frame lifecycle proof, then three guarded real
Kitty runs covering input, resize, latency, TTY recovery, and DMA-BUF
presentation.

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

A bounded **Sophia Wayland Authority** now supports Wayland-only applications by
terminating `wl_surface`, `xdg_toplevel`, buffer attach, damage, and commit
semantics, then emitting the same internal surface transactions as the X
authority. Its boundary is documented in
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
the namespace or metadata boundaries. The Engine never selects, launches, or
names a particular legacy WM. The bridge accepts a configured WM executable and
arguments; xmonad is only the first compatibility proof. Adding support for a
legacy WM may extend the bridge's fake-X11 coverage but must not add WM-specific
logic to Sophia Engine.

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

The archived patch target is tracked in
`research/xlibre/docs/xlibre-routed-input-extension.md`. This is
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
portal commands, presented chrome commands, scanout submissions, scanout
retirements, scanout rejections, in-flight scanouts, WM restart requests, and
the last frame serial. `update_session_runtime` consumes runtime facts such as
`TickStarted`, `XEventsPolled`, `WmLayoutReady`, `FrameScheduled`, and
`FrameRendered`, then emits explicit commands like `PollXEvents`,
`RequestWmLayout`, `ScheduleFrame`, `RenderFrame`, `SubmitScanout`,
`DrainPortalCommands`, and `PresentChrome`. Rendering is therefore not treated
as final presentation. A rendered frame enters `SubmittingScanout`; only a
reduced `ScanoutStateChanged` observation moves the runtime on to portal drain
and chrome presentation. This keeps the event loop assembly testable before
wiring it to real file descriptors.

`SessionRuntimeLoop` is the first reusable shell around that reducer. It accepts
bounded batches of already-observed runtime events, preserves reduced state
between batches, and returns the non-empty command stream for the outer runtime
driver to execute. It still does not own X polling, broker sockets, WM socket
reads, portal execution, renderer commits, or libinput/DRM file descriptors.
Those adapters should translate external facts into `SessionRuntimeEvent`
values, then feed the loop.

`SessionRuntimeObservation` is the bounded intake form for those adapters. It
admits only scalar runtime facts: X event counts, WM layout/restart outcomes,
frame serials, reduced scanout submitted/retired/rejected state,
portal/chrome command counts, and broker health state plus status-message
length. Authority process health follows the same rule:
`AuthorityProcessHealthChanged` records only supervised process kind, coarse
health state, generation, and status-message length. `SessionRuntimeEventBatch`
rejects batches larger than `MAX_SESSION_RUNTIME_OBSERVATION_BATCH` and rejects
broker or authority status lengths beyond the broker health packet cap. It does
not carry raw X events, XIDs, namespace tokens, clipboard bytes, labels, icons,
or renderer buffers.

Concrete producer wiring now follows that rule. Sophia Engine exposes helper
conversions from `WmTransactionUpdate`, `SessionTickReport`,
`RenderFrameReport`, portal command slices, metadata chrome updates, and
notification chrome updates into `SessionRuntimeObservation` values. The CLI
runtime smokes use the same intake path for X capture counts, broker health
decode results, WM transaction results, portal drain counts, chrome presentation
counts, and rendered frame reports. These helpers translate facts; they do not
poll file descriptors or execute runtime commands themselves.
The runtime driver's default scanout adapter answers `SubmitScanout` with a
reduced submitted observation for deterministic tests. A live adapter should
replace that placeholder with backend-owned rendered primary-plane submit and
later page-flip retirement/rejection observations.
`LiveRuntimeDriverIntake` now carries an optional reduced scanout submit state
for that handoff. Backend-live maps rendered primary-plane submit status into
`Submitted`, `Deferred`, or `Rejected`, then the live runtime adapter converts
that fact into `ScanoutStateChanged` when the executor reaches `SubmitScanout`.
`Deferred` is the backpressure case: a previous KMS submission is still waiting
for accepted page-flip evidence, so the runtime advances the TEA loop without
incrementing submissions, retirements, or rejections.
Backend-live also has a command-time rendered scanout adapter for the live
runtime tick. When the executor reaches `SubmitScanout`, that adapter exports
the rendered GBM frame target, submits the primary-plane atomic request, retains
the backend-owned scanout resources, and returns only the reduced runtime state.
This keeps real scanout submission phase-aligned with the TEA loop instead of
precomputing native KMS work before the runtime asks for it.
The native GBM variant can use a reusable backend-live exporter object that owns
render-device discovery and records only reduced export health. That exporter
initializes a persistent renderer-live GBM/EGL rendered-scanout context on the
first valid export, then reuses it across runtime ticks. If context startup or
render-device discovery fails, the runtime sees only reduced scanout rejection;
no file descriptor, path, GBM handle, EGL display, or native error crosses into
Engine state.
The production-shaped native runtime tick combines that persistent exporter
with native page-flip intake. It drains and reduces page-flip evidence before
the next rendered primary-plane submit, so an accepted callback retires the
previous GBM/KMS owner before the exporter reuses or recreates the next rendered
scanout buffer.
The same tick can run with a libinput-shaped input poller owned by the live
runtime assembly. Reduced physical input packets enter the Engine before the
runtime reaches scanout submission, while page-flip evidence still retires old
GBM/KMS owners before the next submit. This proves the sequencing shape for the
future production loop: input readiness, page-flip retirement, rendering, and
atomic scanout are coordinated by backend-live without putting native fds or
device identity into Engine state.
The reusable exporter also records the last reduced frame-target lifecycle:
created, retained, resized, invalidated, or retired. This gives the backend
resize/target-continuity evidence for production scanout while keeping native
GBM/EGL resources and identities private.
KMS scanout readiness now also checks that the reduced GBM/EGL frame target
size matches the selected output size. A valid-looking but mismatched target is
reported as reduced frame-target-size-mismatch and blocks page-flip readiness
before any native primary-plane submit is attempted. The rendered primary-plane
submit path consumes that same reduced readiness status, so active
`SubmitScanout` commands reject before renderer export or native KMS work when
the scanout target is not ready.
For rendered primary-plane scanout, backend-live can also retain the combined
rendered-buffer owner and KMS submission owner internally. Stale page-flip
evidence keeps that owner in flight. Accepted presented page-flip evidence
retires the owner and maps to reduced `Retired` state; resource retirement
failure maps to reduced `Rejected` state. While an owner stays in flight,
backend-live also records a reduced in-flight tick age so a stalled page flip is
observable without exposing native object identity or forcing an unsafe early
retire. A reduced backpressure report classifies the owner as idle, waiting for
page-flip evidence, or stalled past a caller-provided threshold. Threshold `0`
keeps the report observational only and never marks the owner stalled.
Tracked rendered submissions also remember the last reduced page-flip sequence
observed before submit. Retirement requires accepted page-flip evidence newer
than that baseline, so replayed or pre-submit callbacks cannot retire the
current GBM/KMS owner.
If framebuffer/blob cleanup fails during resource creation, submit failure, or
after an accepted page flip, backend-live retains an opaque cleanup owner with
the rendered buffer owner when one exists. The runtime may retry that cleanup
later; it must not drop native handles or pretend cleanup succeeded. Runtime
tick reports and atomic scanout smoke evidence expose only the reduced
cleanup-pending bit.
Device-backed rendered scanout ticks opportunistically retry pending cleanup
once before processing new rendered scanout submission, and report the reduced
cleanup-retry result on the tick.
If cleanup is still pending after that retry, backend-live defers the new
submission. This keeps cleanup debt bounded to one opaque owner and prevents a
new scanout from replacing the retained native cleanup state before the old
framebuffer/blob cleanup has finished.
Those terminal reduced states are queued inside backend-live and drained into
the next runtime tick as scanout lifecycle observations. The shared reducer
records the retirement or rejection without treating it as a fresh render
pipeline transition; only the active `SubmitScanout` response advances into
portal and chrome phases.
The rendered primary-plane runtime tick now consumes the latest accepted reduced
page-flip callback before it drains lifecycle state into the Engine. That lets a
single live tick retire the previous GBM/KMS owner, record reduced `Retired`
state, and submit the next rendered frame without exposing native handles or
temporarily counting the backpressured frame as rejected.

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
frame scheduling/rendering, scanout submission state, portal drain count, and
chrome presentation count.
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
WM policy, frame scheduling, render, scanout submit, portal drain, and chrome
presentation commands in order and reports the resulting runtime counters
beside the frame snapshot/replay counts.

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
Backend-live observes that seam through `LiveAtomicScanoutCommitReport`, a
reduced commit report that records only idle/waiting/committed/rejected state
and the derived page-flip event. It deliberately drops transaction IDs, surface
IDs, and native scanout identity before the runtime tick observes the result.
`LiveAtomicScanoutCommitter` is the backend-owned interface that produces that
report; a real KMS implementation will replace the deterministic fake committer
without widening the runtime observation shape.
If a native page-flip callback is used as the commit trigger, backend-live must
validate it first. Accepted callbacks may advance the atomic commit report only
when the reduced output route matches, the frame serial is monotonic, and any
terminal Engine outcome carries the same frame serial. Rejected callbacks keep
the compositor in a fail-closed waiting or rejected state.
`NativeLibdrmPageFlipEventReader` is the first concrete native reader for that
path. It is compiled only behind `libdrm-events`, reduces DRM page-flip events
through private CRTC routes, and emits only backend-local slots and frame
serials into the existing reduced callback pipeline.
The live rendered-primary-plane runtime path can now consume that reader in one
tick: backend-live reads and polls native page-flip events into the bounded
callback queue, updates reduced poller diagnostics, drains accepted callbacks to
retire in-flight KMS/GBM owners, and only then submits the next rendered
scanout. This is the production sequencing path; manual queue injection remains
only a deterministic test seam.
`NativeLibdrmAtomicScanoutCommitter` is the matching native submit boundary. It
may submit a backend-owned `AtomicModeReq` to DRM/KMS, but a successful ioctl
only means the kernel accepted the request. Sophia still waits for the reduced
page-flip callback before making the visual commit observable.
The first native request builders cover the simple full-output primary-plane
case. The modeset builder packages connector, CRTC, plane, framebuffer, mode
blob, and rectangle properties behind backend-live types. The steady page-flip
builder packages only the plane framebuffer and rectangle properties. Both
export only reduced build status.
The matching KMS selector can choose a connected connector, usable encoder/CRTC,
display mode size, and compatible primary plane from real DRM resources while
keeping those native handles backend-private.
The resource lifecycle seam can then create the initial modeset mode blob,
register a scanout framebuffer from a renderer-owned buffer, and retire the
owned resources without exposing native IDs outside backend-live. Steady
page-flip resources are framebuffer-only and do not require a fresh mode blob.
Renderer-live now exposes a reduced scanout-buffer descriptor for that path:
size, pitch, XRGB8888 format, and GEM handle. Backend-live converts only ready
descriptors into a private DRM buffer adapter before framebuffer registration.
Behind the `gbm-probe` feature, renderer-native-egl can now allocate and own a
GBM scanout buffer object while renderer-live exposes only the reduced
descriptor. The owner object must be retained until backend-live retires the
framebuffer resource derived from that descriptor.
The production-facing renderer path now has a stronger variant:
renderer-native-egl can render a known clear color into a GBM surface, swap it,
lock the front buffer, and expose that XRGB8888 front buffer through the same
reduced descriptor. The raw allocated-buffer exporter remains useful for
resource-shape tests; real scanout smokes should prefer the rendered front
buffer path.
Property discovery for that path now uses the native DRM property APIs and
collapses lookup problems to reduced missing-property groups. The property
handles themselves stay backend-private.
The primary-plane scanout submit seam now chains those pieces together: select
the KMS target, validate a renderer scanout descriptor, create the required
native resources for the submit policy, build the atomic request, submit it,
and retain an opaque submission owner. Initial modesets own a framebuffer and
mode blob; steady page flips own only the framebuffer. That owner is not visual
truth. The runtime may retire it only after native page-flip evidence has passed
the reduced callback checks.
Submitted scanout retirement follows the same rule: an accepted, presented
page-flip callback retires the framebuffer resources; stale, wrong-output, or
rejected callbacks return the owner to the caller so in-flight resources stay
alive.
`LiveBackendRuntimeAssembly::submit_rendered_primary_plane_scanout_with` is the
runtime-facing version of that chain. It starts from the current reduced
`LiveGbmEglFrameTargetRecord`, asks a rendered scanout exporter for a ready
front-buffer descriptor plus its opaque owner, submits the descriptor through
the primary-plane path, and returns a combined owner. The rendered buffer owner
and the KMS submission owner then travel together until accepted page-flip
evidence allows `retire_rendered_primary_plane_scanout_after_page_flip` to drop
both safely. The opt-in hardware smoke follows the same lifetime rule: its
rendered front-buffer owner is wrapped with the primary-plane submission before
waiting for page-flip evidence, so the proof does not validate a framebuffer
whose renderer-owned backing storage has already been released.
The persistent runtime stores this lifecycle in a bounded table keyed by
`OutputId`. Frame targets, page-flip intake, submitted/displayed owners, cleanup
debt, and retirement cannot cross output entries. Native discovery assigns
disjoint connector/CRTC/primary-plane chains deterministically and page-flip
routes carry only the reduced `OutputId` into Engine-facing state.
Runtime rendered-primary-plane submit reports preserve the reduced native submit
stages as well: property discovery, resource creation, atomic request build, and
atomic commit submit. This lets the production loop explain why a scanout was
rejected without exposing DRM object IDs, GBM handles, or authority-bearing file
descriptors. The report also has a schema-versioned reduced log line,
`sophia_runtime_rendered_scanout_submit`, for capturing runtime submit evidence
without depending on Rust debug formatting. Submit schema 6 includes the reduced
output size observed by the runtime, the reduced GBM frame-target size, and
reduced scanout-buffer format, modifier, and plane-count shape, so captured
evidence can prove the rendered buffer was sized for the output snapshot that
reached native submit and show the broad buffer layout handed to KMS. It also
records whether the selected primary plane exposed an `IN_FORMATS` table,
reduced framebuffer-creation detail showing which AddFB path registered the
framebuffer, and a reduced cleanup-pending bit so submit failures that retain
native cleanup debt are visible immediately.
Runtime retirement and cleanup reports expose matching reduced lines:
`sophia_runtime_rendered_scanout_retire` records accepted, waiting, and
retire-failed page-flip outcomes; `sophia_runtime_rendered_scanout_cleanup`
records cleanup retry status and whether native cleanup debt remains. Together
the three runtime lines describe the production submit-to-retire path without
exposing native object identity.
Runtime evidence capture also emits
`sophia_runtime_rendered_scanout_failure` when the proof path cannot produce a
submit report, cannot continue ticking after submit, or times out before
retirement. That line gives operators a reduced failure reason while preserving
the clean-proof rule: only one submitted line plus one clean retired line proves
the runtime scanout path.
Native primary-plane submit can also consume a preselected KMS target snapshot.
That path is required when the Engine has already sized a rendered frame target
from a specific connector/CRTC/plane selection; readiness, buffer production,
and atomic submit must refer to the same reduced target snapshot.
The runtime rendered-primary-plane path performs the same check before export:
after the reduced KMS target is ready and a frame target exists, backend-live
selects the native KMS target once, verifies that the selected size still
matches the frame target, and only then asks the renderer for a scanout buffer.
If the native target changed or disappeared, the runtime reports a reduced
not-ready scanout target and leaves the renderer untouched.
Runtime submits that selected target with page-flip policy, so the reduced
commit flags keep `ALLOW_MODESET` false. Modeset permission remains explicit for
the opt-in hardware smoke and future target-reconfiguration paths.
The shared session runtime now has a matching reduced lifecycle: after
`RenderFrame`, it emits `SubmitScanout` and records submitted, retired, or
rejected scanout state without seeing framebuffer IDs, KMS handles, or GBM
objects. That makes the rendered primary-plane seam the future live answer to
the runtime command rather than an ad hoc backend side path.
The opt-in hardware smoke records that chain through
`LibdrmNativeAtomicScanoutSmokeEvidence`: persistent rendered context startup,
KMS scanout target readiness, GBM export, primary-plane submit, reduced commit
phase, scanout-buffer import status, scope and flags, native page-flip polling,
callback intake, reduced page-flip wait outcome, retirement, retire-time
resource destroy, and the evidence schema version collapse to reduced fields
only. Passing hardware evidence must include an initial modeset phase and a
steady page-flip phase, and each phase must reduce the page-flip wait to
`Retired`. A non-ready target or invalid scanout buffer fails the smoke evidence
before a successful submit can pass. The report deliberately omits
card paths, file descriptors, EGL displays, KMS object IDs, framebuffer IDs, and
GEM handles.

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

The live runtime adapter now has a protocol-neutral authority batch intake.
`AuthorityTransactionIntake` carries only a Sophia `TransactionId` and bounded
`SurfaceTransaction` values. The adapter commits those batches through
`HeadlessEngine::commit_surface_transactions` before projecting committed
surface state into renderable layers. Sophia X Authority can adapt its bounded
`XAuthorityObservedTransactionBatch` into this shape at the process/runtime
edge without making Sophia Engine depend on the X Authority crate.

The long-running Sophia X Authority side channel is the preferred runtime path
for observed drawing and presentation transactions. Socket dispatch attempts a
nonblocking send of `XAuthorityObservedTransactionBatch` into a bounded channel;
the runtime edge converts that batch into `AuthorityTransactionIntake`. Callback
observers remain useful for focused tests, but the session path should consume
the bounded channel so backpressure and disconnection fail closed instead of
allocating unbounded transaction history.

`RuntimeAuthoritySupervisor` is the first process wrapper for this path. It
owns a `ProcessSupervisor` for `SupervisedProcessKind::SophiaXAuthority` and
translates process start, health, exit, and termination into reduced
`AuthorityProcessHealthChanged` observations. It does not carry socket paths,
XIDs, namespace IDs, or authority resource tables into the runtime reducer.

The first runtime-owned compositor backend assembly is deterministic and
headless. `HeadlessCompositorBackendAssembly` holds the Engine, session driver,
frame clock, DRM/KMS output registry, libinput adapter, renderer selection, and
committed surface cache. One `run_tick` polls physical input once, advances the
frame clock, commits authority transaction batches through the live runtime
adapter, runs the common session driver, and renders the produced frame through
the selected renderer. When configured with an `AuthorityTransactionInbox`, the
same tick drains ready protocol-neutral authority batches from a bounded channel
before running the session driver. This proves the ownership boundary before
real kernel file descriptors enter the loop: the assembly coordinates backends,
but it does not own protocol policy, WM layout semantics, portal policy, or
client resources.
The assembly is generic over `NonBlockingInputPoller`, not boxed. This keeps the
input hot path allocation-free and monomorphized while preserving
`QueuedInputPoller` as the default deterministic implementation. Native-shaped
libinput readers can therefore drive the same runtime tick without changing
Sophia Engine or the session driver.

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
The first concrete libinput reader is admitted only behind `libinput-events` and
uses the safe Rust `input` wrapper rather than hand-written FFI. It accepts a
backend-owned `Libinput` context plus a reduced seat/device map, dispatches
ready events, and converts pointer motion, pointer button, and keyboard key
events into `InputEventPacket` values. Native device paths, fd values, seat
names, and libinput error strings do not enter public reports.
The next production input-loop seam is fd readiness gating. The concrete reader
must not become the session selector. Backend-live should observe readiness in
the outer loop, call libinput dispatch only when ready, and continue scanout
ticks when input is idle.
Backend-live now has the first form of that seam as a one-shot readiness-gated
poller. A tick without a readiness token returns an empty reduced input batch
and still advances the runtime. A tick after readiness is observed consumes the
token and calls the wrapped concrete poller exactly once. The future session
loop must own the actual `poll`/`epoll` wait and feed these readiness tokens;
Sophia Engine still sees only `LibinputPollReport` and accepted input packets.
The live backend now also has a reduced session-loop owner for the combined
path. `LiveBackendSessionLoop` owns the page-flip poller and bounded read/emit
budgets. Each tick accepts only `LiveBackendSessionLoopReadiness`, currently an
input-ready bit and a page-flip-ready bit from the outer selector. The runtime
observes the input token, reads native page-flip callbacks only after reduced
page-flip readiness is observed, drains already-pending decoded callbacks under
the bounded emit budget, retries/retires rendered scanout ownership, and submits
the next rendered primary-plane frame in one bounded tick. Real file
descriptors and selector identity remain outside Sophia Engine state.
`LiveBackendReadinessCollector` is the current reduced collector shape: it
records only one-shot readiness booleans and drains them into the session loop.

The first live compositor backend boundary is dependency-neutral. An
`OutputDiscoveryBackend` produces a `DrmKmsOutputRegistry`; an
`InputDiscoveryBackend` produces a `LibinputEventSource`. The current concrete
implementations are still conservative: `SysfsDrmKmsOutputBackend` reads
sysfs-style connector data and `StaticInputDiscoveryBackend` seeds deterministic
input devices. `discover_live_compositor_backend` returns a reduced
`LiveCompositorBackendDiscoveryReport`. If output discovery fails, or no
connected output exists, it returns no selected output and creates no backend
assembly. That is the fail-closed rule for the live compositor seam: protocol
authorities, WM IPC, and portal state are not started just because kernel
discovery partially succeeded.

`sophia-backend-live` is the crate boundary for real kernel-facing backend
dependencies. Today it only wraps the dependency-neutral discovery traits and
can seed a headless assembly from sysfs fixtures and static input descriptors.
Future libdrm, libinput, GBM/EGL, or renderer import code should land in that
crate first. `sophia-engine` should continue to expose stable data contracts
and deterministic tests, not direct kernel IO ownership. The admission rule is
tracked in `docs/live-backend-dependency-policy.md`: libdrm and libinput may
enter through live discovery and polling seams, while GPU imports and MIT-SHM
mapping stay deferred until separate renderer import boundaries exist.

The renderer import boundary is its own seam, documented in
`docs/renderer-import-boundary.md`. Backend discovery produces reduced output
and input records; renderer import admission consumes already-validated
`BufferSource` values and decides whether CPU upload, XPixmap import, DMA-BUF
import, or a future shared-memory path may be used for this frame. This prevents
device enumeration code from growing into buffer ownership.

Physical input now has a request-generation seam. After Sophia Engine produces
an `InputRoute`, `routed_input_request_from_physical_event` combines the
physical `InputEventPacket` with the accepted route and emits an
`XLibreRoutedInputRequest`. The adapter rejects serial mismatches, denied or
unrouted outcomes, missing target windows, and missing local coordinates. A
coalescer flush can be converted into a bounded batch of routed-input requests
without involving WM policy.
Runtime ticks now report this separation explicitly. `PhysicalInputIntakeReport`
mirrors the reduced libinput poll result, records the number of queued physical
events, and reports `PhysicalIntakeOnly` as the routing stage. A backend tick
may ingest and queue physical input, but scene hit-testing and routed-input
request generation remain separate routing-layer calls.

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

The next proof is one visible, interactive X terminal rather than a broad
desktop. The implementation order is:

1. Back the xterm-driven core drawing subset with bounded XRGB8888 pixels.
2. Resolve and compose those pixels into a renderer-owned scanout frame.
3. Run X Authority, backend ticks, scanout, and xterm under one persistent
   session owner.
4. Deliver focused keyboard events through X Authority semantics.
5. Connect the live session to the generic synthetic X11 WM bridge and use
   xmonad as the first proof that a configured legacy WM can provide blind
   layout policy without an Engine-specific integration.
6. Measure repeated-tick latency, queue pressure, frame age, and cleanup debt.
7. Start Wayland Authority only after those X-session gates pass.

XLibre/Xvfb smokes remain regression evidence for compatibility ideas, but they
are not the destination architecture and are not part of the xmonad bridge.

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
