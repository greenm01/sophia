# Active Research Log

This file records decisions and unresolved questions for the active milestone.
Completed evidence is archived in `research-log-archive.md`.

## 2026-07-14: Explicit Final Scanout Retirement

The post-completion X11 allocator failure exposed a teardown ownership gap.
Persistent presentation deliberately retained the last displayed submission,
but the bounded session drained only the in-flight submission. Returning from
the loop therefore dropped the last GBM owner implicitly without first
retiring its framebuffer, mode blob, and imported GEM handles through the live
DRM device.

Persistent runtime shutdown now explicitly retires that displayed submission,
retries any reduced cleanup while the DRM device and renderer context are
still alive, and refuses clean completion if either in-flight or cleanup state
remains. Lifecycle diagnostics bracket the terminal retirement without logging
native handles. On X13, the focused backend regression and native-feature CLI
build pass, followed by ten of ten uninstrumented exact-text native stability
runs with clean evidence and no allocator diagnostic. The three operator-typed
runs remain the physical acceptance gate.

## 2026-07-14: Portal Requests And Grants Are Separate State

Portal policy decisions no longer need to double as execution authority. A
generic I/O-free lifecycle now retains deadline-bound request facts separately
from single-use grants. Allowed requests create active grants bound to source
generation and broker generation; completion, executor failure, expiry,
namespace disconnect, owner change, and broker restart have explicit terminal
transitions. A caller supplies monotonic time, the active set is capped at 64,
and no payload or operating-system handle enters this state. The first broker
IPC slice will use this reducer for every portal kind while clipboard remains
the first concrete executor.

## 2026-07-13: Core Keyboard Map Offsets And Semantic Input Evidence

The X13 stability workload exposed a false-positive input proof: xterm visibly
echoed repeated `^@` control notation, never printed its `received:` result, but
the session passed because fourteen events flushed and later pixels changed.
The `GetKeyboardMapping` decoder read the request padding byte as
`first_keycode` and the real first-keycode byte as `count`. Xterm therefore
cached keysyms for keycodes 0 through 7 while Sophia delivered normal core
keycodes such as 39 for `s`; Xlib translated every delivered key to NUL.

The decoder now reads the protocol body fields at bytes 4 and 5. Both wire byte
orders have regression coverage, and the real-xterm input smoke requires its
shell to receive exactly `sophia`. The live proof likewise uses an owner-only
result channel and emits schema 11 only after exact terminal bytes, flushed
delivery tokens, changed focused-surface pixels, and presentation all agree.
Pixel change alone is no longer input evidence. Kitty remains a separate
Wayland client proof; its X11 mode needs modern extension coverage beyond this
core-keyboard regression.

On X13, the standalone real-xterm smoke and all ten native repetitions reported
exact six-byte `sophia` receipt with no `^@` substitution. Nine native runs
exited cleanly. The tenth emitted complete schema-11 presentation and cleanup
records, then glibc reported `corrupted size vs. prev_size` during process
teardown. That preserves the allocator lifetime issue as a separate unresolved
bug; it does not weaken the now-semantic keyboard result.

## 2026-07-13: X11 Input Target Race And GBM Owner Drop Order

Two fresh dedicated-X13 milestone attempts stopped at different seams. The first
routed and flushed all fourteen physical key events but received no later xterm
pixels. The second reached terminal content and Engine-applied focus, then
aborted with `free(): invalid pointer` before input readiness. Both runs restored
`keyd`, released DRM ownership, and left no Sophia process or core file.

Inspection found that Engine `FocusSurface` control and client-selected keyboard
delivery shared one atomic X window. A late focus command could replace xterm's
VT child with its top-level surface window after the child selected key events.
Those states are now separate: focus control updates only the surface window,
while key delivery retains the latest client window selecting key events and
uses the focused surface only as a fallback.

The native CPU-upload path keeps each locked GBM front buffer and its originating
surface alive through KMS retirement. That owner's destruction order is now
explicit: release the front-buffer lock first, then release the surface. A
shared persistent GBM/EGL surface was tested and rejected after it reproduced a
pre-input guest crash; the proven per-frame EGL surface path remains in place.

`tools/run_x11_live_session_stability.sh` adds bounded normal, lifecycle-trace,
GDB, and core-capture modes. Timeout evidence now distinguishes no client
update, stale buffer generation, unchanged composition, and missing native
presentation. An X13 QEMU rerun now reaches changed pixels for all fourteen
physical key events and exits without the allocator abort. Its independent
pointer-selection proof still times out with observed-but-unrouted events, so
repeated local TTY runs and the full physical milestone gate remain required.

## 2026-07-13: Explicit Portal Taxonomy

The portal milestone began by removing two ambiguous protocol encodings.
`Screenshot` had represented both still capture and recording, while URI-open
requests were labeled as notifications and distinguished only by a type hint.
`PortalTransferKind` now has explicit clipboard, drag-and-drop, file-handoff,
screen-capture, screen-recording, URI-open, and notification values. Each maps
directly to its namespace capability. Reducer and codec regressions cover every
kind; established codec numbers for the five existing values remain stable,
with recording and URI-open using new tags. Request/grant lifecycle separation
is the next portal slice.

## 2026-07-13: Explicit X Session Profiles And Map Isolation

The live X launcher now selects `classic` or `confined` explicitly. Classic
remains the default shared-X group. A confined run receives a fresh registry
namespace with explicit zero portal capabilities, and those immutable facts
flow through every connection admission. The session status record exposes the
selected profile and directional capability bitsets without exposing namespace
identity.

The first simultaneous confined socket proof assigned two clients distinct
namespaces and exposed a real leak: `MapWindow` changed lifecycle state without
checking the runtime resource table. The runtime now performs namespace-aware
window lookup before mapping, so the second client receives native `BadAccess`;
classic same-namespace mapping remains valid. The following socket expansion
closes properties, selections, metadata, event selection, and routed input.

The next socket expansion found the same missing-boundary pattern in property
and selection paths. `ChangeProperty` previously keyed a foreign XID under the
requester's namespace and could emit a metadata candidate without checking the
window owner. Selection ownership and conversion likewise trusted the owner or
requestor XID instead of the admitted namespace. Runtime/dispatch now validate
all three before mutation or portal construction. The wire proof requires
`BadAccess` for foreign property and owner changes, normal
`SelectionNotify(property=None)` for foreign conversion, and zero metadata
candidates.

The final confinement expansion found that the socket bridge updated its
authority-local keyboard target from `CWEventMask` before dispatch authorization.
A rejected foreign event subscription could therefore redirect later input in
the requester's private worker to another namespace's XID. Event-target changes
now occur only after namespace validation. The drawable validator also
classifies a resource once so a foreign window's `CrossNamespaceDenied` is not
overwritten by a failed pixmap fallback. A routed simultaneous-client proof
requires native `BadAccess`, sends a broker-addressed key to the requester, and
verifies that its event target remains the local root; the broker's separate
queue regressions prove delivery stays client-specific. This completes the
bounded Milestone 1 confinement matrix; full XKB, XI2, focus, and grab semantics
remain Milestone 3 work.

The final admission-lifecycle gap was targeted supervisor revocation. Concurrent
workers now report only their session-issued `ClientAdmissionId` to frontend
supervision and retain a cloned socket solely as a disconnect handle. A
`RevokeAdmission` service command shuts down that one socket; the worker still
owns writer shutdown, private-route removal, connection-ledger cleanup, surface
removal observation, and admission-lease revocation in that order. A
pre-admission command is retained until the matching worker attaches, closing
the allocation/worker-registration race. A simultaneous classic-client
regression revokes admission 1, observes its surface removal and inaccessible
old window, then creates another window through the uninterrupted peer. This
completes the namespace/admission foundation and makes the portal broker plus
X11 clipboard the active milestone.

## 2026-07-13: Live Xauthority Ownership

The live X session no longer relies on an unauthenticated owner-only socket. Its
supervisor obtains a fresh 128-bit cookie from the kernel for every run, writes
a standard `FamilyLocal` `MIT-MAGIC-COOKIE-1` record with mode `0600`, syncs the
complete record before exposing its path, passes `XAUTHORITY` to both launched
terminals, and removes the file through explicit and drop cleanup. A private,
owner-only `XDG_RUNTIME_DIR` is preferred; the random, create-new owner-only
file remains safe when the system temporary directory is the fallback.

The frontend validates that cookie before invoking session admission. Policy
sees only `MitMagicCookie1` provenance and kernel peer credentials, never the
secret. A regression proves bad cookies do not invoke policy, while the accepted
connection is admitted once and revoked once. Fresh per-session generation is
the rotation boundary; confined launch credentials remain future policy work.

## 2026-07-13: Per-Connection X Admission Boundary

The native X frontend now calls a protocol-neutral session policy after setup
authentication and before allocating X client or resource-range identity. The
policy receives only the bounded authentication method and kernel Unix peer
credentials; it never receives raw cookie bytes. A successful decision returns
an immutable `ClientAdmissionContext` retained in an admission lease. Native
X11 setup failure represents denial, and teardown or any early worker error
revokes the lease after route and resource cleanup.

The live classic session backs that policy with its session-owned
`NamespaceRegistry`: it requires a peer UID matching the effective session UID,
allocates a distinct admission per connection, and intentionally assigns those
admissions the same classic-shared namespace. This removes the listener-wide
identity shortcut without weakening classic X semantics. Confined launch and
targeted supervisor revocation are now implemented as described above.

## 2026-07-13: X11-First Namespace And Portal Critical Path

Sophia's next architecture work is the native X Server Frontend, not broader
Wayland protocol or DMA-BUF coverage. The two-xterm frontend already proves
bounded concurrent workers, client-attributed transactions, targeted input,
Engine composition, and KMS presentation. Its next risk is no longer basic
visibility; it is admitting clients into the correct trust domain before more
X11 semantics depend on a hardcoded listener namespace.

