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

## Probe-Driven Coverage

External-client coverage is admitted only when a real probe exposes a missing
request, reply, lifecycle, or error path. Current real-client smokes cover
root/window introspection (`xwininfo`, `xprop`), root property mutation
(`xsetroot`), drawing clients (`xclock`, `xeyes`, `xlogo`, `xmessage`), output
introspection (`xrandr --query`), Athena widget behavior (`xcalc`), and
terminal setup/lifecycle and drawing-transaction behavior (`xterm`), and GTK
startup behavior (`zenity`).

Each external smoke must keep `first_error=none`. New compatibility code should
remain bounded and narrow: for example, xcalc admitted `AllocNamedColor`,
`UnmapWindow`, padded one-character `PolyText8`, and normal client-disconnect
teardown without turning Sophia X Authority into a broad X11 conformance
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

Real X11 connection setup and first core request parsing now sit beside this
internal socket seam. The wire parser translates X11 bytes into existing
internal request packets rather than bypassing the authority reducers.

## X11 Wire Start

The first X11 wire milestone is connection setup, not application
compatibility. Sophia X Authority now has a bounded setup parser for byte-order
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
`x11rb` against the Sophia X Authority socket. That path requires a
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

The long-running X Authority path uses a bounded side channel for observed
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

The callback observer helpers remain for focused tests and smoke probes. They
are not the production transport shape.

## MIT-SHM Negotiation

Sophia X Authority advertises a minimal `MIT-SHM` extension surface. This is a
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
X Authority socket and treats the client as the compatibility driver. The probe
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
making Sophia X Authority a full colormap implementation.

The passing proof reached five Engine/Runtime committed authority transactions
with no X protocol error before the harness killed the long-running xeyes
process. The reduced report includes the total request count, sorted opcode
list, runtime counters, and `first_error=none` so future real-client regressions
identify the next compatibility gap directly.

## External xwininfo Probe

`x-authority-xwininfo-root-smoke` launches `xwininfo -root` against a
temporary Sophia X Authority socket and treats root-window introspection as a
separate non-drawing compatibility surface. The probe added only the request
surface xwininfo actually exercised: bounded `GetWindowAttributes`,
`GetGeometry`, `QueryTree`, and `TranslateCoordinates` replies.

The passing proof exits successfully with no Engine transactions because the
client does not draw. Its reduced report still records request count, opcode
set, zero runtime counters, and `first_error=none`, so introspection regressions
remain visible without requiring visual transaction evidence.

## External xprop Probe

`x-authority-xprop-root-smoke` launches `xprop -root` against a
temporary Sophia X Authority socket and treats root property discovery as a
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
Root"` against a temporary Sophia X Authority socket. It exits successfully
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

Cursor support is currently resource lifecycle only. Sophia X Authority accepts
the font-backed cursor resource so legacy clients can proceed, but compositor
cursor presentation remains future Engine/session policy work. `PolyText8`
parses the text item stream and emits conservative core-draw damage for the
drawn glyph bounds; it does not implement full X font rasterization.

The external real-client harness now treats any observed X protocol error as a
smoke failure even if the client already produced authority transactions. This
keeps `first_error=none` as an enforced compatibility invariant.

## External xrandr Probe

`x-authority-xrandr-query-smoke` launches `xrandr --query` against a
temporary Sophia X Authority socket and treats output-size discovery as a
read-only compatibility surface. The probe added only the request surface xrandr
actually exercised: minimal `RANDR` extension advertisement,
`RRGetScreenSizeRange`, and `RRGetScreenResources`.

The first admitted RandR replies are deliberately sparse. Size range reports
the setup root dimensions as the fixed admitted range, and screen resources
returns empty CRTC/output/mode/name lists. This is enough for `xrandr --query`
to observe a bounded screen without giving Sophia X Authority ownership of
native connector, CRTC, provider, lease, monitor, or modeset state.

The external probe trace now includes bounded parse-error request heads. That
keeps future extension work probe-driven when Xlib labels an extension failure
imprecisely in client stderr.
