# Compositor Graphics

**Role:** normative compositor-owned graphics architecture.

This document defines how Sophia describes and renders compositor-owned visual
content. [Architecture](architecture.md) defines visual authority and process
ownership. [Data-Oriented Design](dod.md) defines the records that cross those
boundaries. [Renderer Import Boundary](renderer-import-boundary.md) defines how
client buffers enter the renderer. Compositor graphics share the final
composition and presentation path with those buffers, but they are not client
surfaces and do not weaken the authority boundaries around them.

## Design Direction

Sophia uses a small, renderer-neutral display list for compositor-owned
content. The native renderer lowers that list into specialized EGL/OpenGL
primitives and cached textures. It does not expose graphics-API objects to
Engine, the WM, metadata brokers, portals, or protocol authorities.

The intended path is:

```text
 sanitized metadata     Engine/session state       portal decisions
         │                       │                         │
         └───────────────────────┼─────────────────────────┘
                                 ▼
                  compositor-owned semantic nodes
                                 │
                 bounded immutable display list
                                 │
                ┌────────────────┴────────────────┐
                ▼                                 ▼
      native EGL/OpenGL lowering          CPU/reference lowering
      shaders + cached textures           deterministic validation
                │                                 │
                └────────────────┬────────────────┘
                                 ▼
               ordinary Sophia frame composition
                                 │
                         GBM buffer → DRM/KMS
```

The display list describes visual intent rather than drawing commands for one
graphics API. Its initial vocabulary should remain deliberately small:

- solid rectangles;
- rounded rectangles;
- rounded or gradient borders;
- analytic rounded-rectangle shadows;
- image and cached text textures;
- clips, opacity, placement, and stacking;
- optional offscreen groups when an effect or grouped animation requires them.

This vocabulary covers focus rings, title bars, tab strips, trust badges,
notifications, portal prompts, workspace indicators, selection overlays, and
other shell-owned UI without turning Engine into a general-purpose UI toolkit.
New primitives require demonstrated compositor use. One-off visual novelty is
not sufficient reason to expand the stable Engine boundary.

## Ownership

Sophia Engine owns:

- the semantic compositor nodes included in a frame;
- their stable opaque identities, generations, ordering, geometry, and damage;
- validation of chrome actions against the matching committed surface;
- frame scheduling and the atomic relationship between client content, chrome,
  and the frame submitted for presentation.

The metadata broker and shell own:

- sanitizing labels, icon tokens, trust state, and attention state;
- proposing bounded compositor content from those sanitized facts;
- shell interaction policy that does not belong to the external WM.

The native renderer owns:

- GL programs, uniform layouts, textures, atlases, and offscreen targets;
- lowering semantic nodes to solid draws, shader elements, or texture draws;
- text/image raster caches and their renderer-private lifetime;
- composition with imported CPU and DMA-BUF client layers;
- reduced failure and capability reports.

The backend owns:

- render-device and output authority;
- GBM allocation and scanout ownership;
- DRM/KMS submission, page-flip observation, and final resource retirement.

The external WM does not receive compositor display lists, sanitized labels,
icons, text, pixels, renderer handles, or shader parameters. It continues to
propose layout using opaque surface and workspace facts. A compositor close
button remains an Engine/session action routed to the owning protocol authority,
not a WM command.

## Native Rendering Strategy

The native implementation should extend Sophia's existing EGL/OpenGL
composition path. Recurring geometry is rendered directly with small,
purpose-built shader programs:

- solid color needs no intermediate texture;
- rounded fills and borders use analytic distance calculations;
- shadows use an analytic rounded-rectangle shadow where that visual is
  sufficient;
- gradients use explicit, bounded shader parameters;
- clips use geometry or scissor state appropriate to the primitive;
- opacity uses Sophia's premultiplied-alpha composition convention.

Text-heavy or layout-heavy panels should be rasterized outside the frame hot
path and uploaded as cached premultiplied-alpha textures. Text shaping and
rasterization are separate from GPU composition. Cache keys must include every
fact that changes pixels, including content generation, scale, font/style
selection, color, wrapping constraints, and relevant locale or direction.
Stable content should reuse its texture until one of those facts changes.

