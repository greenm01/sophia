# XLibre Routed Input Extension

This is the patch target for Phase 6. It turns Sophia's compositor-side
hit-test result into an XLibre-controlled delivery path without letting Sophia
send arbitrary events directly to clients.

## Extension Shape

Name:

```text
SOPHIA-ROUTED-INPUT
```

Version:

```text
0.1
```

Requests:

```text
0 QueryVersion
1 RouteEvent
```

`RouteEvent` is fixed-size. The X11 request length is `11` 32-bit words,
including the extension request header.

```text
CARD8   reqType          dynamic extension major opcode
CARD8   routedReqType    1
CARD16  length           11
CARD32  serial_hi
CARD32  serial_lo
WINDOW  target_xid
CARD32  seat
CARD32  device
CARD32  time_msec
INT32   local_x_24_8
INT32   local_y_24_8
CARD16  event_code       1 motion, 2 button, 3 key
CARD16  detail           button or keycode, 0 for motion
CARD32  flags            bit 0 means pressed
```

Sophia owns the dynamic extension opcode lookup. The packet body is represented
in `sophia-protocol` as `XLibreRoutedInputWireRequest`.

The current patch artifact is:

```text
patches/xlibre/0001-add-sophia-routed-input-extension.patch
```

It registers the extension, restricts namespace visibility, validates
`RouteEvent`, resolves target windows through DIX access checks, and delivers
flat pointer motion/button events through normal XLibre event delivery. Use
`tools/check_xlibre_routed_input_patch.sh` to apply the patch to a temporary
XLibre tree and compile `hw/vfb/Xvfb`.

`tools/xlibre_namespace_smoke.sh` runs the runtime smoke against the private
`sophia-xserver` fork. The smoke creates a privileged target window, discovers
the XInput master pointer, sends a raw `RouteEvent` request, requires an
`Accepted` reply, and waits for the target client connection to receive a core
`ButtonPress` at the requested local coordinates.

## Required XLibre Touch Points

- Add a new `Xext/sophia-routed-input/` extension with a small dispatch file.
- Register it with `AddExtension`, following existing extension dispatch
  patterns such as `Xext/xfixes/xfixes.c` or `Xext/geext/geext.c`.
- Add the new extension subdirectory to `Xext/meson.build`.
- Add namespace extension visibility policy in
  `Xext/namespace/hook-ext-access.c`: only the privileged compositor/root
  namespace should be able to query or call this extension.
- Add namespace extension dispatch policy in
  `Xext/namespace/hook-ext-dispatch.c`: hard-coded extension opcodes from
  non-privileged namespaces must still be rejected.
- Resolve `target_xid` with normal DIX resource lookup so
  `Xext/namespace/hook-resource.c` can reject cross-namespace access.
- Reuse existing input delivery machinery from `dix/events.c` and
  `Xext/xinput/exevents.c`; do not bypass grabs, focus, XI2 masks, or delivery
  filters.

## RouteEvent Server Algorithm

1. Validate request length and version.
2. Require the calling client to be privileged in Xnamespace terms.
3. Resolve `target_xid` as a window with DIX access checks enabled.
4. Reject stale or unmapped targets.
5. Resolve the requested device; the first prototype supports master and
   floating pointer motion/button events only.
6. Convert `local_x_24_8` and `local_y_24_8` into event coordinates relative to
   `target_xid`.
7. Enter normal XLibre event delivery with the target window supplied by the
   compositor instead of `XYToWindow`.
8. Preserve normal active grab, passive grab, focus, XI2, and namespace
   behavior.
9. Return a decision code equivalent to Sophia's
   `XLibreRoutedInputOutcome`.

The key design constraint is that this extension replaces target selection
only. It must not behave like XTEST, `SendEvent`, or "write this event directly
to this client."

## Implemented Prototype Behavior

The first routed-input patch is deliberately flat:

- It accepts motion and button routes for master or floating pointer devices.
- It rejects key, touch, tablet, transformed, and slave-device routes.
- It converts target-local 24.8 coordinates into desktop coordinates before
  using XLibre's existing pointer event builder.
- It suppresses raw XI2 events with `POINTER_NORAW`; clients should see the
  delivered window-relative event, not compositor-internal raw motion.
- It installs a routed sprite trace for the supplied target window, then enters
  the normal XI/DIX event path.
- It rejects a device that is already sync-frozen, but ordinary active grabs
  still follow XLibre grab semantics and may redirect delivery to the grab
  owner.

The current runtime proof is intentionally narrow: button 1 at local `(42, 37)`
inside a root-namespace test window on Xvfb. It proves the extension crosses
the wire and reaches client-visible X11 delivery. It does not yet prove
cross-namespace portal policy, transformed routes, or grab edge cases.

## First Prototype Boundary

The first XLibre patch only accepts flat, untransformed pointer routes. Sophia
already rejects transformed routes in `build_flat_routed_input_request`.

Unsupported in the first patch:

- transformed compositor coordinates
- touch events
- tablet valuators
- synthetic key focus changes beyond existing X11 focus semantics
- direct client event injection

## Expected Rejections

The extension should return or expose distinct failure reasons for:

- stale target XID
- denied namespace access
- sync-frozen device state
- focus policy conflict
- unsupported event type

Sophia treats every rejection as final. It must not retry by using XTEST or
direct client delivery.