The chosen dependency order is session-owned namespace admission, then a portal
broker with X11 `CLIPBOARD`/`PRIMARY` as its first complete adapter, then XKB,
grabs, Engine-derived output/resize, and standard presentation semantics.
Classic shared-X intentionally retains same-namespace resource visibility.
Confined sessions use distinct namespaces and explicit capabilities; XID ranges
remain creation/cleanup ledgers rather than access-control lists.

Wayland/Smithay stays supported under maintenance gates. XLibre remains frozen
historical evidence and a possible future provider only if measured native-X
gaps later justify its authority and maintenance cost.

## 2026-07-13: Kitty DMA-BUF Direct-Scanout Boundary

Enabling the experimental DMA-BUF global for guarded Kitty failed before a
usable native presentation and surfaced the misleading scheduler invariant
`native frame was neither submitted nor retained for a later submit`,
disconnecting Kitty. Sophia's current DMA-BUF route is direct KMS scanout,
whose exporter requires the client buffer to match the physical output exactly.
An arbitrary Kitty toplevel therefore cannot be a valid client for that route,
regardless of the exact buffer that reached the failed run.

This is an architecture boundary, not evidence that Kitty can use the current
direct path. The controlled full-output XRGB producer remains the direct
DMA-BUF lifetime proof. The interactive Kitty harness now deliberately does
not advertise DMA-BUF and continues to prove native SHM composition, input,
recovery, and latency. The next DMA-BUF milestone is GPU composition: import
an arbitrary window-sized client DMA-BUF, scale/blend it into a Sophia-owned
output-sized render target, retain it through the target page-flip retirement,
and only then release the client buffer. Only that route can support Kitty
without requiring fullscreen, output-sized buffers.

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
before Kitty started. A later post-repair normal run also aborted after frame 2.
The persistent CPU-upload texture was therefore isolated from imported images:
each import now gets a transient per-frame texture, which is deleted after
`glFinish` before EGLImage destruction. The repaired three-frame proof passed
with three imports, three retirements, and a 16 ms maximum interval. A normal
core-capture 300-frame run and three separate uninstrumented normal 300-frame
runs then all completed: every run had 300 imports, callbacks, and retirements,
no cleanup debt, no surviving process, and 14–16 ms maximum latency. This meets
the bounded controlled gate, while retaining the normal-stability wrapper as a
regression guard for the earlier intermittent abort. The next required evidence
is three guarded native-SHM Kitty runs; a later GPU-composition milestone must
precede any real-Kitty DMA-BUF runs.

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
Those acceptance runs remain on SHM until GPU composition exists. DMA-BUF stays
non-default until a real-Kitty GPU-composition log proves input, recovery,
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
now gated by the controlled first-frame/lifetime proof; arbitrary Kitty buffers
need GPU composition before they can enter this route. It is not yet recorded
as passing hardware evidence. Wayland
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

- What is the smallest immutable admission record that lets the supervisor bind
  listener policy, peer credentials, cookie validation, session generation,
  namespace profile, and capabilities without exporting secrets?
- What bounded broker IPC keeps portal decisions and grant lifecycle pure while
  allowing runtime executors to retain X selection context, payload bytes, and
  later OS handles?
- Which XKB, grab, RandR, resize, and presentation gaps must close before the
  proven two-xterm hardware path can honestly become `session` evidence?

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

## 2026-07-14: Engine Topology, Authority XKB, And Resize Quarantine

Milestone 3 now has three explicit boundaries. First, live Engine output records
become a validated, generation-bearing, at-most-16-output snapshot; X setup and
populated RandR CRTC/output/mode replies derive from it without exposing KMS
object identity. Dynamic RandR subscriptions and events remain separate work.

Second, Engine sends physical input as a `RoutedInputRequest` containing its
selected Sophia surface and global/local coordinates. The X frontend resolves
the owning worker, then a dedicated authority thread owns per-seat xkbcommon
state using a bounded explicit RMLVO configuration. `XKEYBOARD` remains
unadvertised until its map/name/state request surface is implemented.

Third, an X resize transaction whose pixels match a pending requested size is
quarantined with its CPU update. Neither can mutate the committed scene while
the old geometry is active. When every requested surface is ready, the staged
geometry and pixels replay together; timeout discards them and retains the last
committed scene. This closes the path that could display a large white drawing
update at the old top-left geometry, but hardware resize promotion still needs
an operator proof and rollback evidence.

## 2026-07-14: Probe-Backed GTK Startup And Real SHM Pixels

The native frontend now advertises the measured XKB and XI2 startup subset,
including XI version/device discovery, client pointer/focus queries, event
selection, and optional device-property reads. Zenity consequently advances
through normal window creation and software drawing with no X protocol error.

That probe exposed the blank-block cause: `ShmPutImage` validated the segment
but discarded its offset and payload, then materialized a zero-filled damage
buffer. A narrow SysV SHM adapter now validates segment size with `IPC_STAT`,
attaches read-only, copies only a bounded image range, and detaches immediately.
The generic pixel-proof policy now records one 310-by-233 committed surface,
288,920 nonzero bytes, and `first_error=none`. This is software-pixel evidence,
not completion of interactive GTK input, the full XI2/grab contract, or
Milestone 3 hardware promotion.

## 2026-07-14: One Session XKB Description And Stable RandR Identity

The native frontend now compiles one immutable xkbcommon snapshot from the
session RMLVO. Core `GetKeyboardMapping`, XKB `GetMap`, and per-seat event
translation consume that configuration instead of combining a handwritten US
wire map with an independently compiled state machine. The live command accepts
bounded `--xkb-rules`, `--xkb-model`, `--xkb-layout`, `--xkb-variant`, and
`--xkb-options` overrides; a German-layout regression proves that core and XKB
views change together.

RandR CRTC and output identities now derive from Engine `OutputId`, while mode
identity derives from the mode tuple. Reordering a topology snapshot therefore
does not renumber an unchanged output. Focus state is also namespace-local and
window destruction resets only its namespace. Dynamic RandR event diffs,
complete XKB state/name notifications, grabs, and XI2 event delivery remain
Milestone 3 work.

The follow-up dynamic path now acknowledges newer Engine snapshots, populates
`GetMonitors`, and sends mask-selected RandR screen, CRTC, output, and resource
notifications through each client's bounded protocol queue. A deterministic
`--inject-output-size=WIDTHxHEIGHT` live-session hook applies a validated
generation update after client startup, so update behavior can be retained as
evidence without requiring a physical connector hotplug.

The live resize rollback fence is now an exported coordinator rather than
private layout bookkeeping. It owns committed sizes, monotonic compensating
transaction IDs, abandoned-size filtering, and disconnect cleanup. Integration
tests cover successful advancement, timeout rollback construction, rejection of
late abandoned pixels until the old size is confirmed, and cleanup while a
rollback is pending. The live layout uses this coordinator for its existing
geometry-plus-pixels quarantine and compensating configure path.

Core input grabs now have connection identity and namespace-scoped authority
state instead of validation-only request handling. Active pointer/keyboard,
passive key/button with Any detail/modifier conflict checks, implicit button,
owner-events routing, synchronous freeze with bounded deferred input and
`AllowEvents`, ungrabs, and namespace-local `GrabServer` ownership all clean up
on disconnect. Engine still chooses the ordinary target surface and local
coordinates; the authority redirects only when X grab semantics require it.
XI2 generic-event delivery remains the next input-compatibility boundary.

That XI2 boundary now advertises XGE 1.0 and XI 2.0, reports master pointer
button/valuator classes plus the master keyboard key class, retains bounded
per-client selection masks, and emits selected Key, Button, Motion,
Enter/Leave, and Focus generic events. Device events preserve Engine-provided
root/local coordinates as FP16.16 values and follow core grab redirection. One
input delivery acknowledgement is returned only after the writer flushes the
core event and every selected XI2 record generated from it. Raw, touch, and
gesture events remain deliberately outside Milestone 3.

## 2026-07-14: XKB State, Names, And Subscriptions

The X authority now implements the generic XKEYBOARD 1.0 state/name path rather
than a toolkit-specific startup exception. `GetState` reports the last
authority-translated effective modifier state, `GetNames` publishes interned
component atoms derived from the configured session RMLVO, and bounded
`SelectEvents` parsing persists each client's StateNotify detail mask. Modifier
transitions emit the standard 32-byte StateNotify record only when the selected
state detail changed. Focus/hierarchy policy and retained classic/confined
session evidence remain separate open gates.

Window input routing no longer treats event-mask update order as focus. The
connection records CreateWindow parent links, mapped state, and ConfigureWindow
sibling/stack modes. Engine-selected target surfaces begin core propagation at
their owning window, ancestor selection is bounded against malformed cycles,
and root focus resolves through the current mapped stacking order. Scene-level
restack acknowledgement remains an Engine integration/evidence gate.

Retained live-session completion is now schema 12. It binds each completion to
its `classic_shared` or `confined` namespace profile and records whether the
deterministic Engine topology update was applied. The paired Milestone 3 runner
executes the same guarded two-xterm proof once per profile; its verifier requires
the confined startup record to have zero request and publish capabilities, both
runs to include an applied output update, and both to satisfy the existing
startup, composition, input-flush, presentation, resize, and cleanup checks.
The output-update acknowledgement now also carries the number of RandR records
queued to live subscribers. Schema 12 retains that count, and promotion rejects
an accepted topology update that reached no X11 client.

