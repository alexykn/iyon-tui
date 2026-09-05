# L1-03 completion report: text storage and direct text constructors

Date: 2026-09-05. Base: `3e28006` (L1-02 committed).

## What changed

**One owned buffer plus one shared page.** New `NativeTextPage`
(`crates/iyon-tui/src/presentation/api/text.rs`, `native-host`-gated,
`#[doc(hidden)]`): `new(String)` moves the caller's allocation into a single
`Arc`-shared page with no copy; `span(offset, len, style)` borrows a checked
range as a `TextSpan` via a new `TextStorage::PageSlice` variant. Out-of-range
or UTF-8-splitting ranges return `None`; interior validity holds by
construction because the page is built from a `String`. Small spans keep the
existing inline fast case (covered by test, no page involved).

**Direct final text construction.** New `View::native_text_final(spans, wrap,
align)`: one `from_node` with wrap/alignment in the final `TextView` fields
and the already-owned span vector moved into the payload — no modifier chain,
no fluent `Text` wrapper, exactly one root. `text_view_from_spans` in
`view_abi.rs` is rewritten onto it (was
`styled_text(...).wrap(...).text_align(...).into_view()`).

**Length-delimited lanes share the page.**
`utf8_text_spans` and `parse_and_build_text_buffer` validate the whole buffer
once, count it under the new `Counter::TextBytesCopied` perf counter, build
one page, and cut every span from it — no per-span `String` rehydration.
Preflights (span/style count match, span-sum vs `used_bytes`, `MAX_NEW_TEXT_BYTES`
cap, null-pointer guards) run before any work, in the old order; every
rejection keeps its exact `FAST_*` code, including split-multibyte boundaries
(now rejected by the page instead of per-span validation — externally
identical). The NUL-terminated CString lane keeps per-span owned strings
deliberately: each `CString` is already N-API's own allocation, so there is
nothing to share; single-root counting moves into `cstring_to_owned` /
`view_text_create_utf8_impl` with no double count (`text_view_from_owned`
itself is untouched). Owned-string recycling via generated adapters is not
part of this tranche (no recycler exists on this path); revisit only if a
later tranche adds one.

**No SGR remnant on this path.** A search of `iyon-tui-native` and the
presentation module finds no SGR/accelerator code touching text ingress, so
there was nothing to remove.

**Tests.** Four new core tests (`text.rs`): inline fast case preserved,
page-sharing with `Arc::ptr_eq` plus old-root validity after the ingress
handle drops, out-of-bounds/split rejection, trailing-newline preservation.
Seven new native tests (`view_abi.rs`): CString NUL truncation per contract,
span-sum and count-mismatch rejection, split-multibyte rejection with
lead-byte/char-boundary acceptance, embedded/trailing NUL preservation,
empty-string vs empty-span-list distinction, unknown style-ref rejection,
buffer-lane framing/boundary rejection. New `tui_text_lanes.test.ts` (3 tests)
proves lane equivalence from TS: cstring/utf8/buffer lanes render identical
output, NUL and multibyte fixtures included.

**Bench.** New `packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts` (300 nodes,
TS-side N-API call cost, all probes ok): cstring-1span 20.38 µs/node,
cstring-2span-styled 13.86 µs/node, utf8-nul 11.81 µs/node, buffer-6span
16.38 µs/node.

**Seam accounting.** Binding surface 81/81 (`NativeTextPage` added to the
blessed list); mapping +4 `InternalBinding` records (1415 total) with
snapshot regen; `check:tui-binding`, ownership, and declarations posture
unchanged.

## Stop condition

All lane rejections keep their exact error codes (Rust unit tests pin each
one); lane equivalence is proven from TS; the shared-page shape is proven by
`ptr_eq` with old roots surviving ingress-handle drop; allocation reduction
is structural (one buffer + one page per call, no per-span `String`).

## Verification (this session)

- `cargo fmt --all -- --check`: clean (3 hunks applied).
- `sh tools/lint/clippy-gate.sh`: exit 0.
- `cargo test -p iyon-tui-native --all-features --lib`: 43 pass, 0 fail,
  1 ignored (incl. 7 new lane tests).
- `cargo test -p iyon-tui --all-features --lib`: 580 pass, 0 fail, 1 ignored
  (incl. 4 new page/constructor tests).
- `bun test packages/iyon-tui/tests/tui_text_lanes.test.ts`: 3 pass, 0 fail.
- `bun packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts`: all probes ok.
- `bun run tools/api-surface/check-binding.ts`: ALL BINDING CHECKS PASSED.
- `bun run check:ownership`: ALL OWNERSHIP CHECKS PASSED.
