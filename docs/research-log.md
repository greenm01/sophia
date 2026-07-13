# Active Research Log

This file records decisions and unresolved questions for the active milestone.
Completed evidence is archived in `research-log-archive.md`.

## 2026-07-12: Controlled DMA-BUF First-Frame Heap Corruption

The first real controlled DMA-BUF run reached Sophia's full-size 1920x1200
client frame and recorded `sophia_wayland_frame` with `buffer=dmabuf`. The
native process then aborted with `corrupted size vs. prev_size`, disconnecting
the producer before presentation retirement or buffer release could be proven.
This is a renderer/resource-ownership safety failure, not DMA-BUF evidence.

The producer itself now follows the compositor's initial xdg configure rather
than assuming 640x480, and uses a driver-supported explicit linear GBM
allocation. Those corrections moved the test past allocation and target-size
rejection; they did not make the native import/presentation path safe. The
300-frame lifecycle and three-Kitty promotion gates remain blocked pending
allocator/lifetime diagnosis.

The next controlled rerun uses a GDB-backed diagnostic mode with explicit
DMA-BUF stages. The importer now detaches the EGLImage from its GL texture and
finishes that detach before destroying the EGLImage and dropping the imported
client FD. This makes input-image teardown independent from the retained GBM
front-buffer owner.

The GDB-backed three-frame rerun passed: each 1920x1200 frame completed EGL
image creation, rendering, texture detach, image destruction, KMS submission,
page-flip observation, scanout retirement, and client buffer release. The
session exited normally with three imports, three retirements, three callbacks,
no cleanup debt, and a 14 ms maximum submit-to-page-flip interval. The
GDB-backed 300-frame lifetime proof then completed with 300 imports,
submissions, page flips, and retirements, no allocator diagnostic or cleanup
debt, and the same 14 ms maximum submit-to-page-flip interval. A subsequent
normal release 300-frame run nevertheless aborted with `corrupted size vs.
prev_size` after frame 8 (an earlier normal run reached frame 13). This makes
the fault timing-sensitive: the GDB result is diagnostic evidence, not a
completed lifecycle gate. A release-timing trace then completed all 300 frames
with ordered ownership stages and an 18 ms maximum submit-to-page-flip interval.
One uninstrumented rerun and then three separately retained uninstrumented
300-frame runs all completed normally: each reported 300 imports, 300 callbacks
and retirements, no cleanup debt, no surviving process, and a 14 ms maximum
submit-to-page-flip interval. The next full promotion preflight nevertheless
aborted on its first uninstrumented DMA-BUF frame with `free(): invalid pointer`,
before Kitty started. Therefore the prior samples are diagnostic evidence, not a
completed lifecycle gate. The likely unsound seam was rebinding the persistent
CPU-upload texture to each short-lived EGLImage; imports now use a transient
per-frame texture that is deleted after `glFinish` before EGLImage destruction.
The next required evidence is a fresh controlled first-frame proof, then the
three-run lifecycle gate; no Kitty DMA-BUF run may start first.

## 2026-07-12: DMA-BUF Performance Gate and Renderer Safety Boundary

The current native Wayland/Kitty presentation route is SHM-backed and stable
enough to serve as the production fallback, but the latest hardware result was
about 110 ms input-to-presentation and therefore missed the 100 ms budget.
DMA-BUF descriptors are admitted only as a bounded single-plane linear subset;
their native import and presentation path remains explicitly experimental.
There is no passing real-hardware DMA-BUF result at this point.

A controlled external Wayland producer now allocates linear XRGB8888 GBM
buffers, alternates them only after `wl_buffer.release`, and waits for each
frame callback. The first hardware gate uses three frames; the second uses 300
frames to exercise import, presentation, feedback, and retirement lifetime.
Only after both pass may the three independent guarded real-Kitty runs begin.
DMA-BUF stays non-default until every real-Kitty log proves input, recovery,
presentation, and the 100 ms budget.

