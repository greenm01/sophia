# Sophia

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
 └──────────────┬───────────────────┬────────────────────┬───────────────────┘
        ▲      │                   │                    │      ▲
        │      │ opaque snapshots  │ portal events      │      │ chrome data
        │      ▼                   ▼                    ▼      │
 ┌───────────────┐        ┌────────────────┐       ┌─────────────────────────┐
 │  SOPHIA WM    │        │ SOPHIA PORTALS │       │ METADATA BROKER/CHROME │
 │ blind policy  │        │ allow/deny     │       │ redacted UI only       │
 │ layout/focus  │        │ handoff/revoke │       │ labels/icons/badges    │
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
- `docs/xlibre-prototype-regression-map.md` classifies XLibre prototype checks.
- `todo.md` tracks build phases and research milestones.

## Status

Sophia is a research prototype. The current codebase is mostly headless and
test-driven. That is deliberate. The project is first making the data model,
transaction rules, IPC boundaries, portal policy, and authority seams hard to
misuse. Real backend work comes after those contracts hold.

## License

Sophia is licensed under the BSD 3-Clause License. See `LICENSE`.