The paired runner now also requests a deterministic one-shot X11 surface
resize after both terminal surfaces have published. The live layout sends the
client-targeted ConfigureSurface command, validates the matching control
acknowledgement, and keeps the new geometry quarantined until a transaction
with matching resized pixels arrives. Schema 12 reports `surface_resize` only
after that commit; the promotion verifier requires the configure acknowledgement
and pixels marker in both namespace profiles.

The topology path now opens a dedicated authenticated RandR witness before the
Engine update, uses a reply-producing core request as a subscription barrier,
and reads back the resized ScreenChangeNotify record. This replaced the earlier
timing-dependent assumption that xterm itself would subscribe. The witness is
closed before frontend drain; a two-xterm headless live smoke then completed
with four queued RandR records, a matching wire event, committed resized
pixels, and clean process teardown.

Milestone 3 promotion no longer accepts the synthetic-input default. The paired
runner requires readable physical keyboard and pointer event nodes, exact
physical `sophia` plus Return input, flushed delivery, presented text pixels,
and a pointer-driven pixel change in both profiles. Schema 13 separates
automated terminal-content readiness from total operator interaction time, so
the two-second startup budget measures startup rather than typing speed.

## 2026-07-14: Retained Paired Milestone 3 Session

Fresh X13 runs under classic shared-X and a newly allocated zero-capability
confined namespace passed `tools/verify_live_session_milestone3_evidence.sh`.
Both schema-13 completions retained two live CPU layers, exact physical
`sophia` plus Return delivery, pointer-routed pixel changes, matching accepted
authority/runtime transaction counts, four authenticated RandR notifications,
committed configure-plus-pixels resize, native presentation, and no in-flight
or cleanup-pending KMS state.

Classic completed with 94 ms startup readiness, 13 ms maximum composition,
22/22 routed deliveries flushed, and 0 ms measured input-to-presentation.
Confined completed with 90 ms startup readiness, 13 ms maximum composition,
38/38 routed deliveries flushed, and 0 ms measured input-to-presentation. The
operator-bounded elapsed times include deliberate physical interaction and do
not replace the schema-13 startup metric. The ignored retained logs live at
`.evidence/remote-target/tmp/sophia-milestone3-{classic,confined}.log`.

## 2026-07-14: Milestone 4 Buffer Lifetime Foundation

Milestone 4 now has an explicit reduced buffer boundary. Protocol-visible
DMA-BUF descriptors use opaque buffer and fence identities, admit at most four
planes, accept only bounded XRGB8888/ARGB8888 dimensions and byte ranges, and
contain no native renderer objects or file descriptors. Native plane and acquire
fence FDs enter a renderer-private registry. That registry refuses duplicate or
malformed registrations, blocks submission behind an unsignaled acquire fence,
and releases ownership only after page-flip retirement, rejection, or disconnect.

X software drawing now publishes a new immutable handle for every accepted
generation rather than patching a buffer already visible to Engine. External
tests retain the earlier generation and prove its bytes do not change. A matching
CPU lifetime reducer keeps the last committed handle through stale retirement and
rejection, releases the previous handle only after the replacement page flip,
and drains disconnect ownership exactly once.

MIT-SHM now advertises a real extension event base and encodes the standard
Completion event layout when PutImage requested notification and the request was
accepted. The completion is not emitted for rejected window updates. The
workspace all-feature test gate passes offline on X13. Standard DRI3/Present
SCM_RIGHTS transport, client-visible Present feedback, mixed GPU/CPU composition,
and retained Vulkan hardware evidence remain open; the private SOPHIA-PRESENT
prototype is not promoted by this checkpoint.

## 2026-07-14: Standard DRI3/Present Transport Checkpoint

The X authority now negotiates the standard `DRI3` and `Present` extension
names at version 1.2 without routing them through the private
`SOPHIA-PRESENT` opcode. The Unix request reader uses `recvmsg` for the fixed
X11 header, captures up to four SCM_RIGHTS descriptors with close-on-exec set,
then completes the ordinary request payload read. Descriptor ownership is
RAII-bound and unexpected FD arity terminates the malformed connection instead
of leaking or guessing ownership.

The first admitted standard DMA-BUF request is DRI3 `PixmapFromBuffer`. Its
wire decoder preserves the pixmap/drawable and bounded storage metadata, marks
the request as requiring exactly one FD, accepts only 32-bpp XRGB8888/ARGB8888
shapes whose stride and declared storage cover the image, and records only the
authority-owned pixmap identity. The native FD remains borrowed through the
socket trace seam for renderer-side duplication and never enters authority
runtime state. DRI3 fences, PresentPixmap/events, and the live renderer handoff
remain the next transport checkpoint.

## 2026-07-15: DRI3 1.2 Vulkan Transport Proof

The X11 socket output boundary now sends a bounded byte record plus up to four
SCM_RIGHTS descriptors. Standard DRI3 `Open` obtains a duplicated render-device
FD only from the live backend provider and returns it in a one-FD reply; neither
the authority runtime nor Engine stores a device path or native handle.

Mesa's DRI3 1.2 startup required `GetSupportedModifiers`, modifier-bearing
`PixmapFromBuffers`, and the small XFIXES region lifecycle used by Present. The
portable modifier reply advertises linear plus the implicit-modifier sentinel,
and the multi-buffer decoder retains bounded plane strides, offsets, and the
wire modifier in the reduced DMA-BUF descriptor.

The first Vulkan failures were caused by Unix-stream FD association rather than
an AMD modifier. A single `sendmsg` can attach descriptors to bytes preceding
the X11 request that consumes them. The server now queues ancillary FDs in
stream order, leaves them pending across no-FD requests, and drains exactly the
declared arity for each later FD-bearing request. A deterministic regression
sends two descriptors alongside an earlier no-FD XFIXES request and proves that
the following DRI3 pixmap and fence requests consume one each.

On the Void Linux X13 with Mesa RADV, the bounded DRI3 1.2 `vkcube` run remained
healthy for its eight-second proof window: 68 requests, three imported pixmaps
and fences, one accepted standard Present transaction, one committed runtime
surface, and `first_error=none`. This proves Vulkan transport into the Engine
transaction seam; it does not yet claim native KMS presentation of the Vulkan
pixels.

## 2026-07-15: Reusable Renderer-Private DMA-BUF Sources

The renderer lifetime boundary now distinguishes a persistent DRI3 pixmap
source from one in-flight presentation. Plane FDs remain renderer-private and
reusable across Presents, while every presentation receives duplicated plane
and acquire-fence ownership in the existing bounded registry. Page-flip
retirement removes only the in-flight ownership; explicit source removal or
disconnect releases each persistent source once.

External tests use a real xshmfence to prove that an unsignaled acquire fence
holds submission, a trigger makes the presentation ready, page-flip retirement
allows the same source to be presented again, an in-use source cannot be
removed, and disconnect cleanup is idempotent. The complete offline all-feature
workspace suite passes with this reusable lifetime model. Live-session import,
mixed CPU/GPU composition, and page-flip-driven Present feedback remain open.
The X frontend also exposes a cloneable protocol-only feedback router that can
emit Present Complete and Idle after the broker moves into its service thread.
It is intentionally not attached to the current CPU fallback submission: doing
so would acknowledge a page flip that did not contain the imported Vulkan
pixels.

## 2026-07-15: Milestone 4 Live-Presentation Handoff

Commit `11f93ee` leaves Milestone 4 at the boundary between proven protocol
transport and unimplemented native GPU presentation. The frontend publishes
DMA-BUF registrations, fence registrations, and Present submissions through
`XAuthorityObservedTransactionBatch`. `LiveDmaBufPresentationRegistry` owns the
reusable source and per-Present FD model, and
`XServerFrontendProtocolRouter` owns protocol-only completion delivery. No
persistent-session consumer currently connects those pieces, so the bounded
`vkcube` result remains Engine-transaction evidence rather than proof that its
Vulkan pixels reached KMS.

The current live-session assembly is also an explicit architecture debt.
`PersistentNativeScanout` and `PersistentCpuScene` remain in the CLI command;
the latter retains a CPU-only `SurfaceId` projection outside the normative
Engine scene owner. Moving the entire session loop before proving GPU
presentation would broaden the active milestone, while wiring more durable
scene and renderer authority directly into the CLI would deepen the debt.

The chosen continuation is a narrow hybrid extraction. Establish an
Engine/backend-owned live-presentation seam, then move only DMA-BUF import,
acquire-fence polling, mixed CPU/GPU composition, KMS submission correlation,
and page-flip retirement through it. Source and fence FDs transfer immediately
into renderer-private ownership. Engine preserves the last committed
geometry-plus-pixels state while a presentation is pending or rejected. Only a
real page flip containing the imported pixels may route Present Complete, then
Idle, trigger the idle fence, and retire the presentation exactly once. Broader
CLI session-loop extraction and Milestone 5 compatibility work remain deferred
until the software-plus-`vkcube` native KMS matrix passes.

## 2026-07-15: Milestone 4 Mixed-Presentation Implementation

The narrow handoff is now implemented without moving protocol or native object
ownership into Engine. The X frontend assigns typed buffer/fence handles and
routes feedback by exact `TransactionId`. `LivePresentationResourceSession`
immediately duplicates frontend registrations into renderer-private ownership,
polls xshmfences, builds mixed CPU/DMA-BUF frames, and retains reusable DRI3
sources separately from individual Present lifetimes. The native EGL path
supports one-to-four-plane EGLImages, clipped placement, alpha blending, and a
single persistent output composition pass.

Engine now exposes a prepared surface commit for asynchronous presentation.
Preparation does not mutate committed state. Page-flip application revalidates
only surfaces touched by the prepared transaction, which prevents stale GPU
callbacks from overwriting a newer version of the same surface while allowing
unrelated CPU surfaces to continue committing. Rejection and disconnect drop
the candidate. Successful native feedback applies the candidate, routes Present
Complete with Flip mode, retires the renderer presentation and idle fence, then
routes Idle. Teardown converts remaining queued work to Skip/Idle and asserts
that no source, fence, presentation, or cleanup debt remains.

