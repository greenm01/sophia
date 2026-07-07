# Research Log

This file records early decisions, assumptions, and open questions. Keep it
short and chronological.

## 2026-07-07: Project Name

The project is named **Sophia**.

Sophia means the XLibre-centered modern X11 architecture, not a Wayland
compositor with an X11 compatibility sidecar.

## 2026-07-07: Identity

Sophia is a research prototype for a modern X11 session:

- XLibre remains the client-facing X11 server.
- XLibre remains the Xnamespace authority.
- Sophia owns compositor-first input and rendering.
- Sophia uses an external WM policy process.
- Namespace crossings go through explicit portals.

## 2026-07-07: Language Direction

Use Rust for Sophia user-space components.

Use C for narrow XLibre patches and extensions.

Nim remains attractive for experimental external window managers because policy
processes are intentionally outside the compositor hot path.

## 2026-07-07: Architecture Distinction

Rejected framing:

```text
Wayland compositor with an XLibre bridge.
```

Accepted framing:

```text
XLibre-centered modern X11 session with an external display engine.
```

This distinction matters. In Sophia, XLibre is not a guest. It is the X11
authority. Sophia modernizes the input/rendering/policy structure around it.

## 2026-07-07: Current Local XLibre Facts

Local source tree: `/home/niltempus/src/xserver`.

Observed facts:

- Xnamespace exists and is enabled by the `namespace` build option.
- Selection names are rewritten per namespace.
- A draft/runtime `X-NAMESPACE` management protocol skeleton exists.
- XComposite and Damage exist as likely render handoff points.
- Input delivery still flows through legacy `XYToWindow`, sprite trace, grabs,
  focus, and DIX delivery.

Implication: rendering has a plausible existing seam. Compositor-routed input
needs explicit XLibre protocol/server work.

## 2026-07-07: Compositor References

Niri is a reference, not a base. Use it for Rust/Smithay compositor mechanics:
backend structure, frame clock, KMS/libinput patterns, renderer integration,
transaction timeouts, and headless or visual test ideas.

Do not fork niri for Sophia. Niri's central state combines compositor and WM
policy, while Sophia needs an external WM policy process.

Picom is the X-side reference. Use it for XComposite/Damage handling, X window
tree mirroring, top-level/client detection, layer snapshots, render command
planning, and damage calculation across buffered layouts.

Do not copy picom's process architecture. Picom renders back into X. Sophia
should hand X-derived layer snapshots to a compositor that owns scanout.

## 2026-07-07: Roadmap Direction

The next implementation step after docs is a Rust workspace skeleton with passive
types and protocol packets. Do not start with XLibre patches.

The first rendering proof should use mock layer snapshots before connecting to
XLibre. The first XLibre proof should mirror windows and emit snapshots before
routed input work begins.

## 2026-07-07: Engine Backend Boundary

Use niri and Smithay as references for compositor backend boundaries, not as a
base or dependency.

Sophia Engine now treats the headless compositor as the first backend behind a
small `EngineBackend` trait. This keeps backend mechanics separate from passive
protocol packets and leaves room for real output, XComposite import, and test
backends without changing the WM or X Bridge packet shapes.

## 2026-07-07: Blind WM And Compositor Chrome

Sophia WM manages opaque layout nodes, not X11 windows. The WM protocol must not
carry XIDs, namespace IDs, raw titles, app classes, PIDs, or icon pixels.

Sophia Engine is the broker for compositor chrome. It may receive metadata from
Sophia X Bridge, but user-facing titles, icons, trust badges, and attention
state are rendered by the compositor or compositor shell from sanitized chrome
descriptors. This keeps complex layout policy useful without granting it X11
god-mode or namespace visibility.

## 2026-07-07: TEA Boundary Discipline

Sophia will use The Elm Architecture where it matches the authority boundary:
policy components consume snapshots/events, update private policy state, and
emit command packets. This is the default style for Sophia WM, portals, and
session policy.

Sophia Engine is not a global TEA application. It is the compositor authority
and must stay performance and security centric: owned tables, typed IDs,
generation checks, spatial indexes, damage queues, renderer systems, and
auditable hot paths.

## 2026-07-07: X Bridge Probe Start

