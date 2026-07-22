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
- Optional host graphical session: reference clients and documentation lookup.

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
pixels stabilize, the session centers a compositor cursor on the focused
surface, submits that cursor frame, and emits pointer readiness only after the
matching presentation is observed. Return remains suppressed until a physical
pointer button routes. Pointer events must pass Engine surface-only hit-testing
and produce another client pixel change. If the application surface disappears
before selection, the proof fails and restores the session instead of leaving
an empty frame active. X Authority resolves the routed Sophia surface to its
protocol-owned X window; Engine does not receive the XID.

Passing evidence reports `status=bounded_complete`, matching authority batch,
backend tick, and runtime commit counts, nonzero composed pixels, and
`input_pixel_change=true`.

## Real Legacy-WM Policy Session

Run the first real xmonad-backed operator session from a dedicated TTY with:

```sh
~/scripts/start-sophia-tty3
```

The TTY3 launcher verifies the operator is on `/dev/tty3`, resolves or builds
xmonad, temporarily stops a runit-managed LightDM/Xorg session, and restores it
on exit. When several keyboard or pointer devices exist, it asks the operator
which ones Sophia should own before stopping LightDM. It refuses to stop an
unknown Xorg session and retains launcher output in
`/tmp/sophia-tty3-launch.log`. The supervised wrapper builds optimized Sophia
and generic X11 WM bridge binaries, verifies exclusive TTY/DRM/input
conditions, temporarily stops keyd,
and restores it on exit. Sophia supervises the bridge as a generic WM policy
process. Xmonad sees only a synthetic private display; Kitty remains connected
to Sophia X Authority. Engine validates and applies xmonad placement,
stacking, and focus, while physical input and scanout remain Sophia-owned.

The live compatibility gate allows xmonad to request a real client size. Sophia
keeps the previous geometry and pixels until X Authority publishes the matching
replacement buffer, then commits the resized pixels and placement together.
Later drawing requests carry tightly packed damage patches instead of cloning a
fullscreen CPU buffer for every glyph. The headless xmonad proof requires one
configure acknowledgement before layout commits.

The operator wrapper starts with an empty xmonad desktop and registers Kitty as
the terminal action. `Super+Enter` launches Kitty and `Super+Shift+Q` logs out.
Stop the session from another TTY, from the Sophia checkout, with:

```sh
tools/stop_sophia_xmonad_session.sh
```

The wrapper owns the session process group and restores `keyd` during either
shutdown path.

## Native Session Evidence

Native X11 applications connect directly to the Sophia X Server Frontend. The
production launcher uses neither XLibre nor Wayland; both former compatibility
frontends are historical evidence under `research/`.

The live command drives the production Engine/session coordinator, persistent
CPU and DMA-BUF renderer, backend retirement, external WM policy, and native
KMS path documented in `architecture.md`.

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
pass. Use an isolated QEMU `virtio-gpu` guest for milestone acceptance; physical
TTY proofs are optional driver-compatibility diagnostics.

Persistent hardware proofs compile an optimized binary before taking DRM/KMS
ownership and then invoke `target/release/sophia` directly. This keeps compiler
time off the native display and prevents the debug CPU compositor from turning
xterm's incremental startup draws into a long white screen. The identical
headless two-xterm workload measured 605 milliseconds end to end with a
4-millisecond maximum composition after optimization; the earlier debug KMS run
needed 10,784 milliseconds and reached 120 milliseconds per composition.
The optimized two-client launch also admits one primary transaction before the
secondary client starts and composes the Engine-focused surface above
overlapping clients. This prevents faster startup from making initial focus or
visible input evidence depend on client scheduling and surface-ID order.
Dedicated-KMS evidence now passes in 1,487 milliseconds with a 10-millisecond
maximum composition, 23-millisecond input-to-presentation latency, and all 14
X11 input events flushed.
`tools/live_session_two_xterm_hardware_proof.sh` applies a stricter specialized
gate: the complete startup-through-echo proof must finish within 2,000
milliseconds and no CPU composition may exceed 25 milliseconds. Positive
integer overrides are available through
`SOPHIA_TWO_XTERM_STARTUP_BUDGET_MSEC` and
`SOPHIA_TWO_XTERM_COMPOSE_BUDGET_MSEC`; do not raise them to admit a regression.

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

For the complete unattended Milestone 5 acceptance gate, including strict
two-xterm, emergency recovery, and both GTK profiles, run:

```sh
tools/qemu_milestone5_acceptance.sh
```


To exercise the same two-xterm routing shape as the physical KMS gate, rebuild
the initramfs after a Sophia change and run:

```sh
SOPHIA_QEMU_TWO_XTERM=1 tools/qemu_session_harness.sh
```

That mode requires two composed terminal layers and complete reduced X11
input-flush evidence. The aggregate Milestone 5 runner includes it
automatically.

The guest has no disk and no network device. QEMU stays headless, exposes its
display only to an unconnected Unix-domain VNC sink, and emulates virtio-gpu
and a virtio keyboard. It does not pass through or open host DRM/input devices
and does not switch a host VT. The guest owns its own DRM card, starts udev,
opens the virtual input nodes through libinput, runs exactly 300 session-loop
ticks, and powers off. The strict verifier requires clean native submission,
page-flip retirement, xterm pixel export/input change, no callback rejection,
and no in-flight frame or cleanup debt. Evidence defaults to
`/tmp/sophia-qemu-session.log`; build artifacts stay under ignored `.qemu/`.

### Optional Remote Hardware Target

When the development workstation must keep its graphical session, use a second
machine as the dedicated hardware target. The workstation remains the editing
and control station; the target must release its own graphical compositor
before a physical DRM/KMS proof.

Configure any existing SSH host alias and stage the current working tree:

```sh
SOPHIA_REMOTE_HOST=my-test-machine tools/remote_target.sh stage
```

The helper synchronizes source without Git metadata or generated artifacts,
builds the optimized live binary remotely, and prints the physical proof
command. That command must still be entered at the target's dedicated local
text TTY. The helper does not bypass local-TTY, active-compositor,
physical-input, or recovery checks.

Inspect the target without taking DRM master, retrieve evidence, or run the
isolated QEMU integration gate with:

```sh
SOPHIA_REMOTE_HOST=my-test-machine tools/remote_target.sh status
SOPHIA_REMOTE_HOST=my-test-machine tools/remote_target.sh fetch-evidence
SOPHIA_REMOTE_HOST=my-test-machine tools/remote_target.sh qemu
```

`SOPHIA_REMOTE_DIR` selects a deployment path below the remote home and defaults
to `dev/sophia-stack-target`, keeping an existing clone untouched. Synchronizing
does not delete target-only files. `SOPHIA_REMOTE_EVIDENCE_DIR` selects the
local destination; retrieved logs default to ignored
`.evidence/remote-target/`.

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