The current CPU composition copy and 2 ms native idle cadence are a safety
boundary, not merely a tuning choice. Removing the copy or tightening that loop
has reproduced native renderer/exporter heap corruption on hardware. Further
latency work must isolate that ownership fault before changing either setting.

## 2026-07-12: Native Wayland Replaces The Kitty Compatibility Runtime

Sophia's production Kitty path now terminates a private Wayland socket through
the Smithay-backed Sophia Wayland Authority. Engine input routes and layer
records are protocol-neutral; keyboard focus and pointer hit-testing remain in
Engine, while the authority translates accepted routes into `wl_keyboard` and
`wl_pointer` delivery. A real Kitty 0.47.4 process completes the headless smoke
with `DISPLAY` removed, changing nonzero SHM frames, and no X server process.

The installed launcher now uses the native Wayland/KMS session and retains the
independent Ctrl-Alt-Backspace recovery interlock. XLibre is excluded from the
production dependency graph and launcher; its frozen crate, CLI, patches,
scripts, fixtures, and notes live under `research/xlibre`.

The native-scanout session advertises a bounded single-plane linear/implicit
XRGB8888/ARGB8888 DMA-BUF subset. Accepted buffers cross the renderer boundary
as owned descriptors. Their experimental native import/presentation route is
now gated by the controlled first-frame/lifetime proof and three real-Kitty
runs; it is not yet recorded as passing hardware evidence. Wayland
presentation and buffer-release feedback must remain withheld until the
matching KMS submission is observed as presented. The next evidence gate is the
controlled proof, followed by text, navigation, pointer, resize, sub-100 ms
presentation, clean exit, and TTY recovery in real Kitty.

## 2026-07-12: Installed-Session Input Recovery Interlock

The first installed Kitty operator run exposed a control-plane failure: scanout
reached a visible terminal, but normal keyboard delivery failed and the session
had no reliable local escape. The wrapper had stopped `keyd`, placed the TTY in
graphics/raw mode, and disabled XLibre VT switching while relying on the same
live input path for `exit`. A reboot then removed the runtime-directory logs.

The installed launcher now refuses graphics takeover until a separate libinput
guard observes one complete Ctrl-Alt-Backspace chord. A second chord requests a
graceful in-session exit without depending on focus, then lets the independent
guard force bounded wrapper cleanup if the live loop is wedged. Session groups
and XLibre receive TERM followed by a bounded KILL fallback; KD mode, termios,
and the previous `keyd` state are restored afterward. Full Ctrl-Alt-Fn switching
remains deferred until Sophia owns a correct VT/DRM suspend-and-resume cycle.

The input path now emits privacy-safe one-shot stages for poller readiness,
committed focus, observed keys, authority routing, and XTEST injection. Bounded
synthetic text also traverses Engine focus and evdev mapping instead of entering
the authority channel directly. Logs retain the latest and previous run in the
user state directory so a reboot does not erase the failing seam. The dedicated
TTY physical typing and deliberately wedged-session recovery run remain the
hardware acceptance gate.

Guard bring-up also exposed that libinput's path context can reject stable
`/dev/input/by-path` aliases before invoking Sophia's open callback. Native
input admission now canonicalizes configured absolute paths and honors the
requested read/write access mode without logging either path. The rebuilt QEMU
guest passes the 300-tick dual-output gate with all 14 keyboard events routed,
five pointer events routed, changed pixels, 112 native submissions, clean
retirement, and no callback rejection or cleanup debt.

The isolated recovery scenario now exercises the guard contract directly with
a virtio keyboard: one complete Ctrl-Alt-Backspace chord arms the independent
guard, a second chord triggers both the guard and the live loop, and both
virtual outputs drain with no in-flight scanout or cleanup debt before poweroff.
This is deliberately not recorded as host VT restoration evidence. A separate
same-build rerun of the default physical-text QEMU scenario routed all 14 key
events but timed out waiting for the expected xterm pixel change, so that
content regression remains open independently of the recovery gate.