The offline all-feature workspace suite passes, including prepared-commit
merge/stale regressions, real xshmfence wait/trigger tests, repeated-pixmap and
deferred-release tests, mixed-frame backend ownership, multi-plane renderer
validation, and exact transaction routing. The schema-14 session evidence adds
mixed-export, acquire-wait, completion, idle-fence, and live-resource counters.
`tools/live_session_milestone4_hardware_proof.sh` pairs the established software
resize proof with a `vkcube`/CPU mixed session, controlled first acquire delay,
one rejected Present, required later Flip recovery, and strict teardown checks.
Its verifier passes positive and missing-mixed-export fixtures. The exclusive
TTY X13 run is deliberately still unclaimed and is the remaining Milestone 4
exit action.

## 2026-07-15: Milestone 4 Hardware Checkpoint And AMDGPU Mixed-Draw Blocker

The paired X13 gate now proves the software half after a real renderer defect
was isolated. Reusing mixed-composition GL state for the legacy full-screen CPU
upload eventually lost the AMD context. The persistent upload path now restores
its fixed full-screen quad and completes independently; the retained software
run committed the 800x600 configure-plus-pixels resize, flushed all 14 semantic
input events, reported exact text and changed pixels, submitted 36 native
frames, retired 35, and drained with no failure or cleanup debt.

The Vulkan attempt also exposed a transaction-domain mismatch. Present request
generations continue across controlled Skip, while Engine visual generations
advance only on accepted commits. Full-state Present snapshots are now rebased
to the current Engine committed generation immediately before preparation;
external regressions cover the empty baseline and a later post-Skip baseline.
This removed the stale-candidate flood and allowed the real imported image to
reach the renderer.

The remaining failure is specifically the required two-layer native EGL draw.
With the CPU background removed only for diagnosis, the same real `vkcube`
session completed 86 mixed exports and Flip completions, one controlled Skip,
87 matching Idle events and idle-fence triggers, 121 native submissions, 120
retirements, and zero live sources, fences, transactions, or KMS cleanup debt.
Restoring the CPU layer aborts Sophia inside Radeon `glFinish` with
`amdgpu: The CS has been rejected, see dmesg for more information (-2)` before
the first mixed KMS submission.

The failure survived `RADV_DEBUG=nodcc`, explicit CPU/import completion
boundaries, frame-local CPU textures, frame-local vertex buffers, and a
diagnostic layer-order reversal. Those experiments were removed; the retained
implementation keeps only the proven full-screen upload isolation, Present
generation rebase, and EGLImage sampling lifetime order. The next session
should capture the privileged kernel validator message immediately after one
failure, then reduce CPU-texture-plus-imported-image composition to a focused
native-EGL hardware regression before changing more session code. Retained
ignored evidence is under
`.evidence/remote-target/tmp/sophia-milestone4/` and
`.evidence/remote-target/tmp/sophia-milestone4-dmabuf-only/`; neither GPU log
is promotion evidence until the normal paired verifier passes.

## 2026-07-15: Milestone 4 Native-EGL Reduction

The remaining mixed-draw failure now has a bounded reproduction below KMS.
`native-egl-vkcube-mixed-smoke` launches the real native-X `vkcube` transport,
transfers its DRI3 planes through `LivePresentationResourceSession`, combines
them with the full-output CPU background, and invokes the persistent mixed
exporter directly. A watchdog parent reports child exit or timeout, while the
successful child emits schema-1 evidence only after disconnect drains all live
sources, fences, and presentations. Fixture-backed verification rejects a
missing CPU layer or cleanup debt.

Native composition now reports CPU upload, EGLImage create/bind, draw, finish,
and destroy failures separately. The mixed CPU background uses dedicated,
fixed-size texture storage and sub-image updates rather than reallocating the
fullscreen upload texture in the command stream that samples the imported
EGLImage. This is locally covered and the full offline all-feature suite passes,
but it is not hardware promotion evidence until the focused X13 smoke and the
normal schema-14 paired proof both pass. The paired wrapper now retains
privileged before/after kernel logs and driver environment identity so another
AMDGPU rejection cannot lose its validator record.

## 2026-07-15: Milestone 4 Mixed-Presentation Hardware Exit

The isolated mixed exporter passed while the persistent session aborted in the
Radeon command-submission thread. Lifecycle tracing and a GDB backtrace showed
that one CPU-plus-DMA-BUF frame drew, swapped, submitted, page-flipped, and
retired successfully; the asynchronous rejection surfaced in the following
ordinary CPU upload. Reusing the GL context across that imported-image boundary
was the distinguishing lifetime.

The native renderer now destroys the mixed frame's GL context after
`glFinish`, swap, and front-buffer lock. The returned GBM owner retains the
scanout surface independently through KMS submission and page-flip retirement;
the following CPU frame receives a fresh context. DRI3 `Open` also returns an
independently opened same-GPU render node instead of the compositor's primary
KMS node, preserving the protocol/backend ownership boundary.

The retained X13 schema-14 proof passed its strict verifier with 76 mixed Flip
completions, one controlled Skip, 77 matching Idle events and idle-fence
triggers, nine acquire-gate waits, zero submit/retire failures, and zero live
sources, fences, or transactions. The established software xterm/resize half
also passes, completing the Milestone 4 hardware exit.

## 2026-07-16: GTK3 Application Promotion Contract

Milestone 5 uses a direct, bounded Sophia-X client launcher instead of wrapping
applications in xterm. Application evidence correlates the existing live
session record with `sophia_x_application_session schema=1`: exact bounded
stdout, normal exit, reduced zero-error protocol observations, physical text
and pointer-button delivery, resize/redraw, native presentation, and clean
teardown are all mandatory. Error-only X dispatches cross the frontend boundary
as reduced code/opcode/sequence facts but never create empty Engine commits.
Zenity entry dialogs run under fresh classic shared-X and confined sessions;
the operator types without Return and completes the action with a physical OK
click.

## 2026-07-16: Zenity Probe-Driven RandR And XFixes Gaps

The current GTK3 Zenity engine probe exposed two bounded requests after its
package became available locally: RandR `GetOutputProperty` for EDID and XFixes
`SelectSelectionInput`. Sophia now returns a valid empty output-property reply
when no EDID payload is retained and validates the selection window, atom, and
three-bit event mask. The same probe showed that advertising DRI3 without a
render-device provider creates an avoidable `BadImplementation`; socket
advertisement now withholds DRI3 in that configuration so GTK selects MIT-SHM.
The repeated probe commits one surface with 288,920 nonzero software bytes and
`first_error=none`; no broader RandR property store or XFixes event expansion
was inferred.

## 2026-07-16: Sophia X TTY Recovery Is An Acceptance Gate

The first GTK hardware attempt could leave the active text VT black until a
power cycle because the X proof called the raw persistent KMS runner without
the guarded TTY lifecycle already used by native Wayland. The GTK runner now
builds and preflights before takeover, requires an independent
Ctrl-Alt-Backspace guard to arm, saves KD and termios state, runs each Sophia
session in a bounded process group, restores keyd and the console on every exit
path, and records a strict durable recovery line.

The isolated QEMU emergency gate then exposed five modifier deliveries queued
before the final Backspace trigger. Emergency completion now waits for those
deliveries to flush before frontend teardown. The repeated gate proves guard
arm/trigger, exact five-of-five settlement, clean two-head KMS retirement, zero
native cleanup debt, and clean guest shutdown.

## 2026-07-16: Milestone 5 Native Zenity Blocker Retained

Two guarded X13 classic shared-X attempts reached one Engine-owned native KMS
output and then showed a blank screen because Zenity aborted before presenting
a dialog. Both retained logs report GTK thaw-update assertions followed by
`BadRequest` at serial 304, request code 139 (`XFIXES`), minor code 0
(`QueryVersion`). The confined profile never started. The second emergency
chord restored KD mode 0, the exact termios state, keyd, and all Sophia
processes; the recovery record is complete. The earlier false keyd failure was
a service-start race, so the runner now waits boundedly for keyd after `sv up`.

The retained diagnosis was incomplete because wire-parse errors discarded the extension minor opcode and always encoded minor zero. A raw-minor trace on the X13 render-provider path reproduced XFixes request 11 (`SetRegion`) immediately after `CreateRegion`; `QueryVersion` had already succeeded. Sophia now retains extension minor codes, owns namespace-scoped XFixes region lifecycle, validates Present region references, and reclaims regions with the client resource range.

The first corrected run exposed a separate sentinel bug: raw region zero was converted with generation one and compared structurally to the generation-zero `NONE` value. Validity-based optional-resource checks fixed that rejection. The exact X13 sequence now accepts CreateRegion, SetRegion, DRI3 pixmap and fence resources, and Present with `first_error=none`. The non-KMS render-provider smoke reaches an Engine transaction but has no scanout consumer, so its remaining pixel-proof failure is expected and is not session evidence. Fresh guarded classic and confined hardware captures remain required before GTK promotion.

## 2026-07-17: GTK Input Stall Split From Scanout Throughput

The latest guarded X13 classic run presented the Zenity entry dialog but
accepted only five physical key presses before input stopped. The retained
15-second interval contained 984 X requests, including 252 outputless requests,
62 MIT-SHM PutImage requests, 31 CPU compositions, and 30 native submissions.
That showed both avoidable redraw work and socket-output lock contention, but
not a KMS deadlock: presentation continued while keyboard progress stopped.

