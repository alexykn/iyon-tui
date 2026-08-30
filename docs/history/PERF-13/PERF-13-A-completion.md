# PERF-13-A — H3/host transaction seam and runtime substrate

**Status:** COMPLETE
**Branch:** `perf-13`
**Baseline:** `84b7d3b` (`main` after API-H3 merge)
**Implementation:** the commit introducing this completion report (`feat: implement PERF-13-A runtime seam`)

## Delivered

- Added native desired structural revision, visible frame revision, pending and
  committed host epochs.
- Added an environment-owned pending-host queue, edge-triggered wake latch,
  retry blocking, and fair bounded native drains shared by hosts in one native
  environment.
- Added the TypeScript environment wake broker, structured runtime error
  channel, configurable runtime-error listener, explicit `Tui.flush()` barrier,
  counters, and bounded trace capture.
- Added a deferred retained root publication mode: H3 accepts desired structure
  without painting; the frame drain promotes it to the visible root only after
  the candidate frame succeeds. Superseded desired roots release their leases
  without disturbing the visible root.
- Added the plane-neutral native resource registry with weak owner/resource
  records, non-reusable identities, prepared/desired/visible leases, host and
  environment validation, owner-death invalidation, and internal attachment
  fixtures.
- Added backend-neutral semantic `stateAttachment` and `contentAttachment`
  HandleId slots, strong semantic attachment references, and H3 prepare-time
  duplicate, affinity, kind, liveness, and capability validation. No public
  ViewState or content API was added in this tranche.
- Preserved the existing synchronous visible behavior by routing compatibility
  renders through the new explicit frame barrier.

## Stop-gate evidence

- H3 attachment preparation aborts without changing the committed binding.
- Native injected frame failure preserves the old visible frame and leaves the
  desired epoch pending; an explicit retry commits it.
- Shared native environments drain hosts fairly with a bounded budget.
- Automatic retryable errors report through the error channel without throwing
  from or spinning microtasks. Explicit barriers surface them synchronously.
- Focused semantic, transport, harness, resource, broker, and deferred-root
  tests pass.

## Verification

```text
bunx tsc --noEmit --pretty false
bun run check:ownership
bun run check:tui-declarations
bun run check:tui-abi
cargo fmt --all -- --check
cargo clippy -p iyon-tui --features native-host --all-targets -- -D warnings
cargo clippy -p iyon-tui-native --all-targets -- -D warnings
cargo test -p iyon-tui --features native-host application::host::tests
cargo test -p iyon-tui-native
bun test packages/iyon-tui/tests/tui_perf13_a.test.ts \
  packages/iyon-tui/tests/tui_h3_a_semantic.test.ts \
  packages/iyon-tui/tests/tui_h3_b_composition.test.ts \
  packages/iyon-tui/tests/tui_h3_c_transport.test.ts \
  packages/iyon-tui/tests/tui_generated_view_abi.test.ts \
  packages/iyon-tui/tests/tui_harness.test.ts \
  packages/iyon-tui/tests/tui_native_scalar.test.ts
```

The full PERF-12 suite was not rerun.
