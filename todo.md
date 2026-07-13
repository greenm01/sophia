# Sophia Active Roadmap

Sophia is a research prototype. This file tracks only active work and the next
major milestones. Completed work lives in `docs/roadmap-history.md`; detailed
evidence lives in `docs/research-log-archive.md`.

Roadmap rule: keep this file short. A completed item leaves this file when the
next milestone becomes active.

---

## Active Milestone: Native Kitty Presentation Architecture

Current truth:

- The production CLI and installed launcher no longer link, select, or start the
  XLibre bridge. Its frozen sources and evidence live outside the workspace in
  `research/xlibre`.
- A real Kitty 0.47.4 process connects over native Wayland with `DISPLAY`
  removed and software GL, commits SHM buffers, and produces changing nonzero
  pixels through Sophia's protocol-neutral Engine path.
- The live authority supports ordered pipelined commits, xdg configure/ack,
  frame callbacks, buffer release, keyboard/pointer seat delivery, SHM, and
  bounded linear DMA-BUF admission.
- The non-modesetting hardware preflight finds one openable atomic-capable card
  with a connected scanout target and the required atomic properties.
- The guarded native SHM session now exits cleanly on hardware with keyboard
  routing, real KMS submissions, page-flip retirement, and no recovery debt.

Exit criteria:

- [x] Make Engine's committed surface snapshot the single authority: native
  presentation must consume it directly and must not replay Wayland authority
  transactions into a second Engine state. Verified on hardware with a clean
  native Kitty exit after transaction and surface-destruction teardown.
- [x] Add a bounded presentation scheduler: at most one retained pending frame
  per surface, explicit page-flip-to-release ownership, and normal teardown
  ordering. Verified on hardware with 26 Kitty transactions coalesced into 14
  SHM frames, clean native teardown, and no scanout failures.
- [ ] Retain SHM-backed composition resources and coalesce damage so the
  guarded hardware path remains below 100 ms input-to-presentation latency.
- [x] Prove a real software-rendered native Wayland Kitty toplevel handles a
  compositor configure, keeps its old size live until ack, then commits changing
  nonzero pixels at the requested size.
- [ ] Prove the guarded native session accepts exact text/navigation and pointer
  input, meets the 100 ms presentation budget, exits cleanly, and restores its
  TTY and prior `keyd` state.
- [x] Make the installed session launch an arbitrary Wayland client without an
  X server, keeping Kitty confined to acceptance tooling.
- [x] Remove XLibre/Xorg launch paths and production dependencies; retain the
  bridge only as frozen source in the non-workspace research archive.
- [x] Add native KMS presentation wiring for the Wayland session while
  preserving the existing independent TTY recovery guard.

## Next Milestone: Native DMA-BUF Hardware Evidence

- [x] Advertise only bounded single-plane XRGB/ARGB linear formats and validate
  dimensions, modifier, plane count, and stride before admitting an opaque
  `DmaBuf` handle.
- [ ] Validate and harden native EGL import of admitted DMA-BUFs on hardware;
  it stays isolated behind `--experimental-dmabuf` until the single-authority
  presentation scheduler has a passing first-frame KMS proof.
- [ ] Prove hardware Kitty remains within the presentation budget on that path
  while SHM remains available for software clients.

## Deferred

- VRR activation evidence waits for hardware reporting `vrr_capable=1`; the
  current panel cannot prove it.
- Dedicated-TTY xmonad visual evidence and a second legacy-WM smoke no longer
  block the protocol-authority path.
- GTK/XInput2/zenity, clipboard, drag-and-drop, optional Wayland protocols,
  concurrent X clients, and broader desktop compatibility resume after the
  native terminal path is stable.
