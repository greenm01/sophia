# Research Log

This file records early decisions, assumptions, and open questions. Keep it
short and chronological.

## 2026-07-07: Project Name

The project is named **Sophia**.

Sophia means the XLibre-centered modern X11 architecture, not a Wayland
compositor with an X11 compatibility sidecar.

## 2026-07-07: Identity

Sophia is a research prototype for a modern X11 session:

- XLibre remains the client-facing X11 server.
- XLibre remains the Xnamespace authority.
- Sophia owns compositor-first input and rendering.
- Sophia uses an external WM policy process.
- Namespace crossings go through explicit portals.

## 2026-07-07: Language Direction

Use Rust for Sophia user-space components.

Use C for narrow XLibre patches and extensions.

Nim remains attractive for experimental external window managers because policy
processes are intentionally outside the compositor hot path.

## 2026-07-07: Architecture Distinction

Rejected framing:

```text
Wayland compositor with an XLibre bridge.
```

Accepted framing:

```text
XLibre-centered modern X11 session with an external display engine.
```

This distinction matters. In Sophia, XLibre is not a guest. It is the X11
authority. Sophia modernizes the input/rendering/policy structure around it.

## 2026-07-07: Current Local XLibre Facts

Local source tree: `/home/niltempus/src/xserver`.

Observed facts:

- Xnamespace exists and is enabled by the `namespace` build option.
- Selection names are rewritten per namespace.
- A draft/runtime `X-NAMESPACE` management protocol skeleton exists.
- XComposite and Damage exist as likely render handoff points.
- Input delivery still flows through legacy `XYToWindow`, sprite trace, grabs,
  focus, and DIX delivery.

Implication: rendering has a plausible existing seam. Compositor-routed input
needs explicit XLibre protocol/server work.

## 2026-07-07: Compositor References

Niri is a reference, not a base. Use it for Rust/Smithay compositor mechanics:
backend structure, frame clock, KMS/libinput patterns, renderer integration,
transaction timeouts, and headless or visual test ideas.

Do not fork niri for Sophia. Niri's central state combines compositor and WM
policy, while Sophia needs an external WM policy process.

Picom is the X-side reference. Use it for XComposite/Damage handling, X window
tree mirroring, top-level/client detection, layer snapshots, render command
planning, and damage calculation across buffered layouts.

Do not copy picom's process architecture. Picom renders back into X. Sophia
should hand X-derived layer snapshots to a compositor that owns scanout.

## 2026-07-07: Roadmap Direction

The next implementation step after docs is a Rust workspace skeleton with passive
types and protocol packets. Do not start with XLibre patches.

The first rendering proof should use mock layer snapshots before connecting to
XLibre. The first XLibre proof should mirror windows and emit snapshots before
routed input work begins.

## 2026-07-07: Engine Backend Boundary

Use niri and Smithay as references for compositor backend boundaries, not as a
base or dependency.

Sophia Engine now treats the headless compositor as the first backend behind a
small `EngineBackend` trait. This keeps backend mechanics separate from passive
protocol packets and leaves room for real output, XComposite import, and test
backends without changing the WM or X Bridge packet shapes.

## 2026-07-07: Blind WM And Compositor Chrome

Sophia WM manages opaque layout nodes, not X11 windows. The WM protocol must not
carry XIDs, namespace IDs, raw titles, app classes, PIDs, or icon pixels.

Sophia Engine is the broker for compositor chrome. It may receive metadata from
Sophia X Bridge, but user-facing titles, icons, trust badges, and attention
state are rendered by the compositor or compositor shell from sanitized chrome
descriptors. This keeps complex layout policy useful without granting it X11
god-mode or namespace visibility.

## 2026-07-07: TEA Boundary Discipline

Sophia will use The Elm Architecture where it matches the authority boundary:
policy components consume snapshots/events, update private policy state, and
emit command packets. This is the default style for Sophia WM, portals, and
session policy.

Sophia Engine is not a global TEA application. It is the compositor authority
and must stay performance and security centric: owned tables, typed IDs,
generation checks, spatial indexes, damage queues, renderer systems, and
auditable hot paths.

## 2026-07-07: X Bridge Probe Start

Sophia X Bridge uses `x11rb` for the first read-only X11 probe. The initial
probe connects to the configured display, queries Composite, Damage, XFixes,
Shape, and Render with `QueryExtension`, and carries static Xnamespace records
until XLibre exposes namespace discovery data through a reliable protocol.

