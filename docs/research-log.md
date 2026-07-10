# Research Log

This file records early decisions, assumptions, and open questions. Keep it
short and chronological.

## 2026-07-07: Project Name

The project is named **Sophia**.

Sophia initially meant the XLibre-centered modern X11 architecture, not a
Wayland compositor with an X11 compatibility sidecar. This framing was later
superseded by the Engine-centered protocol-authority architecture recorded on
2026-07-08.

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

## 2026-07-08: Engine-Centered Authority Reframe

The XLibre-centered framing is superseded. Sophia is now defined around Sophia
Engine as the permanent visual authority: physical input, scene graph, atomic
surface transactions, rendering, and scanout.

Client protocols live behind protocol authorities:

- Sophia X Authority: long-term modern X subset, inspired by Phoenix.
- Sophia Wayland Authority: future frontend for Wayland-only applications.
- Sophia Native Authority: possible future protocol for Sophia-first tools.

Authorities terminate client protocols and own protocol resources, but they do
not own layout, global shortcuts, compositor chrome, cross-namespace policy, or
scanout. They emit namespace-checked surface transactions and sanitized metadata
candidates to Sophia Engine and metadata/chrome.

Phoenix is important evidence that a clean-room, practical X server subset can
run real applications without carrying the full Xorg/XLibre object graph.
Sophia should learn from that approach while retaining its stricter process and
policy separation.

The macOS/WindowServer lesson becomes an invariant: Sophia must not present new
geometry unless it has matching committed pixels for that geometry. Slow clients
should fail closed by leaving the last committed visual state on screen rather
than exposing black borders, half-resized buffers, or torn layout.

XLibre work remains useful as prototype evidence for X11 semantics,
Xnamespace-style isolation, routed-input experiments, and XComposite/Damage
lessons, but it is no longer the target architecture.

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

Sophia now models the XSync compromise without requiring a live XSync client.
`ResizeSyncCapability` marks each surface/layer as explicit-sync capable or
implicit-only. The engine creates layout epochs only for explicit-sync layers
and can expire a timed-out epoch into a bounded timeout report. The X bridge
owns the class-level reputation tracker that downgrades future snapshots after
repeated timeout strikes; `WM_CLASS` stays bridge-local and is not emitted in
surface or layer snapshots.

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
Backend ticks now expose `PhysicalInputIntakeReport` to keep that boundary
auditable. The report carries the reduced poll result, the number of queued
physical events, and `PhysicalIntakeOnly` as the routing stage. A deterministic
queued-poller test proves the runtime tick leaves the accepted event in the
physical input source; hit-testing and routed-input request generation happen
only when the routing layer is called separately.

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

The generic runtime tick smoke and damage-epoch smoke now also use
`HeadlessSessionDriver`, removing their local command mini-loops. Remaining
direct observation-loop usage is limited to adapter-specific smokes where a full
session tick would obscure the purpose: broker health packet routing and
external WM transaction observation.

`RuntimeDriverAdapter` now turns the headless driver into a shared command
executor rather than a hard-coded synthetic loop. `HeadlessRuntimeAdapter`
preserves deterministic tests, and `LiveRuntimeDriverIntake` covers the
intended replacement sources: X event counts, WM layout/restart observations,
broker health facts, portal command drains, chrome presentation counts, layer
snapshots, and renderer frame reports. The live adapters now expose
non-blocking constructors over those reduced facts, while file-descriptor
polling remains owned by the future session loop.

`x-smoke-live-runtime-wm-socket` now combines the live X capture seam and the
long-lived WM socket seam with that shared executor. The smoke runs inside the
XLibre/Xvfb script, captures real layer snapshots, requests layout from
`sophia-wm-demo serve-socket`, and renders through `LiveRuntimeDriverAdapter`
instead of hand-writing runtime command sequencing.

On July 9, 2026, the opt-in real GBM/EGL validation passed on a host exposing
`/dev/dri/renderD128` and `/dev/dri/renderD129`:
`SOPHIA_RUN_REAL_GBM_SMOKE=1 cargo test --offline -p sophia-backend-live --features gbm-probe,egl-probe`.
The child smoke asserted `EglDrawSmokeStatus::ClearColorReady`,
`LiveRendererPresentationStatus::Ready`, and
`LiveGbmEglFrameTargetAllocationStatus::Ready`; public evidence remains
reduced to `LiveRealGbmSmokeEvidence` status, draw status, presentation status,
and frame-target allocation status.
A second run of the same command passed on the same host in the same session,
again exercising the child-process real render-node path.
After expanding `LiveRealGbmSmokeEvidence` to include reduced frame-target
allocation status, the same opt-in command passed again on July 9, 2026.

