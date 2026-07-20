# Sophia Specification (Draft)

**Role:** proposed project constitution and stopping rules.  
**Status:** DRAFT — open for discussion; not yet normative.

This document defines the durable shape of Sophia: the invariants every design
must preserve, the work the project intentionally refuses, and the process for
changing either. It is deliberately thinner than the subsystem contracts.
Those documents explain mechanisms and report current coverage; this document
exists so the project can make stable decisions and say *no*.

Until this draft is ratified, the existing normative architecture documents
win every disagreement. Ratification must include an explicit review of each
conflict rather than changing precedence implicitly.

## Product Direction

Sophia is an X-centric product built on a protocol-neutral architecture. The
Sophia X Server Frontend is the sole current application frontend; X11 is the
product foundation, not a deprecated migration path. The engine-native
transaction boundary is the reusable product core.

Protocol neutrality is an Engine invariant, not a promise to maintain multiple
frontends. A future compatibility translator or native application interface
may be evaluated from demonstrated product requirements, but no such interface
is currently supported or planned.

## Invariants

These rules hold regardless of profile, frontend, or milestone. A design that
violates one is not a Sophia design.

1. **Atomic application visuals.** Engine does not present new application
   surface geometry without pixels committed for that geometry. Failure keeps
   the last coherent committed state; any timeout degradation is explicit,
   bounded policy. Engine-owned chrome, cursors, and solid scene elements do
   not pretend to be client pixels.
2. **Separated authority.** Ownership follows
   [architecture.md](architecture.md). Sharing a process, crate, or language
   does not transfer authority between Engine, a protocol frontend, the WM,
   portals, session supervision, or chrome.
3. **A blind WM.** Layout policy receives opaque handles and reduced facts. It
   does not receive XIDs, protocol object IDs, namespaces, titles, classes,
   PIDs, paths, clipboard contents, or other client payloads.
4. **One presentation path.** Every client frontend reduces visual work to
   generation-checked engine transactions and committed surface state. Only
   Engine decides composition and presentation; there is no frontend or client
   bypass to scanout. `SurfaceTransaction` and `CommittedSurfaceState` are the
   current wire types, not names this constitution freezes forever.
5. **Fail closed at authority boundaries.** Invalid or unauthorized operations
   use an honest native protocol failure and produce a reduced recorded fact
   where required. Failed input routing never becomes synthetic input. A hung
   or hostile client cannot force a partially committed frame.
6. **Data crosses boundaries, not object graphs.** Cross-authority interfaces
   use bounded packets, snapshots, commands, explicit ownership, and
   generation-checked typed IDs as defined by [dod.md](dod.md). They do not
   share mutable protocol, renderer, or policy objects.

## Hard Architectural Non-Goals

These are not backlog items. Named application evidence may refine a boundary,
but cannot admit behavior that violates one of these rules.

- **Engine does not parse client protocols.** X11 grammar—and any grammar a
  future translator may terminate—remains inside its owning frontend.
- **No speculative replacement protocol.** X11 remains the application
  contract while it can express Sophia's demonstrated requirements. A native
  interface may be extracted only from multiple proven Engine mechanisms after
  an explicit specification amendment; it is not designed ahead of evidence.
- **No client- or frontend-owned scanout.** Clients supply buffers and
  readiness; Engine owns composition, presentation, and scanout.
- **No synthetic-input fallback.** Input synthesis, if ever exposed, is an
  explicitly authorized session or portal operation—not recovery from a
  failed route and not ambient client privilege.
- **No protocol extension may transfer layout, workspace, physical-input,
  scanout, admission, or portal-policy authority to a frontend.** A frontend
  may retain only the protocol-local state needed to translate semantics.

## Compatibility Admission

Protocol breadth is evidence-driven. Before implementation, each requested
behavior is classified into one of three bins:

1. **Translatable.** It reduces to existing Sophia facts without moving an
   authority boundary. Admit the smallest slice supported by a named real
   client, a focused regression, and an explicit exclusion boundary.
2. **Replaced.** Sophia supplies the user-visible capability through a safer
   native mechanism. Document the replacement and give the legacy request an
   honest protocol response; do not emulate authority Sophia intentionally
   removed.
3. **Inexpressible.** Its semantics require violating a hard architectural
   non-goal. Refuse it. Application demand alone cannot admit it.

Discovery can move a request between these bins. In particular, a supposedly
translatable feature becomes inexpressible if implementation reveals that it
would move authority. Wire behavior must be decided per request: success-no-op
is permitted only when it is truthful for the observed client and cannot create
false ownership or synchronization claims.

### Amendment Ratchet