Physical libinput collection now runs on a bounded worker instead of the
authority loop. Outputless X requests skip the shared output-stream lock,
software-only authority batches may coalesce their CPU composition while every
Engine transaction is still applied in order, and cursor-only movement produces
a composed native frame. During the pointer acceptance phase, physical Return
press and release are suppressed and reported instead of aborting the session.
Raw X request tracing and native lifecycle tracing are no longer enabled by the
normal GTK hardware runner; `SOPHIA_M5_GTK_DIAGNOSTIC=1` opts into both.

A bounded local Zenity entry proof then routed and flushed all fourteen
synthetic press/release events for `sophia` plus Return. GTK continued issuing
geometry, property, and SHM redraw requests but never exited or produced the
expected stdout before the semantic timeout. The throughput and lock fixes are
therefore retained, while GTK entry submission remains an explicit Milestone 5 compatibility gap.

## 2026-07-17: Unattended GTK Input Acceptance In QEMU

A direct-kernel, diskless, networkless QEMU guest now runs the real Zenity
entry dialog under both `classic_shared` and `confined` namespace profiles.
The host harness uses QMP only to drive virtio keyboard and mouse devices; the
guest receives those events through the normal physical-input poller. Both
profiles type exact `sophia`, observe changed pixels, route a physical OK-button
click, match Zenity stdout, exit normally, and cleanly retire both virtio-gpu
outputs with `protocol_errors=0`.

The trace-driven compatibility slices added core ChangeGC and CreateCursor,
XIChangeCursor, bounded opaque non-input SendEvent delivery, XIUngrabDevice,
and a protocol-shaped XIQueryPointer reply. It also exposed a proof-loop bug:
Return suppression was scoped to the entire pointer-proof run rather than only
the pre-selection phase. Suppression now ends when pointer selection becomes
ready, and an application proof cannot complete before its primary child exits.
The QEMU result closes the deterministic semantic gap; guarded target-hardware
classic/confined captures with resize remain the promotion gate.


## 2026-07-17: Presented Cursor Gate And Production-Loop Review

The final GTK QEMU regression corrected the earlier pointer-proof conclusion.
Pointer readiness now follows a centered cursor composition and matching native
presentation. Return remains suppressed until a physical pointer button routes,
not merely until pointer readiness. If the application proof surface disappears
before selection, the session exits through bounded cleanup. Both classic and
confined Zenity guests pass the click-then-submit sequence with normal exit and
clean two-output retirement. Fresh paired X13 evidence remains required.

A concurrent architecture review found that the authority boundaries are
implemented, but production orchestration remains duplicated. The 6,500-line
live-session command retains `PersistentCpuScene`, `PersistentBackendRuntime`,
and `PersistentNativeScanout` while Engine, runtime, and backend crates each
carry partial loop abstractions. The next architecture milestone after GTK
promotion is one protocol-neutral coordinator in
`sophia-engine::runtime_driver`: bounded authority intake, Engine
commit/preparation, composition from committed state, backend-private KMS
submission/retirement, then exact protocol feedback. CLI proof logic remains
an observer and supervisor rather than a visual-state owner.


## 2026-07-17: GTK Submit Deadlock Removed

A fresh local QEMU run reproduced the apparent post-submit blank screen after
Zenity had accepted exact physical text and pointer input. The client process
exited, but the CLI session loop synchronously called `read_to_end` on its
piped stdout. An inherited writer could therefore block the visual coordinator
forever, bypassing the 30-second session deadline and preventing native
retirement and console recovery. Application stdout now targets a private
mode-0700 capture directory and mode-0600 file. Once the child exits, the loop
reads at most 4,097 bytes from the regular file without waiting for every
inherited descriptor to close; a regression keeps a writer open while proving
the bounded read completes.

The rebuilt X13-hosted QEMU image passed both GTK profiles. Classic completed
in 4,617 ms and confined in 4,633 ms; both matched `sophia\n`, routed a
physical pointer button, reported `first_error=none`, retired both virtio-gpu
outputs, and ended with zero native cleanup debt. The initramfs builder also
requires xterm explicitly now: it can no longer silently produce a nominally
successful image whose default session scenario fails at boot-time readiness.
The guarded physical X13 resize captures remain the Milestone 5 promotion gate.


## 2026-07-17: Input Delivery Settlement Restores Bounded Sessions

The default xterm QEMU gate exposed a second post-input stall after keyboard and
pointer evidence had already succeeded. Phase tracing proved cursor composition,
KMS submission, and page-flip retirement all returned. The loop instead kept
`input_delivery_wait_started_at` populated after the exact key deliveries
settled. Because ordinary proof sessions advance `--max-ticks` only outside an
active delivery wait, a successful input proof made the session immortal; GTK
was unaffected only because its application-specific proof exits immediately.

Delivery settlement now consumes the wait timestamp exactly once. Later pointer
or emergency batches start their own bounded delivery wait and clear it after
settlement, while the initial key-flush record remains tied to the complete
14-event sequence. A regression covers the consume-once transition. The QEMU
verifier now recognizes current schema 14 and validates either native CPU export
mode: zero GL resources for the preferred direct linear GBM write, or exactly
one reusable GL target/pipeline per output for the fallback. Mixed counters,
recreation, missing uploads, latency violations, and cleanup debt still fail.

The rebuilt X13-hosted image passed every unattended profile. The strict
two-xterm run completed 300 ticks in 6,971 ms with two CPU layers, 8 ms input
presentation, 11 ms maximum composition, 40 submissions, 38 retirements, and
zero cleanup debt. Classic and confined GTK completed normally with exact
stdout, `first_error=none`, pointer selection, and clean two-output retirement.
The emergency profile armed and triggered Ctrl-Alt-Backspace, flushed all five
routed deliveries, and shut down cleanly in 187 ms.


## 2026-07-17: Backend Snapshot Ownership Moves Into Production Coordinator

`HeadlessCompositorBackendAssembly` no longer stores an independent
`Vec<CommittedSurfaceState>`. It owns a `ProductionSessionCoordinator`, and the
existing deterministic and live runtime adapters now receive and return the
coordinator-owned snapshot through one split Engine/state borrow. Public
`with_committed_surfaces`, replacement, input routing, rendering, and runtime
reports retain their behavior, but the live backend has one fewer visual-state
owner before the remaining CLI scene and native sequencing migration.

The focused Engine, all-feature live-backend, and live CLI suites pass. A rebuilt
X13 QEMU image also passed the strict two-xterm gate with two CPU layers, exact
keyboard and pointer routing, 8 ms input presentation, 40 submissions, 38
retirements, and zero cleanup debt. The confined GTK gate passed normal Zenity
exit, exact stdout, `first_error=none`, clean two-output retirement, and zero
native debt. This is an ownership migration, not the Milestone 6 exit: the
legacy runtime adapter still sequences commits and the CLI still owns
`PersistentCpuScene` and `PersistentNativeScanout`.


## 2026-07-17: CPU Pixel Storage Leaves The CLI

Renderer-live now owns a protocol-neutral `LiveCpuBufferRegistry`. It accepts
immutable replacements and packed damage patches, rejects stale generations,
missing bases, metadata changes, invalid bounds, and malformed byte lengths,
and retires unreferenced handles. The X frontend remains responsible for
read-only MIT-SHM admission and emits its existing immutable updates; the CLI
only converts those packets at the renderer boundary. `PersistentCpuScene` no
longer contains a CPU buffer map or applies pixel patches itself.

Four focused registry regressions cover replacement/patch ordering, stale
generation rejection, fail-closed malformed replacement and patch behavior,
and resource retention. The live CLI suite passes. On the rebuilt X13-hosted
image, strict two-xterm QEMU completed 300 ticks with two CPU layers, 7 ms input
presentation, 40 submissions, 38 retirements, and zero cleanup debt. Confined
GTK passed its high-volume SHM redraw path, exact text/pointer proof, normal
exit, `first_error=none`, and clean two-output retirement. The remaining
Milestone 6 scene gap is narrower but explicit: CLI still projects a
`SurfaceId` to geometry/handle table because commit and composition have not yet
been split into coordinator phases.


## 2026-07-17: Authority Commits Once Before Per-Output Projection

The persistent live backend no longer creates one authority inbox per output or
replays the same X transaction through every output Engine. The production
coordinator exposes a bounded commit phase; the primary runtime commits each
batch once, then every output consumes the same immutable committed snapshot and
the same precomputed commit observations. The late-client generation bridge now
runs once at this boundary, before the single commit, rather than priming every
output ahead of replay. A two-output regression proves both output assemblies
end on generation 6 while the runtime records one committed transaction.

The full offline all-feature suite passes. The rebuilt X13-hosted QEMU image
passed the strict two-xterm profile in 7,104 ms with 114 of 114 authority
transactions applied, two CPU layers, 7 ms input presentation, 4 ms maximum
composition, 40 submissions, 38 retirements, and zero cleanup debt. An initial
run without the centralized late-discovery bridge correctly rejected the second
xterm generations as stale; retaining that fail-closed evidence drove the fix.
Confined GTK then passed 54 SHM transactions, exact text and pointer evidence,
normal exit, `first_error=none`, 108 submissions, 106 retirements, and clean
resource shutdown. Classic GTK passed the same application contract, and the
emergency profile flushed all five routed chord deliveries before clean shutdown
in 178 ms. Composition still uses the CLI scene table and runs before
this commit phase; the next slice must compose from the coordinator snapshot.


## 2026-07-18: CPU Composition Consumes Engine Committed State

The persistent CPU path now splits authority preparation from per-output runtime
ticks. Each batch commits once, renderer pixel updates are reconciled against the
resulting immutable `CommittedSurfaceState` slice, and composition resolves
geometry, buffer handles, stacking, readiness, and proof generations from that
slice before any KMS submission. `PersistentCpuScene` retains only the renderer
buffer registry and composition evidence; its independent `SurfaceId` table and
raised-surface state are deleted. Native runtime construction also no longer
requires a pre-commit frame or a blank modeset: KMS initializes with the first
frame composed from committed state.

