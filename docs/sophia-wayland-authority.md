# Sophia Wayland Authority

Sophia Wayland Authority is a future protocol authority for Wayland-only
clients. It must not become Sophia's compositor core. It terminates Wayland
client protocol, owns Wayland object/resource semantics, enforces namespace
checks for those clients, and emits Sophia `SurfaceTransaction` records to
Sophia Engine.

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

## Namespace Rules

Wayland's object isolation is per client connection, but Sophia's isolation is
per namespace. A Wayland client may only observe objects, surfaces, data offers,
and metadata made visible to its namespace.

Cross-namespace clipboard, drag-and-drop, screencopy, URI open, and notification
flows must become Sophia Portal requests. The Wayland Authority can report the
facts of a request; it cannot make cross-namespace policy decisions itself.

## First Implementation Target

The first practical Wayland Authority milestone is not a complete compositor.
It is a deterministic reducer over synthetic Wayland events:

```text
WaylandSurfaceState + WaylandSurfaceEvent -> WaylandSurfaceState + AuthorityCommand
```

The initial reducer should prove:

- attach plus damage does not emit a transaction before commit;
- commit with a ready buffer emits a ready `SurfaceTransaction`;
- commit with an unready import emits a pending transaction;
- null buffer commit produces an unmap/hide artifact;
- stale generation is rejected by Sophia Engine, not patched by the authority.

The second reducer should prove:

- configure serials are authority-owned and protocol-local;
- `ack_configure` does not emit a visual transaction by itself;
- a matching acknowledged commit emits a transaction through the `wl_surface`
  commit path;
- stale or destroyed toplevel state cannot advance committed visual truth;
- polite close maps to `xdg_toplevel.close`, not engine-owned process killing.
