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
now joins this owner behind `--native-scanout`: the same loop queues composed
CPU frames for GL/GBM export, polls native page flips, retires tracked KMS
submissions, and drains cleanup. Reduced schema 4 evidence records successful
submits, deferrals and failures, submit-to-page-flip latency, maximum in-flight
age, callback pressure, nonzero exports, authority drops, and cleanup debt.
The non-native repeated-xterm regression and strict verifier fixtures pass.

The strict persistent hardware proof now passes. Corrected counters first
exposed River ownership and then two real lifetime defects: the runtime retired
the newly displayed framebuffer instead of the previously displayed one, and
the shutdown loop retired a frame while immediately submitting another. The
persistent mode now performs a blocking initial modeset without waiting for an
event, retains the displayed owner until a later accepted page flip replaces
it, and has a retire-only idle/shutdown path. A 30-second TTY3 run completed 46
submissions with 45 steady retirements, six nonzero exports, zero dropped
authority batches, zero rejected callbacks, zero transition failures, and no
in-flight or cleanup debt. A subsequent bounded run also reports nonzero
submit-to-page-flip latency after fixing timestamp association.

Host iteration remains unnecessarily disruptive because River must release the
only DRM card. The next stability harness should boot Sophia in a headless QEMU
guest with `virtio-gpu`, serial control, virtual keyboard input, and no guest
compositor. Use that guest for the 300-tick and repeated-session proofs; retain
the AMD TTY run for final driver and modifier evidence.

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
- Does the isolated QEMU `virtio-gpu` session remain clean for 300 deterministic
  ticks and repeated process restarts?
- Can an operator-typed run produce nonzero physical key routing and changed
  xterm pixels through the existing Engine focus path?

Both questions remain probe-driven: implement the first observed missing path,
then rerun the relevant real-client smoke.
