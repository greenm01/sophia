# Sophia X Server Frontend

The Sophia X Server Frontend is Sophia’s long-term modern X server
implementation. It presents the **X11 API and wire protocol** directly to
applications, then emits `SurfaceTransaction` values to Sophia Engine. It takes
the Phoenix strategic approach: a clean-room implementation of the modern X11
subset real applications require, expanded by compatibility evidence rather
than by reproducing all of Xorg.

It is not a plan for a separate application-facing Sophia display protocol. X11
is the native application API of this path; forward progress happens in the
server architecture, DRM/KMS presentation, and targeted X11 extensions. XLibre
remains a broad-compatibility provider and reference while this frontend gains
coverage. The implementation crate is currently named `sophia-x-authority`; the
name reflects its protocol role and does not narrow the product direction.

The frontend is not the compositor. It terminates X protocol, owns X resource
semantics, applies the selected classic or confined session profile, and
translates client-visible X state into Sophia-owned data.

## Naming

Use **Sophia X Server Frontend** for the component and **X11** for the API it
implements. Use “X Authority” only as a shorthand for its protocol-semantic
role or the existing crate name. Do not describe it as an X11 compatibility shim
or an “X12” replacement protocol.

## Authority Boundary

The frontend owns:

- X client sockets and protocol parsing;
- X resource IDs, atoms, properties, windows, pixmaps, GCs, cursors, and
  colormaps where required for compatibility;
- namespace-aware resource lookup and event subscription;
- X focus, grabs, selections, configure, map/unmap, and lifecycle semantics;
- X drawing completion and buffer readiness;
- reduced metadata candidates for the metadata broker;
- portal request facts for cross-namespace transfers.

The frontend must not own:

- physical input devices;
- compositor scene graph or hit-testing;
- final layout, workspaces, or global shortcuts;
- compositor chrome;
- portal policy decisions;
- renderer imports, frame scheduling, or scanout.

## Production Boundary And Current Implementation Status

The production boundary is deliberately a small, bidirectional contract. It
keeps X11 semantics in the frontend and all physical display authority in
Sophia Engine:

| Direction | Contract | Owner and rule |
| --- | --- | --- |
| X11 client → frontend | local Unix connection, setup authentication, X11 requests, and X resource lifetime | The frontend owns parsing, client identity, XIDs, atoms, windows, properties, selections, grabs, and client-visible replies/events. Production setup authentication must be explicit; an owner-only socket is a transport guard, not a replacement for X11 authorization. |
| Frontend → Engine | `XAuthorityObservedTransactionBatch` containing the originating frontend client when available, `SurfaceTransaction` values, surface removals, and any CPU buffer update | This is the only visual ingress. The batch is bounded; backpressure is an error rather than an unbounded queue. Engine receives Sophia surface data, never raw X11 request parsing or XID ownership. |
| Engine → frontend | `XAuthorityClientInputEvent` and `XAuthorityClientControlCommand` | Engine selects the physical-input target, owns coordinates/hit-testing, resolves that surface to its frontend client from observed transactions, and requests X-visible focus or configure results. The frontend applies only routes addressed to that connection and returns `XAuthorityClientControlAck`. |
| Engine → frontend (next) | output/RandR snapshot and presentation/buffer-release feedback | Engine remains the source of physical output facts, frame retirement, and buffer lifetime. The frontend turns those facts into RandR/configure/present-visible X11 state; it must never infer scanout completion itself. |

