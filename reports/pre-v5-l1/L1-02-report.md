# L1-02 completion report: final node/common-field construction (this tranche)

Date: 2026-09-05. Parent: `37c275d` (L1-01).

## What changed

**One final constructor.** `ViewNodeParts` gains `state_attachment` /
`content_attachment`; `View::from_node` computes aggregate + attachment flags
once via the shared `ViewNode::flags_for` (also used by the retained
single-field `map_node` patch path, so both agree on every flag bit).
`ViewNodeParts::from_view` destructures a base into candidate parts with
payloads staying shared — no payload work on the patch path.

**Candidate assembly.** New `NativeCommonPatch` (plain validated values, size
rules kept inside the core as `width_fill`/`height_fill` bools) plus
`View::native_patched`, applying the exact §6.3 precedence in one final root
with modifier-identical semantics (verified field-by-field against each
modifier body). Both native ingress implementations rewritten onto it:
`view_common_patch_root_impl` (was up to 7 roots) and
`parse_and_build_decorated` (was up to ~12 roots, incl. custom glyphs now
decoding straight into `[String; 8]` with no temp vector). Validation order
and error codes preserved; failures still publish nothing.

**Direct attachment.** `native_content_host` fuses kind creation + port
attachment into one `from_node` (was `new_kind` then `map_node`). The
cross-ABI `view_state_attach_impl` stays a single-root patch — fusing it
would require changing the transport call sequence (§6.8 forbids).

**Axis decode-and-move.** `native_axis_from_children` and both
`native_axis_splice` branches decode straight into `RowChild`/`ColumnChild`
(no intermediate track vector + mapping pass), preserving validation order.

**Uniqueness tracking.** Attachment duplicate collection threads a
per-candidate `seen` hash set (one lookup per attachment) instead of scanning
collected targets; cycle detection and the `"duplicate ViewState attachment"`
rejection are unchanged.

**Tests (step 6).** Three new `ir.rs` tests: final assembly equals the
modifier chain it replaces (`semantic_eq`) with exactly one
`ViewNodesConstructedRust` increment; patching preserves child flags,
attachments, and payload identity (`ptr_eq`); duplicate rejection preserved
plus the positive single-target case.

**Seam accounting.** Binding surface 80/80 (`NativeCommonPatch` added to the
blessed list); mapping +2 `InternalBinding` records with snapshot regen;
`check:tui-binding`, ownership, and declarations all green.

## Stop condition

Decoration/base/attachment goldens and failure cases pass unchanged (full
workspace suite, incl. generated ABI goldens and differential/fuzz tests); a
decorated node allocates one root (proven by counter test); exact-root/NodeId
early returns untouched.

## Verification (this session)

fmt clean, clippy-gate 0, workspace tests 22 binaries ok (default) and 23/23
(all-features), 105 bun tests 0 fail on the restaged addon, typecheck 0,
lint:ts 0 errors at baseline warning count, smoke ok. Ledger rows emptied:
axis/decoration/content-host ingress; text (L1-03), grid (L1-04), and
style/color atoms (L1-07) remain for their tranches.
