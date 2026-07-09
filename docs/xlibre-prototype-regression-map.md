# XLibre Prototype Regression Map

Sophia no longer treats XLibre as the long-term center of the architecture.
These checks remain useful as prototype evidence while Sophia X Authority is
designed and implemented.

## Keep As Prototype References

Keep these until Sophia X Authority has equivalent transaction, namespace, and
portal coverage:

- `tools/check_xlibre_routed_input_patch.sh`: proves the routed-input patch
  still applies and builds against an XLibre-style server tree.
- `tools/xlibre_namespace_smoke.sh`: proves the prototype stack can run against
  an Xnamespace-enabled X server without mutating the local checkout.
- `x-smoke-policy-frame`: keeps the XComposite/Damage-to-frame path covered.
- `x-smoke-runtime-tick` and `runtime-damage-epoch-smoke`: keep the runtime and
  XSync/Damage layout-epoch compromise covered.
- `x-smoke-live-clipboard-portal`: keeps concrete X11 selection deny/handoff
  behavior covered until Sophia X Authority owns selection execution.
- `x-smoke-routed-input`, `x-smoke-routed-input-edges`, and
  `x-stress-routed-input`: keep routed input correctness and latency evidence
  available while the X11 request path remains the fallback.
- `xnamespace-isolation` in `tools/xlibre_namespace_smoke.sh`: keeps isolation
  evidence until Sophia X Authority has live namespace resource tests.

## Retire After Authority Equivalents Exist

These should not become permanent architecture dependencies:

- XComposite pixmap mirroring smokes should retire after Sophia X Authority
  emits ready `SurfaceTransaction` values from X drawing paths.
- XLibre routed-input patch checks should retire after Sophia X Authority owns
  Engine-routed, authority-delivered input for X clients.
- XLibre namespace smokes should retire after Sophia X Authority enforces
  namespace-aware resource lookup, event subscription, grabs, selections, and
  properties in live tests.
- XLibre clipboard live smokes should retire after Sophia X Authority maps X
  selections into Sophia Portal requests and executes allow/deny/handoff
  without an external X server.

## Deferred Optimization Gate

The shared-memory routed-input ring remains deferred. The current X11
`RouteEvent` request path is the baseline and mandatory fallback. Only repeated
stress measurements above the documented latency threshold should reopen SHM
work.
