# The Linux Desktop Problem

The current landscape forces a choice between extremes: decentralized freedom versus centralized bureaucracy.

X11 is a beautiful, asynchronous disaster. Built for diskless terminals and slow networks, it offers a hacker's playground—a shared property tree where any script can move any window. You want a tiling window manager? You write one. You want a global hotkey daemon? You build it. X11 never asks a committee for permission. But X11 tears. It leaves black borders during resizes, and it operates on the flawed assumption that every client is trustworthy.

Wayland stepped in to fix the visual rot. It enforces atomic buffer swaps and secures the desktop. The tearing stopped, but so did the freedom. Wayland makes the compositor a dictator. If you want a screenshot tool or a custom dock, you wait for competing developers to ratify an XML schema. It traded the permissionless joy of the Linux desktop for a bureaucratic straitjacket.

Sophia rejects this false binary.

Sophia is a secure, atomic session stack for the Linux desktop. It shatters the monolithic display server and divides the labor.

# The Engine Dictates the Pixels

Sophia Engine is the absolute visual authority. It hit-tests the scene, schedules the frames, and owns the scanout. It enforces a simple, unbreakable rule: no new geometry appears on the screen without matching, committed pixels. If an application hangs during a resize, Sophia fails closed. The old, perfectly rendered layout remains on the screen. 

## The Authorities Translate the Past

Sophia does not force the world to rewrite its software. It hosts Protocol Authorities. The Sophia X Authority and Sophia Wayland Authority sit at the edges. They terminate the client protocols, enforce strict namespace sandboxes, and translate legacy requests into atomic surface transactions. They do not own workspaces. They do not dictate layout. They merely translate.

## The Window Manager Remains Blind

Layout policy belongs in an external process. The Sophia Window Manager receives opaque layout nodes. It never sees an XID, a window title, or a Wayland object ID. It crunches the geometry and returns command packets. Because the window manager is blind, it cannot leak secure metadata. Because it sits outside the rendering hot path, it can crash, restart, or be rewritten in any language without taking down the session.

Sophia gives you the flawless visual integrity of a modern macOS environment and the hackable freedom of a tiling setup. 

No tearing. No shared mutable state. NOT designed by committee.

# The Sophia Session Stack

Sophia is a secure, frame-perfect session stack for X11 and Wayland.

The project starts from a plain frustration: Linux graphics stacks make you
choose too often. X11 is open and hackable, but it lets clients share too much
state and makes tear-free resizing a negotiation. Wayland fixes the visual
model, then pushes every desktop feature through a compositor and a protocol
process that can move slowly.

Sophia takes a different route. The compositor is the visual authority. Protocol
servers for X11, Wayland, and future clients sit behind it as translation
layers. The window manager stays outside the hot path and sees only opaque
layout nodes. Portals handle deliberate namespace crossing. The goal is a
desktop that keeps the sharp edges people like about X11 without making every
client part of one shared trust domain.

One rule drives the design:

```text
Do not present new geometry unless the matching pixels are ready.
```

Slow clients may lag. Misbehaving clients may get degraded. They should not
tear the desktop, expose black borders, or force the compositor to present a
half-finished resize.

## Architecture

