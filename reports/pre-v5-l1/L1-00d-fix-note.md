# L1-00d fix note: three reproduced defects fixed as explicit correctness commits

Date: 2026-09-05. Parent tranche: L1-00c (`ede6fba`), which characterized these
gaps and deliberately left production code untouched per the freeze rule. The
owner then directed fixes before continuing; each fix below changed its
characterization fixture to the fixed behavior in the same commit.

## 1. Exhaustion atomicity (§9.6) — `36efe16`

**Was:** `append`, `seal`, and `truncate_head` mutated live storage before
preflighting the revision; `clear` emptied storage before checking the content
generation. Stale-revision snapshots could observe never-accepted bytes.
**Fix:** `next_revision` (and the generation `checked_add`) now run before any
storage install in all five mutation paths (`append_utf8`, `replace_utf8`,
`clear`, `seal`, `truncate_head`) in `crates/iyon-tui/src/application/content.rs`.
**Proven by:** the 6-test forced-boundary matrix in `content.rs` (`u64::MAX`
pinning) plus the structural-lane exhaustion test in `host.rs` — all assert
byte/annotation/revision/range/flag/accounting invariance on rejection.

## 2. ANSI scanner 0x9B (§11.5) — `e0cc922`

**Was:** a lone 0x9B byte — always a UTF-8 continuation byte in validated Source
payloads — hit the C1-CSI arm and suppressed all newer revisions while it
remained in the domain (demonstrated with U+00DB "Û", no escape nearby).
**Fix:** `crates/iyon-tui/src/content/text/ansi.rs` reads only the genuine
two-byte C1 CSI (C2 9B) as control; lone 0x9B falls through to ordinary text.
**Proven by:** `tests/tui_ansi_scanner.test.ts` — Û renders styled/inline, later
revisions flow, genuine U+009B CSI still drives SGR, full split matrix green.

## 3. Post-acceptance wake fan-out (§9.6, L1-12 "no lost wake") — `36efe16`

**Was:** `finish_mutation` returned on the first failing subscriber, losing all
later wakes (order-dependent; pinned by test).
**Fix:** every eligible host is attempted; failures are reported afterwards as
`SOURCE_WAKE_FAILED` carrying the accepted revision — never an ambiguous
ordinary rejection that invites a duplicating retry.
**Proven by:** both poison-ordering tests in `host.rs` — error surfaces,
revision installed and readable, all healthy hosts marked pending either way.

## Gates at fix time

`cargo fmt --check` clean, `clippy-gate` exit 0, 23 Rust binaries ok,
105 bun tests 0 fail, `tsc --noEmit` clean, `biome` clean on new files,
`check:ownership` ALL PASSED, `native:smoke` ok on the restaged addon.
`check:tui-declarations`/bunx work re-attempted in the L1-00 closure tranche
now that sandbox restrictions are lifted.