The live backend fake compositor loop smoke now proves the runtime-owned
sequencing path without real scanout: queued input polling, protocol-neutral
authority transaction intake, runtime policy scheduling, CPU-backed frame
rendering, reduced frame-target lifecycle observation, reduced KMS scanout
target observation, and reduced page-flip readiness.

Native backend event intake now has deterministic reduced seams for both output
and input. The `libdrm-events` path includes a bounded native-shaped page-flip
reader feeding the existing reduced callback queue. The `libinput-events` path
includes a bounded native-shaped input reader and `NonBlockingInputPoller`
adapter without admitting a concrete libinput dependency. The combined
validation passed with
`cargo test --offline -p sophia-backend-live --features libdrm-events,libinput-events`.
`LiveBackendRuntimeAssembly` and `HeadlessCompositorBackendAssembly` are generic
over `NonBlockingInputPoller`, preserving `QueuedInputPoller` as the default
while allowing the native-shaped libinput poller to drive the same runtime tick.
This chooses monomorphized backend pollers over boxed adapters for the current
hot path.
Real libdrm and libinput hardware validation gates are now named before the
concrete readers exist: `SOPHIA_RUN_REAL_LIBDRM_EVENTS_SMOKE` and
`SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE`. The gate report records only target and
skipped/requested status, keeping default validation independent of device nodes
and preventing env values, paths, fd identity, seat names, or native error
strings from entering public reports.
The paired smoke report fails closed: without a concrete native reader, an
opted-in real libdrm or libinput smoke returns `BackendUnavailable` instead of
opening devices.