```text
================================================================================
                         HARDWARE AND KERNEL
================================================================================
 [ physical input devices ]                                  [ display output ]
            │                                                        ▲
            │ raw input via libinput                                 │ DRM/KMS
            ▼                                                        │

================================================================================
                    SOPHIA ENGINE: COMPOSITOR AUTHORITY
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Scene graph | spatial hit-testing | damage tracking | frame scheduling     │
 │ Atomic visual commits | rendering | scanout                                │
 └───────────────┬───────────────────┬────────────────────┬───────────────────┘
          ▲      │                   │                    │      ▲
          │      │ opaque snapshots  │ portal events      │      │ chrome data
          │      ▼                   ▼                    ▼      │
 ┌───────────────┐        ┌────────────────┐       ┌─────────────────────────┐
 │  SOPHIA WM    │        │ SOPHIA PORTALS │       │ METADATA BROKER/CHROME  │
 │ blind policy  │        │ allow/deny     │       │ redacted UI only        │
 │ layout/focus  │        │ handoff/revoke │       │ labels/icons/badges     │
 └───────┬───────┘        └────────┬───────┘       └────────────┬────────────┘
         │                         │                            ▲
         │ layout proposals        │ portal commands            │ sanitized
         ▼                         ▼                            │ metadata

================================================================================
                         PROTOCOL AUTHORITY LAYER
================================================================================
 ┌────────────────────────────────────────────────────────────────────────────┐
 │ Sophia X Authority | Sophia Wayland Authority | Sophia Native Authority    │
 │ protocol resources | grabs/focus | selections | namespace checks           │
 └────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                                  │ namespace-checked surface transactions
                                  │ routed input / configure / lifecycle
                                  ▲

================================================================================
                         SANDBOXED CLIENT NAMESPACES
================================================================================
 ┌────────────────────────────────────┐     ┌─────────────────────────────────┐
 │ Namespace A: trusted               │     │ Namespace B: untrusted          │
 │ X terminal | Wayland file manager  │  X  │ X browser | Wayland chat app    │
 └────────────────────────────────────┘     └─────────────────────────────────┘
```

## How It Works

Input reaches Sophia Engine first. The engine owns the scene graph, transforms,
outputs, and frame loop, so it maps physical input to the surface the user can
see. A protocol authority then performs the protocol-specific delivery rules:
focus, grabs, event masks, serials, and namespace checks.

Each authority terminates one client protocol. The Sophia X Authority speaks a
modern X subset. A later Wayland Authority can speak Wayland. A native authority
can serve Sophia-first clients. Authorities own protocol resources and client
semantics; they do not own layout, scanout, global shortcuts, compositor chrome,
or portal policy.

The WM is a policy process. It manages workspaces, focus policy, layouts,
keybindings, and launch decisions, but it does that through opaque handles. It
does not need XIDs, namespace IDs, window titles, app classes, or clipboard
payloads.

Rendering is transaction-based. The WM proposes layout. Authorities provide
pending buffers, damage, constraints, and readiness. Sophia Engine commits
geometry and pixels together on a frame boundary. If a surface is not ready, the
engine keeps the last committed state until policy says otherwise.

## Security Architecture

Sophia starts from a blunt assumption: every client may lie. The window manager
may crash. A protocol may carry thirty years of bad habits. Security is not a
feature added after the desktop works. It is the shape of the system.

### Namespaces At The Edge

Protocol Authorities sit at the edge of the session. When an application
connects, the authority assigns it to a namespace before it can create useful
state. An untrusted browser in one namespace cannot query, inspect, or send
events to a trusted terminal in another.

This is where Sophia breaks with old X11. There is no shared property tree where
every client becomes a potential observer. Cross-namespace lookup fails closed
unless a portal grants a narrow handoff.

### A Blind Window Manager

The WM manages layout, focus, and workspaces, but it does not receive client
identity. It sees opaque layout nodes and `SurfaceId` handles. It does not see
XIDs, Wayland object IDs, namespaces, titles, classes, PIDs, paths, or clipboard
payloads.

That blindness is deliberate. If the WM is compromised, the attacker gets a
geometry calculator, not a desktop-wide spyglass. The process can propose where
rectangles go. Sophia Engine still validates the proposal before anything
reaches the screen.

### Portals, Not Privileged APIs

Namespaces sometimes need to cross. Clipboard, drag-and-drop, file handoff,
screenshots, notifications, and URI opens all require controlled transfer.
Sophia routes those requests through portals.

A portal request is a state machine, not a backdoor:

- **Pending:** the request exists, but the target does not receive the payload;
- **Prompt:** user or policy code sees bounded, sanitized facts;
- **Approval:** a single-use, generation-bound handoff is granted;
- **Revocation:** owner changes, expiry, or policy denial close the transfer.

