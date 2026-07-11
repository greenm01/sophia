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

The first usable live session launcher should become a single command, for
example:

```sh
cargo run --offline -q -p sophia-cli --features atomic-scanout-live -- sophia-live-session --display=:77 --terminal=xterm
```

That command does not exist yet. Its first implementation should assemble only
the boundaries Sophia already owns:

- run live preflight for output/input/backend readiness;
- start a persistent Sophia X Authority display, for example `:77`;
- launch one terminal client against that display;
- drain X Authority transaction batches into runtime;
- run the live backend session loop;
- submit rendered frames through the existing scanout path;
- emit reduced status lines for authority, runtime, scanout, and input health;
- shut down cleanly from the outside control plane.

## First Terminal Milestone

The first app target is `xterm`, not GTK. Codex-in-session requires terminal
rendering and keyboard routing before DBus, portals, or toolkit dialogs matter.

Current evidence:

- `x-authority-xterm-smoke` reaches setup/lifecycle protocol with
  `first_error=none`;
- `x-authority-xterm-render-smoke` reaches text drawing with
  `first_error=none` and commits terminal `SurfaceTransaction` values;
- the render proof uses `xterm -cm -dc` so the smoke tests terminal drawing
  instead of spending the proof window on 256-color palette setup;
- physical keyboard routing to a focused X client is not yet operator-grade.

Current terminal proof:

```sh
cargo run --offline -q -p sophia-cli -- x-authority-xterm-render-smoke
```

The passing reduced evidence is `outcome=proof_window_killed`, `requests=232`,
`opcode_count=28`, `transactions=4`, `runtime_committed=4`,
`runtime_surfaces=4`, and `first_error=none`.

Keep this path probe-driven: add only the opcode, reply, event, drawing, or
resource behavior the real xterm stream demands.

Next live-session blocker:

- build a persistent `sophia-live-session` launcher around the passing terminal
  render proof;
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
