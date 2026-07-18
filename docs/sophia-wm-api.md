# Sophia Window Manager API

**Role:** normative policy-process boundary and compatibility contract.

Sophia Engine has one window-management interface. A Sophia-native window
manager speaks this API directly. A legacy X11 window manager speaks its normal
policy protocol only to the private synthetic X server inside
`sophia-x11-wm-bridge`; the bridge translates that behavior into this same API.
Neither path is an alternate compositor or application authority.

## Ownership

Sophia Engine owns physical input, shortcut matching, committed workspace and
focus state, scene validation, rendering, and scanout. The session owns process
launch, logout, and protocol-specific polite close execution. A WM proposes
bounded policy changes and never receives:

- physical input streams, grabs, or client sockets;
- real XIDs, Wayland object IDs, namespaces, titles, classes, PIDs, or paths;
- client pixels, renderer handles, DRM objects, or portal payloads.

The Engine validates every proposal and preserves the last committed layout when
a WM is absent, incompatible, timed out, malformed, or restarting.

## Version 2 Session Negotiation

WM API version 2 uses the existing Sophia IPC frame version. It does not change
the framing or the protocol versions of brokers and authorities.

After Engine connects to the supervised WM socket, the WM sends one bounded
`WmHello` containing API version 2, capability bits, and at most 256 binding
registrations. Engine rejects unsupported capabilities, duplicate chords or
action IDs, invalid modifier masks, zero action IDs, excessive registrations,
and Ctrl-Alt-Backspace. Engine replies with one `WmSessionDescriptor` containing
the configured outputs, nine opaque workspace IDs by default, the active
workspace for every output, and the named session actions available to that WM.
No layout or action request is sent before this exchange succeeds.

A restart repeats negotiation, restores the committed workspace/output mapping,
then sends a complete relayout snapshot. Negotiation failure leaves applications
and the last frame alive while supervisor policy decides whether to retry or
remain degraded.

## Registered Actions

A binding contains an opaque action ID, a normalized evdev keycode, and a bounded
modifier mask. Engine matches the physical chord before client routing, emits one
activation on the initial press, ignores repeat presses until release, consumes
the matching release, and exposes only the action ID to the WM.

`WmRequestKind::ActionActivated` carries the action ID plus current output,
workspace, focused surface, and an immutable layout-node snapshot. The WM may
respond with the same transactional layout commands used for manage and relayout
requests.

Named session actions are advertised tokens. A WM may request an advertised
token with an optional opaque target surface. It cannot supply an executable,
arguments, environment, signal, or protocol handle. Initial configured tokens
cover terminal, application launcher, Firefox, close-focused, and logout.

## Workspace Model

The initial session creates nine workspaces. Each output displays exactly one
workspace and a workspace is visible on at most one output.

Engine validates and atomically commits:

- activate a workspace on an output;
- swap visible workspaces when the requested workspace is already on another
  output;
- assign or move a surface to a workspace;
- optionally focus a valid visible surface;
- configure and place the visible surfaces.

Activating a hidden workspace replaces the target output's current workspace.
Activating a workspace visible elsewhere swaps the two outputs' workspaces.
Focus follows the target output and falls back to the first focusable visible
surface when the prior focus is no longer visible.

## Legacy X11 WM Profiles

The compatibility bridge is generic, while concrete legacy behavior is selected
by a bounded profile. A profile declares bindings and maps action IDs to either
synthetic policy input or a private Sophia action message.

The bundled xmonad profile preserves familiar focus, layout, workspace,
move-to-workspace, terminal, close, launcher, Firefox, and logout chords.
Policy-only actions become bounded synthetic events on xmonad's private display.
Workspace and session actions use private bridge messages and emerge as normal
Sophia WM commands. They never execute an application on the synthetic display.

The profile supplies generic empty ICCCM/EWMH property data. Metadata-dependent
legacy rules are unsupported by design. Future policy tags require a separate
explicit broker contract and cannot expose raw application metadata.

## Failure Rules

All vectors and strings are bounded before allocation. Unknown action IDs,
unadvertised session tokens, stale surfaces, nonexistent workspaces, duplicate
workspace visibility, invalid geometry, and transaction mismatch reject the
whole proposal. No rejected action falls through to client input. No failed WM
request launches a process, changes focus, or partially mutates workspace state.

## Evidence Levels

Milestone 7 requires identical direct-API and bridge evidence for negotiation,
bindings, focus, layout, workspaces, session actions, restart, and last-layout
preservation. The xmonad QEMU gate uses real client surfaces and virtio input;
internal injection cannot satisfy it.

Milestone 8 adds the normal session launcher, Firefox, the retained X11
application mix, multi-output workspace behavior, and the unattended soak.
Machine-specific DRM/input runs are optional compatibility diagnostics.

