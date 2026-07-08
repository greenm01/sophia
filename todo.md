# Sophia Build Phases

Each phase should leave behind either a working artifact, a testable prototype,
or a sharper research answer. Sophia is a research prototype, so failed
approaches are acceptable when they produce evidence and update the docs.

---

## Phase 0 - Documentation And Repository Shape

Capture the architecture, reference map, and first roadmap before code starts.

**Project shape**
- [x] Seed `README.md` with the original architecture diagram and data path.
- [x] Add `docs/architecture.md`, `docs/dod.md`, `docs/style-guide.md`, and
  `docs/research-log.md`.
- [x] Record that Sophia is XLibre-centered, not Xwayland-centered.
- [x] Record Rust as the user-space implementation language and C as the XLibre
  patch language.
- [x] Add the reference map: niri, picom, river, XLibre.
- [x] Add this roadmap.

**Next documentation checks**
- [x] Add an agent guide once code exists and build/test commands are known.
- [x] Keep docs updated when a research question turns into a decision.

---

## Phase 1 - Rust Skeleton

Create the minimum Rust workspace needed to make data shapes executable without
touching compositor or XLibre code yet.

**Workspace**
- [x] Create a Cargo workspace.
- [x] Add crates or modules for Sophia Engine, Sophia X Bridge, Sophia protocol,
  and a demo Sophia WM.
- [x] Add common tracing and error handling.
- [x] Add a small CLI that can print version and planned component names.

**Data model**
- [x] Add typed IDs: `SurfaceId`, `XWindowId`, `NamespaceId`, `OutputId`,
  `SeatId`, `DeviceId`, `TransactionId`, and `PortalTransferId`.
- [x] Add passive packet structs for `LayerSnapshot`, `DamageFrame`,
  `RenderCommand`, `CompositorSurface`, `InputEventPacket`, `InputRoute`,
  `LayoutTransaction`, and `PortalTransfer`.
- [x] Add dense-table helpers with generation checks where stale references are
  plausible.
- [x] Add unit tests for ID allocation, stale-ID rejection, and snapshot
  immutability.

---

## Phase 2 - Headless Engine Prototype

Prove Sophia Engine can consume frame data before any XLibre integration.

**Headless compositor**
- [x] Use Smithay/niri-inspired backend structure as the reference.
- [x] Add a headless output with deterministic size and scale.
- [x] Accept mock `LayerSnapshot` data and build a frame plan.
- [x] Render or simulate render commands without a real X client.
- [x] Capture `FrameSnapshot` data for tests.

**Verification**
- [x] Test stable layer ordering.
- [x] Test damage aggregation for moved, resized, added, and removed layers.
- [x] Test frame snapshot replay with mock surfaces.

---

## Phase 3 - XLibre Mirror Probe

Connect to XLibre as an X client and mirror enough state to produce Sophia
snapshots.

**X connection**
- [x] Connect with XCB or Rust X11 bindings.
- [x] Confirm required extensions: Composite, Damage, XFixes, Shape, Render.
- [x] Start with static Xnamespace config.
- [ ] Record namespace information when discoverable.

**Window mirror**
- [x] Import the root window tree with async-safe ordering.
- [x] Track map, unmap, destroy, configure, reparent, property, and restack
  events.
- [x] Detect top-level and client windows using ICCCM/EWMH hints.
- [x] Wrap XIDs in `XWindowId` and track generation.
- [x] Emit `XWindowMirror`, `SurfaceSnapshot`, and `LayerSnapshot` values.

**Composite and damage**
- [x] Redirect relevant windows with XComposite.
- [x] Name or otherwise access redirected pixmaps.
- [x] Track Damage events per surface.
- [x] Convert X damage into Sophia `DamageFrame` inputs.

---

## Phase 4 - First X11 Surface On Screen

Put one real X11 client surface into Sophia Engine.

**Rendering path**
- [x] Run an XLibre instance suitable for offscreen or test rendering.
- [x] Launch one simple X11 client in one namespace.
- [x] Run a system Xvfb smoke display for the generic X11 path.
- [x] Add a Sophia-owned simple X11 test client command.
- [x] Add a CPU readback fallback for named XComposite pixmaps.
- [x] Import or read back one XComposite pixmap.
- [x] Convert the pixmap into a compositor texture or temporary CPU buffer.
- [x] Display it in the headless or simple real-output engine.

**Policy**
- [x] Move and resize the surface through Sophia-side policy.
- [x] Keep XLibre as the source of truth for X11 resource identity.
- [x] Verify Xnamespace isolation still blocks cross-namespace visibility.

---

## Phase 5 - External WM Protocol

Split policy from the compositor process.

**Protocol**
- [x] Add blind-WM layout node and compositor-owned chrome packet shapes.
- [x] Define the first manage sequence: new surface, configure size, focus,
  workspace assignment.
- [x] Define the first render sequence: position, z-order, crop, transform.
- [x] Add transaction IDs and outcomes.
- [x] Keep the WM off the per-frame and per-input hot path.

**Demo WM**
- [x] Implement a tiny external WM process.
- [x] Tile or stack mock and X-derived surfaces.
- [x] Restart the WM without killing Sophia Engine.
- [x] Preserve the last committed state while the WM is absent.

---

## Phase 6 - Routed Input Research

Design and prototype compositor-first input for X11 clients.

**Specification**
- [x] Define the smallest XLibre routed-input extension request.
- [x] Include target XID, local coordinates, device identity, event kind, and
  serial.
- [x] Preserve X11 grabs, focus, XI2 semantics, and Xnamespace checks inside
  XLibre.
- [x] Reject any design that sends arbitrary events directly to clients.

**Prototype**
- [x] Build flat, untransformed routed-input request adapter.
- [x] Add wire request body and XLibre patch target notes.
- [x] Add a git-applyable XLibre routed-input patch and build check.
- [x] Land the extension shell in the private `sophia-xserver` fork.
- [x] Deliver flat, untransformed pointer events through an XLibre extension.
- [x] Add an end-to-end Xvfb smoke that observes a routed button event.
- [ ] Add transformed hit-test routes once the flat path is proven.
- [x] Add tests for stale target windows, denied namespaces, grabs, and focus.

---

## Phase 7 - Portals

Add intentional namespace crossing without weakening Xnamespace.

**Clipboard first**
- [ ] Monitor namespaced selections.
- [ ] Keep clipboard private by default.
- [ ] Add explicit export/import policy.
- [ ] Support text targets first.
- [ ] Invalidate transfers when the source owner changes.

**Later portals**
- [ ] Drag-and-drop.
- [ ] File open and save handoff.
- [ ] Screenshots and screen recording.
- [ ] URI open requests.
- [ ] Notifications.