The existing implementation covers the second and third rows for the
single-client live-session prototype: the X11 socket dispatch emits bounded
transaction batches, and the Engine can route key/pointer events plus
focus/configure commands back to that client. Its persistent socket listener
also reuses authority state across *sequential* clients. The frontend now also
offers an opt-in bounded concurrent worker API: callers use
`serve_next_concurrently[_traced]` to accept independent clients and
`wait_for_clients` to reap the accepted batch. It shares only independently
synchronized runtime, atom, property, and connection-lease state; the default
cap is 16 clients and `XServerFrontendConfig::with_max_concurrent_clients`
sets a different nonzero cap. A simultaneous-client regression holds one
client live while a second maps its window, then proves cleanup returns the
lease count to zero. `XServerFrontendRouteBroker` is now the explicit bounded
Engine-facing ingress for simultaneous workers: callers enqueue client-addressed
input or control, call `route_pending`, and each connected routed worker owns
only its private queues. Unknown, disconnected, and backpressured client routes
fail closed; worker teardown unregisters the client route before its cleanup
batch is observed. The persistent live session now uses that brokered worker
transport while it still intentionally accepts one xterm. General concurrent
accept/reap service supervision is the next integration step. Root/output facts
are still fixed setup values. Each accepted
client now gets
a disjoint X11 setup resource-ID range. Every currently supported XID-creating
wire path—window, pixmap, GC, font, colormap, glyph cursor, and reduced
MIT-SHM segment—rejects an XID outside that range with X11 `BadIDChoice` before
it reaches runtime state. Every successful setup also receives a monotonic
frontend client identity and retains an XID-range lease until its connection
ends. The lease is the cleanup ledger key; it does not restrict ordinary
same-namespace references in the classic shared-X profile. Resource cleanup
now reaches an Engine-visible surface-removal path. The classic shared-X
existing-resource policy is explicit: an authenticated client may refer to,
mutate, or destroy an existing resource in its shared namespace, even when a
different connection created its XID. XID-range checks apply only to resource
creation; the lease remains a teardown ledger, not an access-control list.
`XServerFrontendConfig`/`XServerFrontend` make the local socket path and
namespace explicit, reject invalid namespaces, restrict the socket to its
owner, and refuse to replace a non-socket path. The configuration can now
require a session-scoped `MIT-MAGIC-COOKIE-1` value: a bad setup receives a
normal X11 setup-failure reply and the listener remains available for the next
client. The legacy smoke helpers and the configuration default deliberately
remain unauthenticated local sockets. Xauthority-file management, peer-
credential policy, cookie rotation, session launch policy, Engine-backed
multi-client input/control routing, and confined-client routing are still
required before treating the listener as a general local X server.

### Connection Lifecycle

After successful setup, the frontend records the client ID and its setup XID
range as a connection lease. The lease is released only after the client stream
and optional input/control writers have stopped, including an observer or
writer error. This establishes a durable ownership key without changing classic
shared-X semantics: it identifies resources a connection was allowed to create,
but does not prohibit another trusted client in the same namespace from
referring to them.

`DestroyWindow` and connection teardown now use the same authority-side
destruction primitive. Teardown releases the disconnecting lease's supported
windows and CPU buffers, pixmaps, GCs, fonts, cursors, SHM segments, window
properties, and selection ownership. Every destroyed window produces a
surface-removal fact; the Engine removal intake prunes its committed snapshot,
while the live layout, CPU scene, and focus state prune their corresponding
state. A sequential-client proof confirms that a later client receives
`BadWindow` for the former client's window while resources from a different
XID range remain intact.

The existing-resource policy is deliberately profile-specific. The cleanup
ledger establishes which resources disconnect teardown must reclaim, but it is
not an access-control list in the classic profile. The regression suite proves
that a peer with a disjoint creation range can map an existing window in the
same namespace. A confined profile must instead add connection routing and
capability checks before it can permit any cross-client resource operation.

The two session profiles have these precise semantics:

- **Classic shared-X:** one trusted local session assigns all participating
  clients the same namespace. Ordinary X11 inspection, coordination,
  selections, and window-manager interaction remain available within that
  session. Any authenticated client in that namespace can use existing X11
  resources; XID allocation remains per-connection solely to prevent creation
  collisions and make disconnect cleanup precise.
- **Confined:** the launch/authentication layer assigns a distinct namespace
  and explicit capabilities for a client group. Cross-namespace discovery,
  properties, selections, and input are denied unless a narrow portal grants a
  transfer. This profile needs client-aware connection routing before it can be
  enabled; it is not an alias for the current shared listener.

## Modern X11 Compatibility Subset

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

## Probe-Driven Coverage

External-client coverage is admitted only when a real probe exposes a missing
request, reply, lifecycle, or error path. Current real-client smokes cover
root/window introspection (`xwininfo`, `xprop`), root property mutation
(`xsetroot`), drawing clients (`xclock`, `xeyes`, `xlogo`, `xmessage`), output
introspection (`xrandr --query`), Athena widget behavior (`xcalc`), and
terminal setup/lifecycle and drawing-transaction behavior (`xterm`), and GTK
startup behavior (`zenity`).