The full offline all-feature suite passes. A rebuilt X13-hosted QEMU image passed
the strict two-xterm 300-tick profile in 6,824 ms with 123 of 123 authority
transactions applied, two CPU layers, 8 ms input presentation, 2 ms maximum
composition, 40 submissions, 38 retirements, and zero cleanup debt. Confined GTK
passed 56 committed SHM transactions, exact text and pointer evidence, normal
exit, `first_error=none`, 107 submissions, 105 retirements, and clean shutdown.
Classic GTK also passed the final image with exact application evidence and clean
retirement. The emergency profile flushed all five routed chord deliveries and
shut down without native debt in 151 ms. The duplicate
scene milestone item is complete, while CLI ownership of runtime/scanout and
feedback sequencing remains open.


## 2026-07-18: Production Feedback Waits For Asynchronous Retirement

The initial production adapter incorrectly modeled KMS submission and retirement
as one synchronous callback. That shape could not own the real live path without
either blocking a cycle or treating submission as page-flip completion. The
contract now has separate `submit_frame` and `poll_retirements` phases. A
`ProductionRetirement` carries its originating cycle, and protocol feedback is
routed only for records returned by the retirement poll. Submission evidence and
zero or more feedback records remain distinct in each cycle report.

Engine regressions cover ordered immediate retirement, retirement-poll failure
with no feedback, feedback failure after retirement, and a frame held across one
cycle then retired on a later poll. The live closure adapter exposes the same four
callbacks. The full offline all-feature suite passes. This establishes the
correct state-machine seam for moving `PersistentNativeScanout` and Present
Complete/Idle timing out of the CLI; the live path is not yet wired through it,
so the Milestone 6 coordinator and sequencing items remain open.


## 2026-07-18: Backend Owns Page-Flip Retirement Correlation

`PersistentNativeScanout` no longer owns an Engine presentation registry, a
per-head scheduled frame slot, or a reduced UST/MSC feedback queue. The
protocol-neutral `LiveProductionPageFlipTracker` in backend-live now schedules
each submitted output against a production cycle, rejects overlap, validates
monotonic page-flip sequence and timestamp evidence, retires the exact scheduled
frame, and emits a `ProductionRetirement<LiveProductionPageFlipRetirement>` only
after all those gates succeed. Per-output take and discard operations preserve
the existing CPU-frame versus Present-frame separation without exposing backend
state to the frontend.

Regressions prove no retirement exists at submit time, a matching accepted flip
retains the originating cycle and reduced UST/MSC, overlap fails closed, and
non-monotonic callbacks produce no retirement. The full offline all-feature
suite passes. On the rebuilt X13-hosted QEMU image, strict two-xterm completed
300 ticks in 6,970 ms with 120 of 120 transactions applied, 8 ms input
presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
Confined GTK passed 57 SHM transactions, exact text and pointer evidence, normal
exit, `first_error=none`, 113 submissions, 111 retirements, and clean shutdown.
Present protocol routing still resides in the CLI and is the next ownership seam.


## 2026-07-18: Present Complete And Idle Follow Backend Resource Retirement

`LiveProductionPresentFeedbackCoordinator` now owns the presentation resource
session and produces paired, protocol-neutral Complete/Idle outcomes only after
the matching page flip or controlled rejection retires the live presentation.
Missing or already-retired transactions return an explicit error and emit no
feedback. The live CLI translates the reduced Flip/Skip mode to X wire events
and observes counters, but no longer orders resource retirement, Complete, Idle,
or idle-fence accounting independently. Diagnostic abort still tears resources
down without falsely producing client feedback.

Tests prove a client-released DMA-BUF source and presentation retire before the
Flip/Idle outcome, a second completion fails closed, and an unknown Skip emits
nothing. The full offline all-feature suite passes. The guarded X13 native EGL
vkcube diagnostic passed with one CPU layer, one DMA-BUF layer, and zero live
sources, fences, or transactions. A rebuilt strict two-xterm QEMU image completed
300 ticks in 6,919 ms with 123 of 123 transactions applied, 7 ms input
presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
The full real-KMS Milestone 4 proof could not run unattended because X13 sudo
requested a password before any modeset; it remains a later interactive gate.
Prepared Engine commit application and runtime/scanout invocation are still in
`live_session.rs` and remain the next production ownership migration.


## 2026-07-18: GPU Present Uses One Engine Snapshot Across Outputs

The live Present path no longer prepares and applies the same GPU transaction once per
output assembly. It prepares against the primary production coordinator snapshot, applies
that prepared commit exactly once after the matching page flip, and projects the resulting
immutable committed snapshot to the remaining outputs. This removes a multi-output visual
authority fork while preserving per-output scanout state.

A focused coordinator regression proves that applying a prepared Present mutates the
coordinator-owned snapshot, and the full offline all-feature suite passes. The rebuilt X13
QEMU image passed strict two-xterm in 6,907 ms with 120 of 120 authority transactions,
5 ms input presentation, 38 submissions, 36 retirements, and zero phase or cleanup debt.
Classic and confined GTK passed exact physical text and pointer selection, normal Zenity
exit, `first_error=none`, and clean two-output retirement. Emergency recovery flushed all
five routed chord deliveries and shut down cleanly in 187 ms. Runtime/scanout
invocation and the retirement-to-commit trigger still reside in `live_session.rs`; moving
that sequencing behind the production adapter remains the next Milestone 6 boundary.


## 2026-07-18: Coordinator Completes Retired Present Atomically

A matched GPU page flip now enters one `ProductionSessionCoordinator` operation that
applies the prepared Engine commit, captures the resulting immutable snapshot, retires
the backend Present resources, and produces the reduced Complete/Idle outcome. The CLI
requests that operation and translates its outcome, but no longer orders Engine commit
and backend feedback retirement itself. If the prepared baseline is stale, the coordinator
preserves the current snapshot and never invokes the feedback retirement callback.

Regressions prove commit-before-feedback on success and zero feedback calls for a stale
baseline. The full offline all-feature suite passes, the X13 release build succeeds, and
the guarded native EGL/vkcube diagnostic exports one CPU plus one DMA-BUF layer with zero
live sources, fences, or transactions afterward. The retained two-xterm, GTK classic, GTK
confined, and emergency QEMU gates already passed the immediately preceding snapshot; the
remaining production-loop gap is ownership of live runtime/scanout invocation itself.


## 2026-07-18: One Session Coordinator Owns Visual State

`PersistentBackendRuntime` now owns one session-level `ProductionSessionCoordinator`.
Authority commits, Present preparation, retired Present completion, and public committed
state all use that owner. Per-output backend assemblies receive immutable snapshot
projections for rendering and scanout; they are no longer selected as a primary authority.
A regression deliberately changes the first output projection to generation 99, then
proves the session coordinator independently commits generation 5 to 6 exactly once and
overwrites both output projections with its result.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed 300 ticks in 7,013 ms with 117 of 117 authority transactions, 7 ms input
presentation, 42 submissions, 40 retirements, and zero phase or cleanup debt. Confined GTK
committed 58 SHM transactions, accepted exact physical text and pointer selection, exited
normally with `first_error=none`, and retired both outputs cleanly. Live runtime and native
scanout method invocation still need to move behind the production live adapter before the
Milestone 6 coordinator item can close.


## 2026-07-18: Production Output Fanout Owns Runtime And Scanout Order

Engine now defines a protocol-neutral `ProductionOutputRuntimeAdapter`, and backend-live
provides its bounded callback implementation. The session coordinator projects its single
committed snapshot and enumerates outputs. Steady CPU ticks, committed-snapshot ticks, GPU
Present submission, native idle submission, page-flip retirement and cleanup, and displayed
buffer teardown all enter through that fanout. During the audit, retired Present completion
was also corrected to use the session coordinator directly and to project its result to
every output; it no longer mutates the former primary output coordinator.

Engine and backend regressions prove one snapshot reaches every output and projection plus
runtime invocation remain one adapter callback. The full offline all-feature suite passes.
The rebuilt X13 QEMU image passed strict two-xterm in 6,897 ms with 120 of 120 transactions,
7 ms input presentation, 40 submissions, 38 retirements, and zero phase or cleanup debt.
Classic and confined GTK accepted exact physical text and pointer selection, exited normally
with `first_error=none`, and cleanly retired both outputs. Emergency recovery flushed all
five routed chord deliveries and shut down cleanly in 189 ms. The concrete closures still
live beside `PersistentNativeScanout` in `live_session.rs`; extracting that implementation
into backend-live is the remaining runtime/scanout ownership step.


## 2026-07-18: Native Scanout Ownership Leaves The CLI

The concrete native output owner is now backend-live's `LiveProductionNativeScanout`.
It owns real atomic card/session groups, per-head GBM exporters, native callback queues,
page-flip correlation, submission and retirement counters, mixed-frame export, and the
production composed-frame record. The implementation is feature-gated in its own backend
domain module. `live_session.rs` no longer defines or directly names real atomic sessions,
GBM exporter types, or the page-flip tracker, and shrank by roughly 570 lines. The existing
opt-in native lifecycle diagnostic is preserved verbatim at the backend boundary.

Default and all-feature backend builds pass, as does the full offline all-feature suite.
The guarded X13 native EGL/vkcube diagnostic exported one CPU and one DMA-BUF layer with
zero live resources afterward. The rebuilt QEMU image passed strict two-xterm in 6,973 ms
with 120 of 120 transactions, 3 ms input presentation, 38 submissions, 36 retirements, and
zero phase or cleanup debt. Classic and confined GTK passed exact physical text/pointer
selection, normal exit, `first_error=none`, and clean two-output retirement. Emergency
recovery flushed all five routed chord deliveries and shut down cleanly in 164 ms. The
remaining Milestone 6 ownership gap is `PersistentBackendRuntime` plus the CPU composition
callbacks still implemented in the CLI.


