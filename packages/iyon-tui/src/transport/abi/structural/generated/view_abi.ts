// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 90c4574d58806b37acee3d351b3059c11be2f0e023be8583d48fd9df6e008704
// generator_blake3 = c929e3e1f77d78143cb66be8625c910e55d641010957aaa54beca3368dd956b7
import type { NativeTuiHostContract } from "../../../native/addon.ts";
export interface NativeViewAbiMetadata {
  readonly abi_name: string;
  readonly abi_version: number;
  readonly semantic_version: number;
  readonly schema_blake3: string;
  readonly generator_blake3: string;
  readonly generation: number;
  readonly transport: "napi";
  readonly function_count: number;
}

export interface NativeViewAbiHandle {
  /** S6 dispatch-granularity probe; not a semantic/public TUI operation. */
  tuiPerfNapiBatchRuntimeNoop?(count: number): number;
  metadata(): NativeViewAbiMetadata;
  runtimeNoop(): number;
  viewStatusDetail(): number;
  viewRenderRef(base: number): number;
  hostRenderRef(host: NativeTuiHostContract, base: number): number;
  viewStateAttach(base: number, node_id_low: number, node_id_high: number, state_id_low: number, state_id_high: number): number;
  viewContentHostCreate(node_id_low: number, node_id_high: number, content_port_id_low: number, content_port_id_high: number): number;
  viewSpacerCreate(node_id_low: number, node_id_high: number, rows: number): number;
  viewTextLayoutPatchRoot(base: number, node_id_low: number, node_id_high: number, wrap: number, align: number): number;
  viewCommonPatchRoot(base: number, node_id_low: number, node_id_high: number, mask: number, padding_tr: number, padding_bl: number, width_rule: number, height_rule: number, min_width: number, max_width: number, min_height: number, max_height: number, decoration_ref: number): number;
  viewAxisCreateBuffer(node_id_low: number, node_id_high: number, axis_kind: number, gap: number, children: Uint32Array, used_child_count: number): number;
  viewRowCreate0(node_id_low: number, node_id_high: number, gap: number): number;
  viewRowCreate1(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number): number;
  viewRowCreate2(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number): number;
  viewRowCreate3(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number): number;
  viewRowCreate4(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number, track3: number, child3: number): number;
  viewColumnCreate0(node_id_low: number, node_id_high: number, gap: number): number;
  viewColumnCreate1(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number): number;
  viewColumnCreate2(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number): number;
  viewColumnCreate3(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number): number;
  viewColumnCreate4(node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number, track3: number, child3: number): number;
  axisBuilderBegin(axis_kind: number, expected_children: number): number;
  axisBuilderPush(builder_ref: number, track_word: number, child_ref: number): number;
  axisBuilderFinish(builder_ref: number, node_id_low: number, node_id_high: number, gap: number): number;
  axisBuilderAbort(builder_ref: number): number;
  viewAxisSetChild(base_axis_ref: number, node_id_low: number, node_id_high: number, child_index: number, track_word: number, child_ref: number): number;
  viewAxisSpliceBuffer(base_axis_ref: number, node_id_low: number, node_id_high: number, index: number, remove_count: number, children: Uint32Array, used_child_count: number): number;
  viewGridSetCell(base_grid_ref: number, node_id_low: number, node_id_high: number, row: number, column: number, child_ref: number): number;
  viewAxisSetChildPath(base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, axis_index: number, track_word: number, child_ref: number): number;
  viewGridCreateBuffer(node_id_low: number, node_id_high: number, column_gap: number, row_gap: number, words: Uint32Array, used_word_count: number): number;
  viewDiffCreateBuffer(node_id_low: number, node_id_high: number, words: Uint32Array, used_word_count: number, bytes: Uint8Array, used_byte_count: number): number;
  viewHangingCreate(node_id_low: number, node_id_high: number, prefix_ref: number, continuation_ref: number, body_ref: number): number;
  viewContainerCreate(node_id_low: number, node_id_high: number, child_ref: number): number;
  viewClampCreate(node_id_low: number, node_id_high: number, child_ref: number, max_rows: number, overflow_kind: number, overflow_style_ref: number, prefix: string): number;
  viewComponentCreate(node_id_low: number, node_id_high: number, handle_low: number, handle_high: number): number;
  viewDecoratedCreateBuffer(node_id_low: number, node_id_high: number, child_ref: number, style_ref: number, words: Uint32Array, used_word_count: number, bytes: Uint8Array, used_byte_count: number): number;
  viewGridSetCellPath(base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, grid_row: number, grid_column: number, child_ref: number): number;
  viewReleaseMany(refs: Uint32Array, used_ref_count: number): number;
  viewRefForNodeId(node_id_low: number, node_id_high: number): number;
  pathRoot(): number;
  pathChild(parent_path_ref: number, step_kind: number, expected_view_kind: number, selector: number): number;
  viewTextLayoutPatchPath(base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number;
  viewTextLayoutPatchPathD1(base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, wrap: number, align: number): number;
  viewTextLayoutPatchPathD2(base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, wrap: number, align: number): number;
  viewTextLayoutPatchPathD3(base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, wrap: number, align: number): number;
  viewTextLayoutPatchPathD4(base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number;
  editTxnBegin(base_root_ref: number, expected_edit_count: number): number;
  editTxnAddTextLayout(txn_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number;
  editTxnCommitRender(host: NativeTuiHostContract, txn_ref: number): number;
  editTxnAbort(txn_ref: number): number;
  styleAtomCreateCstring(value: string): number;
  styleCreateBits(flags: number, attribute_present: number, attribute_true: number, foreground_ref: number, background_ref: number, theme_atom_ref: number): number;
  viewTextCreateCstring(node_id_low: number, node_id_high: number, text: string, style_ref: number, wrap: number, align: number): number;
  viewTextCreateUtf8(node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, style_ref: number, wrap: number, align: number): number;
  viewTextCreateUtf82(node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, wrap: number, align: number): number;
  viewTextCreateUtf83(node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, span2_bytes: number, style2: number, wrap: number, align: number): number;
  viewTextCreateUtf84(node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, span2_bytes: number, style2: number, span3_bytes: number, style3: number, wrap: number, align: number): number;
  viewTextCreateCstring2(node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, wrap: number, align: number): number;
  viewTextCreateCstring3(node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, text2: string, style2: number, wrap: number, align: number): number;
  viewTextCreateCstring4(node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, text2: string, style2: number, text3: string, style3: number, wrap: number, align: number): number;
  viewTextCreateBuffer(node_id_low: number, node_id_high: number, words: Uint32Array, used_word_count: number, bytes: Uint8Array, used_byte_count: number, wrap: number, align: number): number;
  u8_8(a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number): number;
  u16_8(a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number): number;
  u32_8(a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number): number;
  u32_16(a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number, a8: number, a9: number, a10: number, a11: number, a12: number, a13: number, a14: number, a15: number): number;
  i32_4(a0: number, a1: number, a2: number, a3: number): number;
  f32_4(a0: number, a1: number, a2: number, a3: number): number;
  f64_4(a0: number, a1: number, a2: number, a3: number): number;
  pointer(a0: boolean): number;
  buffer(a0: Uint8Array): number;
  cstring(a0: string): number;
}