The maintained [X11 compatibility matrix](x11-compatibility-matrix.md) records
the exact command, evidence level, narrow proven behavior, and next gate for
each client class. It is the source of truth for admitting new X11 work; the
historical prose below explains why individual compatibility slices exist.

Each external smoke must keep `first_error=none`. New compatibility code should
remain bounded and narrow: for example, xcalc admitted `AllocNamedColor`,
`UnmapWindow`, padded one-character `PolyText8`, and normal client-disconnect
teardown without turning the frontend into a broad X11 conformance
project. xterm admitted `ConfigureWindow` and the bounded setup/drawing paths
needed to reach committed `ImageText8` transactions. Core drawing now applies
GC colors and raster operations to bounded XRGB8888 software buffers, including
a printable-ASCII fixed-cell raster. The real xterm proof locates the expected
glyph sequence in materialized replacement/patch pixels. A separate key-channel smoke
injects `sophia` plus Return and proves later xterm buffer generations change.
zenity admitted selection-owner lookup, server grab/ungrab, root colormap
creation, reduced `MIT-SHM`, additional `RANDR`, and `BIG-REQUESTS` startup paths,
but the current TTY/DBus environment and missing XInput2 support still prevent a
rendered GTK dialog proof.

`XKEYBOARD` reports `present=false`. Advertising only its version handshake
caused real xterm to advance into unsupported XKB map requests once the core
keyboard map became useful. The supported keyboard baseline is core
`GetKeyboardMapping`, `GetModifierMapping`, `KeyPress`, and `KeyRelease`.

## Namespace Model

Every client connection belongs to a `NamespaceId` before it can create
resources. In a classic shared-X profile, trusted clients deliberately share one
namespace and retain ordinary X11 inspection and coordination. In a confined
profile, launch tokens, socket routing, credentials, or a later broker select
separate namespaces. Namespace identity is frontend state and must not leak to
the WM.

Resource rules:

- XIDs are local to the authority and wrapped as `AuthorityLocalId` before
  becoming Sophia data.
- Cross-namespace resource lookup fails closed unless a specific portal flow
  grants a narrow transfer; same-namespace classic-X behavior remains intact.
- Event subscriptions are namespace-scoped in confined profiles.
- Properties may be visible within a namespace, but cross-namespace property
  discovery must not expose titles, classes, PIDs, paths, or atoms that reveal
  another namespace's private clients.
- Grabs and focus are authority semantics. Sophia Engine supplies target
  surfaces and local coordinates; the authority still applies X delivery rules.

## Surface Transactions

Each visible top-level or protocol surface maps to an `AuthoritySurface` owned
by the frontend and a Sophia `SurfaceId` owned by Sophia Engine.

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

The frontend translates compositor lifecycle commands into normal X
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
hit-testing, and sends routed input intent to the frontend:

- target Sophia `SurfaceId`;
- authority-local object ID when known;
- local coordinates after inverse transform;
- seat, device, time, and event kind.

The frontend then applies X semantics:

- focus and grabs;
- event masks;
- XI/XKB delivery rules;
- namespace checks;
- sync-frozen device state.

The authority returns reduced accept/reject outcomes. It must not expose raw
client streams or general event injection capability back to Sophia Engine.

## Phoenix Strategic Direction

Sophia adopts Phoenix’s strategic direction, not its code: build a modern X
server from a clean implementation, deliberately support the X11 features real
applications use, and improve the server beneath the established X11 API. The
useful study areas are:

- connection setup and request dispatch shape;
- minimal resource tables;
- basic window/pixmap/property behavior;
- extension prioritization based on real toolkit compatibility;
- tests or examples that prove GTK/GL/Vulkan application paths.

Sophia-specific differences must remain intact: Engine-owned atomic visual
commits, blind WM policy, and a user-selectable choice between classic shared-X
semantics and confined namespace/capability policy.

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

Real X11 connection setup and first core request parsing now sit beside this
internal socket seam. The wire parser translates X11 bytes into existing
internal request packets rather than bypassing the authority reducers.

## X11 Wire Start

The first X11 wire milestone is connection setup, not application
compatibility. The frontend now has a bounded setup parser for byte-order
markers, protocol version fields, authorization name/data fields, and resource
ID allocation facts. It also has setup success/failure encoders and a local
Unix socket smoke that completes a setup handshake with a synthetic client.

