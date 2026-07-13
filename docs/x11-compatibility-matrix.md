# Sophia X11 Compatibility Matrix

This matrix is the admission record for the **Sophia X Server Frontend**. It
answers a deliberately narrow question: which real X11 client behavior has a
reproducible, bounded proof through Sophia's own X11 server frontend?

It is not an Xorg/XLibre conformance claim. A `proven` entry means the listed
probe completes its defined proof window with `first_error=none`; it does not
imply that nearby X11 requests, extensions, window-manager conventions, or
unlisted application modes work. `startup only` means the application reaches
the stated protocol stage without a client-visible X error but has no rendered
desktop proof yet.

The native frontend is always the subject of this matrix. XLibre can remain an
optional broad-compatibility provider, but its behavior does not promote an
entry here.

## Evidence Levels

| Level | Meaning |
| --- | --- |
| `wire` | Deterministic decoder/dispatcher fixture proves one bounded X11 request, reply, event, or error shape. |
| `client` | A real client completes its bounded protocol proof with `first_error=none`. |
| `engine` | The real client produces a reduced Sophia transaction that commits through Engine state. |
| `hardware` | A bounded one-client proof reaches Engine-owned native presentation and physical input, but does not yet meet the full session-promotion gate. |
| `session` | The client reaches normal startup, routed input, resize, presentation, and teardown backed by Sophia Engine and KMS. |

No current X11 entry meets the complete `session` promotion gate. The guarded
single-client xterm path has `hardware` evidence, but remains fixed-size while
normal resize, client concurrency, output facts, and full X11 input semantics
are completed. The external-probe harness is not a simultaneous-client X
desktop.

## Baseline Matrix

| Client or class | Reproducible command | Proven X11 surface | Evidence | Current status and next gate |
| --- | --- | --- | --- | --- |
| Setup parser | `cargo test --offline -q -p sophia-x-authority --test x11_wire` | byte order, bounded setup fields, setup success/failure encoding, configured MIT-MAGIC-COOKIE-1 acceptance/rejection | `wire` | `proven`; Xauthority-file management, peer credentials, cookie rotation, and launch policy remain before a general local session. |
| Raw core protocol | `cargo run --offline -q -p sophia-cli -- x-authority-x11-smoke` | atoms, property read/write, create/map window, core events | `client` | `proven`; grow only from a captured missing request. |
| Rust X11 client | `cargo run --offline -q -p sophia-cli -- x-authority-x11rb-smoke` | client-compatible setup, root/visual facts, atom/property/window flow | `client` | `proven`; add multi-client XID allocation before using this as a shared-desktop proof. |
| `xdpyinfo` | `cargo run --offline -q -p sophia-cli -- x-authority-xdpyinfo-smoke` | root screen, extension discovery, root properties, focus, GC cleanup | `client` | `proven`; do not turn its empty extension list into an extension-completeness claim. |
| Minimal Xlib lifecycle | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-smoke` | normal Xlib create/property/map/destroy path | `client` | `proven`; retain as the first C/Xlib regression. |
| Core drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-drawing-smoke` | GC lifecycle and `PolyFillRectangle` reduced to a ready transaction | `engine` | `proven`; visual output still needs session evidence. |
| Software image upload | `cargo run --offline -q -p sophia-cli -- x-authority-xlib-put-image-smoke` | bounded `PutImage` CPU-buffer update and ready transaction | `engine` | `proven`; retain buffer ownership/release requirements when real SHM import arrives. |
| Private explicit handoff | `cargo run --offline -q -p sophia-cli -- x-authority-present-pixmap-smoke` | `SOPHIA-PRESENT` query and bounded pixmap transaction | `engine` | `proven` as Sophia-private prototype only; replace with standard DRI3/Present and fences. |
| Root inspection | `cargo run --offline -q -p sophia-cli -- x-authority-xwininfo-root-smoke` | root attributes, geometry, tree, coordinate translation | `client` | `proven`; output/RandR facts remain fixed rather than Engine-derived. |
| Root properties | `cargo run --offline -q -p sophia-cli -- x-authority-xprop-root-smoke` | root property enumeration and bounded reads | `client` | `proven`; broaden only when a client captures a required atom/property pattern. |
| Root mutation | `cargo run --offline -q -p sophia-cli -- x-authority-xsetroot-name-smoke` | root name/property mutation, focus, GC, extension-query flow | `client` | `proven`; no Engine pixels are expected. |
| Simple Xaw drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xlogo-smoke` | create/map/property plus polygon/rectangle drawing | `engine` | `proven`; use as a low-complexity drawing regression. |
| Dialog/message drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xmessage-smoke` | message-window lifecycle and reduced drawing | `engine` | `proven`; widget/toolkit coverage remains limited. |
| Xaw widgets | `cargo run --offline -q -p sophia-cli -- x-authority-xcalc-smoke` | colors, unmap, padded text, normal disconnect cleanup | `engine` | `proven`; grow through xcalc traces rather than generic Xaw support. |
| Clock drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xclock-smoke` | fonts, pixmaps, copy/draw damage, exposure | `engine` | `proven` for the probe window; not proof of every font or drawing path. |
| Eye drawing | `cargo run --offline -q -p sophia-cli -- x-authority-xeyes-smoke` | `QueryColors`, clear, fill-arc draw damage | `engine` | `proven`; colormaps remain reduced. |
| Output query | `cargo run --offline -q -p sophia-cli -- x-authority-xrandr-query-smoke` | bounded RandR version/output query | `client` | `proven` for fixed root facts; connect Engine output snapshots before claiming dynamic RandR. |
| xterm startup | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-smoke` | core setup/lifecycle and compatibility request trace | `client` | `proven` as a lifecycle proof; it intentionally does not require a committed render transaction. |
| xterm render | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-render-smoke` | text drawing becomes a changing CPU-buffer transaction | `engine` | `proven`; add standard buffer handoff and normal resize/presentation feedback. |
| xterm input | `cargo run --offline -q -p sophia-cli -- x-authority-xterm-input-smoke` | bounded core key events advance xterm CPU-buffer generation | `engine` | `proven` for injected core keys; real XKB, grabs, physical input, and multi-client focus are still open. |
| Guarded one-client xterm | `tools/live_session_persistent_hardware_proof.sh` and `tools/live_session_content_hardware_proof.sh` | real xterm pixels reach Engine-owned GBM/KMS presentation; focused physical input changes later pixels | `hardware` | `proven` at the fixed established buffer size; normal resize and the full multi-client session contract remain open. |
| GTK startup | `dbus-run-session -- cargo run --offline -q -p sophia-cli -- x-authority-zenity-smoke` | GTK reaches reduced startup, selections, `MIT-SHM`, `RANDR`, and `BIG-REQUESTS` | `client` | `startup only`; it is not a rendered dialog proof and currently lacks XInput2. |

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

Until then, XLibre remains the optional broad-compatibility lane for
applications that need a wider X server surface.
