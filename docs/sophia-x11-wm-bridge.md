# Sophia X11 WM Bridge

Status: translation core implemented; embedded X server and real xmonad smoke
remain after the interactive xterm milestone.

The Sophia X11 WM Bridge is a legacy window-manager translation daemon. It sits
in the Sophia WM policy slot and lets existing X11 window managers such as i3,
dwm, xmonad, and qtile calculate layouts for a Sophia session without gaining
access to Sophia's protocol authorities, namespaces, metadata, rendering, or
input streams.

The bridge is not Sophia X Authority. It does not run client applications and
does not serve pixels. It is a fake, headless X11 server for one legacy window
manager process.

## Architectural Conflict

Traditional X11 window managers expect to manage a global X server. They listen
for `MapRequest`, inspect properties such as `WM_CLASS` and `_NET_WM_NAME`, and
issue requests such as `ConfigureWindow` or `XMoveResizeWindow`.

Sophia deliberately forbids that model. Sophia Engine sends opaque
`LayoutNodeSnapshot` values to a policy process and accepts only bounded
`WmResponsePacket` proposals. The WM must not see real XIDs, namespaces, raw
metadata, protocol object IDs, buffers, or client sockets.

Letting a legacy X11 WM connect directly to Sophia X Authority would break this
boundary. It would encourage global X-style visibility, leak X-only assumptions
into a protocol-neutral compositor, and leave the WM blind to non-X clients.

The bridge resolves the conflict with a two-faced facade:

- to Sophia Engine, it is a compliant blind WM process speaking the standard
  Sophia WM IPC protocol;
- to the legacy X11 WM, it is a minimal headless X11 server with a synthetic
  window tree.

## Boundary And Ownership

The bridge is an isolated binary/library crate named
`sophia-x11-wm-bridge`. Its first compatibility target is xmonad.

It owns:

- a fake X11 socket for the legacy WM, such as a private `DISPLAY`;
- X11 connection state for the attached WM process;
- synthetic X resource IDs for fake root and top-level windows;
- a bidirectional table mapping Sophia `SurfaceId` values to synthetic
  `XWindowId` values;
- fake X event queues, event masks, and property replies needed by the WM.
- supervision of one xmonad process attached to the private bridge display.

It must not own:

- rendering, pixmap allocation, buffers, frame scheduling, or scanout;
- physical input devices or real input routing;
- Sophia X Authority client resources;
- Wayland Authority resources;
- real client metadata, titles, classes, icons, PIDs, paths, or namespace IDs;
- portal policy or cross-namespace transfer decisions;
- compositor chrome.

The bridge's only authority is translating policy math. Sophia Engine remains
the compositor authority and validates every returned layout proposal.

## Sophia-Facing Protocol

The bridge's Sophia-facing side is the existing WM IPC protocol:

- decode one bounded `WmRequestPacket` from Sophia Engine;
- maintain transaction-local and persistent synthetic window state;
- wait for the legacy WM to express layout through fake X requests;
- encode one bounded `WmResponsePacket` with the same transaction ID.

The bridge must preserve Engine-owned transaction control. It must not mint
Sophia transaction IDs, initiate unsolicited Sophia layout commands, or drive
animations. If the legacy WM is silent, crashes, sends malformed X requests, or
returns no usable layout before the timeout, the bridge should return an empty
or timed-out proposal and let Sophia Engine preserve the last committed layout.

The bridge may emit these Sophia commands:

- `ConfigureSurface` for requested surface sizes;
- `RenderSurface` for compositor-space placement and z-order;
- `FocusSurface` when the legacy WM selects a focus target;
- `AssignWorkspace` only when a legacy workspace signal can be mapped to an
  existing Sophia `WorkspaceId` without exposing metadata.

## Legacy-WM-Facing Protocol

The bridge's X-facing side implements a fake server sufficient to make a legacy
WM run its layout policy. It should start with local Unix sockets only. Network
transparent X is out of scope.

Required baseline:

- X11 connection setup;
- one fake root window per Sophia output/workspace view needed by the bridge;
- synthetic top-level windows corresponding to opaque Sophia layout nodes;
- `MapRequest`, `UnmapNotify`, destroy, configure, focus, and structure events;
- event-mask registration for substructure redirect/notify and property
  changes;
- property reads/writes with generic or blackholed data;
- enough ICCCM/EWMH atoms for common WMs to stay alive.

Ignored by design:

- Render, GLX, XVideo, DRI3, SHM, XComposite, and pixmap content;
- XInput and physical input routing;
- XTEST-style global input injection;
- real clipboard, drag-and-drop, screenshots, URI open, or file handoff;
- any extension whose only purpose is drawing pixels.

The fake server serves policy objects, not application windows. A synthetic
window is only a handle that lets a legacy WM calculate rectangles.