The covered setup fixtures are:

- little-endian and big-endian setup requests;
- valid setup with resource ID base and mask;
- truncated setup input;
- unsupported major protocol version;
- overlarge authorization fields;
- setup success and setup failure reply encoding.

Core request parsing now starts with fixtures for `CreateWindow`, `MapWindow`,
`InternAtom`, `GetAtomName`, `ChangeProperty`, `GetProperty`,
`SetSelectionOwner`, `ConvertSelection`, `CreateGC`, `FreeGC`,
`GetInputFocus`, `QueryExtension`, `ListExtensions`, and `QueryBestSize`.
Runtime-backed wire requests translate into existing internal
`XAuthorityRequestPacket` values before they reach `XAuthorityRuntime`;
property writes and reads land in a minimal namespace-keyed property table.

Minimal client-visible output now covers bounded X error records, 32-byte core
events for `ConfigureNotify`, `MapNotify`, `PropertyNotify`, and
`SelectionNotify`, and variable-length replies for `InternAtom` and
`GetAtomName`/`GetProperty`. The X11 socket smoke completes setup, interns
atoms, sends synthetic `CreateWindow` and `MapWindow` requests, writes a title
property, reads it back, and observes the expected events.

Atom naming is authority-owned and bounded. Sophia preloads the small predefined
set needed by the prototype, preloads the X11 predefined atom range, allocates
dynamic client-interned atoms after that range, and caps atom names at 256
bytes. Metadata-relevant
property writes such as `WM_CLASS`, `WM_NAME`, `_NET_WM_NAME`, and
`WM_PROTOCOLS` produce metadata broker candidates that include only namespace,
window, atom names, type names, value length, and generation. They do not emit
raw titles, classes, icons, paths, or namespace labels to the window manager.

Minimal `GetProperty` is now present. The first real-client-library smoke uses
`x11rb` against the Sophia X Server Frontend socket. That path requires a
client-compatible setup reply with one root, one pixmap format, one depth, and
one TrueColor visual. The smoke connects through the normal X11 setup path,
interns `_NET_WM_NAME` and `UTF8_STRING`, creates a window, writes and reads a
bounded title property, maps the window, and observes `ConfigureNotify` and
`MapNotify`.

Subsequent external probes remain compatibility drivers. Their first failure
should drive the next bounded opcode or reply implementation rather than
guessing ahead.

`xdpyinfo` now passes as the first broader probe. It forced a minimal root
screen in setup, empty extension discovery replies, root property reads for
standard predefined atoms, root input-focus reporting, and no-reply GC lifecycle
requests.

A tiny C Xlib client now also passes. The CLI smoke compiles the probe into
`/tmp`, connects through libX11, interns atoms, creates a simple window, writes
and reads the title through normal Xlib property calls, maps the window, and
destroys it cleanly. That probe added the first minimal `DestroyWindow`
compatibility path.

A drawing-oriented C Xlib client now passes as well. It creates a window,
creates a GC, maps the window, calls `XFillRectangle`, syncs, frees the GC, and
destroys the window. This added `PolyFillRectangle` decode support. Successful
fill requests produce no client-visible X reply, but they do emit a ready
`CoreDraw` surface transaction with rectangle damage in the dispatch path.

The live X11 socket path exposes those dispatch results through an out-of-band
observer callback. This keeps the client-visible X11 stream pure: successful
core drawing still produces no direct X reply, while Sophia Runtime can receive
the ready `SurfaceTransaction` facts through the session side channel. The
compiled Xlib drawing smoke now validates the whole reduced path: `XFillRectangle`
produces one observed transaction, Sophia Engine commits it, and the live
runtime adapter records one authority transaction without exposing XIDs or
namespace metadata.

A software image upload smoke now passes through the same path. The authority
decodes bounded core `PutImage` requests, records the uploaded image extent as
damage, emits a ready CPU-backed `SurfaceTransaction`, and still sends no direct
X11 reply on success. The compiled Xlib `XPutImage` smoke validates that the
observed transaction commits in Sophia Engine and increments Sophia Runtime's
authority transaction counters.

