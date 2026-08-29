# API-H3 — H3-B completion

**Status:** COMPLETE
**Delivery:** Committed immediately after validation, before beginning H3-C.
**Baseline:** H3-A commit `402cb8b4efe5fea2dd8f1cc30719f177de317605`
**Scope:** Make the semantic View authoritative and cut composition off structural transport

## 1. Semantic View cutover

`api/view/view.ts` now constructs and associates private `SemanticViewNode`
values. The existing public View API, NodeId allocation, immutable identity,
child sharing, validation, wide sequence behavior, native path test helpers,
and fluent semantics remain unchanged.

Composition now reads only:

```text
api/view/semantic-node.ts
api/presentation/semantic-style.ts
```

All reuse checks use semantic node identity and normalized semantic values.
`composition/execution.ts` validates component outputs with `semanticNodeOf()`.

## 2. Transitional structural compatibility

The unchanged structural transport route remains available through:

```text
SemanticViewNode -> transport/structural/cold-lowering.ts -> BridgeViewNode
```

The complete lowerer explicitly maps semantic kinds, layout/overflow/diff
values, colors/styles/borders, derivations, and wide sequence sidecars. Its
bridge cache is weakly keyed by semantic node; bridge data is derived and is
not authoritative. `view-bridge.ts` is now only the transitional boundary.

The compact retained edit-word encoding is mapped explicitly and separately
from bridge layout-child discriminants.

Component semantic nodes store JavaScript-local `HandleId`. Runtime handle
registration now passes that identity into the private native-resource seam;
structural transport resolves `HandleId` to native `ComponentId` only while
lowering. Disposal removes the lookup deterministically.

## 3. Ownership enforcement

Added `h3b-composition-transport-seam` to `tools/ownership/check.ts`.
Composition has no imports resolving under `transport/**`; no allowlist is
used.

## 4. Focused coverage

```text
packages/iyon-tui/tests/tui_h3_b_composition.test.ts
```

Covers semantic-authoritative View construction, derived bridge compatibility,
retained composition reuse/change behavior, local component identity, and
HandleId-to-native resolution/disposal.

The H3-A equivalence oracle was retained and updated to exercise the new
semantic-authoritative route.

## 5. Validation

| Gate | Result |
|---|---|
| `bun run typecheck` | PASS |
| `bun test packages/iyon-tui/tests/tui_h3_b_composition.test.ts` | PASS; 3 passed |
| `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` | PASS; 71 passed, 492 assertions |
| `bun run check:ownership` | PASS, including H3-B seam gate |
| `bun run check:tui-declarations` | PASS; 36 reachable declaration files |
| `bun run check:tui-abi` | PASS |
| `git diff --check` | PASS |

No public exports, structural ABI schema, or Rust sources changed. H3-C will
remove bridge construction from the ordinary retained materialization path.
The H3-A/B implementation and this report are committed together. H3-C
must not begin until its own completed tranche is committed.
