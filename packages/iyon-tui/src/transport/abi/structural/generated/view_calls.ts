// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 7a24d016ae0beb92c3015bac0a6dd66f09ba93bb899e0271232d576bc586c2bd
// generator_blake3 = 8988baafda3c5ed74ab4450221eca5cd9a1b7fcd82dd351d571694bcb307ca65
import type { NativeViewAbiHandle, NativeTuiHostContract } from "../../../native/addon.ts";
export type ViewAbiSymbols = NativeViewAbiHandle;

const ERROR_BIT = 0x8000_0000;
const CACHE_MISS = 0x8000_0004;

export class NativeAbiStatusError extends Error {
  readonly status: number;
  readonly detail: number;

  constructor(status: number, detail: number) {
    super(`native ABI status 0x${status.toString(16)}`);
    this.name = "NativeAbiStatusError";
    this.status = status;
    this.detail = detail;
  }
}

function checkedRef(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, result: number): number {
  if (result === 0 || result >= ERROR_BIT) {
    const detail = result === CACHE_MISS ? runtime.viewStatusDetail() : 0;
    throw new NativeAbiStatusError(result, detail);
  }
  return result;
}

export function runtimeNoop(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle): number {
  const result = runtime.runtimeNoop();
  return result;
}

export function viewStatusDetail(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle): number {
  const result = runtime.viewStatusDetail();
  return result;
}

export function viewRenderRef(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base: number): number {
  const result = runtime.viewRenderRef(base);
  return checkedRef(symbols, runtime, result);
}

export function hostRenderRef(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, host: NativeTuiHostContract, base: number): number {
  const result = runtime.hostRenderRef(host, base);
  return result;
}

export function viewStateAttach(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base: number, node_id_low: number, node_id_high: number, state_id_low: number, state_id_high: number): number {
  const result = runtime.viewStateAttach(base, node_id_low, node_id_high, state_id_low, state_id_high);
  return checkedRef(symbols, runtime, result);
}

export function viewContentHostCreate(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, content_port_id_low: number, content_port_id_high: number): number {
  const result = runtime.viewContentHostCreate(node_id_low, node_id_high, content_port_id_low, content_port_id_high);
  return checkedRef(symbols, runtime, result);
}