Changing an invariant, hard non-goal, or established classification requires a
single recorded decision containing:

1. the named application or user workflow and retained evidence;
2. the current classification and why it is insufficient;
3. the proposed boundary and its explicit exclusions;
4. the affected authority, security, and failure semantics;
5. updates to this specification and every affected subsystem contract; and
6. focused tests plus the appropriate real-client or session gate.

The [X11 compatibility matrix](x11-compatibility-matrix.md) remains the
admission record for X11 behavior. Conformance means its evidence-backed
slices, not everything an Xorg server happens to accept.

## X11 Scope Decisions

These are current classifications, not claims that every listed wire response
is already implemented.

| Behavior | Classification | Supported direction |
| --- | --- | --- |
| Multiple X screens | Replaced | One X screen backed by Engine's multi-output topology |
| Non-TrueColor visuals | Replaced | Bounded TrueColor visuals; toolkit-side rendering |
| Core font server/general rasterization | Replaced | Bounded compatibility glyphs; clients render application text |
| Indirect GLX rendering | Replaced | Selected GLX discovery only where rendering reduces to DRI3/Present |
| DRI1/DRI2 | Replaced | DRI3/Present and Engine-owned presentation |
| TCP display transport | Out of product scope | Authenticated local Unix transport |
| Endian-swapped clients | Out of product scope | Local same-endian clients |
| XTEST as ambient client privilege | Inexpressible | Explicitly authorized session/portal input synthesis, if admitted |
| Server-wide `GrabServer` authority | Inexpressible | Bounded frontend-local compatibility behavior that cannot stall Engine or peers |

For `GrabServer` especially, “accept and no-op” is not a blanket rule: the
frontend must not report synchronization or exclusion it cannot provide. A
real-client trace must select the least misleading bounded response.

## Future Protocol Compatibility

X11 is the only supported application protocol. Sophia is not a Wayland
compositor and makes no current Wayland application-compatibility promise. The
retired Wayland prototype is historical evidence that Engine transactions,
routed input, and presentation were not inherently X-shaped; retaining that
lesson does not retain the implementation as a product surface.

A future protocol translator is admissible only through the amendment ratchet
and must satisfy all of these conditions:

1. a named important application or workflow cannot use the native-X product;
2. the translator reduces to existing Sophia transactions and authorities;
3. it does not import shell, workspace, global-focus, physical-input, scanout,
   admission, or portal policy from the external protocol ecosystem;
4. it does not shape Engine APIs around protocol-local objects or sequencing;
5. its dependency, security, validation, and maintenance costs are bounded;
   and
6. it does not displace the active native-X product milestone without an
   explicit reprioritization decision.

Architectural openness permits later evaluation. It does not create a backlog
item, tier progression, compatibility commitment, or obligation to follow an
external protocol's extension ecosystem.

## Isolation And Denial Semantics (Proposed)

Confined profiles apply these principles by operation class:

1. **Observation may be reduced.** A foreign-resource query may return empty,
   root-only, or otherwise redacted native facts when doing so cannot create an
   inconsistent client-visible state.
2. **Mutation fails honestly.** Unauthorized property, selection, event-mask,
   and similar mutations return the appropriate native failure. Silent success
   must not let multiple clients believe they own one resource.
3. **Eligible denial may enter a portal flow.** A user-meaningful,
   portal-supported crossing can produce a reduced request and, after policy
   approval, a bounded generation-tied grant. Other denials are terminal; not
   every protocol failure prompts the user.

Classic shared-X deliberately permits same-namespace inspection and mutation
where the admitted X11 semantics require it. It still rejects malformed,
unauthenticated, unsupported, stale, or resource-invalid operations; “shared”
does not mean “cannot fail.”

## Document Authority

| Document | Governing content |
| --- | --- |
| [architecture.md](architecture.md) | Ownership, process relationships, atomic transaction model |
| [dod.md](dod.md) | Data boundaries, typed IDs, bounded records, private state |
| [namespaces-and-portals.md](namespaces-and-portals.md) | Admission, profiles, capabilities, grant lifecycle |
| [sophia-wm-api.md](sophia-wm-api.md) | Versioned blind-WM policy contract |
| [sophia-x-authority.md](sophia-x-authority.md) | X11 frontend contract and current coverage |
| [x11-compatibility-matrix.md](x11-compatibility-matrix.md) | Evidence-admitted X11 results |

While this document is a draft, the documents above retain their existing
authority. After explicit ratification, this specification governs invariants,
hard non-goals, and amendment rules; subsystem documents govern mechanism and
current status. A conflict must be resolved in the documents, not left to an
informal precedence rule.
