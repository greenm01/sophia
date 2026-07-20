# Wayland Research Archive

This tree freezes Sophia's former Smithay-backed Wayland frontend after the
project adopted an X-centric product direction. Nothing under this directory
is a Cargo workspace member, installed launcher dependency, release gate, or
supported runtime path.

The prototype proved that Sophia Engine's surface transactions, routed input,
buffer ownership, and presentation boundaries were not inherently X-shaped.
It is retained for design provenance and for a possible future compatibility
translator; it is not a promise of future Wayland support.

Contents:

- `sophia-wayland-authority/`: the former protocol frontend and its reducer
  tests;
- `cli/`: the removed application-session command;
- `tools/` and `tools/fixtures/`: retired Kitty SHM and controlled linear
  DMA-BUF proofs and evidence verifiers;
- `docs/`: the final subsystem contract and maintenance status.

The last retained results were real Kitty over SHM with Engine-routed input
and KMS presentation, plus a controlled linear DMA-BUF first-frame and
300-frame lifecycle proof. Protocol-specific configure acknowledgements, frame
callbacks, and buffer release remain historical behavior rather than current
Sophia contracts.

Active equivalents protect the architectural lessons:

- `sophia-protocol` and `sophia-engine` transaction tests cover
  protocol-neutral surface IDs, generation checks, readiness, atomic geometry
  and pixels, and surface removal;
- the native X session gates cover routed input, SHM pixels, presentation,
  failure recovery, and frontend teardown;
- the native X DRI3/Present gates cover DMA-BUF ownership, fences, page-flip
  feedback, and resource retirement.

The retirement commit's Git history preserves Wayland-specific code formerly
embedded in the shared CLI live-session module and backend maintenance adapter.