## 2026-07-18: CPU Composition State Moves Into Renderer-Live

Renderer-live now owns `LiveProductionCpuScene`: the CPU buffer registry, immutable update
admission, committed-surface handle retention, focus-aware stacking, cursor composition,
focused-surface visual-detail inspection, composition evidence, and per-output composed
frame creation. The X boundary only converts authority SHM records into neutral
`LiveCpuBufferUpdate` values and observes reduced reports. Backend-live also owns composed
frame records and the high-level page-flip cleanup/retry and displayed-output release
operations, so CLI callbacks no longer invoke low-level native cleanup APIs or lifecycle
logging. The protocol-neutral authority, renderer, scanout, and feedback adapter roadmap
item is complete.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed in 6,995 ms with 117 of 117 transactions, 7 ms input presentation, 40 submissions,
38 retirements, and zero phase or cleanup debt. The guarded native mixed diagnostic exported
one CPU and one DMA-BUF layer with zero live resources. Classic and confined GTK accepted
exact physical text and pointer selection, exited normally with `first_error=none`, and
retired both outputs cleanly. Emergency recovery flushed all five routed chord deliveries
and shut down cleanly in 162 ms. `PersistentBackendRuntime` remains the last large CLI
visual-control wrapper; its X routing and proof observations must be separated from the
protocol-neutral production state machine before the Milestone 6 ownership item can close.


## 2026-07-18: CPU And Present Batches Enter Production Runtime Cycles

CPU authority batches now enter `ProductionSessionCoordinator::run_cycle`, which owns the
commit, immutable-snapshot composition, output submission, retirement poll, and feedback
order. The X session loop supplies translated authority updates and observes only the reduced
submission report. Present batches now likewise cross a single runtime-owned GPU production
entry point: CPU-background composition, per-output frame creation, native initialization,
and Present scheduling no longer occur in the outer CLI loop. Present remains asynchronous;
its prepared Engine state is committed only after the matching page flip through the existing
coordinator retirement gate.

The full offline all-feature suite passes. The rebuilt X13 target passed the guarded mixed
CPU-plus-DMA-BUF diagnostic with zero live sources, fences, or transactions. Its QEMU image
passed strict two-xterm in 6,966 ms with 117 of 117 authority transactions, 8 ms input
presentation, 40 submissions, 38 retirements, and zero cleanup debt. Classic and confined
GTK accepted exact physical text and pointer selection, exited normally with
`first_error=none`, and retired both outputs cleanly. Emergency recovery flushed all five
routed chord deliveries and shut down cleanly in 161 ms. The remaining Milestone 6 boundary
is structural: extract the runtime-owned visual control object from the X router, proof
counters, and process supervision, then delete the legacy committed-snapshot entry points.


## 2026-07-18: X Present Routing Leaves The Visual Runtime

The production visual runtime no longer stores an `XServerFrontendProtocolRouter` or any X
Present completion, idle, fence, or disconnect proof counters. It emits reduced
`LivePresentFeedbackOutcome` values through an injected protocol-neutral sink. The separate
`XPresentSessionObserver` translates those records to X wire events and owns all session-proof
accounting; shutdown consumes the renderer disconnect report outside visual control. A direct
regression proves the runtime sink receives the paired reduced outcome unchanged.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 6,988 ms with 117 of 117 authority transactions, 8 ms input presentation, 40 submissions,
38 retirements, and zero cleanup debt. Classic GTK accepted exact physical text and pointer
selection, exited normally with `first_error=none`, and retired both outputs cleanly. The
remaining structural extraction is X authority-batch and resource translation plus session
supervision around the protocol-neutral runtime, followed by deletion of legacy committed-
snapshot entry points shared with the Wayland maintenance path.


## 2026-07-18: X Authority Batches Stop At The Production Boundary

The X session loop now translates each projected authority batch once into a protocol-neutral
production record containing Engine transactions and surface removals, renderer DMA-BUF and
fence registrations, Present submissions, and release handles. X resource IDs, client IDs,
protocol errors, and authority-specific CPU update records do not cross that boundary.
`PersistentBackendRuntime` no longer accepts `XAuthorityObservedTransactionBatch`; CPU and GPU
production entry points consume only the reduced production batch plus renderer updates.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 7,008 ms with 117 of 117 authority transactions, 7 ms input presentation, 42 submissions,
40 retirements, and zero cleanup debt. The guarded native mixed diagnostic translated and
exported one CPU plus one DMA-BUF layer with zero live sources, fences, or transactions. The
remaining Milestone 6 ownership work is moving the now-neutral visual control implementation
out of the CLI module and retiring its legacy committed-snapshot APIs.


## 2026-07-18: Backend Owns Production Intake Records

Backend-live now defines and exports the neutral authority batch, DMA-BUF registration, fence
registration, and Present submission records consumed by production visual control. The CLI
retains only the X-to-production translation function; it no longer defines the records or
their file-descriptor ownership shape. This makes the next runtime extraction a movement of
behavior behind an already backend-owned input contract rather than another protocol rewrite.
The full offline all-feature suite passes; runtime behavior is unchanged from the immediately
preceding strict QEMU and guarded native mixed evidence.


## 2026-07-18: Present Rebase Policy Moves Into Engine

The full-state Present generation rebase now lives beside `ProductionSessionCoordinator` in
`sophia-engine::runtime_driver`. The visual runtime no longer reaches back into a CLI library
module to reconcile skipped authority generations with the last visible Engine generation.
The former CLI module is only a compatibility re-export for its retained tests. The full
offline all-feature suite passes; this is a dependency-boundary change with no runtime
behavior change.


## 2026-07-18: Backend Owns The CPU Production Adapter

Backend-live now owns `LiveProductionCpuCycleAdapter`. It applies renderer updates after the
Engine commit, composes or coalesces from the immutable committed snapshot, creates native
frames for every output, invokes one narrow output-runtime callback, and returns reduced
composition timing and evidence. The CLI no longer implements `ProductionPresentationAdapter`
or defines the CPU production frame record; its remaining callback projects the snapshot and
invokes backend runtime/scanout objects pending their final owner extraction.

The full offline all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm
completed in 6,971 ms with 117 of 117 authority transactions, 7 ms input presentation, 40
submissions, 38 retirements, and zero cleanup debt. Classic and confined GTK accepted exact
physical text and pointer selection, exited normally with `first_error=none`, and retired both
outputs cleanly after 54-56 CPU compositions. The next extraction is GPU scheduling and the
concrete per-output runtime owner; legacy committed-snapshot entry points remain only for the
Wayland maintenance path and tests.


## 2026-07-18: Backend Owns Present Resource Admission

`LiveProductionPresentFeedbackCoordinator` now consumes the backend-owned production batch
directly to register DMA-BUF sources and fences and to process source/fence releases. The CLI
visual wrapper no longer clones file descriptors or sequences presentation-resource lifetime
admission. The full offline all-feature suite passes; behavior is covered by the immediately
preceding guarded native mixed and strict QEMU evidence.


## 2026-07-18: Backend Owns Present Scheduling State

Backend-live now owns `LiveProductionPresentScheduler`: queued and submitted Present state,
first-frame acquire delay, fence polling, bounded timeout rejection, controlled-rejection
proof policy, diagnostic triggering, and acquire/rejection counters. The CLI visual wrapper
asks for a reduced gate decision and supplies native scanout availability; it no longer owns
the scheduling tables or timing state. A backend regression proves delayed acquire admission
and one-shot controlled rejection with a registered DMA-BUF presentation.

The full offline all-feature suite passes. The rebuilt X13 guarded native mixed diagnostic
crossed the new scheduler, exported one CPU plus one DMA-BUF layer, and ended with zero live
sources, fences, or transactions. The remaining central Milestone 6 extraction is the
concrete per-output runtime owner and the legacy committed-snapshot APIs shared with Wayland.


## 2026-07-18: GTK QEMU Gate Now Proves Resize Redraw

The retained classic and confined GTK QEMU profiles previously passed input and native
presentation while reporting `surface_resize=disabled`, even though Milestone 5 requires a
CPU\/SHM redraw after an Engine-owned resize. Both guest profiles now request 640x360, and
the host harness rejects evidence unless the application record carries the complete semantic
tail: zero protocol errors, exact physical text, routed pointer selection, committed resize,
CPU\/SHM buffer path, native presentation, and clean teardown.

On the rebuilt X13 QEMU image, classic and confined Zenity each committed the resize with a
configure acknowledgement and changed pixels, accepted exact `sophia` input plus pointer
selection, exited normally with `first_error=none`, and retired both virtio-gpu outputs with
zero cleanup debt. Strict two-xterm also passed in 6,989 ms with 117 of 117 authority
transactions, 40 submissions, 38 retirements, and zero phase or cleanup debt. The remaining
Milestone 5 promotion gate is the deliberately operator-driven paired physical X13 capture.


## 2026-07-18: Production X Cursor Repaint Stops Replacing Engine State

The physical-pointer cursor repaint no longer composes frames in the outer X session loop or
calls the legacy committed-snapshot replacement entry point. A visual-runtime repaint method
reads the production coordinator snapshot, asks renderer-live to compose the cursor, creates
per-output frames, and submits them through the backend-owned output runtime set. The remaining
snapshot replacement API is named and called only by the Wayland maintenance adapter and its
regression; production X has no caller.

