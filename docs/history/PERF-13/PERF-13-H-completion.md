# PERF-13-H completion

## Scope

PERF-13-H removes the superseded streaming implementation and its tests. The
retained content path is now the only production path:

```text
Source -> Funnel -> ContentPort -> Connector -> ContentHost
```

Deleted surface and implementation include the old History stream units,
stream scheduler/pane/transfer modules, snapshot/projector facades, lifecycle
request shims, compatibility counters, and direct object-based content/View
bindings. The remaining `StreamOffset`/`StreamRange` types are source-rooted
coordinates used by the canonical projection API.

History, ViewSlot, and ScrollPane publish retained references through the
canonical generated host ABI. A failed retained preparation or materialization
leaves the committed content unchanged; there is no second object-payload
fallback.

## Hardening

- Source lifetime is environment-owned and independent of any host lifetime.
- ContentPort and Connector ownership is explicit; teardown releases every
  Source membership before Source disposal.
- Native binding methods are reference-based for History, ViewSlot,
  ScrollPane, and host root publication.
- The staged addon verifies the removed native classes and object methods are
  absent, while retaining the required generated N-API and feature-gated
  direct-FFI surfaces.
- The ownership gate scans production sources for deleted stream facades,
  schedulers, registries, lifecycle shims, and object-based native bindings.

## Verification

Passed on the final working tree:

- `cargo fmt --all -- --check`
- `CARGO_BUILD_JOBS=1 cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `CARGO_BUILD_JOBS=1 cargo test --workspace`
- `bunx tsc --noEmit`
- `bun run check:tui-declarations`
- `bun run check:ownership`
- `CARGO_BUILD_JOBS=1 bun run native:stage`
- `bun run native:smoke`
- `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` — 95 passed
- `bun run perf:content`
- direct-FFI staging and a focused direct-FFI T15 smoke case

The full PERF-12 benchmark suite was intentionally not run.
