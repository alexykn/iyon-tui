# L1-01 completion report: unsupported internal binding contract (this tranche)

Date: 2026-09-05. Parent: `e0ea28b` (L1-00 closure).

## What changed (step by step)

1. **Unpublish + posture.** `crates/iyon-tui/Cargo.toml` gains `publish = false`
   with a rationale comment; `lib.rs` crate docs state the runtime-implementation
   status and point at `binding`; the authoring `prelude` module is deleted
   (zero in-repo users — every prior "prelude" hit was `napi::bindgen_prelude`).
2. **Narrow seam.** New `crates/iyon-tui/src/binding/mod.rs`: a single flat
   facade of 78 re-exports in §5.3 lane sections, no implementation, no
   prelude, no blanket module re-exports. Per-item `native-host`/`perf-counters`
   gates mirror the root so featureless builds stay clean (verified for
   default, all-features, and perf-counters-only configurations).
3. **Native on the seam.** All four native files import only via
   `iyon_tui::binding` (use-blocks plus qualified paths, including the test
   module and cfg-gated perf call sites). Pure path rewrite — no behavior
   change; diff is import lines plus fmt reflow from shortened paths.
4. **Checks.** New `tools/api-surface/check-binding.ts` (`bun run
   check:tui-binding`, wired into `package.json`, ARCHITECTURE machine checks,
   and all four CI workflows): native-seam allowlist, 78-item blessed surface
   (no silent widening), and forbidden authoring posture (prelude absent,
   manifest unpublished, no root binding re-export). Mapping
   `tools/api-surface/mappings/iyon-tui.toml` deliberately updated
   (1369 → 1409: −38 prelude, +78 binding — 44 cloned alias projections,
   34 fresh `InternalBinding` records for previously unmapped gated items)
   and the ownership snapshot regenerated; `check:ownership` fully green.
   TS declaration and framework-purity gates untouched and passing.
5. **ARCHITECTURE.md.** "Public API parity ... is mandatory" replaced with the
   L1-01 rule: TS public compatibility for authors plus internal ABI/runtime
   conformance for the binding (generated ABI checks + `check:tui-binding`).
6. **Ledger.** `reports/pre-v5-l1/L1-01-ledger.md` names every remaining native
   production fluent-constructor use with owner function and target tranche
   (L1-02 axis/decoration, L1-03 text, L1-04 grid/diff, L1-07 style/color),
   plus the external-integration-test migration plan. Explicitly not an
   allowance; final DSL deletion stays in L1-13.
7. **Tests.** Untouched by design: unit tests already live in the module tree,
   `tests/ui` compile-fail contracts already encode the non-authoring posture,
   and all 15 integration files compile and pass against the retained root.

## Stop condition

Native addon and current tests work through the new seam (staged + smoked +
105 bun tests green); no new representation or registry introduced — the facade
is re-exports only, and the mapping/snapshot diff is the complete surface delta.

## Verification (this session)

fmt clean, clippy-gate 0, `cargo test --workspace` 22 binaries ok,
`--all-features` 23/23 ok, doctests 6/6, tui-abi-gen 7/7, typecheck 0,
lint:ts 0 errors / 380 warnings (baseline count, zero new), declarations
37/37, binding 3/3, ownership ALL PASSED, native:smoke ok.
