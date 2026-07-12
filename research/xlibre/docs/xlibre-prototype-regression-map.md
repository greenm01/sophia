# XLibre Prototype Regression Map

Status: retired and archived. XLibre is not a Sophia workspace member or
runtime option. This map records the active Sophia-owned replacements that made
each prototype dependency removable.

| Archived lesson | Active replacement evidence |
| --- | --- |
| XComposite/Damage updates become immutable visual transactions | `sophia-x-authority` tests `shm_and_core_draw_updates_become_ready_cpu_buffer_transactions`, `present_pixmap_update_becomes_ready_surface_transaction`, and `cpu_buffer_patches_materialize_and_resize_replacements_keep_generation_order` |
| Namespace-scoped resource and event lookup | `sophia-x-authority` tests `resource_lookup_is_namespace_scoped`, `event_subscriptions_do_not_cross_namespaces`, and `drawing_updates_fail_closed_for_cross_namespace_or_unknown_windows` |
| Cross-namespace selection mediation | `sophia-x-authority` selection tests plus `sophia-portal` clipboard tests covering approval generation, denial, revocation, and bounded handoff |
| Engine-selected input target with authority-owned delivery | `sophia-engine` routed-input/focus tests and `sophia-wayland-authority` seat delivery; Engine requests contain only `SurfaceId` and transformed coordinates |
| Atomic attach/damage/commit and delayed release | `sophia-wayland-authority` reducer tests covering configure/ack, pipelined commits, presentation callbacks, detach, destruction, and buffer release |
| Real GPU-only terminal compatibility | `tools/wayland_kitty_smoke.sh` and `tools/wayland_kitty_hardware_proof.sh` use native Wayland with no X server |
| Interactive capture/readback latency | Removed by native SHM and direct EGL DMA-BUF paths; `tools/audit_no_xlibre_runtime.sh` rejects reintroduction into the live dependency graph |

The retired shared-memory routed-input ring, XTEST adapter, Xnamespace patch,
XComposite readback, and XLibre keymap setup have no active runtime successor
because Sophia-owned authorities make those bridge mechanisms unnecessary.
