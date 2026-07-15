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

No current X11 entry meets the complete `session` promotion gate. The guarded
two-xterm path has retained `hardware` evidence with concurrent client-targeted
input and KMS composition. Engine-derived initial output facts, authority-local
XKB state, and resize pixel quarantine are implemented, but normal resize
evidence, dynamic RandR events, XKB wire requests, grabs/XI2, presentation
feedback, and confined admission remain. External probes are not session
evidence.

## Baseline Matrix

| Client or class | Reproducible command | Proven X11 surface | Evidence | Current status and next gate |
| --- | --- | --- | --- | --- |
| Setup parser and admission | `cargo test --offline -q -p sophia-x-authority --test x11_wire` plus `cargo test --offline -p sophia-cli --features atomic-scanout-live live_xauthority_file_is_owner_only_valid_and_removed_on_drop -- --exact` | byte order, bounded setup fields, setup success/failure encoding, MIT-MAGIC-COOKIE-1 gating/provenance, peer-credential denial, concurrent identity allocation, revocation on disconnect/error/supervisor command, peer-preserving cleanup, and owner-only Xauthority removal | `wire` | `proven`; each live classic session publishes a fresh cookie and requires same-UID peers, while the live launcher can explicitly allocate a confined group. Independently credentialed groups on one listener remain before a general local session. |
| Raw core protocol | `cargo run --offline -q -p sophia-cli -- x-authority-x11-smoke` | atoms, property read/write, create/map window, core events | `client` | `proven`; grow only from a captured missing request. |
| Rust X11 client | `cargo run --offline -q -p sophia-cli -- x-authority-x11rb-smoke` | client-compatible setup, root/visual facts, atom/property/window flow | `client` | `proven`; setup assigns disjoint client XID ranges, concurrent classic clients retain same-namespace access, and policy-assigned confined clients reject foreign map, property, selection-owner, selection-requestor, and event-mask operations without metadata leakage or input redirection. Core grabs, XGE/XI2 delivery, and the required XKB map/state/name path are implemented; deterministic hierarchy/focus evidence remains open. |
| Core selections and portal mediation | `cargo test --offline -q -p sophia-x-authority --test x11_wire cross_namespace_executor_installs_property_and_notifies_requestor -- --exact` plus `cargo test --offline -q -p sophia-portal --test socket` | same-namespace `PRIMARY`, owner replacement and `SelectionClear`; cross-namespace `CLIPBOARD`, authority-private source proxy, broker grant/payload correlation, requestor property install, `TARGETS`, UTF-8 text, and native failure notification | `wire` | `proven`; allowed, denied, stale, expired, disconnected, unsupported, and executor-failure paths are bounded and fail closed. Large `INCR`, XFixes selection notifications, and Xdnd remain later compatibility work. |
| `xdpyinfo` | `cargo run --offline -q -p sophia-cli -- x-authority-xdpyinfo-smoke` | root screen, extension discovery, root properties, focus, GC cleanup | `client` | `proven`; do not turn its empty extension list into an extension-completeness claim. |
| Minimal Xlib lifecycle | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-smoke` | normal Xlib create/property/map/destroy path | `client` | `proven`; retain as the first C/Xlib regression. |
| Core drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-drawing-smoke` | GC lifecycle and `PolyFillRectangle` reduced to a ready transaction | `engine` | `proven`; visual output still needs session evidence. |
| Software image upload | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-put-image-smoke` | bounded `PutImage` CPU-buffer update and ready transaction | `engine` | `proven`; retain buffer ownership/release requirements when real SHM import arrives. |
| Private explicit handoff | `cargo run --offline -q -p sophia-cli -- x-authority-present-pixmap-smoke` | `SOPHIA-PRESENT` query and bounded pixmap transaction | `engine` | `proven` as Sophia-private prototype only; replace with standard DRI3/Present and fences. |
| Root inspection | `cargo run --offline -q -p sophia-cli -- x-authority-xwininfo-root-smoke` | root attributes, geometry, tree, coordinate translation | `client` | `proven`; live setup root size is Engine-derived, while dynamic resize notification remains open. |
| Root properties | `cargo run --offline -q -p sophia-cli -- x-authority-xprop-root-smoke` | root property enumeration and bounded reads | `client` | `proven`; broaden only when a client captures a required atom/property pattern. |
| Root mutation | `cargo run --offline -q -p sophia-cli -- x-authority-xsetroot-name-smoke` | root name/property mutation, focus, GC, extension-query flow | `client` | `proven`; no Engine pixels are expected. |
| Simple Xaw drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xlogo-smoke` | create/map/property plus polygon/rectangle drawing | `engine` | `proven`; use as a low-complexity drawing regression. |
| Dialog/message drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xmessage-smoke` | message-window lifecycle and reduced drawing | `engine` | `proven`; widget/toolkit coverage remains limited. |
| Xaw widgets | `cargo run --offline -q -p sophia-cli -- x-authority-xcalc-smoke` | colors, unmap, padded text, normal disconnect cleanup | `engine` | `proven`; grow through xcalc traces rather than generic Xaw support. |
| Clock drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xclock-smoke` | fonts, pixmaps, copy/draw damage, exposure | `engine` | `proven` for the probe window; not proof of every font or drawing path. |
| Eye drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xeyes-smoke` | `QueryColors`, clear, fill-arc draw damage | `engine` | `proven`; colormaps remain reduced. |
| Output query | `cargo run --offline -q -p sophia-cli -- x-authority-xrandr-query-smoke` | bounded RandR version/output query | `client` | `proven` for populated snapshot-derived CRTC/output/mode/monitor facts and acknowledged, mask-selected dynamic screen/CRTC/output/resource notifications; live sessions can inject a validated deterministic size update without physical hotplug. |
| xterm startup | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-smoke` | core setup/lifecycle and compatibility request trace | `client` | `proven` as a lifecycle proof; it intentionally does not require a committed render transaction. |
| xterm render | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-render-smoke` | text drawing becomes a changing CPU-buffer transaction | `engine` | `proven`; add standard buffer handoff and normal resize/presentation feedback. |
| xterm input | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-input-smoke` | bounded core key events make the proof shell receive exactly `sophia` and advance xterm CPU-buffer generation | `engine` | `proven`; live Engine input crosses as a routed surface request and is translated by the session RMLVO. Core and XKB maps share that snapshot; GetState/GetNames and selected StateNotify delivery are implemented. Deterministic multi-client hierarchy/focus evidence remains open. |
| Two-client xterm routing | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-two-client-smoke` | two real xterms receive distinct client-targeted core keys through the bounded concurrent frontend and each advances CPU pixels | `engine` | `proven` for brokered CPU-buffer routing and service drain; it is not KMS-backed multi-app session evidence. |
| Guarded one-client xterm | `tools/live_session_persistent_hardware_proof.sh` and `tools/live_session_content_hardware_proof.sh` | real xterm pixels reach Engine-owned GBM/KMS presentation; focused physical input changes later pixels | `hardware` | `proven` at the fixed established buffer size; normal resize and the full multi-client session contract remain open. |
| Guarded two-xterm session | `tools/live_session_two_xterm_hardware_proof.sh` | focused-terminal input plus a supervised second xterm flush every routed X11 event, produce later CPU pixels, and reach KMS presentation with at least two CPU layers | `hardware` | `proven`: retained run completed in 1,487 ms with 10 ms maximum composition, 23 ms input-to-presentation, all 14 events flushed, and clean KMS teardown. Normal resize evidence, dynamic RandR, full XKB/grabs/XI2, and confined admission remain before `session`. |
| GTK software dialog | `dbus-run-session -- cargo run --offline -q -p sophia-cli -- x-authority-zenity-smoke` | GTK negotiates XKB/XGE/XI2 and commits a nonzero `MIT-SHM` image | `engine` | `proven` for nonzero software pixels with `first_error=none`; selected XI2 Key/Button/Motion/Enter/Leave/Focus delivery and master device classes are implemented, while a retained interactive GTK hardware proof remains open. |

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