export function viewSpacerCreate(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, rows: number): number {
  const result = runtime.viewSpacerCreate(node_id_low, node_id_high, rows);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchRoot(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base: number, node_id_low: number, node_id_high: number, wrap: number, align: number): number {
  const result = runtime.viewTextLayoutPatchRoot(base, node_id_low, node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewCommonPatchRoot(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base: number, node_id_low: number, node_id_high: number, mask: number, padding_tr: number, padding_bl: number, width_rule: number, height_rule: number, min_width: number, max_width: number, min_height: number, max_height: number, decoration_ref: number): number {
  const result = runtime.viewCommonPatchRoot(base, node_id_low, node_id_high, mask, padding_tr, padding_bl, width_rule, height_rule, min_width, max_width, min_height, max_height, decoration_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewAxisCreateBuffer(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, axis_kind: number, gap: number, children: Uint32Array, used_child_count: number): number {
  const result = runtime.viewAxisCreateBuffer(node_id_low, node_id_high, axis_kind, gap, children, used_child_count);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate0(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number): number {
  const result = runtime.viewRowCreate0(node_id_low, node_id_high, gap);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate1(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number): number {
  const result = runtime.viewRowCreate1(node_id_low, node_id_high, gap, track0, child0);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate2(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number): number {
  const result = runtime.viewRowCreate2(node_id_low, node_id_high, gap, track0, child0, track1, child1);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate3(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number): number {
  const result = runtime.viewRowCreate3(node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate4(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number, track3: number, child3: number): number {
  const result = runtime.viewRowCreate4(node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2, track3, child3);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate0(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number): number {
  const result = runtime.viewColumnCreate0(node_id_low, node_id_high, gap);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate1(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number): number {
  const result = runtime.viewColumnCreate1(node_id_low, node_id_high, gap, track0, child0);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate2(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number): number {
  const result = runtime.viewColumnCreate2(node_id_low, node_id_high, gap, track0, child0, track1, child1);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate3(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number): number {
  const result = runtime.viewColumnCreate3(node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate4(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number, track3: number, child3: number): number {
  const result = runtime.viewColumnCreate4(node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2, track3, child3);
  return checkedRef(symbols, runtime, result);
}

export function axisBuilderBegin(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, axis_kind: number, expected_children: number): number {
  const result = runtime.axisBuilderBegin(axis_kind, expected_children);
  return checkedRef(symbols, runtime, result);
}

export function axisBuilderPush(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, builder_ref: number, track_word: number, child_ref: number): number {
  const result = runtime.axisBuilderPush(builder_ref, track_word, child_ref);
  return result;
}

export function axisBuilderFinish(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, builder_ref: number, node_id_low: number, node_id_high: number, gap: number): number {
  const result = runtime.axisBuilderFinish(builder_ref, node_id_low, node_id_high, gap);
  return checkedRef(symbols, runtime, result);
}

export function axisBuilderAbort(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, builder_ref: number): number {
  const result = runtime.axisBuilderAbort(builder_ref);
  return result;
}

export function viewAxisSetChild(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_axis_ref: number, node_id_low: number, node_id_high: number, child_index: number, track_word: number, child_ref: number): number {
  const result = runtime.viewAxisSetChild(base_axis_ref, node_id_low, node_id_high, child_index, track_word, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewAxisSpliceBuffer(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_axis_ref: number, node_id_low: number, node_id_high: number, index: number, remove_count: number, children: Uint32Array, used_child_count: number): number {
  const result = runtime.viewAxisSpliceBuffer(base_axis_ref, node_id_low, node_id_high, index, remove_count, children, used_child_count);
  return checkedRef(symbols, runtime, result);
}

export function viewGridSetCell(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_grid_ref: number, node_id_low: number, node_id_high: number, row: number, column: number, child_ref: number): number {
  const result = runtime.viewGridSetCell(base_grid_ref, node_id_low, node_id_high, row, column, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewAxisSetChildPath(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, axis_index: number, track_word: number, child_ref: number): number {
  const result = runtime.viewAxisSetChildPath(base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, axis_index, track_word, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewGridCreateBuffer(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, column_gap: number, row_gap: number, words: Uint32Array, used_word_count: number): number {
  const result = runtime.viewGridCreateBuffer(node_id_low, node_id_high, column_gap, row_gap, words, used_word_count);
  return checkedRef(symbols, runtime, result);
}

export function viewDiffCreateBuffer(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, words: Uint32Array, used_word_count: number, bytes: Uint8Array, used_byte_count: number): number {
  const result = runtime.viewDiffCreateBuffer(node_id_low, node_id_high, words, used_word_count, bytes, used_byte_count);
  return checkedRef(symbols, runtime, result);
}

export function viewHangingCreate(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, prefix_ref: number, continuation_ref: number, body_ref: number): number {
  const result = runtime.viewHangingCreate(node_id_low, node_id_high, prefix_ref, continuation_ref, body_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewContainerCreate(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, child_ref: number): number {
  const result = runtime.viewContainerCreate(node_id_low, node_id_high, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewClampCreate(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, child_ref: number, max_rows: number, overflow_kind: number, overflow_style_ref: number, prefix: string): number {
  const result = runtime.viewClampCreate(node_id_low, node_id_high, child_ref, max_rows, overflow_kind, overflow_style_ref, prefix);
  return checkedRef(symbols, runtime, result);
}

export function viewComponentCreate(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, handle_low: number, handle_high: number): number {
  const result = runtime.viewComponentCreate(node_id_low, node_id_high, handle_low, handle_high);
  return checkedRef(symbols, runtime, result);
}

export function viewDecoratedCreateBuffer(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, child_ref: number, style_ref: number, words: Uint32Array, used_word_count: number, bytes: Uint8Array, used_byte_count: number): number {
  const result = runtime.viewDecoratedCreateBuffer(node_id_low, node_id_high, child_ref, style_ref, words, used_word_count, bytes, used_byte_count);
  return checkedRef(symbols, runtime, result);
}

export function viewGridSetCellPath(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, grid_row: number, grid_column: number, child_ref: number): number {
  const result = runtime.viewGridSetCellPath(base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, grid_row, grid_column, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewReleaseMany(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, refs: Uint32Array, used_ref_count: number): number {
  const result = runtime.viewReleaseMany(refs, used_ref_count);
  return result;
}

export function viewRefForNodeId(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number): number {
  const result = runtime.viewRefForNodeId(node_id_low, node_id_high);
  return checkedRef(symbols, runtime, result);
}

export function pathRoot(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle): number {
  const result = runtime.pathRoot();
  return checkedRef(symbols, runtime, result);
}

export function pathChild(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, parent_path_ref: number, step_kind: number, expected_view_kind: number, selector: number): number {
  const result = runtime.pathChild(parent_path_ref, step_kind, expected_view_kind, selector);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPath(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number {
  const result = runtime.viewTextLayoutPatchPath(base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD1(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, wrap: number, align: number): number {
  const result = runtime.viewTextLayoutPatchPathD1(base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD2(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, wrap: number, align: number): number {
  const result = runtime.viewTextLayoutPatchPathD2(base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD3(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, wrap: number, align: number): number {
  const result = runtime.viewTextLayoutPatchPathD3(base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD4(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number {
  const result = runtime.viewTextLayoutPatchPathD4(base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function editTxnBegin(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, base_root_ref: number, expected_edit_count: number): number {
  const result = runtime.editTxnBegin(base_root_ref, expected_edit_count);
  return checkedRef(symbols, runtime, result);
}

export function editTxnAddTextLayout(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, txn_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number {
  const result = runtime.editTxnAddTextLayout(txn_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align);
  return result;
}

export function editTxnCommitRender(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, host: NativeTuiHostContract, txn_ref: number): number {
  const result = runtime.editTxnCommitRender(host, txn_ref);
  return checkedRef(symbols, runtime, result);
}

export function editTxnAbort(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, txn_ref: number): number {
  const result = runtime.editTxnAbort(txn_ref);
  return result;
}

export function styleAtomCreateCstring(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, value: string): number {
  const result = runtime.styleAtomCreateCstring(value);
  return checkedRef(symbols, runtime, result);
}

export function styleCreateBits(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, flags: number, attribute_present: number, attribute_true: number, foreground_ref: number, background_ref: number, theme_atom_ref: number): number {
  const result = runtime.styleCreateBits(flags, attribute_present, attribute_true, foreground_ref, background_ref, theme_atom_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, text: string, style_ref: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateCstring(node_id_low, node_id_high, text, style_ref, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf8(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, style_ref: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateUtf8(node_id_low, node_id_high, bytes, used_bytes, style_ref, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf82(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateUtf82(node_id_low, node_id_high, bytes, used_bytes, span0_bytes, style0, span1_bytes, style1, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf83(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, span2_bytes: number, style2: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateUtf83(node_id_low, node_id_high, bytes, used_bytes, span0_bytes, style0, span1_bytes, style1, span2_bytes, style2, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf84(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, bytes: Uint8Array, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, span2_bytes: number, style2: number, span3_bytes: number, style3: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateUtf84(node_id_low, node_id_high, bytes, used_bytes, span0_bytes, style0, span1_bytes, style1, span2_bytes, style2, span3_bytes, style3, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring2(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateCstring2(node_id_low, node_id_high, text0, style0, text1, style1, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring3(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, text2: string, style2: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateCstring3(node_id_low, node_id_high, text0, style0, text1, style1, text2, style2, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring4(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, text2: string, style2: number, text3: string, style3: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateCstring4(node_id_low, node_id_high, text0, style0, text1, style1, text2, style2, text3, style3, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateBuffer(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, node_id_low: number, node_id_high: number, words: Uint32Array, used_word_count: number, bytes: Uint8Array, used_byte_count: number, wrap: number, align: number): number {
  const result = runtime.viewTextCreateBuffer(node_id_low, node_id_high, words, used_word_count, bytes, used_byte_count, wrap, align);
  return checkedRef(symbols, runtime, result);
}

