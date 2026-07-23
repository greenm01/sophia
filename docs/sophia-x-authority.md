# Sophia X Server Frontend

**Role:** subsystem contract and current implementation status.

The Sophia X Server Frontend is Sophia’s long-term modern X server
implementation. It presents the **X11 API and wire protocol** directly to
applications, then emits `SurfaceTransaction` values to Sophia Engine. It takes
the Phoenix strategic approach: a clean-room implementation of the modern X11
subset real applications require, expanded by compatibility evidence rather
than by reproducing all of Xorg.

It is not a plan for a separate application-facing Sophia display protocol. X11
is the native application API of this path; forward progress happens in the
server architecture, DRM/KMS presentation, and targeted X11 extensions. XLibre
is a retired prototype and possible future compatibility provider, not an
active integration lane. The implementation crate is currently named
`sophia-x-authority`; the name reflects its protocol role and does not narrow
the product direction.

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
| Frontend → Engine/live-presentation intake | `XAuthorityObservedTransactionBatch` containing the originating frontend client when available, `SurfaceTransaction` values, surface removals, CPU buffer updates, DMA-BUF registrations, fence registrations, and Present submissions | This is the only visual ingress. The batch is bounded; backpressure is an error rather than an unbounded queue. A live consumer must transfer native FDs immediately into renderer-private ownership; Engine scene records receive Sophia surface and buffer facts, never raw X11 parsing, XID ownership, or native renderer objects. |
| Engine → frontend | `RoutedInputRequest` plus `XAuthorityClientControlCommand` | Engine selects the physical-input surface and owns global/local coordinates. The frontend resolves the surface to its connection, applies authority-local XKB/pointer state, and requests X-visible focus or configure results. It returns client-labeled delivery/control acknowledgements. |
| Engine → frontend | bounded `OutputTopologySnapshot` | Engine remains the source of physical output facts. Setup and populated RandR resources derive from the current validated generation; accepted live updates produce mask-selected screen, CRTC, output, and resource notifications. |
| Engine/backend → frontend | `XServerFrontendProtocolRouter` presentation feedback | The cloneable protocol-only router emits Present Complete and Idle by exact `TransactionId`. It cannot route input, mutate scene state, import buffers, or submit scanout. The persistent session calls Complete only after the imported frame receives real page-flip feedback, then triggers release and calls Idle. |

The existing implementation covers every row. X11 socket dispatch
emits bounded, client-attributed transaction batches; Engine routes key/pointer
events plus focus/configure commands back to the owning client; accepted Engine
topology updates drive RandR notifications; and the broker can clone a
protocol-only Present feedback router. The persistent native session transfers
registrations into backend-owned resource storage, renders mixed CPU/DMA-BUF
frames, and routes the final row only from matching page-flip retirement. The
frontend supports bounded concurrent
workers: callers use
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
batch is observed. The persistent live session now uses the bounded brokered
service loop: it admits and reaps routed workers up to the configured cap, and
on shutdown it stops admission before draining connected workers. A two-client
socket regression proves each worker receives only its own routed key and
configure request, returns a client-labeled acknowledgement, and tears down
cleanly. `x-authority-xterm-two-client-smoke` now launches two independent
real xterms against a two-worker frontend, waits for both client routes and CPU
surfaces, routes distinct key sequences by client ID, and proves two successive
pixel changes before draining the service. The live launcher still starts one
xterm by default, but `--secondary-terminal` starts and supervises a second
xterm on the same bounded frontend. `tools/live_session_two_xterm_hardware_proof.sh`
remains the earlier KMS-backed CPU-layer gate. The stricter
`tools/live_session_milestone3_hardware_proof.sh` now supplies paired `session`
evidence under classic-shared and fresh zero-capability confined profiles. Both
runs require physical keyboard and pointer input, authenticated RandR delivery,
configure-plus-pixels resize, two retained CPU layers, and clean KMS teardown.
Initial Engine focus is
acknowledged by the owning X11 client before the input proof begins, so
either focused terminal can demonstrate delivery. X11 map/configure lifecycle
updates no longer overwrite a surface's committed-pixel generation. Live setup
and populated RandR CRTC/output/mode resources use Engine-derived topology
facts. Each accepted client now gets
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
consume an immutable session-created `NamespaceContext`; the legacy constructor
creates a classic-shared context for existing smoke callers. Production config
installs a protocol-neutral admission policy: after setup authentication and
before X client/resource allocation, the policy receives bounded authentication
provenance plus kernel peer credentials and returns a distinct immutable
`ClientAdmissionContext`. Denial receives a normal X11 setup-failure reply. An
admission lease revokes the context after resource/route cleanup and also on
early worker errors. The live classic session requires the peer UID to match
the session user and deliberately assigns each connection the same shared
namespace through the session registry. The configuration can also require a
session-scoped `MIT-MAGIC-COOKIE-1`; raw cookie bytes never enter the admission
record. Legacy smoke helpers still default to unauthenticated local sockets.
The live supervisor publishes a fresh kernel-random cookie in a standard
owner-only Xauthority record, supplies only the path to launched terminals, and
removes the file on teardown. Multiple independently credentialed confined
groups on one listener remain before treating the listener as a general local
X server.

