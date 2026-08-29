# API-H3 — H3-D completion

**Status:** COMPLETE
**Baseline:** H3-C `df90d5f` (`feat: move retained transport to semantic nodes`)
**Final commit:** the commit introducing this report
**Platform:** macOS arm64
**Bun:** `1.4.0`

## 1. Publication seam

Added the composition-owned contract in:

```text
packages/iyon-tui/src/composition/publication.ts
```

```ts
interface PreparedStructuralPublication {
  commit(): void;
  abort(): void;
}

interface StructuralPublicationTarget {
  preparePublication(output: View): PreparedStructuralPublication | undefined;
  needsPublication?(output: View): boolean;
}

interface StructuralScopeProjection {
  readonly view: View;
  readonly target: StructuralPublicationTarget;
  dispose(): void;
}
```

`RetainedExecutionRuntime` now consumes the named structural contracts.
Runtime root targets, component-scope projections, ViewSlot builder targets,
and ScrollPane builder targets all use prepare/commit/abort publication.

The legacy `ScopeProjection.install(output)` fallback is deleted. A refused
projection preparation now aborts the enclosing batch just like every other
structural target. The existing child-before-parent staging order, sideband
`needsPublication`, rollback, and deferred disposal behavior are unchanged.

A standalone in-memory publication smoke check verified successful commit,
prepare refusal, producer rollback, and preservation of the last committed
output.

## 2. Native-path cleanup

Moved all native retained path metadata and constructors from `api/view` to:

```text
packages/iyon-tui/src/transport/structural/retained-path.ts
```

Moved items include:

- native path selectors and view-kind tags;
- path lineage and transaction edit metadata;
- path-aware semantic text patch construction;
- path lineage/transaction WeakMaps;
- path constructors and transaction accessors.

`api/view/view.ts` retains only semantic identity helpers. Packed axis track
words were removed from semantic construction; wide semantic edits now accept
semantic track facts and structural encoding remains transport-owned. Existing
path, transaction, wide-axis, and grid tests/benchmarks were redirected to the
private transport owner.

## 3. Compatibility architecture deletion

Deleted:

```text
packages/iyon-tui/src/transport/structural/view-bridge.ts
packages/iyon-tui/src/transport/structural/component-view.ts
```

All cold callers now use `lowerColdView()` directly. Component semantic View
construction remains in `api/view/view.ts`; physical HandleId-to-ComponentId
resolution is isolated in:

```text
packages/iyon-tui/src/transport/structural/component-id.ts
```

`cold-lowering.ts` is now the final complete derived fallback. It retains only
a weak semantic-to-bridge cache. The reverse bridge-to-semantic association,
bridge derivation sidecars, bridge sequence sidecars, and bridge semantic
helpers were removed from framework production code.

The direct-FFI benchmark retains its independent bridge candidate through a
benchmark-local metadata adapter rather than restoring framework bridge
sidecars.

`transport/structural/ir.ts` is now wire/bridge-only: schema constants,
physical records, and bridge drafts. Semantic View records, semantic
sequence/derivation sidecars, and construction clone/merge helpers are gone.

## 4. Architecture documentation and enforcement

Updated `ARCHITECTURE.md` with the final H3 ownership model:

```text
api/view                 immutable semantic View and NodeId
composition              semantic reuse, scopes, and structural publication
runtime                  lifecycle and concrete target binding
transport/structural    ABI encoding and physical/native retention
```

Extended `tools/ownership/check.ts` with `h3d-residual-cleanup`. It checks:

- deleted migration modules remain absent;
- composition and `api/view` contain no bridge/native-retention symbols;
- `api/view` has no structural transport/ABI imports;
- structural transport has no composition imports;
- the named publication contract exists;
- legacy projection installation is absent;
- bridge derivation/sequence sidecars are absent;
- component transport contains no semantic construction.

## 5. Final source-scan output

```text
composition/api-view physical-symbol scan: 0 matches
structural -> composition scan: 0 matches
api/view -> transport scan: 0 matches
view-bridge.ts absent
component-view.ts absent
```

The historical H3-A/B documents retain their historical migration references;
no production source reference remains.

## 6. Validation

| Gate | Result |
|---|---|
| `bun run check:tui-abi` | PASS; generated ABI unchanged |
| `bun run typecheck` | PASS |
| `bun run check:tui-declarations` | PASS; 38 reachable declarations, public types nameable |
| `bun run check:ownership` | PASS; H3-B, H3-C, and H3-D rules |
| `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` | PASS; 75 tests, 502 assertions |
| `cargo test --workspace` | PASS; 735 Rust tests, plus native/public/integration suites |
| `git diff --check` | PASS |
| publication seam smoke | PASS |

No Rust source or structural ABI schema changed, so Rust formatting and
Clippy were not rerun for this TypeScript-only tranche.

## 7. Remaining work

H3-D leaves no temporary H3 bridge aliases or non-transactional publication
paths. The remaining H3-E work is enforcement consolidation and final
behavior/performance/integration gates, including direct-FFI parity and the
external consumer workflow.

Deferred beyond H3:

- **PERF-13 state plane:** generic state transport and future state publication;
- **PERF-13 content plane:** source/content transport and stream handoff;
- **future structural optimization:** only if profiling justifies it;
- **unrelated public API debt:** unchanged by H3.
