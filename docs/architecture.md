# Architecture

This doc maps Sophia's processes and the boundaries between them. The data model
is in `dod.md`; code-level rules live in `style-guide.md`.

Sophia is XLibre-centered. XLibre is not a guest compatibility server hidden
inside a Wayland desktop. It remains the authority X11 clients talk to. Sophia
adds a modern display engine and an external policy layer around that authority.

## Processes

```text
kernel input / DRM
        |
        v
Sophia Engine
  - libinput devices
  - scene graph
  - compositor timing
  - output/scanout
        |
        +--------------------+
        |                    |
        v                    v
Sophia WM              Sophia X Bridge
  - layout policy        - XComposite/Damage watcher
  - focus policy         - X11 window metadata
  - keybindings          - XLibre privileged requests
  - app launch           - routed-input adapter
        |                    |
        +---------+----------+
                  |
                  v
                XLibre
                  |
                  v
        Xnamespace-isolated X11 clients
```

## Load-Bearing Boundaries

### Engine to WM

The WM protocol is a policy boundary. The WM receives state changes that need
policy decisions: new windows, destroyed windows, output changes, keybindings,
workspace changes, and focus-affecting events.

The WM is not on the per-frame or per-input hot path. Sophia Engine keeps the
last committed policy state if the WM crashes or restarts.

The protocol should be sequence-oriented:

- **Manage sequence** for state that affects clients: size, focus, fullscreen,
  workspace assignment, activation.
- **Render sequence** for compositor-only state: position, z-order, crop,
  decoration geometry, opacity, transforms.

### Engine to XLibre Rendering

XComposite and Damage are the first render seam. XLibre redirects windows to
offscreen pixmaps and reports changed regions. Sophia X Bridge names or imports
those pixmaps, tracks damage, and hands frame packets to Sophia Engine.

This seam exists today in broad shape. It needs measurement and glue, not a new
theory.

The first implementation should accept ordinary X11 limitations:

- X11 clients do not have Wayland-style configure/commit acknowledgements.
- Frame-perfect resize needs heuristics at first.
- Slow or non-cooperative clients may force a timeout frame.

### Engine to XLibre Input

This is the hard seam.

Current XLibre still routes pointer events through the legacy flat-window path:
coordinate to window, sprite trace, grabs, focus, then delivery. That cannot
represent compositor-side transforms, scaled scenes, 3D workspaces, or other
visual effects where rendered geometry diverges from XLibre's 2D tree.

Sophia needs a routed-input path:

```text
Sophia Engine hit-tests the real scene
        |
        v
target XID + local coordinates + device event packet
        |
        v
XLibre routed-input extension
        |
        v
DIX delivery with X11 grabs, focus, XI2, and Xnamespace checks preserved
```

The extension must not become "send arbitrary event directly to client." XLibre
still owns X11 delivery semantics. Sophia only supplies the visual target and
local coordinates that XLibre cannot compute by itself.

### Xnamespace Portals

Namespaces are private by default. Cross-namespace operations go through portal
services, not ad hoc server exceptions.

Initial portal candidates:

- clipboard and selections
- drag-and-drop
- file-open/file-save handoff
- screenshots and screen recording
- URI open requests
- notifications

The portal rule is the same everywhere: data crosses as an explicit packet with
source namespace, target namespace, type, size, policy decision, and lifetime.

## XLibre Responsibilities

XLibre remains responsible for:

- X11 protocol parsing and replies
- client resource ownership
- XID allocation and lookup
- Xnamespace enforcement
- X11 selections and clipboard ownership
- X11 grabs, focus, and delivery semantics
- ICCCM/EWMH compatibility surface

Sophia should not duplicate those concepts in another object graph. It should
mirror only the data it needs for rendering and policy.

## Sophia Responsibilities

Sophia owns:

- physical input devices
- output configuration
- scene graph and transforms
- damage aggregation and frame scheduling
- final composition
- global shortcuts
- compositor-to-WM policy protocol
- portal UI hooks

Sophia Engine can cache XLibre state, but XLibre remains the source of truth for
X11 resources.

## First Research Thread

The first useful proof is not a full desktop. It is a vertical slice:

1. Start XLibre with Xnamespace enabled.
2. Launch one X11 client in one namespace.
3. Redirect that client's window through XComposite.
4. Show it in Sophia Engine's scene.
5. Move and resize it through Sophia WM policy.
6. Deliver flat, untransformed input or explicitly mark transformed input
   unsupported until routed input exists.
7. Verify namespace isolation still works.

That slice proves the rendering seam and the process split. Routed input is the
next research milestone.
