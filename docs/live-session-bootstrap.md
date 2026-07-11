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
  sophia-live-session --display=:177 --max-runtime-ms=2500 --inject-text=sophia
```

Passing evidence reports `status=bounded_complete`, matching authority batch,
backend tick, and runtime commit counts, nonzero composed pixels, and
`input_pixel_change=true`.

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
other compositor owns DRM master and must be stopped before this proof.

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
- physical keyboard routing to a focused X client is not yet operator-grade.

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

- pass the strict persistent GBM/KMS proof from an exclusive TTY;
- route physical keyboard input into the focused X terminal surface;
- prove operator-typed text reaches terminal pixels before running Codex inside
  Sophia.

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
