# AGENTS.md - Sophia

Sophia is an XLibre-centered modern X11 session prototype. XLibre remains the
X11 authority; Sophia owns compositor-first input/rendering and an external WM
policy split.

Read these docs before changing code:

- `README.md` - project identity and original architecture diagram.
- `docs/architecture.md` - process boundaries and reference map.
- `docs/dod.md` - data-oriented design rules.
- `docs/style-guide.md` - Rust/C/XLibre implementation discipline.
- `todo.md` - current phased roadmap.

## Working Rules

1. Keep niri, picom, river, and XLibre as references at their correct
   boundaries. Do not turn Sophia into a fork of any of them.
2. Keep data passive. Types and packet structs should not grow hidden authority.
3. Add source in small crates or modules by subsystem: protocol, engine, bridge,
   portal, WM.
4. Start with mock/headless artifacts before XLibre patches.
5. Update `todo.md` and `docs/research-log.md` when a research question becomes
   a decision.

## Verification

Run from the repository root:

```sh
cargo test
```

Docs-only changes do not require a build.