The bridge now has the stable discovery seam: server-discovered namespace
records can replace static records, and discovered window ownership can annotate
the X mirror for frames and clients. A live X-NAMESPACE query remains protocol
specific work, but Sophia can record discovered namespace information once a
query source provides it.

This does not redirect windows or mutate X server state yet. Window-tree import,
event selection, Composite redirection, and Damage tracking remain separate
Phase 3 steps.

The first root-tree importer remains read-only. It walks the tree breadth-first,
wraps raw XIDs as `XWindowId` values with an initial mirror generation, and
records map state from `GetWindowAttributes`. Event tracking, ICCCM/EWMH
top-level detection, and Composite/Damage redirection are still pending.

Mirror event tracking now normalizes X11 notify events into Sophia-owned
`XMirrorEvent` values. Applying those events updates map state, parent/child
links, destroy cleanup, stack rank, and metadata staleness without exposing raw
X11 event objects to the rest of Sophia. Live event selection and dispatch are
still separate bridge-loop work.

Client detection now combines EWMH `_NET_CLIENT_LIST` with ICCCM `WM_STATE`.
The bridge annotates mirrored windows with the detected client window and the
nearest root child as the toplevel frame. It does not classify window type yet;
that remains later EWMH metadata work.

The bridge now emits cloned `XWindowMirror` values, protocol `SurfaceSnapshot`
values, and preliminary `LayerSnapshot` values. X geometry is read during tree
import and updated from configure events. Layer snapshots intentionally use
`BufferSource::None` until XComposite pixmap redirection/import is implemented.

Composite redirection support now selects unique mapped client windows from the
mirror and redirects them with XComposite manual updates. The bridge negotiates
the Composite version before redirecting. Naming redirected pixmaps and wiring
those pixmap IDs into `SurfaceSnapshot` remains the next Composite step.

The bridge can now name redirected client windows with XComposite
`NameWindowPixmap` and store the resulting pixmap IDs in a compositor-owned
`CompositePixmapMap`. Mirrored surfaces use the visible/toplevel mirror window
for stable Sophia surface identity, but use the detected client XID to resolve
their `BufferSource::XPixmap`. Importing or reading those pixmaps into a real
renderer texture remains Phase 4 work.

Damage tracking now has a bridge-owned `DamageTracker` that creates X Damage
objects for redirected client windows, maps Damage notify events back to client
XIDs, and accumulates pending Sophia `Region` values per window. This is still
surface-local damage; converting it into output/frame `DamageFrame` packets is
the next Phase 3 step.

X damage can now be drained into Sophia `DamageFrame` values using the current
surface snapshots. The first conversion translates client-local damage
rectangles by the snapshot geometry and records affected Sophia surfaces. This
is enough for the headless engine path; precise frame/client offset handling
should be measured once real decorated X11 clients are imported.

Phase 4 starts with a conservative CPU readback fallback. Sophia X Bridge can
query a named XComposite pixmap's geometry, read it with X11 `GetImage` in
`ZPixmap` format, store the bytes in a bridge-owned `CpuBufferStore`, and
rewrite `SurfaceSnapshot` sources from `XPixmap` to `CpuBuffer`. This is not the
final renderer path, but it gives the first real-client proof a simple handoff
before GPU texture import exists.

Sophia now includes a minimal `sophia x-test-client` command that connects to an
X display, creates a mapped input/output window, draws a simple filled rectangle,
and holds the connection open for a bounded duration. This avoids depending on
external clients like `xterm` or `xclock` during Phase 4 smoke tests.

The first live readback smoke passed against system `Xvfb` on `:119`. With
`sophia x-test-client --seconds=30` holding one mapped window,
`sophia x-smoke-readback` mirrored two windows, produced one surface, redirected
one Composite target, read back one named pixmap, and captured 256000 bytes into
the CPU buffer path. This validates the generic X11 path; XLibre-specific
namespace startup remains a separate unchecked Phase 4 item.

The CPU-buffer-backed X surface now reaches Sophia Engine's headless frame path.
`sophia x-smoke-frame` captures the XComposite pixmap, converts the surface into
a renderable layer, plans a headless frame, and replays it. The live `Xvfb`
smoke produced one layer, one render command, one replay step, and one damage
rectangle from the same 256000-byte readback.

