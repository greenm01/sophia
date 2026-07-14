# Sophia Documentation Map

Sophia documentation is divided by purpose. A document's role determines what
wins when prose disagrees.

## Normative Architecture

- [Architecture](architecture.md) defines process ownership and the boundaries
  between Engine, protocol authorities, runtime, WM, portals, and chrome.
- [Namespaces and Portals](namespaces-and-portals.md) defines session identity,
  admission, isolation profiles, capabilities, portal lifecycle, and
  cross-namespace failure behavior.
- [Data-Oriented Design](dod.md) defines the packet, snapshot, typed-ID, and
  private-state rules used across those boundaries.
- [Style Guide](style-guide.md) defines source-layout and implementation
  discipline.

Normative documents describe both current and target contracts. They must label
unimplemented target behavior explicitly.

## Subsystem Contracts And Current Status

- [Sophia X Server Frontend](sophia-x-authority.md) records the native X11
  frontend boundary, implemented surface, and remaining production gaps.
- [Sophia Wayland Authority](sophia-wayland-authority.md) records the
  Smithay-backed frontend boundary and its maintenance status.
- [Sophia X11 WM Bridge](sophia-x11-wm-bridge.md) records the optional legacy-WM
  policy adapter. It is not an application authority.
- [Renderer Import Boundary](renderer-import-boundary.md), [Live Backend
  Dependency Policy](live-backend-dependency-policy.md), and [Live Session
  Bootstrap](live-session-bootstrap.md) define backend/runtime seams.

Subsystem documents may describe implementation details, but they may not
override the ownership and trust rules in the normative architecture.

## Evidence And Active Work

- [X11 Compatibility Matrix](x11-compatibility-matrix.md) is the admission
  record for native X11 client behavior.
- [Validation](validation.md) lists reproducible validation commands and gates.
- [Active Roadmap](../todo.md) contains only incomplete work and measurable
  milestone exits.
- [Active Research Log](research-log.md) contains current investigations and
  retained evidence.

## Historical Material

- [Roadmap History](roadmap-history.md) archives completed milestones.
- [Research Log Archive](research-log-archive.md) preserves completed or
  superseded experiments.
- `research/xlibre/` preserves the retired XLibre prototype and its regression
  lessons outside the production workspace.

Historical documents are evidence, not current architecture. XLibre bridge
types, XComposite mirror paths, and prototype routed-input extensions must not
be cited as active Sophia interfaces.