The first guarded physical-TTY rerun then proved the interlock on hardware: the
first chord armed before takeover and the second returned to the text TTY
without rebooting. Persistent logs showed physical poller readiness, committed
focus, observed and routed keys, and XTEST injection, but Kitty received no
usable input. The installed XTEST protocol definition identifies the stopped
seam: FakeInput's `time` field is a delivery delay in milliseconds, while the
adapter had supplied libinput's monotonic event timestamp. The compatibility
injector now requests zero-delay delivery and synchronously checks each XTEST
request instead of treating a queued unchecked request as successful.

That hardware rerun also exposed the compatibility renderer's full-frame cost:
45 frames in 25.9 seconds, 8.74 MB read and hashed per frame, a 397 ms libinput
lag warning, and a maximum 798 ms submit-to-page-flip interval. The live bridge
now maintains X Damage trackers and a CPU-buffer base, emits one clipped packed
patch per damaged surface, and falls back to replacement only for initial,
resized, missing-base, or at-least-half-surface updates. XTEST uses an
independent connection and worker so capture cannot block channel draining.
The optimized dummy-XLibre proof presented routed xterm input in 17 ms with
three steady patches, 1.26 MB total readback, and a 9 ms maximum capture.

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
will connect the live session to the documented generic X11 WM bridge. The
bridge is an embedded minimal X server with synthetic windows; a configured
legacy WM is layout policy only and receives no physical input, raw metadata,
namespaces, or real client XIDs. Sophia Engine speaks only its generic WM IPC
and does not know which legacy WM the bridge supervises.

The xmonad source is cloned at `~/src/xmonad` commit `a9a8b5c` as the first
compatibility reference. It is not vendored and is not a Sophia runtime
dependency. The embedded server and real two-window xmonad process proof pass.
The remaining gate is to feed opaque live-session surface snapshots through the
same generic bridge socket and apply validated proposals to presented surfaces.

## Active Questions

- Which second legacy WM provides the smallest independent compatibility proof
  that the generic launcher and fake-X11 coverage are not xmonad-shaped?
- Can one live-session xterm be laid out through the bridge while operator input
  still routes through Engine focus and changes presented pixels?

These questions remain probe-driven: implement the first observed missing path,
then rerun the relevant real-client smoke.

## 2026-07-11: Readable And Resizable Xmonad Session Path

The first physical xmonad attempt exposed that the earlier pixel-change proof
was not a visual proof: unimplemented core drawing painted damage bounds white,
and the partial fixed glyph table rendered ordinary punctuation as question
marks. The operator stopped the session without treating that run as milestone
evidence.

X Authority now preserves GC raster values, executes the xterm-used text,
fill, line, clear, copy, and image paths against bounded XRGB buffers, and
covers printable ASCII in its deterministic fixed raster. The real-xterm smoke
scans materialized pixels for the expected `Sophia` glyph sequence and reports
`ascii_marker_match=true`; nonzero bytes alone no longer pass it.

CPU drawing publication now distinguishes full replacements from tightly
packed damage patches. Resize keeps a buffer handle's generation monotonic,
publishes one correctly sized replacement, and applies later patches in order.
With fixed size constraints removed, the real-xmonad headless session completes
one configure acknowledgement, commits the resized layout and focus, and
observes changed pixels after injected input without authority backpressure.
The dedicated-TTY visual rerun remains the final gate.

## 2026-07-11: Real Xterm Through Generic Xmonad Policy

`sophia-live-session` now supervises an arbitrary Sophia WM socket process via
generic executable/argument flags. With `sophia-x11-wm-bridge` selected as that
process, the live session sends one opaque xterm surface to real xmonad, Engine
validates the response, and the committed placement drives composition,
hit-testing, backend visual state, and scanout. A headless integrated run proves
one moved surface, committed Engine focus, and a later injected terminal pixel
change. No xmonad identity enters Engine or the live-session policy path.

