# The Linux Desktop Problem

The current landscape forces a choice between extremes: decentralized freedom versus centralized bureaucracy.

X11 is a beautiful, asynchronous disaster. Built for diskless terminals and slow networks, it offers a hacker's playground—a shared property tree where any script can move any window. You want a tiling window manager? You write one. You want a global hotkey daemon? You build it. X11 never asks a committee for permission. But X11 tears. It leaves black borders during resizes, and it operates on the flawed assumption that every client is trustworthy.

Wayland stepped in to fix the visual rot. It enforces atomic buffer swaps and secures the desktop. The tearing stopped, but so did the freedom. Wayland makes the compositor a dictator. If you want a screenshot tool or a custom dock, you wait for competing developers to ratify an XML schema. It traded the permissionless joy of the Linux desktop for a bureaucratic straitjacket.

Sophia rejects this false binary.

Sophia is a secure, atomic session stack for the Linux desktop. It shatters the monolithic display server and divides the labor.

# The Engine Dictates the Pixels

Sophia Engine is the absolute visual authority. It hit-tests the scene, schedules the frames, and owns the scanout. It enforces a simple, unbreakable rule: no new geometry appears on the screen without matching, committed pixels. If an application hangs during a resize, Sophia fails closed. The old, perfectly rendered layout remains on the screen. 

## The Authorities Translate the Past

Sophia does not force the world to rewrite its software. It hosts protocol
frontends. The **Sophia X Server Frontend** presents the real X11 API to
applications, while the Sophia Wayland Authority presents Wayland. Both reduce
client requests into atomic surface transactions. They do not own workspaces or
dictate layout; they translate client protocol into Sophia Engine facts.

X11 is not a deprecated migration path in Sophia. The long-term X Server
Frontend is a Sophia-owned, modern X server implementation: X11 remains the
application API while Sophia modernizes its rendering, presentation, and output
architecture underneath. A classic shared-X profile preserves the inspectable,
scriptable model people value; confined namespaces are an explicit session
choice, not a reason to erase that model. Sophia does not currently plan a
separate application-facing “Sophia native” display protocol.

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
 │ Sophia X Server Frontend | Sophia Wayland Authority                         │
 │ X11 resources/grabs      | Wayland objects | protocol-specific checks      │
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

Each frontend terminates one client protocol. The Sophia X Server Frontend
implements a modern, compatibility-driven X11 subset; the Wayland Authority
serves Wayland clients through Smithay. Sophia adds X11 extensions only where a
real need cannot be served by the established protocol. There is no planned
third application protocol for “Sophia-first” clients. Frontends own protocol
resources and client semantics; they do not own layout, scanout, compositor
chrome, or portal policy.

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

Protocol frontends sit at the edge of the session. A **classic shared-X**
profile puts trusted applications in one namespace and deliberately preserves
the inspectable, scriptable X11 object model. A confined profile assigns clients
to separate namespaces before they can create useful state: an untrusted browser
then cannot query, inspect, or send events to a trusted terminal without an
explicit handoff.

Cross-namespace lookup fails closed unless a portal grants a narrow handoff.
Sophia treats this as a user and session-policy choice, not a reason to deny
trusted local users the shared-X workflow they selected.

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
  drag-and-drop, files, screen capture/recording, notifications, and URI
  handoff.
- **Metadata Broker and Chrome** turn protocol metadata into redacted
  compositor UI without giving the WM namespace visibility.

## Documentation

- `docs/README.md` maps normative contracts, subsystem status, evidence, and
  historical material.
- `docs/architecture.md` maps processes and load-bearing boundaries.
- `docs/namespaces-and-portals.md` defines admission, isolation profiles,
  capabilities, grants, and cross-namespace failure behavior.
- `docs/dod.md` defines Sophia's data-oriented design rules.
- `docs/sophia-x-authority.md` defines the long-term Sophia X Server Frontend
  and its X11 compatibility boundary.
- `docs/x11-compatibility-matrix.md` records the real-client evidence that
  admits each native X11 compatibility slice.
- `docs/style-guide.md` records implementation discipline.
- `docs/research-log.md` captures active decisions and research questions.
- `docs/research-log-archive.md` preserves completed research and validation
  evidence.
- `research/xlibre/docs/xlibre-prototype-regression-map.md` maps retired XLibre
  checks to active Sophia-owned regressions.
- `todo.md` tracks only active milestones and measurable exits.

## Status

Sophia is a research prototype. The primary development track is now the native
Sophia X Server Frontend. The bounded portal reference flow is complete; X11
buffer and presentation semantics are the active milestone. The completed
session-correctness milestone retains paired classic-shared and fresh confined
two-xterm evidence through Engine-owned composition and KMS. Both profiles use
physical keyboard and pointer input, Engine-derived output facts, authenticated
RandR delivery, configure-plus-pixels resize, and clean teardown; retained X13
runs reported 94/90 ms startup readiness and 13 ms maximum composition.

Standard DRI3 1.2 now carries FD-bearing `Open`, modifier-bearing multi-plane
pixmaps, xshmfences, and Present transactions through the native frontend. The
persistent renderer imports those typed resources, gates acquire fences,
composes DMA-BUF and CPU layers, and applies the prepared Engine state only
after matching native page-flip feedback. Complete-before-Idle delivery,
idle-fence triggering, rejection preservation, and exact teardown are covered
by the offline suite. The software X13 half and an isolated DMA-BUF-only run
pass, but the required CPU-plus-`vkcube` mixed draw currently reaches a Radeon
command-stream rejection in `glFinish`; that native-EGL blocker is the
remaining Milestone 4 boundary.

The namespace-keyed X resource model, profile/capability/admission types,
session-owned in-memory registry, explicit classic/confined live launch
profiles, same-UID admission, per-client revocation, fresh owner-only
Xauthority publication/removal, portal request/grant lifecycle, and owner-only
broker IPC already exist. The bounded cross-namespace enforcement matrix,
targeted admission cleanup, and authority-private native `CLIPBOARD`/`PRIMARY`
source-proxy flow are proven for `TARGETS`, `UTF8_STRING`, and bounded UTF-8
`text/plain`.

The Smithay-backed Wayland Authority remains functional and supported. Real
Kitty uses native Wayland SHM, Engine-routed input, and KMS; controlled DMA-BUF
direct-scanout lifecycle evidence is retained as a renderer regression. New
Wayland protocols and arbitrary client GPU composition are deferred while the
X11, namespace, and portal architecture matures.

XLibre is not a production dependency, feature, workspace member, or launcher
path. Its frozen source and prototype evidence live under `research/xlibre`.
Sophia may reconsider an optional provider only if measured native-X gaps later
justify that authority and maintenance cost.

## License

Sophia is licensed under the BSD 3-Clause License. See `LICENSE`.
