# Sophia

Sophia is a research prototype for a modern X11 session.

XLibre remains the client-facing X11 server, resource authority, window-system
authority, and Xnamespace isolation layer. Sophia supplies the modern display
engine around it: compositor-first input, frame-aware rendering, and an external
window-manager policy process.

Sophia is not Xwayland and not a Wayland compositor with X11 compatibility as a
sidecar. It is an XLibre-centered attempt to modernize X11 by moving physical
input, final composition, and display timing out of the legacy server hot path
while preserving X11 client compatibility.

## The Modernized X11 Architecture

```text
       [ Physical Input Devices / Kernel ]
                       │
                       │ (1) Raw Input Events (Mouse clicks, Keystrokes)
                       ▼
 ┌───────────────────────────────────────────┐
 │             MODERN COMPOSITOR             │◄───┐
 └─────────────────────┬─────────────────────┘    │
                       │                          │ (5) Atomic Layout Updates
                       │ (2) WM Keybindings       │     & Window Frames
                       ▼                          │
 ┌───────────────────────────────────────────┐    │
 │       WINDOW MANAGER (Policy Engine)      ├────┘
 └───────────────────────────────────────────┘
         │
         │ (3) Spawns apps with Namespace env tokens
         ▼
 ┌────────────────────────────────────────────────────────────────────────┐
 │                      XNAMESPACES SANDBOX LAYER                         │
 │                                                                        │
 │  ┌──────────────────────────────────┐  ┌────────────────────────────┐  │
 │  │      NAMESPACE A (Trusted)       │  │   NAMESPACE B (Untrusted)  │  │
 │  │                                  │  │                            │  │
 │  │  [Terminal]        [File Manager]│  │  [Untrusted Web Browser]   │  │
 │  └──────┬───────────────────┬───────┘  └─────────────┬──────────────┘  │
 └─────────┼───────────────────┼────────────────────────┼─────────────────┘
           │                   │                        │
           │ (4) Standard X11 Protocol Traffic          │
           ▼                   ▼                        ▼
 ┌────────────────────────────────────────────────────────────────────────┐
 │                        MODIFIED XSERVER (XLibre)                       │
 │                                                                        │
 │  [XID Virtualization Registry]                                         │
 │   ├── Namespace A Matrix: Maps local XIDs -> Compositor surfaces       │
 │   └── Namespace B Matrix: Isolated from Namespace A completely         │
 └─────────────────────────────────┬──────────────────────────────────────┘
                                   │
                                   │ (6) Redirected Offscreen Pixmaps
                                   │     (via XComposite Extension)
                                   ▼
                       [ Back to Modern Compositor ]
                       [ (Combines buffers frame-perfectly) ]
                                   │
                                   ▼
                           [ Display Screen ]
```

## Data Path

**Path 1 and 2, the hot path.** Input reaches Sophia's compositor first. The
compositor owns the actual scene graph, transforms, and output geometry, so it
can map physical coordinates to visual surfaces without asking XLibre to guess
from a flat legacy window tree. Global shortcuts are handled by the compositor
and forwarded to the external WM over Sophia's private policy protocol.

**Path 3 and 4, the sandbox path.** The WM or session launcher starts apps with
namespace-specific X11 credentials. Apps still speak ordinary X11 to XLibre.
XLibre applies Xnamespace isolation so a client can see and affect only the
resources inside its namespace.

**Path 5 and 6, the render loop.** The WM sends atomic policy updates to the
compositor. XLibre redirects X11 windows to offscreen pixmaps through XComposite
and reports damage. Sophia imports those updates into its scene graph and
presents coherent frames.

## Project Shape

- **Sophia Engine** owns physical input, the scene graph, frame scheduling, and
  display output.
- **Sophia WM** owns policy: layout, focus policy, keybindings, workspaces, and
  launch decisions.
- **XLibre** owns X11 protocol compatibility, resources, selections, grabs, and
  Xnamespace enforcement.
- **Sophia X Bridge** is the privileged integration layer between XLibre and the
  compositor.
- **Sophia Portals** mediate intentional namespace crossing for clipboard,
  drag-and-drop, file access, screenshots, and notifications.

## Documentation

- `docs/architecture.md` maps processes and load-bearing boundaries.
- `docs/dod.md` defines Sophia's data-oriented design rules.
- `docs/style-guide.md` records implementation discipline.
- `docs/research-log.md` captures early decisions and open research questions.