Sophia-side policy can now move and resize the captured X surface before frame
planning. `sophia x-smoke-policy-frame` converts captured layers into opaque
layout nodes, asks the demo WM to tile them, applies the resulting
`LayoutTransaction` in Sophia Engine, and replays the frame. The live `Xvfb`
smoke produced one placement, focus assignment, one render command, one replay
step, and two damage rectangles covering the old and new geometry.

The first local XLibre namespace smoke now passes. A minimal XLibre `Xvfb` was
built from `~/src/xserver` into `/tmp/sophia-xlibre-build` with Xnamespace
enabled and XDM-AUTH-1 disabled for the local build. MIT-MAGIC-COOKIE-1 auth is
still used for the root and namespaced clients. Started with
`-namespace /tmp/sophia-xlibre-smoke/ns.conf`, the server reports Composite,
DAMAGE, and X-NAMESPACE support.

With one `sophia x-test-client` launched under the `sophia_untrusted`
namespace, the root-authorized Sophia bridge successfully ran
`sophia x-smoke-policy-frame`: four mirrored windows, one surface, one
placement, a focus assignment, one render command, one replay step, and two
damage rectangles. Running the same bridge smoke with the untrusted namespace
credentials failed with an X Access error on `GetWindowAttributes`; the XLibre
server log reported that access to the real root window was blocked. This keeps
XLibre as the XID/resource authority while confirming that namespace credentials
cannot perform global tree inspection.

`tools/xlibre_namespace_smoke.sh` captures this manual proof as a repeatable
smoke harness.

The first external WM process boundary is now concrete. The `sophia-wm-demo`
binary accepts a small argument protocol for manage, relayout, and remove
requests, runs the same blind policy code as the library, and writes a command
response that Sophia can reduce into a `LayoutTransaction`. The process sees
opaque surface IDs, workspace/output IDs, bounds, and geometry; it still does
not receive XIDs, namespace IDs, titles, classes, PIDs, or icon pixels.

`sophia x-smoke-external-wm` captures X-derived surfaces, sends their opaque
layout nodes through the external WM process, commits the returned transaction
in Sophia Engine, and replays a headless frame. The repeatable XLibre namespace
smoke now runs this external-WM path after the in-process policy smoke. An
integration test also starts the WM process twice around the same
`HeadlessEngine`, proving the engine can preserve committed layers while the WM
is absent and then accept a new transaction after the WM restarts.

The routed-input seam now has its first data contract. `XLibreRoutedInputRequest`
carries serial, seat, device, time, target XID, local coordinates, and event
kind. `XLibreRoutedInputDecision` keeps the server-side decision explicit:
accepted, stale target, denied namespace, sync-frozen device state, focus
policy, or unsupported event. Sophia X Bridge can build the request only for
flat, identity-transform routes today; transformed input remains intentionally
unsupported until the flat path is proven against an XLibre extension.

The protocol crate also carries a fixed wire request body for the future
`SOPHIA-ROUTED-INPUT` extension. The patch target is documented in
`docs/xlibre-routed-input-extension.md`, based on local XLibre touch points:
`AddExtension` dispatch under `Xext`, namespace visibility/access hooks under
`Xext/namespace`, and event delivery/grab/focus behavior under `dix/events.c`
and `Xext/xinput/exevents.c`.

The first XLibre patch artifact now lives at
`patches/xlibre/0001-add-sophia-routed-input-extension.patch`. It adds a
git-applyable `SOPHIA-ROUTED-INPUT` extension that registers with XLibre, hides
the extension from non-superPower namespaces, validates `RouteEvent` packets,
resolves the target window through normal DIX access checks, and enters normal
pointer delivery with the compositor-supplied target window.
`tools/check_xlibre_routed_input_patch.sh` applies the patch to a temporary
XLibre copy and builds `hw/vfb/Xvfb`.

After creating the private `greenm01/sophia-xserver` fork, the extension was
applied directly to that fork. The fork version gates both `hook-ext-access.c`
and `hook-ext-dispatch.c`, so sandboxed clients cannot discover the extension
via `QueryExtension` and cannot invoke it by hard-coding the major opcode.

The flat routed-pointer prototype now builds in the fork. The extension accepts
motion and button routes for master or floating pointer devices, rejects key,
touch, tablet, transformed, and slave-device routes, converts target-local 24.8
coordinates to desktop coordinates, and asks XLibre to build pointer events
with `POINTER_NORAW`. DIX now has a routed-window variant of the motion check:
it installs the target window's sprite trace instead of using `XYToWindow`, then
continues through the existing XI/DIX grab, focus, mask, and delivery path. A
sync-frozen device returns `RejectedActiveGrab`; ordinary active grabs remain
normal XLibre authority and can redirect accepted routes according to X11 grab
semantics.

