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

### TEA Boundary Rule

Sophia uses The Elm Architecture as a policy-boundary discipline where it fits:
snapshots and events enter a policy process, that process updates its private
model, and it emits explicit command packets back to the authority that can
execute them.

This applies strongly to Sophia WM and portals. The WM consumes opaque layout
node snapshots and focus/workspace events, updates its layout model, and emits
`LayoutTransaction` commands. Portals consume transfer requests and policy
events, update transfer state, and emit allow, deny, revoke, or handoff
commands.

This does not apply as a universal compositor architecture. Sophia Engine is
performance and security centric: it owns libinput, scene graph state,
hit-testing, damage tracking, frame scheduling, and final scanout. Its hot paths
should be data-oriented systems over owned tables and precomputed snapshots, not
a single app-wide message loop.

### Compositor Strategy

Sophia Engine should follow Smithay-style compositor structure, using niri as a
read-only reference for how a production Rust compositor organizes backends,
renderers, outputs, frame clocks, input devices, and headless tests.

Sophia should not fork niri. Niri combines compositor and window-management
policy in one central state object, while Sophia deliberately splits policy into
Sophia WM. The reusable idea is the compositor machinery, not the process model.

Sophia X Bridge should follow picom conceptually for the X side. Picom imports
the X window tree, tracks top-levels and stacking, redirects windows with
XComposite, consumes Damage, builds flat layer snapshots, and computes damage
across buffered layouts. Sophia needs those same data products, but it must hand
them to Sophia Engine instead of rendering back into the X root.

Do not turn Sophia into a traditional X compositor. XLibre remains the X11
authority, but Sophia owns final scanout and physical input.

### Engine to WM

The WM protocol is a policy boundary. The WM receives state changes that need
policy decisions: new windows, destroyed windows, output changes, keybindings,
workspace changes, and focus-affecting events.

The WM is blind to X11 identity and namespace identity. It manages opaque layout
nodes keyed by Sophia `SurfaceId`, not XIDs. The protocol must not expose raw
window titles, classes, icons, PIDs, namespace IDs, or X11 resource IDs.

User-facing chrome such as titles, icons, attention indicators, and trust badges
belongs to Sophia Engine or a compositor-shell component fed by a metadata
broker. The broker may consume X metadata from Sophia X Bridge, but it exports
sanitized compositor chrome data separately from WM layout data.

The WM is not on the per-frame or per-input hot path. Sophia Engine keeps the
last committed policy state if the WM crashes or restarts.

The protocol should be sequence-oriented:

- **Manage sequence** for state that affects clients: size, focus, fullscreen,
  workspace assignment, activation.
- **Render sequence** for compositor-only state: position, z-order, crop,
  decoration geometry, opacity, transforms.
- **Chrome sequence** for compositor-owned presentation metadata: redacted
  display labels, icon tokens, trust badges, and attention state. This sequence
  is not consumed by the WM.

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

## Reference Boundaries

Use each reference at the boundary where it is strongest:

- niri: Rust/Smithay backend patterns, frame scheduling, renderer integration,
  headless test scaffolding.
- picom: XComposite/Damage flow, X window mirror, layer snapshots, render
  command planning, damage over buffer age.
- river: external WM protocol shape, manage/render sequence thinking, crash
  isolation for policy.
- XLibre: namespace enforcement, X11 delivery semantics, future routed-input
  protocol.
