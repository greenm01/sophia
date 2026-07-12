# XLibre Research Archive

This tree freezes Sophia's former XLibre prototype after the production Kitty
path moved to Sophia's native Wayland authority. Nothing under this directory
is a Cargo workspace member, installed launcher dependency, release gate, or
supported runtime path.

Contents:

- `sophia-x-bridge/`: the former XComposite/Damage bridge crate;
- `cli/`: removed bridge-only CLI commands and the compatibility session;
- `patches/`: the routed-input X server experiment;
- `tools/` and `tools/fixtures/`: retired smokes, verifiers, and evidence;
- `docs/`: the original protocol design and the final regression map.

The archive is retained for design provenance. Active replacements and the
tests that protect their architectural lessons are listed in
`docs/xlibre-prototype-regression-map.md`.