The full CLI all-feature suite passes. On the rebuilt X13 QEMU image, strict two-xterm completed
in 6,941 ms with 120 of 120 transactions, exact keyboard and pointer proofs, 42 submissions,
40 retirements, and zero cleanup debt. Resize-enabled classic and confined GTK accepted exact
text and pointer selection, committed 640x360 CPU\/SHM redraws, exited normally with
`first_error=none`, and cleanly retired both outputs. The guarded native mixed diagnostic
exported one CPU and one DMA-BUF layer with zero live sources, fences, or transactions.


## 2026-07-18: Mixed Diagnostic Contract Moves Behind Backend Boundary

The native mixed-export completion record and its reduced evidence schema now live in
backend-live beside the native scanout diagnostic that produces them. The CLI only downcasts
the backend error, prints the reduced record, and applies command-level pass criteria; it no
longer defines a renderer\/scanout result type inside session supervision. A backend regression
freezes the exact schema. The rebuilt guarded X13 diagnostic still exported one CPU and one
DMA-BUF layer and retired all sources, fences, and transactions. This removes one CLI-specific
dependency that pinned the remaining neutral visual-control implementation to
`live_session.rs`.


## 2026-07-18: Visual Runtime Intermediate Records Move To Backend

Backend-live now owns the prepared-authority record and reduced CPU production submission
record used between visual-control phases. The CLI no longer defines internal records carrying
Engine commits, active transactions, backend ticks, renderer composition evidence, or compose
timing. Together with the backend-owned mixed diagnostic contract, this leaves the visual
control implementation dependent only on types already owned by engine, renderer-live, and
backend-live, preparing the concrete wrapper movement without changing runtime behavior.


## 2026-07-18: Concrete Visual Control Leaves The CLI

Backend-live now owns `LiveProductionVisualRuntime`, including the production coordinator,
per-output runtime set, renderer transaction projection, CPU and GPU cycle entry points, Present
resource admission and scheduling, native submission and retirement, cleanup, and reduced
feedback routing. The `PersistentBackendRuntime` type and roughly 950 lines of implementation
are gone from `live_session.rs`. CLI code constructs the runtime, translates X batches, supervises
clients, records proof evidence, and requests high-level service; it no longer defines visual
state-machine behavior. A reduced diagnostics snapshot replaces direct CLI access to Present
resource and scheduler internals.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,916 ms with 120 of 120 transactions, 40 submissions, 38 retirements, exact keyboard and pointer
proofs, and zero cleanup debt. Resize-enabled classic and confined GTK passed exact text, pointer
selection, committed 640x360 CPU\/SHM redraw, normal exit, `first_error=none`, native presentation,
and clean teardown. Emergency recovery flushed all five routed chord events and completed in 167
ms. The guarded native mixed diagnostic exported one CPU and one DMA-BUF layer with zero live
resources. Milestone 6 ownership, fail-closed, and retained-gate migration items are complete.
The remaining exit item is moving asynchronous GPU-service and retirement trigger timing from
the CLI event loop into `sophia-engine::runtime_driver`.


## 2026-07-18: Production X Uses One Native Service Poll

`LiveProductionVisualRuntime::service_native` now owns the asynchronous native service order:
page-flip retirement and cleanup first, eligible queued Present work second, and pending native
frames last. It returns one reduced report with the optional backend tick and phase observations.
The production X event loop no longer inspects pending exporter frames or separately invokes
retirement, GPU scheduling, and native idle submission. Wayland retains its specialized
maintenance service because it correlates client buffer release to its own submission counters.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,986 ms with 117 of 117 transactions, 40 submissions, 38 retirements, and zero cleanup debt.
Resize-enabled classic and confined GTK passed exact input, pointer selection, committed resize
redraw, normal exit, `first_error=none`, native presentation, and clean teardown. The remaining
Milestone 6 exit gap is exact rather than structural: GPU Present prepare\/retire sequencing still
lives in backend visual control and must enter `sophia-engine::runtime_driver` before that module
is the only production visual coordinator.


## 2026-07-18: Full-State Present Preparation Enters Runtime Driver

`ProductionSessionCoordinator::prepare_full_state_present` now owns authority-generation rebasing
and Engine preparation against its committed snapshot. Backend visual control no longer reaches
through `coordinator.engine()` or independently selects the preparation baseline. The same
coordinator already owns matching-retirement application and suppresses feedback when that
baseline is stale, so both sides of the asynchronous prepared-commit gate now remain in
`runtime_driver`. The external regression deliberately supplies generation 99 and proves the
coordinator rebases and commits it against the visible generation.

The full offline all-feature suite passes. The rebuilt guarded X13 mixed path crossed the new
coordinator entry point, exported one CPU plus one DMA-BUF layer, and retired all sources, fences,
and transactions. The remaining Milestone 6 coordinator gap is asynchronous KMS service adapter
shape, not Engine Present preparation or retirement ownership.


## 2026-07-18: Runtime Driver Owns Asynchronous KMS Phase Order

`ProductionAsyncServiceCoordinator` consumes reduced in-flight, cleanup, queued-Present, and
pending-frame observations and requests at most the ordered Retire, SchedulePresent, and
SubmitPendingFrame phases. Backend-live executes those requests and feeds updated observations
back after each action; it no longer encodes asynchronous phase order. A runtime-driver regression
proves dynamic behavior: retirement unlocks Present scheduling, and a resulting in-flight frame
suppresses pending submission in the same service pass. Together with coordinator-owned CPU
cycles and full-state Present prepare\/retire gates, this closes the last Milestone 6 architecture
checkbox.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm in
6,973 ms with 117 of 117 transactions, 40 submissions, 38 retirements, and zero cleanup debt.
Resize-enabled classic and confined GTK passed exact input, pointer selection, committed resize
redraw, normal exit, `first_error=none`, native presentation, and clean teardown. The guarded
native mixed path exported one CPU and one DMA-BUF layer with zero live resources. Milestone 6
implementation is complete; its overall promotion remains coupled to the operator-driven paired
physical X13 GTK evidence still required by Milestone 5.


## 2026-07-18: Post-Milestone-6 Native Stability And Physical Evidence Audit

The documented unattended X13 native stability gate passed 10 of 10 release runs against the
runtime-driver-owned phase state machine. Every retained record passed exact terminal text,
changed pixels, native presentation, callback validation, and zero in-flight or cleanup debt.

The durable Milestone 5 physical GTK store was audited rather than assumed valid. Its classic
record ends at pointer readiness with zero routed pointer events and has no application-session
completion; its confined record is empty; recovery records `emergency=true`. Those artifacts
cannot satisfy the current paired verifier. The remaining daily-driver promotion action is a
fresh local-TTY run of `tools/live_session_milestone5_gtk_hardware_proof.sh`, followed by the
three-class aggregate verifier. It requires a person to arm the independent guard, type exact
text, and physically click each dialog, so it cannot be completed through unattended SSH or
QEMU without weakening the stated acceptance criterion.


## 2026-07-18: GTK Client Exit Hang And Post-Proof Completion Watchdog

The first X13 run with routed pointer buttons accepted exact physical `sophia` text, routed
the OK click, and presented the surface-removal frame, then held a blank screen until the
emergency chord restored the TTY. Reduction found a completion-phase deadline vacuum: once
the text proof completes and a button routes, the keyboard-sequence and pointer-selection
deadlines are disarmed and the global runtime budget intentionally stays out of input proofs,
so any stall after the proof loops without a bound. The loop exit requires the primary
client's reaped exit status; a toolkit that destroys its window but never exits leaves that
term false forever. On a bare text TTY without a session bus address, GTK finalization is the
prime suspect for the missing exit.

The session now bounds the post-removal wait: when the application proof surface is gone and
the client has not exited within five seconds, the loop fails closed with the exact exit-term
states instead of presenting blank frames. Application-proof clients launch under
`dbus-run-session --` when no bus address exists and the runner resolves on `PATH`, giving
the toolkit a bounded per-client bus that exits with the client. The first watchdog draft
armed on proof-complete-plus-button and falsely expired inside the QEMU click-then-submit
sequence; the retained trigger is surface removal, which is the actual abnormal state.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm,
and resize-enabled classic and confined GTK passed exact text, routed button selection,
committed 640x360 redraw, normal exit, `first_error=none`, native presentation, and clean
two-output retirement. Fresh paired physical X13 evidence remains the acceptance gate; if the
watchdog fires there, its record names the missing exit term.


## 2026-07-18: X13 GTK Blank Session Reduced To Tap Policy And Pointer Deadline

A fresh classic hardware run accepted exact physical `sophia` input, committed the 640x360
GTK resize, presented the software cursor, and routed sustained touchpad motion. It emitted no
pointer-button record, no application-session record, and no bounded-completion record before
the independent emergency chord restored the TTY cleanly. X13's libinput report confirmed that
the ELAN touchpad supports tapping but defaults tap-to-click to disabled.

The native path-based libinput owner now enables tap-to-click for every tap-capable admitted
device, verifies the applied state, and exports only reduced device/tap counts. The proof loop
now distinguishes motion observed/routed from button observed/routed. Its selection deadline
remains armed after cursor pixels change and ends only after both a routed button and pointer
pixel evidence; this closes the prior unbounded state where motion canceled the only pointer
deadline. Cursor repaint also fails closed if an application proof produces no composed layer
or only the bounded software-cursor footprint.

The full offline all-feature suite passes. The rebuilt X13 QEMU image passed strict two-xterm
in 6,880 ms with 19 of 19 input deliveries, 40 submissions, 38 retirements, and zero native
debt. Resize-enabled classic and confined GTK passed exact text, routed button selection,
normal exit, `first_error=none`, and clean two-output retirement. A bounded non-KMS smoke
against the real ELAN path reported `devices=2 tap_capable=1 tap_enabled=1` and completed its
xterm pixel proof. Paired physical X13 evidence remains the acceptance gate before GTK3
software promotion.