The Engine-to-X-Authority control seam now supports bounded configure/focus
commands and reduced acknowledgements keyed by `SurfaceId`. Probing arbitrary
full-output xterm resizing exposed a repaint loop in the core-drawing path, so
the first real one-client gate pins min/max size to the established live buffer
and uses xmonad for placement, stacking, and focus. Removing that constraint is
tracked explicitly rather than overstating resize compatibility.

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

The physical VRR gate now has a dedicated two-phase runner and strict reduced
evidence verifier. During implementation, the proof exposed that the native
page-flip builder carried `VRR_ENABLED`, but the modeset branch ignored the
same policy request. Modeset request construction now supports the property and
fails closed when its handle is absent. `tools/vrr_hardware_proof.sh` derives an
Enabled decision for one opaque, unoccluded fullscreen surface and commits
`VRR_ENABLED=true`, then derives an Ineligible decision for an overlay-present
scene and commits the fixed-refresh `false` fallback. It requires presented and
retired callbacks for both phases. The destructive AMD run is still pending
because it must be performed from the dedicated TTY, not the active graphical
session.

`tools/operator_keyboard_hardware_proof.sh` similarly packages the remaining
operator gate without guessing an input node. The operator supplies a stable
`...-event-kbd` path, waits for the physical-input readiness marker, and types
the expected lowercase proof text. Existing persistent-session evidence rejects
the run unless physical keys route through Engine focus and later xterm pixels
change.

## 2026-07-11: Exact Operator Input Evidence

The first AMD operator attempts exposed two proof-harness defects rather than a
new authority boundary. `keyd` exclusively owned the AT keyboard, so opening its
physical event node succeeded while libinput observed zero events. With `keyd`
stopped, Engine routed 27 events, but the original five-second deadline still
expired before a later xterm transaction changed the composed checksum.

The combined helper now detects an active `keyd`, stops it through an explicit
interactive `sudo sv down keyd`, and installs an EXIT trap that restores the
service. It has no separate Enter-to-begin prompt, preventing that confirmation
key from becoming the first exact-proof event.
The physical proof requires exact press/release pairs for the configured
lowercase text plus Return after Engine focus routing and evdev-to-core-X
translation. It gives the operator 15 seconds to complete that bounded sequence,
then starts a separate five-second pixel-settle deadline. The scanned-out xterm
shows the expected input, and readiness is withheld until those nonzero prompt
pixels are page-flip-confirmed on the primary output. Keyboard delivery freezes
after the exact Return release so operator retries cannot weaken the evidence.
Schema 2 input evidence records
expected events, matched events, and the later pixel change. The AMD acceptance
run now passes: all 14 events matched, xterm pixels changed, and the one-output
native session completed 62 submissions, 61 callbacks/retirements, 22 nonzero
exports, and zero overlap, phase, callback, transition, or cleanup failures.

Requiring a nonzero prompt baseline then exposed that the earlier QMP pixel
claim was a false positive: late prompt drawing changed the initial blank frame,
while routed key events still targeted the last mapped top-level X window.
Xterm selects core key events on its VT child through `CWEventMask`. Sophia X
Authority now parses that bounded value from `CreateWindow` and
`ChangeWindowAttributes`, retains the selected X window inside the authority,
and uses the last mapped window only as a fallback. Input readiness also
requires 500 milliseconds of quiescence after nonzero prompt pixels so event
selection cannot race the external sender. A rebuilt strict QEMU run
then matched all 14 events and changed pixels after the fully drawn prompt before
continuing through pointer and dual-output evidence. Raw X window identity never
crosses into Engine or WM state.