The runtime routed-input smoke is now wired into
`tools/xlibre_namespace_smoke.sh`. The smoke runs against the private
`sophia-xserver` fork, creates a root-namespace target window, discovers the
XInput master pointer with `XIQueryDevice(AllMaster)`, sends a raw
`SOPHIA-ROUTED-INPUT RouteEvent`, and waits for the client-side core
`ButtonPress` event. The first passing run reported:

```text
x-smoke-routed-input display=<default> opcode=167 target=0x400000 device=2 outcome=Accepted event=button1@42,37
```

This proves the v1 flat button path across the actual X11 wire protocol. The
remaining routed-input work is no longer basic extension delivery; it is edge
coverage around grabs/focus and the later transformed-coordinate path.

The Engine-to-WM boundary is now locked to Engine-only transactions. Sophia
Engine mints every transaction ID, sends a `WmRequestPacket`, waits for one
bounded `WmResponsePacket`, validates the proposal, and commits or rejects the
result. The WM cannot initiate transactions or drive animations. Engine owns
animation timing, frame-clock interpolation, cancellation, and timeout policy.

The first durable IPC codec is in `sophia-protocol`. It uses a 24-byte
`SOPH`/version/message-kind/transaction/payload-length/reserved header and
manual little-endian parsing. It does not cast bytes into Rust structs and does
not use a generic serializer. Payloads are capped at 64 KiB, repeated items are
bounded, and malformed frames fail closed.

Portal and chrome action policy are also settled at the boundary level.
Clipboard portals are async transfer state machines: denial maps to normal X11
selection failure, approval is single-use and generation-bound, and source owner
changes revoke pending transfers. Compositor close buttons are Engine/session
policy, not WM policy: Engine hit-tests chrome, validates a surface generation
and closability, and Sophia X Bridge attempts the polite X11 close path before
any future escalation.

The first `sophia-portal` crate now makes the clipboard policy executable. A
clipboard import starts pending/private, only text targets are accepted, explicit
deny emits a fail-selection command, matching-generation approval emits a
handoff command, and stale source generations revoke pending transfers. The
remaining portal gap is X Bridge integration: monitor namespaced selections and
turn ICCCM/XFixes events into portal requests.

X Bridge now has the first selection-monitoring seam. It can subscribe to
XFixes owner-change notifications for standard clipboard selections, convert
`XfixesSelectionNotify` into Sophia-owned events, attribute owners through the
mirrored namespace table, and bump per-selection owner generations. The next
portal slice is to turn those owner records plus paste requests into concrete
`ClipboardTransferRequest` values.

The routed-input adapter now has a transformed-route path. The old flat helper
still rejects non-identity transforms, preserving the original proof boundary,
but `build_routed_input_request` accepts transformed `InputRoute` values when
the Engine has already supplied finite target-local coordinates. XLibre still
receives the same target XID/local point request and remains responsible for
DIX delivery, grabs, focus, and namespace checks.

The routed-input smoke now records the fixed request size and elapsed dispatch
round trip for the X11 request path. This gives Sophia a measurement hook before
any motion coalescing or shared-memory ring work. It is deliberately a smoke
metric, not a claim about end-to-end input latency.

The routed-input SHM decision is now represented as measurement data instead of
speculation. `RoutedInputDispatchStats` summarizes dispatch samples and only
recommends considering a shared-memory ring when measured dispatch time exceeds
the caller's threshold. The request path remains the baseline and fallback.

Sophia now has a live routed-input stress command:
`sophia-cli x-stress-routed-input`. It repeats accepted `RouteEvent` requests
against one patched-XLibre target window and reports min/average/p95/max
request/reply dispatch time. This gives the SHM question a concrete gate:
prototype the route ring only if repeated measurements show the X11 request
path exceeding the chosen threshold.

The transport selector now codifies X11 fallback. `SharedMemoryRing` is selected
only when measurements recommend considering SHM and the ring is available.
Unavailable or failed SHM falls back to `X11Request`, preserving XLibre's normal
request/reply path as the correctness baseline.

The XLibre namespace smoke script now patches a temporary XLibre source copy
when the configured `XSERVER_SRC` does not already contain Sophia's routed-input
extension. The first local stress run completed 1000 accepted routed pointer
motion requests through the X11 request path:

