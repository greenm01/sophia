# Sophia X11 Compatibility Matrix

**Role:** current native-X compatibility evidence and admission record.

This matrix is the admission record for the **Sophia X Server Frontend**. It
answers a deliberately narrow question: which real X11 client behavior has a
reproducible, bounded proof through Sophia's own X11 server frontend?

It is not an Xorg/XLibre conformance claim. A `proven` entry means the listed
probe completes its defined proof window with `first_error=none`; it does not
imply that nearby X11 requests, extensions, window-manager conventions, or
unlisted application modes work. `startup only` means the application reaches
the stated protocol stage without a client-visible X error but has no rendered
desktop proof yet.

The native frontend is always the subject of this matrix. XLibre is a retired
prototype and deferred possible provider; its behavior cannot promote an entry
here.

## Evidence Levels

| Level | Meaning |
| --- | --- |
| `wire` | Deterministic decoder/dispatcher fixture proves one bounded X11 request, reply, event, or error shape. |
| `client` | A real client completes its bounded protocol proof with `first_error=none`. |
| `engine` | The real client produces a reduced Sophia transaction that commits through Engine state. |
| `hardware` | A bounded one-client proof reaches Engine-owned native presentation and physical input, but does not yet meet the full session-promotion gate. |
| `session` | The client reaches normal startup, routed input, resize, presentation, and teardown backed by Sophia Engine and KMS. |

The paired Milestone 3 two-xterm entry meets the complete CPU-backed `session`
promotion gate under both classic-shared and fresh confined profiles. It is the
current baseline for physical input, Engine-derived output, RandR updates,
configure-plus-pixels resize, KMS presentation, and teardown. Other entries
retain only their listed evidence level. In particular, standard DRI3/Present
`vkcube` transport is `engine` evidence until the imported Vulkan pixels pass
mixed composition, page-flip-driven feedback, and the full session gate.

## Baseline Matrix

