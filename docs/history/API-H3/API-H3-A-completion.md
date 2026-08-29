# API-H3 — H3-A completion

**Status:** COMPLETE
**Baseline:** `main` at `1539afd0b53f58c699f146630ca1e3ad84961c5b`
**Delivery:** Committed immediately after validation; the final commit is the H3-A tranche commit in repository history.
**Platform:** Bun 1.4.0 on Darwin arm64
**Scope:** Semantic View foundation and bridge equivalence oracle

## Delivery policy

Every completed H3 tranche must be committed immediately before starting the
next tranche. H3 tranches remain stacked on one feature branch; a tranche is
not complete until its implementation, completion report, and validation are
committed together.

## 1. Implementation

Added the private semantic foundation without changing the production
bridge-backed View construction or rendering route:

```text
packages/iyon-tui/src/api/view/semantic-node.ts
packages/iyon-tui/src/api/presentation/semantic-style.ts
```

The semantic model now has independent representations for:

- all twelve View kinds;
- semantic NodeId and View association helpers;
- text, diff, layout, grid, overflow, decoration, and component values;
- copied/frozen colors, styles, borders, style-state maps, and text spans;
- text-layout, common-scalar, axis-set, axis-splice, and grid-cell derivations;
- read-only axis/grid sequence contracts and weak sequence sidecars.

Semantic component nodes carry JavaScript-local `HandleId`; no native
`ComponentId`, bridge schema version, ABI discriminant, or packed transport
field is present in the semantic model.

`PersistentSeq.values()` provides the read-only sequence contract without
changing its branch factor, mutation algorithms, or asymptotic behavior.

## 2. Equivalence oracle

Added the test-only oracle and matrix in:

```text
packages/iyon-tui/tests/tui_h3_a_semantic.test.ts
```

It independently maps the current bridge representation into semantic values
and verifies:

- every current View family and all structural fields;
- all wrap/alignment modes, grid tracks, cell placement, overflow variants,
  diff line kinds/terminations, and decoration fields;
- shared child identity and semantic NodeId preservation;
- bridge style/color lowering equivalence;
- construction-time snapshot and freeze behavior;
- all five retained derivation families and packed-field decoding;
- PersistentSeq compatibility with the semantic sequence interface;
- absence of bridge/native-retention dependencies in the new semantic modules.

The oracle is test-only and is not used by production rendering.

## 3. Validation

| Gate | Result |
|---|---|
| `bun run typecheck` | PASS |
| `bun test packages/iyon-tui/tests/tui_h3_a_semantic.test.ts` | PASS; 8 passed, 0 failed |
| `bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests` | PASS; 68 passed, 0 failed, 472 assertions |
| `bun run check:tui-declarations` | PASS; 36 reachable declaration files |
| `bun run check:ownership` | PASS; all ownership gates |
| `bun run check:tui-abi` | PASS |
| `git diff --check` | PASS |

No Rust or structural ABI files were changed. H3-B remains responsible for
making the semantic model authoritative for production View construction.