A private `SOPHIA-PRESENT` extension now models the first explicit buffer
handoff without claiming full X Present support. `QueryExtension` advertises a
fixed private major opcode for that extension only. Minor opcode `0` presents an
XPixmap handle for a namespace-owned window, emits a ready
`BufferSource::XPixmap` transaction, and remains reply-free on success. The CLI
present-pixmap smoke validates the raw X11 socket path through Engine commit and
Runtime authority counters.

## Runtime Transport

The long-running X Server Frontend path uses a bounded side channel for observed
surface transactions. Successful X11 drawing and present requests still write no
client-visible success reply when the X11 protocol does not require one. Instead
the authority packages ready `SurfaceTransaction` values into
`XAuthorityObservedTransactionBatch` records and attempts a nonblocking send to
the runtime-owned queue.

Backpressure is explicit. If the queue is full, the authority reports
`Backpressure` and stops the socket helper rather than allocating an unbounded
buffer or silently dropping visual facts. If the receiver has gone away, the
authority reports `Disconnected`; supervision can then restart the authority
process. This keeps the X11 client stream separate from Sophia Runtime's
transaction intake while preserving the fail-closed rule.

The reciprocal routed-worker transport is also bounded.
`XServerFrontendRouteBroker` owns input and control ingress plus a registration
table for live routed workers. `serve_next_concurrently_routed[_traced]`
registers a connection only after successful X11 setup, gives it one private
input queue and one private control queue, and unregisters those queues before
the release cleanup is reported. The Engine-side loop calls `route_pending`;
it never broadcasts an event and it receives an explicit error for an unknown,
disconnected, or full client queue. Control acknowledgements return through the
same broker labeled with the worker client identity. This is an in-process
contract that now carries the persistent one-client live session; it becomes a
general service only when the session owns bounded concurrent accept/reap
supervision.

The callback observer helpers remain for focused tests and smoke probes. They
are not the production transport shape.

## MIT-SHM Negotiation

The frontend advertises a minimal `MIT-SHM` extension surface. This is a
compatibility step, not a shared-memory import implementation.

`QueryExtension("MIT-SHM")` returns a private major opcode, and minor opcode `0`
replies to `ShmQueryVersion` with protocol version `1.2`, `shared_pixmaps =
false`. Unsupported minor opcodes fail closed as native X request errors.

`ShmAttach` records only namespace-local segment metadata: the synthetic segment
XID, the client-provided `shmid`, the read-only bit, and a generation. The
authority does not call `shmat`, map host memory, or expose the segment to
Sophia Engine in this first pass.

`XShmPutImage` is decoded and admitted as a bounded draw transaction when the
segment is namespace-local and the target is a known window. The transaction uses
the requested destination rectangle as damage and a reduced CPU-buffer handle; it
does not map or trust the client-provided shared-memory bytes. A missing or
cross-namespace segment returns a bounded `BadAccess` error. Pixmap targets are
accepted as local resource activity without emitting a surface transaction.
Invalid or already-gone detach requests are ignored as cleanup no-ops, while
cross-namespace detach remains rejected.

Real MIT-SHM import is deferred until Sophia has a compositor backend that can
consume the mapped bytes through a bounded renderer import path. Mapping
client-provided shared memory with `shmat` would add host-memory lifetime,
detach, namespace cleanup, and crash-recovery obligations before the engine can
use the data. Until that backend exists, core `PutImage`, reduced `ShmPutImage`
transactions, and private `SOPHIA-PRESENT` remain the supported pixel handoff
seams.

## External xclock Probe

`x-authority-xclock-smoke` launches `xclock` against a temporary Sophia
X Server Frontend socket and treats the client as the compatibility driver. The probe
added only the request surface xclock actually exercised: printable atom names,
pixmap resources, copy-area flow, basic font replies, list-font replies,
window-attribute and subwindow mapping no-ops, expose events, and bounded core
draw transactions for line, segment, polygon, rectangle, image, and copy damage.

The passing proof reached mapped exposure and seven Engine/Runtime committed
authority transactions with no X protocol error before the harness killed the
long-running xclock process. Its reduced report now includes the explicit
`outcome=proof_window_killed`, total request count, unique major-opcode count,
and sorted major-opcode list, so future regressions show which compatibility
surface changed without exposing XIDs or namespace IDs. The authority still does
not become a full X server: unsupported requests remain fail-closed, and only
reduced transaction facts cross into runtime.