The live launcher accepts `--namespace-profile=classic|confined`. Classic is
the default and intentionally shares its namespace among launched terminals.
Confined allocates a fresh group namespace with explicit zero portal
capabilities. Admission contexts preserve that immutable profile. A concurrent
wire regression assigns two clients separate confined contexts and proves
cross-namespace map, property mutation, and selection ownership fail with
native `BadAccess`; foreign selection conversion returns
`SelectionNotify(property=None)`, and denied property mutation emits no metadata
candidate. These proofs caught and closed missing validation on map, property,
selection-owner, selection-requestor, drawable, and event-target paths. A
routed two-client regression additionally proves that a rejected foreign
event-mask request cannot redirect a confined worker's key events: input stays
on the broker-addressed client and retains that worker's root target. Separate
confined-group credentials on one listener remain active work. The routed
service now accepts targeted revocation by `ClientAdmissionId`; it disconnects
only the matching worker and leaves route/resource/surface/admission cleanup in
that worker's ordinary teardown path. The simultaneous classic-client proof
verifies its peer remains usable.

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
  transfer. This profile is explicitly launchable, uses client-aware routing,
  and has paired Milestone 3 session evidence; it is not an alias for the
  classic shared namespace.

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
- bounded direct-Mesa GLX bootstrap for clients that render through the proven
  DRI3/Present path.

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
The two-client xterm smoke repeats that proof for two separate client IDs with
distinct `alpha` and `bravo` input sequences, proving the broker does not
broadcast those Engine routes. It remains CPU-buffer evidence, not KMS-backed
multi-app session promotion evidence.
zenity admitted selection-owner lookup, root colormap creation, core
ClientMessage delivery, reduced `MIT-SHM`, `RANDR`, `BIG-REQUESTS`, and the
probe-backed XKB/XI2 startup subset. Its regression now copies the client-owned
SysV SHM payload through a bounds-checked safe adapter and requires a committed
nonzero software image with `first_error=none`.

`XKEYBOARD` now advertises the probe-backed UseExtension, event-selection,
per-client flags, and GetMap subset. Engine-routed evdev keys are translated
inside the frontend with a bounded deterministic RMLVO configuration (default
`evdev`/`pc105`/`us`) and per-seat XKB effective-modifier state. Core
`GetKeyboardMapping`, `GetModifierMapping`, `KeyPress`, and `KeyRelease` remain
the client-visible compatibility baseline.

## Namespace Model

The normative identity, profile, admission, capability, and grant contract is
in [namespaces-and-portals.md](namespaces-and-portals.md). This section records
the X-specific consequences.

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

The normative broker and grant lifecycle is in
[namespaces-and-portals.md](namespaces-and-portals.md). The native X frontend
owns namespace-keyed selection state. The session broker, bounded
policy-provider IPC, expiry/revocation lifecycle, and first concrete
cross-namespace `CLIPBOARD`/`PRIMARY` source-proxy executor are complete for
`TARGETS`, `UTF8_STRING`, and bounded UTF-8 `text/plain`. Other portal kinds
still require their own evidence-driven executors.

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

For the persistent X11 session, each Engine-addressed input event also carries
an opaque delivery token. The owning client worker acknowledges that token only
after it has serialized and flushed the corresponding X11 event; route rejection
and socket-write failure are distinct reduced outcomes. The live proof waits for
every final keyboard token before it starts its bounded redraw/presentation
window. Tokens contain no XID, keycode, text, or device identity.

The persistent xterm proof waits until the focused surface's own CPU buffer has
visual detail instead of treating its initial uniform background fill as
application readiness. Another client drawing cannot satisfy this gate. This
prevents synthetic proof keys from racing the proof shell's first prompt.

## Remaining Production Gaps

The frontend is sufficient for paired xterm sessions, mixed Vulkan presentation,
and deterministic GTK3 Zenity behavior. Retained classic and confined QEMU profiles now require a committed 640x360 resize with changed CPU/SHM pixels in addition to exact input, native presentation, normal exit, and clean teardown. Its remaining production gaps are
explicit and evidence-driven:

- GTK3 promotion requires classic-shared and confined QEMU sessions proving
  virtio text, a presented cursor, pointer selection, resize redraw, normal
  exit, and clean native teardown.
- XFixes selection notifications and complete client-selected cursor-image
  presentation are not yet general contracts. Existing selection-input, region,
  and cursor request handling covers only retained probes.
- Large selection transfers through `INCR` and full Xdnd execution remain
  deferred behind the bounded clipboard reference flow.
- Render and Sync are not advertised. GLX advertises only the captured
  direct-Mesa bootstrap needed by Kitty: GLVND vendor selection, deterministic
  visuals/FBConfigs, direct context/window lifecycle, and drawable attributes.
  Indirect rendering and server-side GLX buffer submission remain deferred.
- RandR provides Engine-derived observation and notifications, not general
  client-controlled mode management. MIT-SHM intentionally copies admitted
  image ranges and does not expose shared pixmaps.
