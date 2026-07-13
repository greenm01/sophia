# Live Session Bootstrap

This document records the path for bootstrapping a Sophia live session while
keeping a reliable development control plane.

## Control Plane

Do not run Codex inside the first Sophia live session. Keep Codex, git, logs,
and recovery commands outside Sophia until Sophia can render an interactive
terminal and route keyboard input to it.

Recommended operator layout:

- TTY1 or SSH/tmux: Codex and recovery control plane.
- TTY3: Sophia live session experiments.
- Optional host X/Wayland session: reference clients and documentation lookup.

This keeps the development agent available if scanout, input, or session
supervision fails.

## Target Shape

The first bootstrap live session launcher is now a single command:

```sh
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- sophia-live-session --terminal=xterm
```

This default mode binds `:77` unless `--display=:NUMBER` is supplied, launches
one xterm, and keeps the X Authority server, live backend runtime, and composed
CPU scene alive until xterm exits or the outside control plane stops Sophia.
It refuses an active display socket rather than replacing it. Keep the outside
TTY control plane. Physical keyboard input can be enabled with one or more
explicit comma-separated event nodes:

```sh
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- \
  sophia-live-session --display=:77 \
  --input-devices=/dev/input/by-path/platform-i8042-serio-0-event-kbd
```

The backend opens those nodes through libinput, reduces events to protocol-
neutral packets, asks Engine's seat focus state for a committed surface, then
maps accepted evdev keys to core X events. Reduced status reports only event and
routed-key counts, never device paths.

A bounded persistence/input regression is:

```sh
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- \
  sophia-live-session --display=:177 --max-runtime-ms=6000 --inject-text=sophia
```

For an external libinput proof, replace `--inject-text` with
`--expect-physical-text=sophia` and pass `--input-devices=`. The session emits a
`sophia_live_session_input ... status=ready` marker after pixels and focus are
stable, pauses bounded tick accounting for at most five seconds, and succeeds
only after routed physical keys change later terminal pixels.

Add `--expect-physical-pointer` to extend that bounded proof. After keyboard
pixels stabilize, the session emits a physical-pointer readiness marker and
requires libinput pointer events to pass Engine surface-only hit-testing and
produce another client pixel change. X Authority resolves the routed Sophia
surface to its protocol-owned X window; Engine does not receive the XID.

Passing evidence reports `status=bounded_complete`, matching authority batch,
backend tick, and runtime commit counts, nonzero composed pixels, and
`input_pixel_change=true`.

## Real Legacy-WM Policy Session

Run the first real xmonad-backed operator session from a dedicated TTY with:

```sh
tools/run_sophia_xmonad_session.sh
```

The wrapper resolves/builds xmonad, builds Sophia and the generic X11 WM
bridge, verifies exclusive TTY/DRM/input conditions, temporarily stops keyd,
and restores it on exit. Sophia supervises the bridge as a generic WM policy
process. Xmonad sees only a synthetic private display; the real xterm remains
connected to Sophia X Authority. Engine validates and applies xmonad placement,
stacking, and focus, while physical input and scanout remain Sophia-owned.

The live compatibility gate allows xmonad to request a real client size. Sophia
keeps the previous geometry and pixels until X Authority publishes the matching
replacement buffer, then commits the resized pixels and placement together.
Later drawing requests carry tightly packed damage patches instead of cloning a
fullscreen CPU buffer for every glyph. The headless xmonad proof requires one
configure acknowledgement before layout commits.

The operator wrapper launches a clean interactive `/bin/sh` so this milestone
tests deterministic fixed-font ASCII rather than user prompt/Xft compatibility.
Exit the shell normally, or stop the session from another TTY with:

```sh
tools/stop_sophia_xmonad_session.sh
```

The wrapper owns the session process group and restores `keyd` during either
shutdown path.

## Kitty Compatibility Session

Kitty needs XKB plus a real OpenGL window path that Sophia X Authority does not
yet implement. The temporary compatibility launcher runs a private XLibre dummy
server with software GL and imports Kitty through the existing XComposite
authority boundary:

```sh
tools/install_sophia_session.sh
```

