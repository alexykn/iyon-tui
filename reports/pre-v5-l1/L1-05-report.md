# L1-05 completion report: typed retained-state ingress

Date: 2026-09-05. Base: `8e1b0cc` (L1-04 committed).

## What changed

**Finite property schema (§7.1, step 1).** 17 `[[state_property]]` rows in
`tools/tui-abi/view_abi.toml` (10 geometry bits, 7 presentation bits), each
with stable per-domain id, TS patch key + diagnostic name, native value
kind, nullability, clear support, capability rule (`node-kind`, plus
`+axis` for alignment), and word/string lane widths. Bit ids are dense per
domain so masks stay u32.

**Generator extension (step 2).** `tui-abi-gen` gains `model.StatePropertySpec`,
`validate_state_properties` (dense ids, known domains/value kinds/
capabilities, lane caps, unique names), and a new `render_state` module
emitting two outputs: `state_envelope.ts` (TS packers for
already-normalized patches + clear lists, mask/lane constants, capability
tables, `STATE_WAKE_DRAIN`) and `view_state_schema.rs` (per-domain
`ALL/NULLABLE/CLEARABLE` masks, word/string counts, per-property
`ID_*`/`*_NAME`/offset/len consts, enum/attr/edge codes, and a generated
`check_envelope` enforcing unknown-bits, `null ⊆ set`, null-legality,
`clear ∩ set = ∅`, set-vs-clear lane expectations, and exact lane lengths).
The manifest pins the property table; the insta snapshot covers it.
Deliberate split: TS validation (exact public errors) stays hand-written in
`control.ts`; generation owns the envelope protocol (masks, ids, offsets,
codes) so the two sides cannot drift. TS diagnostic names generate into the
packers; Rust `*_NAME` consts feed the reader messages.

**Native envelope transport (steps 3, 5–7).** `view_state.rs` is rewritten:
`setGeometry/setPresentation` take `(set_mask, null_mask, clear_mask,
words, strings)`; clears take `(set, null, clear, clear_all)` with lanes
absent; style-state ops keep their `(key, value)` strings. Decoders validate
the header first, then terminate directly in `ViewStateGeometryPatch` /
`ViewStatePresentationPatch` / clear lists — no `serde_json::Value`, no
property strings, no `parse_*`, no `View`, no `view_*_patch` calls
(verified by inspection: the file references none). Wake results are the
primitive u32 `WAKE_SCHEDULE_ENVIRONMENT_DRAIN` bit instead of a JSON
object. `color_spec`'s string half is extracted as shared `color_spec_str`
(the TS packer normalizes `{type:"ansi"}` objects to `ansi:N` strings;
canonical values identical); `text_attribute` is shared for style lanes.
Atomicity is unchanged in shape (§7.2): full decode → single record
`mutate`; a malformed envelope never reaches the record.

**TS transport (steps 3–4).** Public `ViewState` method names, signatures,
and every validation error are unchanged; `control.ts` gains envelope
wrappers (normalize, then pack) and re-exports `STATE_WAKE_DRAIN` so the
semantic API never imports `/generated/` (ownership gate required this
reroute). `addon.ts` contract + private resource interface carry the mask
args and numeric wake. `NativeStateWake` (shared with the content plane) is
untouched.

**Parity + cleanup (step 8).** New `tui_state_envelope.test.ts` (5 tests):
geometry on text/axis end to end incl. null/clear/no-key-clear, full-lane
axis envelope, no-partial-patch atomicity, and exact pins for every TS
validation error (unknown keys, bad scalars, bad alignment/edges/glyphs/
attrs, duplicate/unknown clear keys, `undefined` omission). New native
decoder tests (5): stable ids, full geometry decode incl. semantic null,
presentation decode incl. themed style, header-violation rejections,
clear-all vs list vs empty distinction. The two old JSON-parser tests are
gone with the parsers. Existing presentation/style-state suites pass
unchanged through the new path.

## Stop condition

State mutation decodes masks/lanes straight into canonical override
patches; no semantic View is constructed and no structural transport is
entered on the state path. Null (`Some(None)`), clear (list vs `None`
clear-all), base reveal, and remount flows behave exactly as before
(envelope tests + perf13 suites). Invalid envelopes fail whole
(header-first decode; atomicity test).

## Verification (this session)

- Baselines: perf13_b 11 pass, native 44 pass.
- `cargo test --workspace --all-features`: exit 0, 23 binaries ok
  (native 47: 44 − 2 parser tests + 5 envelope tests; generator 10: 7 + 3 new).
- `bun test packages/iyon-tui/tests/` on the restaged addon: 103 pass,
  0 fail (98 + 5 new). `tsc --noEmit` clean.
- `generate`/`check:tui-abi` clean (existing outputs change only in banner
  hashes + manifest property table); `cargo fmt --check`, `clippy-gate`,
  binding, ownership, declarations all pass.
- Incidents: the new tests caught only test bugs (gap/alignment capability
  rules on text/column, `style` needing a real `StyleRef`) — all pre-existing
  core rules, confirmed in `capabilities.rs`, tests adjusted. Native-only
  messages for TS-prevalidated conditions use envelope terminology
  (unreachable via the public API); every public error string is
  byte-identical. `biome.jsonc` gains one ignore line for the new generated
  dir, mirroring the existing generated ignores.
