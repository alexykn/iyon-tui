# PERF-12 T15 transport-parity assessment

**Assessment revision:** `019a048b7c6fa8e6cc6c1e4f0e6635dc78c1b0b7`
**Candidates:** generated safe N-API and feature-gated direct FFI
**Purpose:** establish that the comparison arms contain the same finalized
transport-independent implementation before timing results are considered.

## Findings

The current direct oracle is not the historical S5 checkout. It uses the
current finalized retained implementation and current native host, with only
the physical ABI lowering changed.

| Area | Generated N-API arm | Direct-FFI oracle | Parity result |
|---|---|---|---|
| semantic IR and `View` construction | `src/ir.ts`, `src/values/view.ts` | the same current files | shared |
| retained DAG, leases, hints, cutoff, recovery | `src/retained_dag.ts` | `bench/direct_ffi/retained_dag.ts` | control flow is identical; only opaque-handle/host types and calls differ |
| materialization families and scratch lanes | `src/generated/view_materialize.ts` | `bench/direct_ffi/view_materialize.ts` | same materializer families and retained caps; only call lowering differs |
| ABI wrapper set | generated safe methods | generated-equivalent direct wrappers | 57/57 wrapper names match the canonical manifest |
| derivation paths | text/common scalar, axis, splice, grid, path | the same derivation branches | shared retained logic |
| native host, layout, paint, cache, leases | current `crates/iyon-tui` | current `crates/iyon-tui` with `direct-ffi` exports | shared Rust implementation |
| feature policy | default artifact | `direct-ffi` artifact only | direct symbols remain gated |

The direct ABI uses `bun:ffi` only inside `bench/direct_ffi/**` and only when
the native addon is staged with `direct-ffi`. The product/default package still
uses generated safe N-API over opaque handles.

## Issues found and corrected before authoritative timing

1. **Invalid stateful workload.** The draft created a new supposedly stable
   subtree for every sample, so `SHARED_PATH` and exact-identity cases did not
   preserve identity across updates. The workload factory now creates one
   persistent scenario and advances it sequentially. Stable subtrees are
   reused; wide axis/grid and path derivations advance from the prior root.
2. **Incomplete matrix runner.** The draft had only isolated case runners. A
   process-isolated authoritative orchestrator now covers the complete common
   matrix, selected large cutoffs, wide axis/grid cases, and path derivations,
   alternating candidate order by workload block.
3. **Missing fallback phase visibility.** Large rebuilt cases that exceeded the
   retained/cold budget fell through to the authoritative host fallback without
   a phase record. The case runners now record that fallback host-commit phase,
   keeping all four timing arrays aligned.
4. **No memory recheck.** A separate one-million-operation memory case now runs
   both transports with maintenance and live-state snapshots.
5. **Missing supplemental gates.** Separate process-isolated runners now cover
   typed multi-edit transactions and a generic terminal trace containing stable
   history-like content, progressive stream-like updates, status/layout changes,
   and structural insertions.

## Deliberately not changed

- No direct-FFI implementation was deleted or disabled.
- No transport-independent retained optimization was removed or bypassed.
- No product/application concepts were added to the generic framework.
- The benchmark does not automatically select a transport; the owner decides
  after reviewing the evidence.