The atomic scanout seam now has a backend-live report shape.
`LiveAtomicScanoutCommitReport` reduces the Engine's `PageFlipCommitOutcome` to
idle/waiting/committed/rejected state plus `LivePageFlipEvent`, without
transaction IDs, surface IDs, or KMS object identity. This is the report a real
KMS page-flip backend should update after the native commit boundary.
`LiveAtomicScanoutCommitter` is the matching backend-owned trait; the fake
implementation proves the runtime assembly can commit through a backend object
instead of writing page-flip state directly.
The commit path now also accepts reduced page-flip callback evidence. The
callback must survive backend-live intake checks before an atomic report can be
published, and terminal Engine outcomes must agree with the callback frame
serial. This keeps stale native events from advancing committed visual state.
The first concrete libdrm reader is now feature-gated behind `libdrm-events`.
`NativeLibdrmPageFlipEventReader` wraps a `drm::control::Device`, reduces
`PageFlipEvent` through private CRTC routes, and feeds the same
`LibdrmNativePageFlipReader` contract used by the deterministic fake.
`NativeLibdrmAtomicScanoutCommitter` adds the corresponding atomic request
submit boundary. The submit report says only submitted, would-block, or
rejected; it deliberately does not publish committed visual state without the
accepted page-flip callback path.
The native atomic request builder can now assemble the standard full-output
primary-plane property set from backend-private KMS handles. The public evidence
is reduced to built or invalid-size status; KMS object and property identity
remain private.
The native KMS target selector can now reduce connector, encoder, CRTC, mode
size, and primary-plane discovery into selected or missing resource-group
status. The selected native handles can feed the primary-plane request builder
once submit-time resources are supplied.
The native resource lifecycle seam can now create the initial modeset mode
blob, register a scanout framebuffer from a DRM buffer, validate target/buffer
size, and destroy owned resources after use. The steady page-flip resource path
registers only the framebuffer and does not require a mode blob. The
request-builder test path now runs target selection, resource creation,
property discovery, and atomic request construction together.
Renderer-live and backend-live now share a reduced scanout-buffer descriptor
contract. Renderer-live exports only size, pitch, XRGB8888 format, and GEM
handle facts; backend-live validates that descriptor and turns it into a private
DRM buffer adapter for framebuffer registration.
The native GBM scanout exporter now lives behind `gbm-probe`. Raw GBM handle
extraction stays in `sophia-renderer-native-egl`, while renderer-live returns an
owned buffer object and the reduced descriptor. This gives the future hardware
smoke a concrete object to keep alive until framebuffer retirement.
The matching property discovery seam can resolve the required connector, CRTC,
and plane property handles through a real DRM device or a deterministic fake.
Discovery failures are reduced to read failure or missing resource-property
groups before the builder can run.
The native primary-plane scanout submit chain now exercises the whole reduced
submit path with deterministic fakes: KMS target selection, renderer descriptor
validation, mode blob/framebuffer creation, atomic request build, and atomic
submit. The result intentionally stops at `SubmittedWaitingForPageFlip`; a
future hardware smoke must wire the retained submission owner to native
page-flip evidence before resources are retired.
Submitted scanout owners now retire only through a reduced accepted/presented
page-flip callback report. Rejected or stale callback reports return the owner
to the caller, which preserves resource lifetime across the exact failure cases
that would otherwise turn a pending kernel scanout into a use-after-retire bug.
Rendered primary-plane backpressure now has its own reduced runtime state:
`Deferred`. When the executor asks for another `SubmitScanout` while an earlier
KMS submission is still awaiting page-flip evidence, backend-live reports
`Deferred` instead of `Rejected`. Backend-live now also reports the reduced
runtime tick age of the in-flight owner so repeated deferrals can be diagnosed
without releasing GBM/KMS resources early. The runtime continues through portal
and chrome phases without altering in-flight scanout counts.
That age now feeds a reduced backpressure classifier: idle, waiting for
page-flip evidence, or stalled past a caller-provided threshold. It is a
diagnostic/policy hook only; resource retirement still requires accepted
page-flip evidence.
Tracked rendered submissions now also bind themselves to the last reduced
page-flip sequence observed before submission. A replayed accepted callback at
that baseline is treated as waiting, while a newer callback can retire the
owner. This closes the lower-level replay seam without exposing native sequence
identity outside backend-live.
Resource cleanup after accepted page flip is now retryable. Resource-creation
cleanup after framebuffer registration failure and submit-time cleanup after
request-build or atomic-submit failure are retryable as well. If framebuffer or
mode-blob destruction fails, backend-live clears the in-flight scanout owner
when one exists, but keeps an opaque cleanup owner with the rendered buffer
owner. A later retry can finish cleanup without leaking native IDs into runtime
state. Runtime ticks and atomic smoke evidence now expose only a reduced
cleanup-pending bit for this case.
The device-backed rendered scanout tick now retries one pending cleanup before
new scanout submission and records the reduced retry result. This gives the live
backend a forward-progress path for cleanup debt without adding native identity
to the Engine-facing report.
Cleanup debt now backpressures rendered primary-plane submission. If the retry
still leaves cleanup pending, the runtime reports a reduced deferred scanout
instead of submitting another buffer and risking loss of the retained cleanup
owner.
The reusable native GBM rendered-scanout exporter now records reduced
frame-target lifecycle state across exports. This makes retained, resized, and
invalidated targets observable while preserving the rule that native GBM/EGL
identity stays behind backend-live.
Reduced KMS scanout readiness now rejects frame targets whose size no longer
matches the selected output. This catches resize/target drift before native
primary-plane submit and maps it to a reduced page-flip-not-ready state.
Rendered primary-plane submit now consumes that reduced readiness status. When
the KMS scanout target is not ready, the runtime reports reduced
scanout-target-not-ready and skips renderer export and native primary-plane
submit.
The page-flip callback queue now carries the latest accepted reduced callback
report. `run_tick_with_rendered_primary_plane_scanout_with` consumes that report
before draining lifecycle states, so one live tick can retire the old rendered
primary-plane owner and submit the next rendered frame while keeping native
object identity private.
The live runtime assembly now also has a reusable native GBM rendered-scanout
exporter path. The exporter owns backend render-device discovery across runtime
ticks, records only reduced export attempt count/status plus context-open
attempt count/status, and initializes a persistent renderer-live GBM/EGL
rendered-scanout context only when the runtime reaches `SubmitScanout`.
Invalid targets fail before render-device opening; unavailable render devices or
degraded context startup fail closed as reduced scanout export failure and
runtime rejection.
The opt-in atomic scanout hardware smoke now has an operator-facing CLI path:
`tools/atomic_scanout_smoke.sh` verifies preflight, then runs the feature-gated
`sophia atomic-scanout-smoke` command with
`SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1`. The CLI parent spawns a child process
for the destructive proof and emits reduced timeout evidence if the child hangs
waiting for a page flip. Backend-live owns the actual proof phases: it opens a
primary DRM card, enables the atomic KMS capabilities, allocates the GBM
scanout buffer from a persistent GBM/EGL rendered-scanout context in the same
handle namespace, submits primary-plane scanout, waits for a native page-flip
callback, validates the reduced callback, and retires the submitted resources.
The reduced evidence records context startup separately from GBM export,
includes the reduced KMS scanout target readiness status, and reports reduced
retire-time resource destroy status when cleanup fails. Non-ready scanout
targets fail the evidence before the smoke can treat export or submit as
sufficient. This path compiled, the local preflight failed closed with
`DeviceDirectoryUnavailable`, and a skipped-preflight local run reduced to
`NoPrimaryCard`; real hardware evidence remains to be recorded.
Primary-plane submit now also has a preselected-target entry point. The opt-in
atomic smoke uses one KMS selection snapshot for frame-target sizing, readiness
evidence, and atomic submit instead of selecting again after rendering. A
deterministic fake test proves the helper honors the supplied snapshot.
The runtime rendered-primary-plane submit path now mirrors that discipline. It
rechecks the native KMS target snapshot before asking the renderer for a scanout
buffer, verifies the selected size against the reduced frame target, and then
submits the same snapshot. A fake drift test proves a disappeared native target
returns reduced scanout-target-not-ready with zero export attempts.
Atomic submit policy is now explicit. The direct primary-plane smoke helper
still reports reduced commit flags with modeset permission, while runtime
rendered-primary-plane submit uses page-flip policy and reports `ALLOW_MODESET`
as false. This prevents the steady scanout loop from accidentally becoming a
modeset loop.
`LibdrmNativeAtomicScanoutSmokeEvidence` now includes reduced request scope and
commit flags, so captured opt-in hardware evidence can prove the commit policy
and request shape used by the submit that generated the page-flip evidence. The
schema now separates initial modeset evidence from steady-state page-flip
evidence so a passing capture proves the post-modeset scanout path as well.
The live runtime now has a native page-flip intake tick for rendered
primary-plane scanout. It reads/polls the feature-gated libdrm page-flip reader
into the bounded callback queue, updates reduced poller diagnostics, retires
the in-flight owner from accepted evidence, and submits the next rendered frame
in the same runtime tick. A fake-reader test proves the ordering without
opening DRM devices.
That path now has a persistent native GBM/EGL exporter variant as well. The
runtime can drain native page flips, retire the previous GBM/KMS owner, and
then attempt the next rendered primary-plane export through the reusable
backend-live exporter. The deterministic coverage uses an unavailable render
device to prove the ordering and fail-closed reduced states without touching
real hardware.
The libinput backend now admits the safe Rust `input` wrapper behind
`libinput-events`. `NativeLibinputEventReader` owns a backend-created
`Libinput` context, dispatches it through the existing bounded reader contract,
and reduces pointer motion, pointer button, and keyboard key events through a
caller-provided reduced seat/device map. The first concrete smoke constructs an
empty path-based context and proves idle polling without opening devices or
publishing native paths, fds, seat names, raw device identity, or libinput error
strings. Real device-opening validation remains behind
`SOPHIA_RUN_REAL_LIBINPUT_EVENTS_SMOKE`.
The live runtime now also has cross-feature coverage for input plus scanout in
one tick. A libinput-shaped poller feeds a physical pointer event while native
libdrm page-flip intake retires the previous rendered GBM/KMS owner and the
runtime submits the next rendered primary-plane frame. This is still a
deterministic fake-reader test, but it proves the sequencing contract needed by
the production loop before backend-live owns real fd readiness.
Input readiness now has a one-shot backend-live gate. `LiveInputReadinessGatedPoller`
wraps the concrete or fake input poller and returns an empty reduced batch until
the outer loop observes readiness. Polling consumes the token, so repeated
dispatch requires repeated readiness observations. The runtime coverage proves a
tick continues while the gate is idle, leaves queued input untouched, then polls
and ingests that input after readiness is marked.
Backend-live now exposes a reduced session-loop owner for the combined
production path. `LiveBackendSessionLoop` owns the native page-flip poller and
bounded read/emit budgets; each tick supplies only reduced readiness facts.
`LiveBackendReadinessCollector` records input-ready and page-flip-ready as
one-shot booleans and drains them before the tick. The runtime observes the
gated input token, reads native page-flip callbacks only after reduced
page-flip readiness, drains already-pending decoded callbacks under the bounded
emit budget, retires accepted rendered scanout ownership, and submits the next
rendered primary-plane frame in one bounded tick. Deterministic coverage proves
idle input readiness does not block scanout, queued page-flip callbacks are not
read until page-flip readiness is observed, and ready input can be ingested in
the same tick as page-flip retirement and next-frame submit.
Tracked rendered-primary-plane retire reports now preserve the reduced native
destroy status from accepted page-flip retirement. The runtime can distinguish
clean retirement from retryable framebuffer/blob cleanup debt without exposing
framebuffer IDs, mode-blob handles, GBM objects, or driver errors.

## 2026-07-10: Atomic Scanout Hardware Evidence

The local non-hardware atomic scanout gate passes, including GBM/EGL scanout
feature tests, backend-live scanout intake tests, and strict reduced verifier
fixtures.

The opt-in hardware smoke cannot complete in this environment. Its preflight
stops before modesetting with reduced status `DeviceDirectoryUnavailable` and
zero primary card counts. The remaining proof must run on a DRM-master-capable
machine and produce two passing reduced evidence lines: `InitialModeset` and
`SteadyPageFlip`.

## Open Questions

- Should Sophia's compositor/display engine be a fully separate process or a new
  XLibre DDX backend during the first prototype?
- How much frame-perfect resize can be achieved with XComposite/Damage alone?
- Where should the X11 WM facade live: Sophia WM, Sophia X Bridge, or a separate
  helper?
