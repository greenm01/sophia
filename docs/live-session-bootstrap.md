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

The current `--proof` implementation assembles a one-shot proof from boundaries Sophia
already owns:

- bind a generated Sophia X Authority display for one terminal proof;
- launch one terminal client against that display;
- drain X Authority transaction batches into runtime;
- run the deterministic live composition/scanout path;
- run a second real xterm with bounded core key events and require changed
  terminal pixels;
- emit reduced status for authority, runtime, composition, keyboard, and known
  native-presentation/physical-input/persistence work.

It is not a persistent interactive session or hardware-visible proof yet. It
reports
`status=bootstrap_cpu_pixels_x11_keyboard_ready_native_presentation_pending`
when the terminal authority/runtime/composition and injected-keyboard pixel
proofs pass.

Explicit display binding, such as `--display=:77`, is still a target for the
persistent live session. The current proof-mode launcher intentionally uses the
same generated display path as the X Authority real-client smokes because low
display numbers can still stall in xterm palette/setup before text damage.

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

- hardware-prove the composed terminal frame through native GL/GBM scanout;
- extend `sophia-live-session` from one-shot proof mode to a persistent X
  Authority and live backend loop;
- route physical keyboard input into the focused X terminal surface;
- prove typed text reaches the terminal before running Codex inside Sophia.

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