The first AMD VRR attempt then exposed a property-name mismatch. The connector
advertises the kernel-standard lowercase `vrr_capable`, while the selected CRTC
does expose uppercase `VRR_ENABLED`. Discovery had searched for uppercase
`VRR_CAPABLE` and therefore rejected capable hardware before building an atomic
request. The lookup now uses `vrr_capable` with the old uppercase spelling kept
only as a compatibility fallback for deterministic fixtures.

Non-destructive inspection after that correction reports connector 100, CRTC
86, `VRR_ENABLED` present, and `vrr_capable=0`. The current eDP panel is not VRR
capable, so activation/fallback evidence cannot be produced on this hardware.
The gate remains open for a connector reporting capability `1`; Sophia does not
override the value or treat a property contract without capability as proof.

## 2026-07-12: Temporary XLibre Compatibility Provider For Kitty

Kitty's installed X11 backend requires XKB and a working OpenGL context, while
Sophia X Authority deliberately does not yet advertise XKB, GLX, DRI3, or
Present. Pretending that Kitty was another core-drawing probe would therefore
produce a launcher that connected but could never render.

The first usable compatibility checkpoint instead reactivates the historical
XLibre bridge as an explicitly temporary protocol authority. XLibre runs on the
dummy video driver with software GL, no physical input devices, no TCP listener,
and a private MIT cookie. A persistent XComposite adapter owns the XIDs and
named pixmaps, converts readbacks into opaque `XLibrePrototype` surface
transactions, and never exposes client identity to Engine or the WM. Engine
continues to own physical input, focus routing, composition, frame scheduling,
and KMS. Core key events return through a bridge-private XTEST adapter until the
Sophia-owned X Authority has native GPU-buffer coverage.

The first real headless run used Kitty 0.47.4 against XLibre 1.25.1.8. It
materialized one 925 KB nonzero Kitty surface. Capture checksum deduplication
reduced a four-second run from 29 repeated batches to six actual pixel changes;
injected `sophia` plus Return then changed the composed checksum and completed
in 2.6 seconds. Native TTY presentation remains the operator gate.

The first installed-session input proof then showed that capture correctness
alone was insufficient: Kitty echoed typed characters several seconds late.
The launcher had used a debug build, the session cloned and repeatedly scanned
each 1280x720 frame, physical input was polled only after rendering, and native
export recreated its EGL/GL setup for every frame. The launcher now runs the
release binary; XLibre sessions acquire libinput on a bounded worker; the main
loop drains input before waiting for X transactions and again before composing;
CPU composition borrows source storage, row-copies clipped spans, and computes
its checksum/nonzero count in one pass; and the native renderer reuses one EGL
context and GL pipeline per output. KMS still receives a fully completed GL
frame because the atomic path does not yet provide an explicit native fence.

Schema 9 records the maximum composition, input-dispatch gap, queue depth and
dwell, upload, and persistent-resource counts. The final Kitty dummy rerun
presents input in 40 milliseconds with 8-millisecond CPU composition and
11-millisecond MIT-SHM capture. The stricter QEMU final-key-to-primary-output
measurement is 37 milliseconds. The dual-output QEMU proof
creates exactly two native targets and pipelines with zero recreations, drains
155 page flips without cleanup debt, and confirms that PRIME GEM cleanup treats
the driver's already-closed `EINVAL` result as idempotent success. Degraded
XGetImage remains operational but is rejected for interactive evidence.

The next operator run exposed a keymap mismatch hidden by ordinary typing.
Sophia correctly translated Linux input codes with the evdev `+8` convention,
but device-less dummy XLibre had selected its legacy `xfree86` keycode table.
Letter positions overlap between those tables; navigation positions do not, so
evdev keycode 111 (`Up`) arrived as `Print`. The private server now loads the
evdev XKB rules before launching a client and fails startup unless Up, Left,
Right, and Down resolve at keycodes 111, 113, 114, and 116. Sophia X Authority's
minimal core map now advertises the same navigation keysyms for direct clients.