| Client or class | Reproducible command | Proven X11 surface | Evidence | Current status and next gate |
| --- | --- | --- | --- | --- |
| Setup parser and admission | `cargo test --offline -q -p sophia-x-authority --test x11_wire` plus `cargo test --offline -p sophia-cli --features atomic-scanout-live live_xauthority_file_is_owner_only_valid_and_removed_on_drop -- --exact` | byte order, bounded setup fields, setup success/failure encoding, MIT-MAGIC-COOKIE-1 gating/provenance, peer-credential denial, concurrent identity allocation, revocation on disconnect/error/supervisor command, peer-preserving cleanup, and owner-only Xauthority removal | `wire` | `proven`; each live classic session publishes a fresh cookie and requires same-UID peers, while the live launcher can explicitly allocate a confined group. Independently credentialed groups on one listener remain before a general local session. |
| Raw core protocol | `cargo run --offline -q -p sophia-cli -- x-authority-x11-smoke` | atoms, property read/write, create/map window, core events | `client` | `proven`; grow only from a captured missing request. |
| Rust X11 client | `cargo run --offline -q -p sophia-cli -- x-authority-x11rb-smoke` | client-compatible setup, root/visual facts, atom/property/window flow | `client` | `proven`; setup assigns disjoint client XID ranges, concurrent classic clients retain same-namespace access, and policy-assigned confined clients reject foreign map, property, selection-owner, selection-requestor, and event-mask operations without metadata leakage or input redirection. Core grabs, XGE/XI2 delivery, XKB map/state/name, and deterministic parent/stack-based focus propagation are implemented; retained scene-restack evidence remains open. |
| Core selections and portal mediation | `cargo test --offline -q -p sophia-x-authority --test x11_wire cross_namespace_executor_installs_property_and_notifies_requestor -- --exact` plus `cargo test --offline -q -p sophia-portal --test socket` | same-namespace `PRIMARY`, owner replacement and `SelectionClear`; cross-namespace `CLIPBOARD`, authority-private source proxy, broker grant/payload correlation, requestor property install, `TARGETS`, UTF-8 text, and native failure notification | `wire` | `proven`; allowed, denied, stale, expired, disconnected, unsupported, and executor-failure paths are bounded and fail closed. Large `INCR`, XFixes selection notifications, and Xdnd remain later compatibility work. |
| `xdpyinfo` | `cargo run --offline -q -p sophia-cli -- x-authority-xdpyinfo-smoke` | root screen, extension discovery, root properties, focus, GC cleanup | `client` | `proven`; do not turn its empty extension list into an extension-completeness claim. |
| Minimal Xlib lifecycle | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-smoke` | normal Xlib create/property/map/destroy path | `client` | `proven`; retain as the first C/Xlib regression. |
| Core drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-drawing-smoke` | GC lifecycle and `PolyFillRectangle` reduced to a ready transaction | `engine` | `proven`; visual output still needs session evidence. |
| Software image upload | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-put-image-smoke` | bounded `PutImage` CPU-buffer update and ready transaction | `engine` | `proven`; retain buffer ownership/release requirements when real SHM import arrives. |
| Historical private explicit handoff | `cargo run --offline -q -p sophia-cli -- x-authority-present-pixmap-smoke` | `SOPHIA-PRESENT` query and bounded pixmap transaction | `engine` | `proven` only as a retained prototype regression; standard DRI3/Present supersedes it and no application promotion may depend on it. |
| Root inspection | `cargo run --offline -q -p sophia-cli -- x-authority-xwininfo-root-smoke` | root attributes, geometry, tree, coordinate translation | `client` | `proven`; live setup and later topology facts are Engine-derived, while the paired Milestone 3 row carries the dynamic RandR and resize session evidence. |
| Root properties | `cargo run --offline -q -p sophia-cli -- x-authority-xprop-root-smoke` | root property enumeration and bounded reads | `client` | `proven`; broaden only when a client captures a required atom/property pattern. |
| Root mutation | `cargo run --offline -q -p sophia-cli -- x-authority-xsetroot-name-smoke` | root name/property mutation, focus, GC, extension-query flow | `client` | `proven`; no Engine pixels are expected. |
| Simple Xaw drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xlogo-smoke` | create/map/property plus polygon/rectangle drawing | `engine` | `proven`; use as a low-complexity drawing regression. |
| Dialog/message drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xmessage-smoke` | message-window lifecycle and reduced drawing | `engine` | `proven`; widget/toolkit coverage remains limited. |
| Xaw widgets | `cargo run --offline -q -p sophia-cli -- x-authority-xcalc-smoke` | colors, unmap, padded text, normal disconnect cleanup | `engine` | `proven`; grow through xcalc traces rather than generic Xaw support. |
| Clock drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xclock-smoke` | fonts, pixmaps, copy/draw damage, exposure | `engine` | `proven` for the probe window; not proof of every font or drawing path. |
| Eye drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xeyes-smoke` | `QueryColors`, clear, fill-arc draw damage | `engine` | `proven`; colormaps remain reduced. |
| Output query | `cargo run --offline -q -p sophia-cli -- x-authority-xrandr-query-smoke` | bounded RandR version/output query | `client` | `proven` for populated snapshot-derived CRTC/output/mode/monitor facts and acknowledged, mask-selected dynamic screen/CRTC/output/resource notifications; live sessions can inject a validated deterministic size update without physical hotplug. |
| xterm startup | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-smoke` | core setup/lifecycle and compatibility request trace | `client` | `proven` as a lifecycle proof; it intentionally does not require a committed render transaction. |
| xterm render | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-render-smoke` | text drawing becomes a changing CPU-buffer transaction | `engine` | `proven`; retain as the focused software transaction probe, while the paired Milestone 3 row carries resize, presentation, and teardown evidence. |
| xterm input | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-input-smoke` | bounded core key events make the proof shell receive exactly `sophia` and advance xterm CPU-buffer generation | `engine` | `proven`; live Engine input crosses as a routed surface request and is translated by the session RMLVO. Core and XKB maps share that snapshot; GetState/GetNames and selected StateNotify delivery are implemented. The paired Milestone 3 row carries physical multi-client input and focus evidence. |
| Two-client xterm routing | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-two-client-smoke` | two real xterms receive distinct client-targeted core keys through the bounded concurrent frontend and each advances CPU pixels | `engine` | `proven` for brokered CPU-buffer routing and service drain; it is not KMS-backed multi-app session evidence. |
| Guarded one-client xterm | `tools/live_session_persistent_hardware_proof.sh` and `tools/live_session_content_hardware_proof.sh` | real xterm pixels reach Engine-owned GBM/KMS presentation; focused physical input changes later pixels | `hardware` | `proven` at the fixed established buffer size; normal resize and the full multi-client session contract remain open. |
| Guarded two-xterm session | `tools/live_session_two_xterm_hardware_proof.sh` | focused-terminal input plus a supervised second xterm flush every routed X11 event, produce later CPU pixels, and reach KMS presentation with at least two CPU layers | `hardware` | `proven`: retained run completed in 1,487 ms with 10 ms maximum composition, 23 ms input-to-presentation, all 14 events flushed, and clean KMS teardown. This earlier gate is retained but superseded for promotion by the paired Milestone 3 session proof. |
| Paired Milestone 3 session | `tools/live_session_milestone3_hardware_proof.sh` | run guarded two-xterm KMS evidence under classic shared-X and a fresh zero-capability confined namespace, including authenticated RandR delivery, configure-plus-pixels resize, exact physical keyboard input, and pointer-driven pixels | `session` | `proven`; retained schema-13 X13 runs passed strict paired verification with two CPU layers, all routed events flushed, physical text and pointer pixel changes, four RandR notifications, committed resize pixels, clean KMS teardown, 94/90 ms classic/confined startup readiness, 13 ms maximum composition, and 0 ms measured input-to-presentation latency. |
| GTK software dialog | Engine smoke plus `tools/qemu_milestone5_acceptance.sh` | GTK negotiates XKB/XGE/XI2, reads bounded RandR output properties, subscribes to XFixes selection changes, commits nonzero `MIT-SHM` pixels, accepts exact virtio text and an OK-button click, redraws after resize, and exits normally | `session` | `proven`: the unattended classic and confined QEMU profiles pass exact text, presented centered cursor, button-gated Return, committed 640x360 CPU/SHM resize redraw, normal exit, native two-output presentation, and clean retirement. The strict three-class verifier passes with retained xterm and Vulkan session evidence. Direct hardware runs are optional compatibility diagnostics. |
| Firefox native-X trace and daily-driver session | `cargo run --offline -q -p sophia-cli -- x-authority-firefox-smoke`, `tools/qemu_xmonad_m8_mix_acceptance.sh`, and `tools/qemu_xmonad_m8_soak_acceptance.sh` | bounded native-X startup plus an offline six-stage keyboard, pointer, resize, dialog, `CLIPBOARD`, `PRIMARY`, close, and cleanup proof in the two-output xmonad session | `session` | `proven`: the standalone trace completed 396 requests across 45 opcodes with no unexpected error; three consecutive mixed gates passed; the retained 1,891,936 ms soak completed 22 Firefox restarts and 66 total managed closes with zero unexpected protocol errors and clean teardown. Event sequences are socket-write ordered, and multi-toplevel close selects exact, ancestor, or unique `WM_DELETE_WINDOW` targets before bounded termination. |
| Vulkan DRI3/Present transport | `cargo run --offline -q -p sophia-cli -- x-authority-vkcube-smoke` | Mesa RADV negotiates DRI3 1.2 Open/modifiers/multi-plane pixmaps, xshmfences, XFIXES regions, and standard Present | `engine` | `proven` on X13 for a bounded 68-request trace with three imported pixmaps and fences, one accepted Present transaction, one committed runtime surface, and `first_error=none`; it does not prove imported Vulkan pixels in the persistent renderer or native KMS page-flip feedback. |
| Kitty direct GLX bootstrap | `cargo run --offline -q -p sophia-cli -- x-authority-kitty-smoke` | GLVND selects Mesa; Kitty receives ARGB+sRGB FBConfig, creates direct contexts/windows, imports a depth-32 DRI3 buffer, submits Present, and tears down Sync fences/colormaps | `engine` | `proven` on the local AMD render node for a bounded 200-plus-request trace with one accepted Present transaction, one committed runtime surface, every required direct-GLX stage present, and `first_error=none`. The smoke terminates after its first-frame proof; guarded TTY3 input, normal logout, and recovery remain the session gate. Indirect GLX is deliberately unsupported. |
| Kitty interactive input | `cargo run --offline -q -p sophia-cli -- x-authority-kitty-input-smoke` | real Kitty must consume routed `ll` plus Return and submit a later Present | `engine` | `proven` with installed Kitty 0.48.0: six routed core key events produce the exact shell result and three post-input Presents. The root cause was overlapping extension event allocation: GLX event base zero caused libX11 to replace core event converters, so it received but rejected KeyPress/KeyRelease. All advertised traditional extension ranges are now above core events and mutually disjoint. Physical TTY3 proof remains required before xmonad promotion. |
| Vulkan mixed native session | `tools/live_session_milestone4_hardware_proof.sh` | paired software xterm/resize and `vkcube` plus CPU-layer native sessions; controlled acquire delay, rejection recovery, mixed export, page-flip Complete/Idle, idle fence, and exact teardown | `session` | `proven` on X13: the retained schema-14 GPU run completed 76 mixed Flip events, one controlled Skip, 77 matching Idle events and idle-fence triggers, nine acquire-gate waits, zero submit/retire failures, and zero live resources. The paired software xterm/resize baseline also passes. |

## Admission Rule

Every native X11 change must update this matrix when it changes a real-client
result. The change should include:

1. The exact client, command, environment precondition, and version if it
   affects request behavior.
2. The first missing request, reply, event, or extension fact, captured in
   bounded trace data without publishing client metadata or payloads.
3. A focused wire/authority regression plus the corresponding real-client
   command. The client output must retain `first_error=none`.
4. The smallest matrix row or row update that states both what is proven and
   what is deliberately not proven.

An unsupported request must remain a normal client-visible X11 error. Do not
advertise an extension merely to advance a probe, add unrelated Xorg behavior
speculatively, or route raw X11 state into Sophia Engine.

## Promotion Gate

An application class can move from `engine` to `session` evidence only when a
real client proves all of the following through the native frontend:

- normal startup and teardown;
- configured size and output facts derived from Engine rather than fixed setup
  constants;
- Engine-selected keyboard and pointer delivery, including the applicable X11
  focus/grab semantics;
- buffer readiness, delayed release, and presentation feedback on the chosen
  SHM or DRI3/Present path;
- committed Sophia Engine state and real KMS presentation; and
- the applicable classic shared-X or confined-namespace policy.

Until then, unsupported applications remain unsupported by the native session.
Reconsider an optional XLibre provider only after measured compatibility gaps
justify its authority and maintenance cost.
