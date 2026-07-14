# Sophia Wayland Authority

**Role:** subsystem contract and maintenance status.

The first native authority slice is implemented: real Kitty connects through a
private Smithay-backed socket, commits SHM buffers, receives Engine-routed
keyboard/pointer input, and reaches native KMS presentation. During the current
X11-first roadmap, this authority remains supported under correctness,
security, recovery, and regression gates rather than protocol-expansion work.

Sophia Wayland Authority is the native protocol authority for Wayland-only
clients. Its bounded first implementation terminates a private Smithay Wayland
socket, owns protocol object/resource semantics, assigns each client a Sophia
namespace, and emits Sophia `SurfaceTransaction` records to Sophia Engine. It
must not become Sophia's compositor core.

Sophia Engine remains the only owner of physical input, scene graph state,
workspace layout, compositor chrome, frame scheduling, renderer imports, atomic
visual commits, and scanout.

## Authority Boundary

Sophia Wayland Authority owns:

- Wayland client sockets and object ID tables;
- `wl_surface` state, roles, callbacks, buffer attachments, damage, and commit
  ordering;
- protocol-local resource lifetime and errors;
- namespace-scoped object lookup and event delivery;
- reduced metadata candidates for compositor chrome;
- portal request facts for clipboard, drag-and-drop, screenshots, URI open,
  notifications, and other cross-namespace flows.

Sophia Wayland Authority must not own:

- physical input devices;
- compositor scene graph hit-testing;
- final layout, workspaces, or global shortcuts;
- compositor chrome presentation;
- portal policy decisions;
- frame scheduling, renderer imports, or scanout.

## Non-Ownership Contract

The Wayland Authority exists to terminate Wayland protocol, not to become the
desktop shell. This distinction is mandatory because Sophia's architecture only
works if there is one compositor authority: Sophia Engine.

The Wayland Authority must never own:

- workspaces or workspace switching;
- tiling, stacking, fullscreen policy, or final window placement;
- global shortcuts, keybindings, gestures, or launcher policy;
- compositor chrome, title bars, trust badges, shadows, animations, or panels;
- physical input devices or raw input grabs;
- output configuration, mode setting, frame pacing, page flips, or scanout;
- cross-namespace portal policy;
- unsanitized metadata presentation to the WM.

Allowed authority state is protocol-local:

- Wayland object IDs and resource lifetimes;
- role state for `wl_surface`, `xdg_surface`, and `xdg_toplevel`;
- configure serials and acknowledgements;
- protocol-local focus/grab state after Engine routing;
- namespace-scoped data-device offers and selections;
- protocol errors, disconnects, and lifecycle cleanup.

The practical rule is simple: if a decision affects what the whole desktop
looks like, where a surface lives, which namespace may cross a boundary, or
what gets scanned out, it belongs outside the Wayland Authority.

## Implemented First Slice

The current authority implements `wl_compositor`, `wl_subcompositor`,
`xdg_wm_base`, one `wl_output`, SHM ARGB/XRGB buffers, frame callbacks, buffer
release, and an Engine-routed keyboard/pointer seat. The native-scanout session
also advertises a deliberately narrow linux-dmabuf global: one linear or
implicit XRGB8888/ARGB8888 plane with bounded dimensions and validated stride.

SHM contents are copied into immutable CPU registrations before entering the
Engine and are the currently verified presentation route. Admitted DMA-BUFs
remain opaque outside the renderer boundary. Their native import and KMS
presentation route is experimental, enabled only with `--experimental-dmabuf`
until controlled and real-Kitty hardware proofs pass. That route must withhold
`wl_buffer.release` until Sophia observes presentation of the matching KMS
submission; admission metadata remains available so the client can legally
reattach the same buffer after release. Clipboard, drag-and-drop, popups,
explicit synchronization, decorations, and broader optional protocols remain
future slices.

## `wl_surface` Transaction Mapping

Wayland already has a useful split between pending surface state and committed
surface state. Sophia should preserve that shape instead of translating Wayland
into an X-like implicit damage model.

Each `wl_surface` has authority-local pending state:

- attached buffer, if any;
- buffer scale and transform;
- accumulated surface damage and buffer damage;
- opaque/input regions where supported by protocol state;
- frame callback requests;
- previous Sophia committed generation;
- mapped Sophia `SurfaceId`, once the surface is role-valid and visible.

`wl_surface.attach` does not emit a Sophia transaction by itself. It only
updates authority-local pending state.

`wl_surface.damage` and `wl_surface.damage_buffer` do not emit a Sophia
transaction by themselves. They accumulate bounded damage in pending state. The
authority should normalize this into Sophia `Region` data before emission.

`wl_surface.commit` is the transaction boundary. On commit, the authority
validates the pending state and emits one of these outcomes:

- no `SurfaceTransaction` when the surface is not visible, has no role, or the
  commit only updates non-visual state;
- `Pending` when a referenced buffer or synchronization primitive is not yet
  importable by Sophia's renderer boundary;
- `Ready` when geometry, buffer, damage, and previous committed generation are
  coherent;
- `Failed` when the Wayland client commits invalid protocol state that cannot
  produce a safe visual transaction;
- `TimedOut` only when an authority-owned timeout policy closes a previously
  pending buffer wait.

The emitted `SurfaceTransaction` must use `AuthorityKind::SophiaWayland` and an
authority-local ID derived from the Wayland object table. The WM never receives
the Wayland object ID, namespace ID, app ID, title, PID, or socket identity.

## Buffer Readiness

The authority should treat buffer readiness as the only path to visual truth:

- SHM buffers are ready after the authority has copied or otherwise pinned the
  committed byte range needed for the transaction.
- DMA-BUF buffers are ready after dimensions, format/modifier, plane metadata,
  and synchronization state are valid for the renderer import boundary.
- Explicit synchronization must be represented as pending until the acquire
  fence or equivalent readiness signal is satisfied.
- Null buffer commits unmap or hide the corresponding Sophia surface rather
  than presenting an empty visual transaction.

Sophia Engine commits only ready transactions on its presentation boundary. A
slow Wayland client therefore behaves the same as a slow X client under the
authority-native model: the old committed geometry and old committed buffer stay
visible until a complete transaction is ready.

## Generation Rules

The authority must track the last committed Sophia generation for each mapped
surface. Every emitted `SurfaceTransaction` carries
`previous_committed_generation`. Sophia Engine rejects stale transactions if its
current committed generation does not match.

This is the same optimistic concurrency rule used by Sophia X Authority. It
prevents an old commit, delayed buffer import, or confused authority worker from
overwriting newer visual truth.

## `xdg_toplevel` Configure, Ack, And Lifecycle

`xdg_toplevel` gives a `wl_surface` a desktop role. Sophia Wayland Authority
owns that role state, but it still does not own workspace policy or final
geometry. The WM proposes layout through Sophia Engine; the authority translates
accepted engine geometry into protocol-specific configure sequences.

The authority should keep protocol-local toplevel state:

- role object ID and mapped Sophia `SurfaceId`;
- last configure serial sent to the client;
- last configure size and state set;
- acknowledged configure serials;
- pending resize/fullscreen/maximized hints requested by the engine;
- lifecycle state: created, configured, mapped, unmapping, destroyed;
- polite-close state for `xdg_toplevel.close` and client destroy handling.

Configure flow:

1. Sophia Engine accepts layout/policy intent and asks the authority to
   configure a toplevel surface to protocol-visible bounds.
2. Sophia Wayland Authority sends `xdg_toplevel.configure` and
   `xdg_surface.configure` with a serial it owns.
3. The client replies with `xdg_surface.ack_configure`.
4. A later `wl_surface.commit` that corresponds to the acknowledged configure
   may emit a Sophia `SurfaceTransaction`.

`ack_configure` is not a visual commit. It only proves that the client accepted
the configure serial. The visual commit still happens through `wl_surface.commit`
plus buffer readiness, and Sophia Engine still decides when the resulting
`SurfaceTransaction` becomes committed visual truth.

State mapping:

- Configure sent, not acked: authority state is waiting for client protocol
  acknowledgement; no ready visual transaction should be emitted for that
  configure.
- Acked, no matching surface commit: transaction remains absent or pending
  depending on whether a buffer wait has started.
- Acked plus committed ready buffer: emit `Ready` `SurfaceTransaction`.
- Client commits a buffer for stale configure state: emit a transaction with the
  current `previous_committed_generation`; Sophia Engine rejects stale visual
  updates if its generation has moved on.
- Client destroys the role or surface: emit lifecycle artifacts that cause the
  engine to unmap/hide the Sophia surface and release protocol-local resources.

Lifecycle commands must stay polite first. A compositor chrome close button or
WM policy close request should become an authority command to send
`xdg_toplevel.close`. Forced termination is a supervisor/runtime policy outside
the authority's normal protocol path.

The WM remains blind to Wayland role IDs, app IDs, titles, namespaces, and
configure serials. It sees only opaque layout nodes and proposes geometry.

## Input Delivery

Wayland input must follow the same Sophia routing rule as every other protocol:
Sophia Engine owns physical devices and visual hit-testing; the protocol
authority owns protocol-correct delivery.

The Engine produces an accepted route:

- physical seat and device identity;
- Sophia `SurfaceId`;
- output/global position;
- surface-local coordinates after scene transforms;
- event serial and monotonic event time;
- keyboard focus intent, pointer focus intent, or touch target intent.