```text
x-stress-routed-input iterations=1000 accepted=1000 request_bytes=44 min_us=13 avg_us=14 p95_us=16 max_us=45 threshold_us=500 recommendation=KeepX11RequestPath
```

That result does not justify prototyping the SHM route ring yet. The next SHM
step should remain gated on repeated measurements that exceed the threshold.

Sophia Engine now has `RoutedInputCoalescer` for the first input hot-path
optimization. It coalesces only stable pure pointer motion, keeps the latest
motion until the frame boundary, and flushes immediately for button/key input,
target crossings, and explicit drag/grab/focus barriers. This keeps the
optimization above the XLibre request path and avoids speculative SHM work.

Routed-input optimization should remain layered behind the working X11 extension
path. Sophia may coalesce stable pure-motion routes at frame boundaries and may
later add an Engine-to-XLibre shared-memory route ring if profiling proves the
socket request path is the bottleneck. The first ring should be unidirectional;
bidirectional rejection/status rings are deferred until measurement justifies
the added coupling and memory-ordering complexity.

The first Phase 8 session seam is implemented as an engine reducer:
`SessionEvent::ChromeAction` validates a `ChromeActionRequest` and emits
`SessionCommand::RequestPoliteClose` only for accepted close requests. This
command is meant for Sophia X Bridge dispatch, not WM IPC.

The WM is notified only from the later lifecycle consequence:
`SessionEvent::SurfaceRemoved` emits a `WmRequestKind::SurfaceRemoved` packet.
This keeps close intent and layout policy separated; the WM wakes after actual
surface removal, not after a compositor chrome click.

Phase 6.5 now has the first Engine-owned WM socket transport. The transport
writes one bounded `WmRequestPacket`, reads one bounded `WmResponsePacket`, and
rejects transaction mismatches before the response can be applied. Timeout
recovery and WM restart policy remain separate runtime work.

The transaction application helper now preserves the last committed layout on
missing, malformed, oversized, or mismatched WM responses. It returns a timed-out
`TransactionCommit` with the IPC error attached, leaving restart policy as the
remaining runtime concern.

The restart policy seam is now represented as data. IPC failures produce
`WmRuntimeAction::RestartWm`, while successful IPC with a rejected layout
proposal keeps the WM running. Process spawning and supervision can consume this
decision later without changing transaction semantics.

The second portal reducer now covers drag-and-drop policy. A DnD handoff starts
pending/private, stores a bounded offered-type list hint, requires explicit
generation-matching approval, and emits abstract handoff or cancel commands.
Xdnd event monitoring and concrete X11 message handling remain X Bridge work.

File open/save handoff policy is now represented as a portal reducer too. It
keeps open/save intent and bounded file type metadata private until explicit
approval, validates suggested filenames so policy never stores path-like names,
and emits abstract handoff/cancel commands. Runtime file brokering is still
future work.

Screenshot and screen-recording policy now have a reducer. It stores only
capture mode, redacted scope, supported MIME type, size hint, and generation
state, then emits abstract handoff/cancel commands. Compositor pixels, buffers,
and streaming remain outside portal policy.

URI-open policy now has a reducer. It validates bounded URIs against a small
scheme allowlist (`http`, `https`, `mailto`, `tel`), keeps requests pending
until explicit approval, and emits abstract handoff/cancel commands. The current
protocol encoding uses a `uri-open:` type hint on the generic portal transfer
path until a dedicated URI kind is justified.

Notification policy now has a reducer, completing the first Phase 7 portal
policy pass. It bounds summary/body/action text, records urgency, requires
generation-matching approval, and emits abstract deliver/drop commands.
Compositor presentation and notification action execution remain runtime work.

Phase 9 starts the session-runtime layer. The first piece is a data-only
supervisor reducer in `sophia-runtime`: runtime process events plus a bounded
restart policy produce explicit `StartProcess`, delayed restart, `GiveUp`, or
no-op commands. It covers WM, portal broker, and metadata broker process kinds
without spawning processes yet. This lets Engine decisions such as
`WmRuntimeAction::RestartWm` become runtime policy inputs later, while keeping
process supervision out of compositor frame/input code.

The first engine-to-runtime adapter now maps `WmRuntimeAction` into supervisor
policy. `KeepRunning` leaves the supervisor idle. `RestartWm` feeds a
`SupervisorEvent::RestartRequested` into the runtime reducer for the
`WindowManager` process kind. Process spawning is still separate runtime work.