After that one-time install, switch to a dedicated TTY and run `sophia`. The
launcher builds Sophia in release mode, verifies KMS ownership, and stops and
later restores `keyd`. Before changing the TTY mode it starts an independent libinput guard and
requires one press-and-full-release of Ctrl-Alt-Backspace. That first chord
proves the selected keyboard path and arms recovery. Once Kitty is visible,
press Ctrl-Alt-Backspace again to end the session and restore the TTY, or close
Kitty normally or run `sophia stop` from an outside control plane.

The guard remains separate from the live session process and therefore still
terminates a wedged input/focus loop. Shutdown gives Sophia a bounded graceful
window, then kills stuck children before restoring the saved KD mode, termios,
and `keyd` state. Ctrl-Alt-Fn live switching remains unsupported because Sophia
does not yet implement the required VT/DRM suspend-and-reacquire lifecycle.
Reduced logs persist across reboot under
`${XDG_STATE_HOME:-$HOME/.local/state}/sophia/kitty-session`; each log retains
the latest and one previous run without recording keycodes, text, or device
paths.

For the bounded Kitty latency gate, keep the outside control plane connected
and launch:

```sh
sophia --max-runtime-ms=30000 --expect-physical-text=sophia --exit-after-input-proof
```

Arm the guard as prompted, then type `sophia` followed by Enter once Kitty is
visible. After the TTY is restored, verify the persistent evidence with:

```sh
research/xlibre/tools/verify_xlibre_kitty_latency_evidence.sh \
  "${XDG_STATE_HOME:-$HOME/.local/state}/sophia/kitty-session/session.log"
```

The launcher disables Kitty's remembered window size, requests a 1280x720 X11
window, and centers the first compatibility surface without scaling it. The
device-less XLibre server is explicitly loaded with the evdev XKB rules before
Kitty starts; the launcher verifies Up, Left, Right, and Down at keycodes 111,
113, 114, and 116 so physical navigation keys match Sophia's evdev input.
The gate requires an unfallbacked MIT-SHM capture path, no readback larger than
1280x720 XRGB, no libinput processing-lag warning, clean native-scanout drain,
and input pixels presented within 100 milliseconds of the final proof key.
Physical events are collected on a bounded worker and drained before capture;
CPU composition uses borrowed buffers and row copies, while native scanout
retains its EGL context, shaders, texture, and vertex buffer across frames.
Schema 9 additionally rejects composition above 25 milliseconds, MIT-SHM
capture above 30 milliseconds, upload above 50 milliseconds, page-flip above
100 milliseconds, queue dwell above 25 milliseconds, or native target/pipeline
recreation at the fixed proof size.
Ordinary XGetImage remains available as a degraded diagnostic path, but its
evidence is intentionally rejected by this interactive gate.

XLibre receives no physical devices and does not own scanout. Engine routes
physical keys to the focused opaque surface; the compatibility adapter delivers
them through a private XTEST connection. This is a usability bridge, not the
long-term X Authority GPU path and not an Engine dependency.

On an exclusive TTY with no other DRM master, persistent native presentation is
enabled by the gated `--native-scanout` flag. The bounded hardware wrapper
starts xterm, injects terminal input, keeps GBM/KMS and page-flip ownership in
the same session loop, and verifies its reduced evidence:

```sh
tools/live_session_persistent_hardware_proof.sh
```

The verifier requires at least one nonzero terminal frame export, successful
submit and retirement, no rejected page-flip callbacks, no dropped authority
batches, and no in-flight frame or cleanup debt at shutdown. A running River or
other compositor owns DRM master and must be stopped before this proof. The
persistent path performs a blocking initial modeset, retains the displayed
framebuffer until a later frame replaces it, and uses event-driven nonblocking
page flips for steady updates. Both the bounded proof and a 30-second TTY3 run
pass. Use an isolated QEMU `virtio-gpu` guest for repeated development and keep
the physical TTY proof for final driver evidence.

The isolated guest harness is now the default repeated native-session path.
On Void, install QEMU once if it is missing:

```sh
sudo xbps-install -S qemu-system-amd64
```

Then build the direct-kernel guest initramfs and run the proof as an ordinary
user:

```sh
tools/build_qemu_session_initramfs.sh
tools/qemu_session_harness.sh
```

To exercise the same two-xterm routing shape as the physical KMS gate, rebuild
the initramfs after a Sophia change and run:

```sh
SOPHIA_QEMU_TWO_XTERM=1 tools/qemu_session_harness.sh
```

