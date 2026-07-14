# Namespaces And Portals

**Role:** normative architecture and target API contract.

Sophia assigns trust at session admission, before a client can create useful
protocol state. Namespaces isolate client resources. Portals mediate the narrow
operations that intentionally cross that isolation boundary.

The current repository has typed namespace/profile/capability/admission values,
a session-owned in-memory namespace registry, namespace-keyed X resource
tables, an X configuration seam that accepts immutable namespace context, a
live X session that allocates and revokes its classic context through the
registry, per-connection X admission and revocation, and pure portal reducers.
Confined-client launch/routing, broker IPC, prompt policy, expiry, and concrete
native-X portal execution are target work tracked in `todo.md`.

## Ownership

The session supervisor owns namespace allocation, profile selection, admission,
and revocation. A protocol authority receives an immutable client admission
context; it does not mint namespace identity or reinterpret the selected
profile. Sophia Engine consumes namespace-checked surface transactions, but it
does not decide whether two clients may share protocol resources.

The target value contract is:

```text
NamespaceProfile = ClassicShared | Confined

NamespaceContext {
    id: NamespaceId,
    profile: NamespaceProfile,
    capabilities: NamespaceCapabilities,
}

ClientAdmissionContext {
    client_id,
    namespace: NamespaceContext,
    auth_provenance,
}
```

`NamespaceId` remains an opaque, nonzero Sophia ID. It is not an XID, Wayland
object ID, PID, UID, or reusable authorization token. `auth_provenance` records
the bounded admission mechanism and session generation; it never contains a raw
cookie or credential secret.

Pure cross-crate values belong in `sophia-protocol`. Namespace allocation,
admission, and revocation belong in `sophia-runtime` and its session supervisor.
Each frontend owns the protocol-specific adapter that consumes an admitted
context.

## Session Profiles

### Classic Shared-X

A trusted X session assigns participating clients the same namespace. Those
clients retain ordinary shared-X inspection, coordination, selections, and
existing-resource access. Per-connection XID ranges prevent creation collisions
and provide cleanup attribution; they are not access-control lists.

### Confined

A confined client group receives a distinct namespace and an explicit
capability set. Resource lookup, event subscription, properties, selections,
input delivery, metadata, and transfers fail closed across namespaces unless a
specific portal flow grants the operation.

The live X launcher selects this profile with
`--namespace-profile=confined`. Each run allocates a fresh confined namespace
with explicit zero capabilities; all clients deliberately launched as part of
that group share it. Separate group credentials on one listener remain future
supervisor work. The socket suite proves that two policy-assigned confined
namespaces cannot map each other's windows, mutate their properties, claim
their selections, forge a selection requestor, or emit metadata from a rejected
foreign property write. It also proves that a rejected foreign event-mask
request cannot redirect brokered input: the addressed worker retains its root
target, while the broker's private queues keep delivery client-specific.
Broader XKB, XI2, and grab semantics remain X11 session-correctness work rather
than a namespace exception.

Capabilities bound which portal operations a namespace may request or publish.
They do not provide ambient cross-namespace access and do not replace a portal
decision for a particular transfer.

Wayland object isolation remains connection-local, but Sophia still assigns a
namespace context so portal and metadata policy use the same trust model across
frontends.

## Admission

An X listener is transport, not identity. After setup authentication and before
allocating an X client identity or resource range, accepting a connection
consults a session admission interface using peer credentials, configured
policy, and the bounded `MIT-MAGIC-COOKIE-1` result when enabled. Successful
admission returns one immutable `ClientAdmissionContext`; failure sends normal
X11 setup failure and creates no client resources. The live classic session
currently requires a kernel-authenticated peer UID matching the session user,
allocates a distinct registry admission for every connection, and revokes it
after connection cleanup. Deterministic regressions cover denial, simultaneous
admissions, normal disconnect, and dispatch-failure cleanup.

The production frontend must not infer identity from one hardcoded
`NamespaceId`. Classic policy may deliberately return the same namespace in
distinct admission contexts; confined policy must return the namespace and
capabilities selected for that client group. Cookie creation, Xauthority-file
publication, rotation, removal, and raw-secret handling are supervisor
responsibilities. The live supervisor generates a fresh kernel-random cookie
per session, publishes a standard owner-only record only after it is fully
written, passes its path to launched clients, and removes it on normal or error
teardown. The frontend validates configured setup authorization before calling
policy and never exposes raw cookie material to admission records, diagnostics,
or Engine data.

Disconnect revokes the connection context, releases its creation ledger, clears
its owned selection generations, unregisters input/control routes, and closes
active grants whose validity depended on that client.

The WM remains namespace-blind. It receives opaque `SurfaceId` layout nodes and
never receives namespace IDs, XIDs, credentials, titles, classes, PIDs, paths,
or portal payloads.

## Portal Contract

Portal policy is a reducer over bounded facts. Runtime execution owns payloads,
file descriptors, protocol replies, UI, and external launchers. Sophia Engine
is not a portal broker and portal policy is never part of the input or frame hot
path.

The target portal taxonomy is explicit:

- clipboard;
- drag-and-drop;
- file handoff;
- screen capture;
- screen recording;
- URI open;
- notification.

The current `PortalTransferKind::Screenshot` and missing URI-open kind are
implementation debt; the protocol taxonomy must be reconciled in the broker
milestone rather than encoded through MIME or type-hint conventions.

A request contains a transfer ID, source and target namespaces, kind, source
generation, bounded metadata, and a deadline. It carries no raw application
object ID, pixel buffer, clipboard payload, file descriptor, URI launcher, or
unbounded string.

The lifecycle has two layers:

```text
request: Pending -> Allowed | Denied | Revoked
grant:   Active  -> Completed | Revoked | Expired
```

An allowed decision creates a scope- and generation-bound grant. Grants are
single-use unless their kind explicitly defines a bounded stream. A stale
source generation, source or target disconnect, deadline expiry, policy
failure, executor failure, or broker restart revokes or denies the request.
Unknown states fail closed.

The policy provider receives only sanitized request facts. A deterministic
headless provider supplies allow/deny decisions for tests. A later prompt UI is
an interchangeable policy provider, not authority or Engine logic.

## X11 Clipboard And PRIMARY Reference Flow

Same-namespace selections use ordinary X11 semantics. A selection request whose
owner and requestor are in different namespaces becomes a portal request.

The X frontend retains requestor XID, selection atom, target atom, property,
timestamp, and source-owner generation. The broker sees only namespace IDs,
transfer kind, normalized target/MIME, bounded size facts, and generation.

The first complete adapter supports `CLIPBOARD` and `PRIMARY` with `TARGETS`,
`UTF8_STRING`, and bounded UTF-8 `text/plain` data. Approval is valid only for
the observed owner generation. Denial, expiry, stale ownership, unsupported
targets, or executor failure becomes normal X11 selection failure through
`SelectionNotify(property = None)`. The session is never frozen while policy is
pending.

Large `INCR` transfers and broader target conversion remain compatibility work
after the bounded reference flow is proven.

## Invariants

- Namespace identity is assigned once at admission and never inferred from
  client-controlled metadata.
- Classic sharing is explicit session policy; confinement is not simulated by
  treating XID ranges as ownership ACLs.
- Cross-namespace lookup and delivery fail closed without a live grant.
- Portal grants authorize one bounded operation, not general resource
  visibility.
- Engine and WM never receive credentials, portal payloads, or raw
  protocol-request context.
- Protocol denial maps to native protocol failure, never synthetic input.
- Policy reducers remain deterministic and I/O-free; executors own effects.