Process spawning now has a thin runtime executor. `ProcessSupervisor` owns one
supervised child process and consumes `SupervisorCommand` values from the
reducer. `StartProcess` sleeps for the bounded reducer-supplied delay, spawns
the configured program, and reports `ProcessStarted`; `poll` reports
`ProcessExited`; wrong-process and double-start commands are rejected. The
executor does not decide restart budgets, inspect WM state, or touch compositor
input/rendering paths.

WM absence now has an explicit layout cache. `LastCommittedLayout` stores the
last successfully committed layer set. Successful WM transactions replace the
cache, while timed-out or missing WM responses restore the cache into the active
layer list. This makes the restart behavior concrete: while the WM is gone,
Sophia keeps scanning out the last committed layout instead of accepting
uncommitted or partially updated layout state.

The active roadmap is now gap-oriented instead of a long historical checklist.
Completed phase detail stays in this research log; `todo.md` now tracks current
runtime assembly, backend work, rendering work, routed-input work, and compact
completed milestones.

The first session-runtime assembly seam is executable in the engine crate as a
headless session tick. The tick accepts either fresh layer snapshots or an
explicit request to restore the last committed layout, then produces a
`FrameSnapshot` and `ReplayReport`. This keeps runtime-loop semantics testable
before real DRM/KMS, libinput, or a full X event loop exists.

Clipboard portal execution now has its first bridge seam. XFixes selection owner
updates can be converted into source-namespace generation events, including
owner-loss events that carry forward the previous known namespace. The portal
side applies those events to revoke stale pending clipboard transfers. This is
owner-generation/revocation wiring only; full paste import still needs X11
SelectionRequest/requestor context.

The first headless runtime smoke is now a CLI command:
`sophia-cli x-smoke-runtime-tick`. It captures X-derived layers through Sophia X
Bridge, feeds them into the session tick, updates the last-committed layout
cache, plans a headless frame, and replays it. The XLibre namespace smoke runs
this command as the first integrated proof of capture -> session tick -> frame
replay. It is still a one-shot coordinator, not the final continuous event loop.

The backend track now has its first frame-scheduling seam. `FrameClock` produces
output-scoped frame ticks, and `DeterministicFrameClock` drives the headless
session tick without wall-clock dependence. This keeps the session runtime
repeatable while leaving a clean replacement point for a future DRM/KMS clock.

Renderer/import work now has the same kind of replacement point. Sophia Engine
validates and replays a frame before giving it to a `FrameRenderer`; the initial
`CpuFallbackRenderer` reports requested `XPixmap`, `DmaBuf`, or CPU-buffer
imports while using CPU readback as the deterministic fallback execution path.
This keeps proof-grade rendering intact while isolating the future GPU import
backend behind a small interface.

Rendering has moved one step past CPU-readback-only reports. Import reports now
carry an explicit `ImportedBufferHandle`, and `ImportCapableRenderer` can use
native `XPixmap` and `DmaBuf` handles when those paths are enabled. Unsupported
handle types remain visible as CPU-readback fallbacks instead of being hidden in
renderer-private state.

XComposite pixmap lifetime is now explicit in Sophia X Bridge. The composite
pixmap map stores `CompositePixmapRecord` values with per-window generations.
Pixmap replacement returns both the new current record and the retired record;
window removal returns the retired record. This creates a concrete release point
for future GPU imports and prevents silent raw-pixmap overwrites.

Frame scheduling now joins frame-clock ticks, X Damage, and layout epochs.
`LayoutEpochState` tracks pending surfaces for an atomic layout change, and
`schedule_frame_from_damage` waits until damage exists and the active epoch has
observed damage for all pending surfaces. This is still a deterministic
scheduler seam, not the final continuous compositor event loop.

Resize behavior measurement is now a deterministic sample derived from layout
epoch state. `measure_resize_behavior` reports elapsed time, timeout policy,
completion, timeout status, and pending surfaces. This gives future live slow
client smokes a concrete metric target instead of ad hoc visual inspection.

Routed-input grab/focus edges now have deterministic smoke coverage.
`smoke_routed_input_edges` reports active-grab and focus-policy rejection
decisions as closed routes, and `sophia-cli x-smoke-routed-input-edges` exposes
the check for repeatable smoke scripts. This does not replace live DIX grab
testing; it proves Sophia's bridge side refuses rejected delivery.

