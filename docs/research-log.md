# Active Research Log

This file records decisions and unresolved questions for the active milestone.
Completed evidence is archived in `research-log-archive.md`.

## 2026-07-10: Roadmap And Documentation Review

The xterm compatibility stream currently reaches `ImageText8`, emits four
ready `SurfaceTransaction` values, commits them through runtime, and passes the
deterministic composition/scanout lifecycle proof. Core drawing now updates
bounded XRGB8888 software buffers, renderer-live composes those bytes, and the
native EGL adapter can upload the composed frame into a GBM front buffer.
The TTY3 content proof now exports an exact composed xterm checksum through the
native GL/GBM path, submits that buffer to KMS, observes accepted page-flip
retirement, and drains cleanup. Requested and exported checksum evidence match;
the remaining presentation work is persistent-session ownership, not pixel
upload correctness.

The active milestone is therefore persistent session ownership, hardware
terminal-content presentation, and physical keyboard delivery. Injected core
key events already produce changed pixels in a real xterm. Pixel bytes remain
outside Sophia Engine and the blind WM protocol.

The persistent launcher now owns an explicit local display, one xterm, the X
Authority server, one live backend runtime, and the latest composed CPU scene.
A bounded real-xterm run passes repeated authority/runtime ticks and injected
pixel change. Building it exposed and fixed static drawing generations: X
Authority now advances a window generation after each emitted visual
transaction, so long-running Engine commits remain contiguous. Native scanout
is still outside this owner.

Physical keyboard plumbing now enters the persistent owner through explicit
libinput event nodes. `InputFocusState` in Sophia Engine validates a seat's
focused surface against committed visual state before X Authority maps evdev
keycodes and modifiers to core events. The TTY keyboard node opens in a bounded
run, but the noninteractive validation process observed zero physical events;
an operator typing run is still the evidence gate.

Wayland is gated behind the operator-grade X session. Before Wayland, Sophia
will run xmonad through the documented X11 WM bridge. The first bridge is an
embedded minimal X server with synthetic windows; xmonad is layout policy only
and receives no physical input, raw metadata, namespaces, or real client XIDs.

The xmonad source is cloned at `~/src/xmonad` commit `a9a8b5c` as a compatibility
reference. It is not vendored and is not a Sophia runtime dependency. The
bridge translation core exists; the embedded server and real process smoke
remain open because no Haskell/xmonad executable is installed.

## Active Questions

- Which xmonad startup request is the first unsupported request after setup,
  root event-mask selection, atom/property access, and synthetic lifecycle?
- Which physical-input ownership path can supply libinput keys to the focused X
  channel without duplicating Engine focus policy?

Both questions remain probe-driven: implement the first observed missing path,
then rerun the relevant real-client smoke.