Sophia Wayland Authority consumes that route and applies Wayland semantics:

- map Sophia `SurfaceId` to the namespace-local `wl_surface`;
- verify the target client belongs to the route's namespace;
- apply protocol-local seat focus state;
- deliver `wl_pointer`, `wl_keyboard`, or `wl_touch` events with authority-owned
  serials where Wayland requires them;
- preserve grabs, popups, and implicit grabs as authority state;
- reject or ignore routes that point at stale, unmapped, destroyed, or
  cross-namespace surfaces.

The authority must not read `/dev/input`, perform global scene hit-testing, or
choose workspace focus policy. It can maintain protocol-local focus and grab
state only after the Engine supplies the visual target and the namespace check
passes.

Input failures should be data, not fallback privileges:

- stale route: drop and report a reduced rejected-route observation;
- namespace mismatch: deny and report a security rejection;
- destroyed surface: drop and clear protocol-local focus if required;
- active protocol grab: deliver to the grab owner only if the grab is valid for
  the same namespace and seat;
- compositor chrome hit: handled by Sophia Engine before the authority sees the
  event.

This keeps the compositor-first invariant intact while preserving Wayland's
client-visible seat, serial, focus, and grab behavior.

## Portal Inputs

Wayland protocols that expose cross-client or cross-namespace capability must
be translated into Sophia Portal requests. The Wayland Authority may collect
protocol facts, but it cannot approve the transfer itself.

Clipboard and primary selection:

- `wl_data_device` selection ownership is namespace-local authority state.
- A paste from another namespace becomes a Sophia clipboard portal request.
- Denial should produce a native Wayland failure path: no offer, cancelled
  offer, or empty/incompatible transfer depending on the protocol point.
- Approval must be single-use and tied to the source generation known at prompt
  time.
- Source owner changes revoke pending approvals before bytes are transferred.

Drag-and-drop:

- Drag source, target surface, MIME types, action set, and serial are authority
  facts.
- Cross-namespace drop intent becomes a Sophia drag-and-drop portal request.
- The authority should not expose target app identity, namespace labels, file
  paths, or titles to the WM.
- Denial maps to native cancellation/no-action semantics.
- Approval maps to a bounded handoff command whose lifetime is owned by the
  portal state machine.

Screencopy and capture-style requests:

- Any request that would expose pixels outside the requesting namespace becomes
  a Sophia screen-capture portal request.
- The Wayland Authority must not grant compositor-wide capture just because a
  Wayland extension exists.
- The Engine owns the final pixels; the Portal owns consent and scope; the
  authority only reports protocol-local request facts.
- Approved capture should return only the bounded region, output, surface, or
  namespace scope granted by policy.

URI open, notifications, and file handoff:

- Protocol-specific requests become reduced portal facts.
- The Portal decides whether the target namespace or host service may receive
  the request.
- The authority receives only the resulting command: deny, handoff, revoke, or
  protocol-local completion.

Portal request packets must be bounded and sanitized. They may include MIME
types, requested action, source/target `SurfaceId`, transfer generation, and
small display strings if already sanitized. They must not include raw file
contents, unbounded strings, namespace secrets, PIDs, socket paths, or protocol
object IDs visible to the WM.

## Namespace Rules

Wayland's object isolation is per client connection, but Sophia's isolation is
per namespace. A Wayland client may only observe objects, surfaces, data offers,
and metadata made visible to its namespace.

Cross-namespace clipboard, drag-and-drop, screencopy, URI open, and notification
flows must become Sophia Portal requests. The Wayland Authority can report the
facts of a request; it cannot make cross-namespace policy decisions itself.

## Maintenance And Deferred Work

The deterministic `WaylandSurfaceState + WaylandSurfaceEvent` reducers already
cover ordered attach/damage/commit, ready and pending buffers, null-buffer
unmap, stale generations, configure/ack lifecycle, protocol close, and
namespace-local input delivery. Keep those behaviors and the real SHM/Kitty
path as regressions.

Current maintenance gates are:

- native SHM startup, changing pixels, keyboard/pointer delivery, frame
  callbacks, buffer release, presentation, and normal teardown;
- TTY and input-service recovery after normal or failed sessions;
- the controlled linear DMA-BUF first-frame and retained 300-frame lifetime
  proofs;
- the rule that Smithay owns protocol infrastructure while Sophia Engine alone
  owns the scene, rendering, physical input, and scanout.

Additional Wayland protocols, arbitrary client DMA-BUF GPU composition, broader
desktop compatibility, and new portal adapters are deferred behind the native
X11 namespace, portal, session, and presentation milestones. A correctness,
security, recovery, or dependency-boundary regression may still interrupt that
ordering.