Offscreen rendering is an explicit tool rather than the default. It is
appropriate when a group must fade as one unit, when an effect consumes already
composed pixels, or when reuse avoids repeated work. Each offscreen allocation
must have bounded dimensions and renderer-owned lifetime.

The renderer may choose different implementations for the same semantic node
when capabilities differ. Degradation must remain deterministic and reduced:
for example, a shadow may be omitted or simplified according to explicit
policy, but native failure must not leak GL errors or renderer objects across
the boundary.

## Damage And Atomic Presentation

Compositor-owned nodes participate in the same frame and damage model as client
layers. Creating, removing, moving, restyling, or changing the opacity of a node
damages both its previous and current extents. A stable node with an unchanged
generation must not force full-output damage.

Chrome does not get a presentation shortcut. A visual response to focus,
attention, trust, portal, or metadata state becomes visible only through an
Engine-planned frame and the ordinary rendered scanout lifecycle. When chrome
is attached to a client surface, its geometry must be derived from the same
committed surface state used for that frame. Pending client geometry must not
move committed chrome ahead of matching client pixels.

Animations are Engine-clocked state. The Engine or session reducer determines
the semantic state for each frame; the renderer only draws that immutable
state. The WM, metadata broker, and renderer do not drive independent animation
timelines.

## Text And Metadata Safety

Only sanitized, bounded text may reach compositor text layout. Protocol-local
titles, classes, paths, namespace identity, and arbitrary markup do not pass
directly into the renderer. Markup, if Sophia admits it for a specific shell
surface, must be generated from trusted compositor templates with untrusted
content escaped.

Text caches and diagnostics must not become metadata side channels. Cache
identity remains renderer-private. Reduced reports may expose counts, sizes,
generations, cache outcomes, or timings, but not rendered strings, glyph
content, client titles, paths, or texture bytes.

## Current And Target State

### Implemented

- Engine frame plans carry ordered client layers with targets, clips,
  transforms, opacity, and damage.
- The native GBM/EGL path composes CPU and DMA-BUF textures using rectangular
  placement, scissor clipping, scaling, and premultiplied-alpha blending.
- The production path exports the rendered GBM front buffer and retains its
  resources through KMS page-flip retirement.
- Engine has generation-checked `ChromeDescriptor` and `ChromeActionRequest`
  records for sanitized compositor metadata and actions.

### Target

- A bounded renderer-neutral compositor display list is part of immutable
  Engine frame planning.
- The native renderer provides the initial solid, rounded, border, shadow,
  image, and cached-text implementations.
- The CPU/reference path implements enough of the same semantics for
  deterministic tests and degraded operation.
- Per-node generations and old/new extents feed damage without redrawing stable
  compositor content.
- Capability degradation and cache behavior are observable only through reduced
  reports.

Until those target items are implemented, existing chrome descriptors are
metadata and action records, not a claim that production compositor chrome is
already rendered.

## Architectural Reference: Niri

Niri is a useful architectural reference for this component. Its compositor UI
is assembled from damage-aware render elements rather than routed through one
general drawing engine. It combines:

- client and offscreen textures;
- solid-color elements;
- specialized GLES shaders for rounded borders, gradients, clipping, shadows,
  blur, and animation effects;
- Pango/Cairo rasterization for text-heavy panels, followed by cached texture
  upload;
- a common ordered render-element stream for final composition.

Sophia adopts the separation demonstrated by that design: semantic compositor
UI, specialized native primitives, cached raster content, and one final
damage-aware composition path. Sophia does not adopt Niri's Smithay types,
Wayland authority model, renderer ownership, or source code. Niri is
GPL-3.0-or-later and tightly integrated with Smithay; it is a design reference,
not a dependency or implementation source.

Sophia's version must remain subordinate to its own boundaries:

- Engine remains the sole visual and input authority;
- compositor nodes use Sophia typed IDs and immutable frame values;
- the renderer remains private behind reduced reports;
- the WM remains metadata-blind;
- client-buffer readiness and compositor chrome share Sophia's atomic
  presentation lifecycle;
- EGL/OpenGL, GBM, DMA-BUF, and KMS objects remain in their existing native
  ownership domains.

The value of the reference is the shape of the solution: use the smallest
native primitive that expresses recurring compositor content, cache expensive
raster work, and compose everything through one frame model.