- A general local listener still needs independently credentialed confined
  groups; the current classic and explicitly launched confined profiles remain
  the supported admission shapes.

Before expanding those surfaces, the protocol-neutral `runtime_driver` owns visual phase order and production X uses one backend service poll; the remaining promotion gate is the unattended paired QEMU GTK acceptance run. Machine-specific hardware runs remain optional compatibility diagnostics. The frontend contract does not change: it emits
bounded observed batches and consumes routed input, control, topology, and
presentation feedback without owning scene, renderer, or KMS state.

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

## Historical v0 Internal Socket Runtime

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

A private `SOPHIA-PRESENT` extension remains as historical prototype evidence
for the first explicit buffer-handoff reducer. It is not the forward path and
must not be extended or used for application promotion. The CLI
present-pixmap smoke retains only its bounded regression value.

Standard DRI3 1.2 and Present are the active path. The socket boundary uses
`recvmsg`/`sendmsg` to carry bounded SCM_RIGHTS records in both directions,
queues ancillary FDs in Unix-stream order across requests that consume none,
and drains exactly the declared arity for each FD-bearing request. DRI3 `Open`
gets one independently opened same-GPU render-node FD from a backend provider
without storing a device path in Engine or the authority runtime. `PixmapFromBuffer`,
modifier-bearing `PixmapFromBuffers`, `FenceFromFD`, supported-modifier queries,
Present `Pixmap`/`SelectInput`/`QueryCapabilities`, and the bounded XFIXES region
lifecycle required by Mesa are implemented.

The Mesa RADV `x-authority-vkcube-smoke` reaches one accepted standard Present
transaction and one committed runtime surface with `first_error=none`; it
remains the transport-only proof. `LiveDmaBufPresentationRegistry` owns reusable
source and per-Present FD lifetimes. The persistent session now connects it to
mixed rendering and exact page-flip retirement, while
`tools/live_session_milestone4_hardware_proof.sh` is the stricter GPU-to-KMS
promotion gate. The retained X13 run completes repeated CPU-plus-Vulkan mixed
exports and page flips, one controlled Skip/recovery, matching Complete/Idle
and idle-fence activity, and exact cleanup. Imported-image GL context state is
retired at the mixed-export boundary while the GBM scanout owner remains alive
through KMS retirement.

## Runtime Transport

The long-running X Server Frontend path uses a bounded side channel for observed
surface transactions and buffer-lifetime facts. Successful X11 drawing and
present requests still write no client-visible success reply when the X11
protocol does not require one. Instead the authority packages ready
`SurfaceTransaction` values, CPU updates, DMA-BUF/fence registrations, Present
submissions, and removals into `XAuthorityObservedTransactionBatch` records and
attempts a nonblocking send to the runtime-owned queue.

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
contract that now carries the persistent live service. The service polls the
owner-only listener without blocking the Engine path, admits at most the
configured worker count, and receives `StopAccepting` from session supervision.
That command never kills a client: it closes only new admission and drains
workers whose client streams have ended.

The callback observer helpers remain for focused tests and smoke probes. They
are not the production transport shape.

## MIT-SHM Negotiation

The frontend advertises a bounded `MIT-SHM` software-image surface. It performs
an immutable copy for each admitted update rather than retaining a client
shared-memory mapping.

`QueryExtension("MIT-SHM")` returns its assigned major opcode, and minor opcode
`0` replies to `ShmQueryVersion` with protocol version `1.2`, `shared_pixmaps =
false`. Unsupported minor opcodes fail closed as native X request errors.

`ShmAttach` records namespace-local segment metadata: the synthetic segment
XID, client-provided `shmid`, read-only bit, and generation. For an admitted
`XShmPutImage`, a narrow SysV SHM adapter validates segment size, attaches
read-only, copies only the bounded image range into a new immutable CPU-buffer
generation, and detaches immediately. The authority and Engine do not retain a
client mapping.

A missing, malformed, or cross-namespace segment returns a bounded native X11
error. Pixmap targets remain local resource activity without a surface
transaction. Invalid or already-gone detach requests are cleanup no-ops, while
cross-namespace detach is rejected. When notification was requested, an
accepted window update produces the standard MIT-SHM Completion event; a
rejected update does not.

The GTK Zenity probe retains nonzero software pixels with `first_error=none`.
Core `PutImage` and MIT-SHM therefore form the software baseline. Standard
DRI3/Present is the GPU handoff path; the private `SOPHIA-PRESENT` prototype is
not a production alternative.

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

RandR size range and screen resources sample a validated, bounded Engine
topology generation. Replies contain synthetic protocol-local CRTC, output,
mode, and name IDs plus output and CRTC detail; those IDs do not claim ownership
of native connectors or KMS objects. Accepted live topology updates emit the
selected screen, CRTC, output, and resource notifications. The paired
Milestone 3 session retains an authenticated witness for those events and
configure-plus-pixels resize evidence.

The external probe trace now includes bounded parse-error request heads. That
keeps future extension work probe-driven when Xlib labels an extension failure
imprecisely in client stderr.