## External xeyes Probe

`x-authority-xeyes-smoke` launches `xeyes` against a temporary Sophia X
Authority socket and keeps authority coverage probe-driven. The probe added the
request surface xeyes actually exercised after xclock: bounded `QueryColors`,
`ClearArea`, and `PolyFillArc` handling. Arc and clear operations reduce to core
draw damage transactions; color replies return bounded RGB records without
making the frontend a full colormap implementation.

The passing proof reached five Engine/Runtime committed authority transactions
with no X protocol error before the harness killed the long-running xeyes
process. The reduced report includes the total request count, sorted opcode
list, runtime counters, and `first_error=none` so future real-client regressions
identify the next compatibility gap directly.

## External xwininfo Probe

`x-authority-xwininfo-root-smoke` launches `xwininfo -root` against a
temporary Sophia X Server Frontend socket and treats root-window introspection as a
separate non-drawing compatibility surface. The probe added only the request
surface xwininfo actually exercised: bounded `GetWindowAttributes`,
`GetGeometry`, `QueryTree`, and `TranslateCoordinates` replies.

The passing proof exits successfully with no Engine transactions because the
client does not draw. Its reduced report still records request count, opcode
set, zero runtime counters, and `first_error=none`, so introspection regressions
remain visible without requiring visual transaction evidence.

## External xprop Probe

`x-authority-xprop-root-smoke` launches `xprop -root` against a
temporary Sophia X Server Frontend socket and treats root property discovery as a
read-only compatibility surface. The probe added only the request surface xprop
actually exercised after xwininfo: bounded `ListProperties` decoding and replies
for namespace-local property atom sets.

The passing proof exits successfully with no Engine transactions because the
client only introspects root properties. The reduced report records request
count, opcode set, zero runtime counters, and `first_error=none`. Sophia X
Authority does not synthesize a broad global root-property catalog here; it
reports the properties the namespace-local table actually owns.

## External xsetroot And xlogo Probes

`x-authority-xsetroot-name-smoke` launches `xsetroot -name "Sophia
Root"` against a temporary Sophia X Server Frontend socket. It exits successfully
through existing property, input-focus, GC lifecycle, and extension-query paths,
proving a small root-property mutation case without Engine transactions.

`x-authority-xlogo-smoke` launches `xlogo` and reaches committed
Engine/Runtime authority transactions through the existing create/map/property
and polygon/rectangle drawing surface. It did not require new X protocol
coverage, which makes it a useful regression probe for the bounded drawing
paths already admitted by xclock and xeyes.

## External xmessage Probe

`x-authority-xmessage-smoke` launches `xmessage Sophia` and treats
legacy text UI behavior as the next compatibility driver. The probe added only
the request surface xmessage actually exercised after xlogo: bounded
`CreateGlyphCursor`, `FreeCursor`, `SetClipRectangles`, and `PolyText8`.

Cursor support is currently resource lifecycle only. The frontend accepts
the font-backed cursor resource so legacy clients can proceed, but compositor
cursor presentation remains future Engine/session policy work. `PolyText8`
parses the text item stream and emits conservative core-draw damage for the
drawn glyph bounds; it does not implement full X font rasterization.

The external real-client harness now treats any observed X protocol error as a
smoke failure even if the client already produced authority transactions. This
keeps `first_error=none` as an enforced compatibility invariant.

## External xrandr Probe

`x-authority-xrandr-query-smoke` launches `xrandr --query` against a
temporary Sophia X Server Frontend socket and treats output-size discovery as a
read-only compatibility surface. The probe added only the request surface xrandr
actually exercised: minimal `RANDR` extension advertisement,
`RRGetScreenSizeRange`, and `RRGetScreenResources`.

The first admitted RandR replies are deliberately sparse. Size range reports
the setup root dimensions as the fixed admitted range, and screen resources
returns empty CRTC/output/mode/name lists. This is enough for `xrandr --query`
to observe a bounded screen without giving the frontend ownership of
native connector, CRTC, provider, lease, monitor, or modeset state.

The external probe trace now includes bounded parse-error request heads. That
keeps future extension work probe-driven when Xlib labels an extension failure
imprecisely in client stderr.