Transformed scene hit-testing now exists in Sophia Engine. The engine walks
renderable layers from top stack rank downward, inverts each layer transform
against the physical pointer position, checks untransformed layer geometry, and
emits an `InputRoute` with local coordinates. That route feeds the existing
physical-input-to-`XLibreRoutedInputRequest` adapter.

The SHM route-ring item remains measurement-gated. Current repeated routed-input
measurements keep the X11 request path, so the roadmap now moves speculative SHM
work into a deferred section and opens the next ungated track: continuous
runtime assembly.

Continuous runtime assembly now has a data-only reducer in `sophia-runtime`.
`SessionRuntimeState` and `update_session_runtime` coordinate X polling, WM
policy, frame scheduling/rendering, portal draining, chrome presentation, and WM
restart requests through explicit commands. This keeps the future loop testable
before it owns file descriptors or process-local engine state.

The reducer is now connected to `sophia-cli x-smoke-runtime-tick`. The smoke
still captures X-derived layers and runs a headless session tick, but it now
wraps that work with runtime events and reports reducer counters for commands,
frames, X events, portal drains, and chrome presentation.

Runtime scheduling from X Damage and layout epochs now has a deterministic CLI
smoke. `sophia-cli runtime-damage-epoch-smoke` builds a damage frame, completes
a layout epoch through the frame scheduler, and drives the runtime reducer
through render, portal drain, and chrome presentation phases. The XLibre smoke
script runs it beside the live capture smokes.

Portal and metadata brokers now have process-supervised placeholders.
`RuntimeBrokerSupervisors` starts and polls `PortalBroker` and `MetadataBroker`
process supervisors, and `sophia-cli runtime-brokers-smoke` exposes the check.
The placeholders use ordinary process supervision only; real broker IPC remains
future work.

The DRM/KMS backend now has a data-only output skeleton. `DrmKmsMode`,
`DrmKmsOutputDescriptor`, and `DrmKmsOutputRegistry` track connector/CRTC IDs,
mode, scale, and Sophia output IDs without opening real DRM devices. The
descriptor can seed engine output state, giving later device discovery a typed
target that already works with frame planning.

The libinput backend now has a matching data-only event source skeleton.
`LibinputDeviceDescriptor` records seat/device/kind, and
`LibinputEventSource` accepts queued `InputEventPacket` values only from
registered device/seat pairs. This gives future file-descriptor polling a
checked intake queue without putting physical input on the X bridge path.

Physical input now connects to routed-input request generation. Sophia Engine
can combine an accepted `InputRoute` with the original `InputEventPacket` to
produce an `XLibreRoutedInputRequest`, and coalescer flushes can become request
batches. The adapter rejects mismatched serials, closed route outcomes, missing
target XIDs, and missing local coordinates before anything reaches XLibre.

Notification portal commands now have a compositor chrome presentation seam.
`NotificationChromePresenter` stages bounded notification requests, presents
them only after a `DeliverNotification` command, and dismisses pending or
visible state after `DropNotification`. This keeps approval/revocation in the
portal reducer while making compositor-owned presentation state concrete.

Sanitized metadata broker output now has a concrete Engine endpoint.
`SanitizedChromeMetadata` contains only compositor-safe fields and
`ChromeBroker` converts it into `ChromeDescriptor` state with label validation
and generation checks. This makes the metadata/chrome path useful without
exposing XIDs, namespace IDs, raw titles, classes, PIDs, or icon pixels to the
WM protocol.

The WM socket path now has a long-lived supervised smoke. `sophia-wm-demo` can
serve the binary WM IPC protocol over a Unix socket, handling repeated
Engine-minted transactions without process-per-request startup. The CLI smoke
starts that server through `ProcessSupervisor`, sends one layout request, kills
the child, feeds `ProcessExited` through the restart reducer, starts a new WM
process, and sends a second layout request. Both transactions must commit and
the smoke records the changed child PID.

Clipboard denial now has an executable portal-to-X11 failure smoke:
`sophia-cli portal-clipboard-deny-smoke`. The smoke creates a pending clipboard
transfer, denies it through `ClipboardPortal`, observes `FailSelection`, and
uses Sophia X Bridge to produce a native `SelectionNotify` failure with
`property = None`. This proves the denial maps to normal X11 selection failure;
full live paste handling still needs live request dispatch and approved data
handoff.

