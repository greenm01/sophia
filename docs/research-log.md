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

## Open Questions

- Should Sophia's compositor/display engine be a fully separate process or a new
  XLibre DDX backend during the first prototype?
- What is the smallest routed-input extension that preserves X11 grabs, focus,
  XI2, and Xnamespace checks?
- How much frame-perfect resize can be achieved with XComposite/Damage alone?
- Where should the X11 WM facade live: Sophia WM, Sophia X Bridge, or a separate
  helper?
- Which IPC format should Sophia use for its internal protocols?