That optional mode requires two composed terminal layers and complete reduced
X11 input-flush evidence; it does not replace the final physical DRM-master
proof.

The guest has no disk and no network device. QEMU stays headless, exposes its
display only to an unconnected Unix-domain VNC sink, and emulates virtio-gpu
and a virtio keyboard. It does not pass through or open host DRM/input devices
and does not switch a host VT. The guest owns its own DRM card, starts udev,
opens the virtual input nodes through libinput, runs exactly 300 session-loop
ticks, and powers off. The strict verifier requires clean native submission,
page-flip retirement, xterm pixel export/input change, no callback rejection,
and no in-flight frame or cleanup debt. Evidence defaults to
`/tmp/sophia-qemu-session.log`; build artifacts stay under ignored `.qemu/`.

The destructive TTY3 terminal-content presentation proof is:

```sh
tools/live_session_content_hardware_proof.sh
```

It composes real xterm pixels at the selected KMS mode, queues that exact frame
for native GL/GBM export, and requires matching requested/exported checksums plus
the existing submit-to-page-flip-retire verifier. Current evidence passes at
1920x1200 with nonzero terminal pixels and no cleanup debt.

The separate `--proof` implementation assembles a one-shot proof from
boundaries Sophia already owns:

- bind a generated Sophia X Authority display for one terminal proof;
- launch one terminal client against that display;
- drain X Authority transaction batches into runtime;
- run the deterministic live composition/scanout path;
- run a second real xterm with bounded core key events and require changed
  terminal pixels;
- emit reduced status for authority, runtime, composition, keyboard, and known
  native-presentation/physical-input/persistence work.

It is not a hardware-visible proof. It reports
`status=bootstrap_cpu_pixels_x11_keyboard_ready_native_presentation_pending`
when the terminal authority/runtime/composition and injected-keyboard pixel
proofs pass.

Proof mode intentionally uses a generated display and rejects `--display`.
Explicit display ownership belongs to persistent mode.

## First Terminal Milestone

The first app target is `xterm`, not GTK. Codex-in-session requires terminal
rendering and keyboard routing before DBus, portals, or toolkit dialogs matter.

Current evidence:

- `x-authority-xterm-smoke` reaches setup/lifecycle protocol with
  `first_error=none`;
- `x-authority-xterm-render-smoke` reaches text drawing requests with
  `first_error=none`, commits terminal `SurfaceTransaction` values, and emits
  inspectable XRGB8888 glyph pixels;
- the render proof uses `xterm -cm -dc` so the smoke tests terminal drawing
  instead of spending the proof window on 256-color palette setup;
- `x-authority-xterm-input-smoke` proves bounded core key events change a later
  real xterm buffer generation;
- physical keyboard routing to a focused X client passes the exact AMD TTY
  proof; the combined real-xmonad physical operator gate remains open.

Current terminal proof:

```sh
cargo run --offline -q -p sophia-cli -- x-authority-xterm-render-smoke
```

Request and transaction counts vary with xterm startup timing. Passing evidence
requires `first_error=none`, committed authority transactions, at least one CPU
buffer, and nonzero pixel bytes.

Keep this path probe-driven: add only the opcode, reply, event, drawing, or
resource behavior the real xterm stream demands.

Next live-session blockers:

- route physical keyboard input into the focused X terminal surface;
- prove operator-typed text reaches terminal pixels before running Codex inside
  Sophia;
- inject QMP virtual-keyboard events and prove they change xterm pixels through
  the physical libinput route; the guest already opens the virtual input nodes.

## Input Milestone

After xterm renders, route keyboard input:

- backend-live observes reduced libinput readiness;
- the session loop converts accepted keyboard packets into engine/session input
  events;
- Sophia Engine chooses the focused X surface;
- X Authority receives a bounded key event request for that surface;
- an interactive smoke proves typed text appears in the terminal path.

Only after terminal rendering and keyboard routing pass should we run:

```sh
DISPLAY=:77 xterm -geometry 120x36 -title "Sophia Codex" -e codex
```

## Deferred

The GTK/zenity path remains valuable, but it is not the Codex bootstrap blocker.
GTK currently reaches startup protocol with `first_error=none` under
`dbus-run-session`, then stops at host portal/display state and missing XInput2
coverage. Continue that path after the terminal control loop is usable.
