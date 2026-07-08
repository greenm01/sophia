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

## Open Questions

- Should Sophia's compositor/display engine be a fully separate process or a new
  XLibre DDX backend during the first prototype?
- How much frame-perfect resize can be achieved with XComposite/Damage alone?
- Where should the X11 WM facade live: Sophia WM, Sophia X Bridge, or a separate
  helper?