Denial maps back to native protocol failure. Clients do not get synthetic input.
They do not get to freeze the session while they wait.

### The Engine Owns Visual Truth

Sophia Engine is the only component with global visual knowledge. It owns
hit-testing, frame scheduling, rendering, and scanout. It does not own high-level
layout policy.

The engine enforces the visual security rule: geometry and pixels commit
together. A slow or hostile client cannot make Sophia present a half-resized
window, black border, or stale buffer stretched into a new shape. The last good
frame stays visible until a complete transaction is ready or explicit policy
chooses to degrade it.

### Small Surface Area

Sophia keeps protocol complexity at the edges. Authorities translate client
protocols. The WM receives blind policy data. Portals handle deliberate
cross-namespace transfer. The rendering hot path stays small and data-oriented.

That is the security pitch: fewer global privileges, fewer trusted processes,
and fewer ways for one client to learn what another client is doing.

## Project Shape

Sophia is split by authority, not by convenience.

- **Sophia Engine** owns physical input, visual state, frame scheduling,
  transaction commits, rendering, and display output.
- **Protocol Authorities** own client compatibility and translate protocol
  state into Sophia surface transactions.
- **Sophia WM** owns layout, focus policy, keybindings, workspaces, and launch
  decisions.
- **Sophia Portals** handle intentional namespace crossing: clipboard,
  drag-and-drop, files, screenshots, notifications, and URI handoff.
- **Metadata Broker and Chrome** turn protocol metadata into redacted
  compositor UI without giving the WM namespace visibility.

## Documentation

- `docs/architecture.md` maps processes and load-bearing boundaries.
- `docs/dod.md` defines Sophia's data-oriented design rules.
- `docs/sophia-x-authority.md` defines the long-term modern X authority.
- `docs/style-guide.md` records implementation discipline.
- `docs/research-log.md` captures early decisions and open research questions.
- `docs/research-log-archive.md` preserves completed research and validation
  evidence.
- `research/xlibre/docs/xlibre-prototype-regression-map.md` maps retired XLibre
  checks to active Sophia-owned regressions.
- `todo.md` tracks build phases and research milestones.

## Status

Sophia is a research prototype. Deterministic tests protect the data model and
authority boundaries, while AMD TTY evidence proves native GBM/KMS allocation,
atomic submit, page-flip retirement, cleanup, and exact operator keyboard input
changing presented xterm pixels. The isolated QEMU harness passes 300 persistent
native-session ticks with two independently owned outputs, distinct content,
page-flip-paced fixed refresh, and routed keyboard/pointer input. Real xmonad
also passes as blind two-window layout policy against synthetic windows; it is
a compatibility proof, not a hard-coded Engine component. The generic live
session now applies real xmonad placement, resize, and focus to one real xterm.
Its headless gate requires a readable fixed-font ASCII marker, one acknowledged
configure, and a later injected-input pixel change; the dedicated-TTY operator
visual gate remains.
Real Kitty now connects to Sophia's private native Wayland authority with
`DISPLAY` removed. SHM frames pass through the protocol-neutral Engine path;
the native session additionally admits a bounded linear DMA-BUF subset behind
an experimental flag. SHM is the verified default. The DMA-BUF importer now has
a controlled first-frame/lifetime proof and a three-run real-Kitty promotion
gate, but no real hardware DMA-BUF pass has been recorded yet. The installed
launcher uses the native Wayland path and retains the independent
Ctrl-Alt-Backspace recovery guard. XLibre is no longer a production dependency,
feature, workspace member, or launcher path; its frozen sources live under
`research/xlibre`. The remaining Kitty gates prove resize,
keyboard/navigation/pointer input, sub-100 ms presentation, clean TTY recovery,
and then DMA-BUF performance. VRR evidence still requires a display whose
connector reports `vrr_capable=1`.

## License

Sophia is licensed under the BSD 3-Clause License. See `LICENSE`.