Clipboard requestor execution now has a headless bridge seam:
`sophia-cli portal-clipboard-request-smoke`. The bridge reduces an X-shaped
`SelectionRequest`, selection owner monitor state, namespace mirror state, and
resolved target atom name into a cross-namespace `ClipboardTransferRequest`
plus native failure reply context. The smoke denies that request through the
portal and confirms the X11 reply remains a normal `SelectionNotify`
`property = None` failure.

The requestor path now dispatches the real x11rb `Event::SelectionRequest`
variant into `ClipboardPortal`. `dispatch_clipboard_selection_request_event`
rejects non-selection events, missing namespace attribution, same-namespace
requests, and unsupported targets before producing a portal prompt. Approved
clipboard data handoff is still open.

Approved clipboard handoff now has a bounded text artifact:
`sophia-cli portal-clipboard-handoff-smoke`. A matching-generation approval
returns `HandoffClipboard`; Sophia X Bridge validates the command against the
original request, caps the UTF-8 payload, and produces the property bytes plus a
successful `SelectionNotify` with the request property. Live X property writes
and send-event delivery remain the next portal execution step.

The live X clipboard portal smoke now covers request -> deny and request ->
approved handoff: `sophia-cli x-smoke-live-clipboard-portal`. Against Xvfb it
creates owner/requestor windows, receives real `SelectionRequest` events from
`ConvertSelection`, verifies denial returns `SelectionNotify(property=None)`,
then approves a second request, writes the bounded UTF-8 property, sends success
notify, and reads the property bytes back from X.

Broker IPC now has its first bounded packet contract. `BrokerHealthPacket`
covers only portal/metadata broker identity, coarse health state, generation,
and an optional short status message. It deliberately excludes raw client
metadata, namespace IDs, XIDs, portal payloads, paths, URIs, and icon bytes.
This gives the supervised placeholder processes a safe first IPC target before
the real portal and metadata broker protocols are wired.

The portal broker placeholder now has a bounded IPC health smoke:
`sophia-cli portal-broker-health-smoke`. It constructs a portal `Ready`
packet, frames it as `BrokerHealth`, decodes it, and reports the bounded
message length and frame size. This proves the control path without granting
the portal broker any generic payload channel.

The metadata broker placeholder now has the symmetric bounded IPC health smoke:
`sophia-cli metadata-broker-health-smoke`. It uses the same `BrokerHealth`
frame and keeps sanitized chrome metadata on its separate protocol path.

Broker health now routes into `SessionRuntimeState`. The reducer records
portal and metadata broker health independently, stores health state,
generation, and status-message length only, and ignores stale generations. The
health smokes now prove frame decode plus runtime-state routing.

The runtime reducer now has a reusable batch shell. `SessionRuntimeLoop`
preserves `SessionRuntimeState` across batches of `SessionRuntimeEvent` values
and returns only executable, non-empty commands. This keeps the next runtime
step focused on adapters: X event intake, WM socket responses, broker health,
portal execution, chrome presentation, and renderer completion can be translated
into bounded facts without moving file-descriptor polling into reducer logic.

The first adapter form is now explicit. `SessionRuntimeObservation` accepts only
bounded scalar facts from external runtime sources, and
`SessionRuntimeEventBatch` converts those observations into reducer events after
checking batch size and broker status length. This gives concrete X bridge, WM
transport, broker IPC, portal, chrome, and renderer code a shared intake seam
without creating a general payload bus through the security broker.

Concrete producer wiring now reaches that seam. Sophia Engine maps WM
transaction updates, session/render reports, portal commands, and chrome updates
into runtime observations; the CLI runtime smokes feed X capture counts, broker
health packets, WM transaction observations, portal drain observations, chrome
counts, and rendered frame observations through `SessionRuntimeLoop`. The next
step is to collapse the per-smoke mini-loops into a reusable headless session
driver that executes runtime commands through these adapters.

`HeadlessSessionDriver` now performs that collapse for deterministic headless
ticks. It starts from `TickStarted`, drains the runtime command queue, translates
each command through concrete headless adapters, and records the rendered
session tick plus final runtime state. The new
`headless-session-driver-smoke` proves this path without requiring live X or
real broker sockets.

## Open Questions

- Should Sophia's compositor/display engine be a fully separate process or a new
  XLibre DDX backend during the first prototype?
- How much frame-perfect resize can be achieved with XComposite/Damage alone?
- Where should the X11 WM facade live: Sophia WM, Sophia X Bridge, or a separate
  helper?