Sophia X Bridge uses `x11rb` for the first read-only X11 probe. The initial
probe connects to the configured display, queries Composite, Damage, XFixes,
Shape, and Render with `QueryExtension`, and carries static Xnamespace records
until XLibre exposes namespace discovery data through a reliable protocol.

This does not redirect windows or mutate X server state yet. Window-tree import,
event selection, Composite redirection, and Damage tracking remain separate
Phase 3 steps.

The first root-tree importer remains read-only. It walks the tree breadth-first,
wraps raw XIDs as `XWindowId` values with an initial mirror generation, and
records map state from `GetWindowAttributes`. Event tracking, ICCCM/EWMH
top-level detection, and Composite/Damage redirection are still pending.

Mirror event tracking now normalizes X11 notify events into Sophia-owned
`XMirrorEvent` values. Applying those events updates map state, parent/child
links, destroy cleanup, stack rank, and metadata staleness without exposing raw
X11 event objects to the rest of Sophia. Live event selection and dispatch are
still separate bridge-loop work.

Client detection now combines EWMH `_NET_CLIENT_LIST` with ICCCM `WM_STATE`.
The bridge annotates mirrored windows with the detected client window and the
nearest root child as the toplevel frame. It does not classify window type yet;
that remains later EWMH metadata work.

The bridge now emits cloned `XWindowMirror` values, protocol `SurfaceSnapshot`
values, and preliminary `LayerSnapshot` values. X geometry is read during tree
import and updated from configure events. Layer snapshots intentionally use
`BufferSource::None` until XComposite pixmap redirection/import is implemented.

Composite redirection support now selects unique mapped client windows from the
mirror and redirects them with XComposite manual updates. The bridge negotiates
the Composite version before redirecting. Naming redirected pixmaps and wiring
those pixmap IDs into `SurfaceSnapshot` remains the next Composite step.

The bridge can now name redirected client windows with XComposite
`NameWindowPixmap` and store the resulting pixmap IDs in a compositor-owned
`CompositePixmapMap`. Mirrored surfaces use the visible/toplevel mirror window
for stable Sophia surface identity, but use the detected client XID to resolve
their `BufferSource::XPixmap`. Importing or reading those pixmaps into a real
renderer texture remains Phase 4 work.

Damage tracking now has a bridge-owned `DamageTracker` that creates X Damage
objects for redirected client windows, maps Damage notify events back to client
XIDs, and accumulates pending Sophia `Region` values per window. This is still
surface-local damage; converting it into output/frame `DamageFrame` packets is
the next Phase 3 step.

X damage can now be drained into Sophia `DamageFrame` values using the current
surface snapshots. The first conversion translates client-local damage
rectangles by the snapshot geometry and records affected Sophia surfaces. This
is enough for the headless engine path; precise frame/client offset handling
should be measured once real decorated X11 clients are imported.

Phase 4 starts with a conservative CPU readback fallback. Sophia X Bridge can
query a named XComposite pixmap's geometry, read it with X11 `GetImage` in
`ZPixmap` format, store the bytes in a bridge-owned `CpuBufferStore`, and
rewrite `SurfaceSnapshot` sources from `XPixmap` to `CpuBuffer`. This is not the
final renderer path, but it gives the first real-client proof a simple handoff
before GPU texture import exists.

Sophia now includes a minimal `sophia x-test-client` command that connects to an
X display, creates a mapped input/output window, draws a simple filled rectangle,
and holds the connection open for a bounded duration. This avoids depending on
external clients like `xterm` or `xclock` during Phase 4 smoke tests.

The first live readback smoke passed against system `Xvfb` on `:119`. With
`sophia x-test-client --seconds=30` holding one mapped window,
`sophia x-smoke-readback` mirrored two windows, produced one surface, redirected
one Composite target, read back one named pixmap, and captured 256000 bytes into
the CPU buffer path. This validates the generic X11 path; XLibre-specific
namespace startup remains a separate unchecked Phase 4 item.

## Open Questions

- Should Sophia's compositor/display engine be a fully separate process or a new
  XLibre DDX backend during the first prototype?
- What is the smallest routed-input extension that preserves X11 grabs, focus,
  XI2, and Xnamespace checks?
- How much frame-perfect resize can be achieved with XComposite/Damage alone?
- Where should the X11 WM facade live: Sophia WM, Sophia X Bridge, or a separate
  helper?
- Which IPC format should Sophia use for its internal protocols?