The first xmonad milestone is policy-only. Sophia retains physical input,
global keybindings, workspace commands, and focus validation. The bridge does
not forward raw keyboard events into xmonad, so native xmonad Mod-key bindings
are outside the first milestone. The server is embedded in the bridge; Xvfb is
not used.

The official xmonad source may be checked out under `~/src/xmonad` and inspected
as a compatibility reference. It is not vendored, linked, or required at
Sophia runtime. The real smoke resolves the xmonad executable through `PATH`
with an explicit environment override for local builds.

The reference checkout is currently at commit `a9a8b5c`. The workspace crate
owns bounded synthetic XID allocation, lifecycle event reduction, and
metadata-blind configure/focus translation. The host currently has no `ghc`,
`cabal`, `stack`, or `xmonad` executable, so real startup protocol capture is
gated on installing a Haskell toolchain or xmonad package.

## Inbound Translation: Engine To Legacy WM

When Sophia Engine needs policy, it sends a `WmRequestPacket`.

For `ManageSurface`:

1. Decode the opaque `LayoutNodeSnapshot`.
2. Mint a synthetic `XWindowId` if this `SurfaceId` has not been seen before.
3. Create or update a fake top-level window record with the node's geometry,
   capabilities, state, workspace, generation, and synthetic XID.
4. Emit a fake `MapRequest` or equivalent lifecycle event to the legacy WM.
5. Answer metadata/property queries with generic placeholders.

For `RelayoutWorkspace`:

1. Update the fake root/output bounds.
2. Ensure every node has a synthetic X window.
3. Emit configure/map/unmap/focus/property events needed to make the WM
   recompute layout.
4. Wait for resulting legacy WM requests and translate them back to Sophia
   commands.

For `SurfaceRemoved`:

1. Resolve the synthetic XID for the removed `SurfaceId`.
2. Emit destroy/unmap events to the legacy WM.
3. Remove the synthetic mapping after the legacy event is queued.

## Outbound Translation: Legacy WM To Engine

The bridge intercepts legacy WM requests against synthetic windows.

`ConfigureWindow`, `XMoveResizeWindow`, and equivalent requests become:

- `ConfigureSurface` when width or height changes;
- `RenderSurface` when x, y, stack order, crop, or transform-equivalent facts
  change.

Focus requests become `FocusSurface` only if the synthetic XID maps to a
focusable current `SurfaceId`.

Workspace requests are conservative. If a legacy WM expresses workspace changes
through EWMH desktop atoms, the bridge may map known numeric desktops to Sophia
`WorkspaceId` values. Unknown, string-based, or metadata-derived workspace rules
must be ignored or reduced to the current workspace.

Unknown X requests are acknowledged only when doing so is necessary to keep the
WM alive. They must not escape the bridge or mutate Sophia state.

## Metadata Spoofing

Legacy WMs often use class names, titles, roles, and EWMH properties for rules.
Sophia's blind WM boundary does not permit raw metadata in layout policy, so the
bridge must spoof or redact it.

Default property behavior:

- `_NET_WM_NAME`: empty or generic title such as `Sophia Surface`;
- `WM_NAME`: same as `_NET_WM_NAME`;
- `WM_CLASS`: generic instance/class, such as `sophia-surface`;
- `WM_WINDOW_ROLE`: empty;
- `_NET_WM_PID`: absent;
- icons: absent;
- namespace facts: never represented.

This intentionally degrades legacy WM rule systems that depend on real titles
or classes. A later sanitized-label mode may be designed, but it must be a
separate metadata-broker policy decision and must never expose raw client
identity to arbitrary legacy WM code.

## Failure Behavior

The bridge is untrusted policy glue. Failure must be contained.

- If the legacy WM crashes, Sophia Engine keeps the last committed layout and
  the bridge process can be restarted by the normal supervisor path.
- If the legacy WM sends invalid X requests, the bridge rejects or ignores them
  locally.
- If the legacy WM proposes geometry for an unknown or stale synthetic XID, the
  bridge drops that proposal.
- If the bridge cannot produce a valid `WmResponsePacket`, Sophia Engine treats
  it like any other WM IPC failure and preserves visual state.

The bridge is outside the rendering and input hot paths, so its failure should
not blank the desktop, expose client buffers, or interrupt scanout.

## Lifecycle And Retirement

This bridge exists to bootstrap usability while native Sophia WMs mature. It is
useful because existing tiling WMs already encode years of layout behavior, but
it is not the ideal policy API for Sophia.

Native Sophia WMs should remain the preferred long-term path. They speak the
blind IPC protocol directly, avoid fake X server complexity, and can represent
Sophia concepts without global X assumptions.

When native Sophia WMs are sufficient for daily use, this bridge should remain
optional compatibility code or be deleted.
