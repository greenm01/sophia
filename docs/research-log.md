# Active Research Log

This file records decisions and unresolved questions for the active milestone.
Completed evidence is archived in `research-log-archive.md`.

## 2026-07-11: Isolated Virtio-GPU Session Evidence

Sophia now has a direct-kernel QEMU initramfs builder and a headless session
harness. The guest has no storage or network device, uses serial control and an
unconnected Unix-domain VNC display sink, and owns emulated virtio-gpu and
virtio-keyboard devices. It starts udev, mounts devpts, launches real xterm,
opens the virtual input nodes through libinput, and runs persistent native
scanout for an exact `--max-ticks=300` budget without host DRM or VT access.

The passing run completed 300 session ticks, 42 native submissions, 41 steady
retirements, 41 accepted page-flip callbacks, two nonzero terminal exports,
injected terminal pixel change, and zero submit failures, retire failures,
rejected callbacks, saturated callback queues, in-flight frames, or cleanup
debt. The strict verifier accepted `/tmp/sophia-qemu-session.log`.

Guest bring-up exposed two real cross-driver defects. AddFB2 fallback passed a
linear modifier while clearing `DRM_MODE_FB_MODIFIERS`, which violated the DRM
crate's flag/value invariant; the implicit fallback now wraps the same planes
with `modifier=None`. Virtio-gpu also reports repeated zero page-flip sequence
values. Native CRTC routes now normalize driver values into strictly increasing
Sophia-local serials, preserving stale-event rejection across repeated values
and 32-bit sequence wrap. Focused regressions cover both fixes.

The guest virtual keyboard is present and opens through libinput, but the
current proof uses Sophia's bounded X key injection for the pixel-change check.
QMP-driven virtual-key input remains the next isolated input proof.

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
submissions, and drains cleanup. Reduced schema 6 evidence records successful
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
- Can an operator-typed run produce nonzero physical key routing and changed
  xterm pixels through the existing Engine focus path?

These questions remain probe-driven: implement the first observed missing path,
then rerun the relevant real-client smoke.

## 2026-07-11: QMP Keyboard Proof And Presentation Boundaries

The isolated session no longer uses Sophia's internal core-X key injector for
its input claim. The guest announces readiness only after xterm pixels and
Engine-owned focus are stable. The host then sends `sophia` and Return through
QMP `input-send-event`, virtio-keyboard, the kernel input path, libinput, Engine
focus validation, and X Authority. The passing run observed and routed all 14
press/release events, changed later xterm pixels, completed exactly 300 session
ticks, submitted 46 native frames, retired 45 steady page flips, and drained
without rejected callbacks, failed transitions, or cleanup debt. Tick counting
pauses for a bounded five-second physical-input window so readiness at the last
scheduled tick cannot race QMP delivery.

The guest also exposes virtio-mouse and libinput maps pointer events to a
separate Engine device ID. The completed pointer slice performs QMP word
selection in the typed xterm. Five motion/button events pass through libinput,
Engine surface-only hit-testing/focus, and core X MotionNotify/Button events;
all five route and a second terminal pixel change is observed. The first drag
attempt exposed that targeting the last mapped X window was insufficient even
though all input reached Engine. Pointer events now carry only the routed
Sophia surface, and X Authority resolves that surface through its internal
surface/window table. This preserves the authority boundary: Engine never
receives or interprets the client XID.

Native presentation now has independent per-output scanout ownership, damage,
frame clocks, in-flight state, and retirement, proved with two QEMU heads. The
physical multi-connector AMD gate remains. Fixed-refresh evidence requires each
output to follow its own page-flip timeline without overlapping submission. VRR
remains a hardware proof gate: the property contract and Engine eligibility
policy exist, default off, but activation and fixed-refresh fallback still need
capable hardware evidence.

## 2026-07-11: Bounded Per-Output Timelines And Two-Connector QEMU Topology

Engine output discovery is now bounded to 16 descriptors. Backend assembly no
longer advances one global deterministic clock: it seeds an independent clock
for each discovered output using that output's fixed refresh rate. A separate
presentation registry tracks pending damage, one in-flight frame, exact
retirement, and the last retired serial per output. Two-output regressions prove
that 60 Hz and 120 Hz timelines advance independently, one output cannot submit
over an unretired frame, and a mismatched retirement cannot clear ownership.
These are scheduling invariants; the clock is not yet driven by DRM vblank.

A single virtio-gpu device configured with two scanouts exposed two connector
objects but only one connected connector, so it was rejected as multi-monitor
evidence. The accepted harness uses two isolated virtio GPU devices with one
scanout each. The guest reports two connectors and both connected; Engine
discovers two and creates two presentation timelines. That topology was the
prerequisite for native multi-output ownership, which is recorded below.

## 2026-07-11: Dual-Output Native Presentation And Fixed-Refresh Vsync

The persistent runtime now owns a bounded table of output-scoped frame targets,
callback intake, scanout submissions, displayed buffers, cleanup debt, and
retirement state. Native selection deterministically assigns disjoint
connector/CRTC/primary-plane chains, groups page-flip routes by DRM card, and
supports explicit selections so one card cannot silently resubmit its first
connector for every output.

The isolated QEMU session owns both virtio GPU outputs. Output 1 presents the
terminal while output 2 presents a deterministic Engine proof marker in the
extended desktop region; their checksums must differ. The 300-tick gate requires
nonzero per-output exports, submissions, callbacks, and retirements, plus zero
callback rejection, cleanup debt, overlapping submission, or non-monotonic
page-flip phase. Keyboard and pointer proofs remain mandatory in the same run.

VRR property discovery recognizes connector `VRR_CAPABLE` and CRTC
`VRR_ENABLED`. The Engine decision defaults off and permits enable only for one
opaque, unoccluded fullscreen surface without overlays or required composition.
Atomic page-flip request construction fails closed if VRR is requested without
the enable property. Activation and fallback remain an AMD hardware gate;
virtio-gpu is not accepted as VRR evidence.
