// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 8fcc9af81022fc96af24b4f5904c019d099084cbba60e24bd6c01699c1ac30c6
// generator_blake3 = de90d6c9ff4fe3d9ad72e91ce00e7e3d95124e664f97b21fd584dbcc9a37f6e4
import type { Pointer } from "bun:ffi";
import type { linkViewAbi } from "./view_abi";
export type ViewAbiSymbols = ReturnType<typeof linkViewAbi>["symbols"];

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

function checkedRef(symbols: ViewAbiSymbols, runtime: Pointer, result: number): number {
  if (result === 0 || result >= ERROR_BIT) {
    const detail = result === CACHE_MISS ? symbols.viewStatusDetail(runtime) : 0;
    throw new NativeAbiStatusError(result, detail);
  }
  return result;
}

export function runtimeNoop(symbols: ViewAbiSymbols, runtime: Pointer): number {
  const result = symbols.runtimeNoop(runtime);
  return result;
}

export function viewStatusDetail(symbols: ViewAbiSymbols, runtime: Pointer): number {
  const result = symbols.viewStatusDetail(runtime);
  return result;
}

export function viewRenderRef(symbols: ViewAbiSymbols, runtime: Pointer, base: number): number {
  const result = symbols.viewRenderRef(runtime, base);
  return checkedRef(symbols, runtime, result);
}

export function hostRenderRef(symbols: ViewAbiSymbols, runtime: Pointer, host: Pointer, base: number): number {
  const result = symbols.hostRenderRef(runtime, host, base);
  return result;
}

export function viewSpacerCreate(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, rows: number): number {
  const result = symbols.viewSpacerCreate(runtime, node_id_low, node_id_high, rows);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchRoot(symbols: ViewAbiSymbols, runtime: Pointer, base: number, node_id_low: number, node_id_high: number, wrap: number, align: number): number {
  const result = symbols.viewTextLayoutPatchRoot(runtime, base, node_id_low, node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewCommonPatchRoot(symbols: ViewAbiSymbols, runtime: Pointer, base: number, node_id_low: number, node_id_high: number, mask: number, padding_tr: number, padding_bl: number, width_rule: number, height_rule: number, min_width: number, max_width: number, min_height: number, max_height: number, decoration_ref: number): number {
  const result = symbols.viewCommonPatchRoot(runtime, base, node_id_low, node_id_high, mask, padding_tr, padding_bl, width_rule, height_rule, min_width, max_width, min_height, max_height, decoration_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewAxisCreateBuffer(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, axis_kind: number, gap: number, children: NodeJS.TypedArray | DataView, used_child_count: number): number {
  const result = symbols.viewAxisCreateBuffer(runtime, node_id_low, node_id_high, axis_kind, gap, children, children, used_child_count);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate0(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number): number {
  const result = symbols.viewRowCreate0(runtime, node_id_low, node_id_high, gap);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate1(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number): number {
  const result = symbols.viewRowCreate1(runtime, node_id_low, node_id_high, gap, track0, child0);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate2(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number): number {
  const result = symbols.viewRowCreate2(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate3(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number): number {
  const result = symbols.viewRowCreate3(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2);
  return checkedRef(symbols, runtime, result);
}

export function viewRowCreate4(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number, track3: number, child3: number): number {
  const result = symbols.viewRowCreate4(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2, track3, child3);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate0(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number): number {
  const result = symbols.viewColumnCreate0(runtime, node_id_low, node_id_high, gap);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate1(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number): number {
  const result = symbols.viewColumnCreate1(runtime, node_id_low, node_id_high, gap, track0, child0);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate2(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number): number {
  const result = symbols.viewColumnCreate2(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate3(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number): number {
  const result = symbols.viewColumnCreate3(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2);
  return checkedRef(symbols, runtime, result);
}

export function viewColumnCreate4(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, gap: number, track0: number, child0: number, track1: number, child1: number, track2: number, child2: number, track3: number, child3: number): number {
  const result = symbols.viewColumnCreate4(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2, track3, child3);
  return checkedRef(symbols, runtime, result);
}

export function axisBuilderBegin(symbols: ViewAbiSymbols, runtime: Pointer, axis_kind: number, expected_children: number): number {
  const result = symbols.axisBuilderBegin(runtime, axis_kind, expected_children);
  return checkedRef(symbols, runtime, result);
}

export function axisBuilderPush(symbols: ViewAbiSymbols, runtime: Pointer, builder_ref: number, track_word: number, child_ref: number): number {
  const result = symbols.axisBuilderPush(runtime, builder_ref, track_word, child_ref);
  return result;
}

export function axisBuilderFinish(symbols: ViewAbiSymbols, runtime: Pointer, builder_ref: number, node_id_low: number, node_id_high: number, gap: number): number {
  const result = symbols.axisBuilderFinish(runtime, builder_ref, node_id_low, node_id_high, gap);
  return checkedRef(symbols, runtime, result);
}

export function axisBuilderAbort(symbols: ViewAbiSymbols, runtime: Pointer, builder_ref: number): number {
  const result = symbols.axisBuilderAbort(runtime, builder_ref);
  return result;
}

export function viewAxisSetChild(symbols: ViewAbiSymbols, runtime: Pointer, base_axis_ref: number, node_id_low: number, node_id_high: number, child_index: number, track_word: number, child_ref: number): number {
  const result = symbols.viewAxisSetChild(runtime, base_axis_ref, node_id_low, node_id_high, child_index, track_word, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewAxisSpliceBuffer(symbols: ViewAbiSymbols, runtime: Pointer, base_axis_ref: number, node_id_low: number, node_id_high: number, index: number, remove_count: number, children: NodeJS.TypedArray | DataView, used_child_count: number): number {
  const result = symbols.viewAxisSpliceBuffer(runtime, base_axis_ref, node_id_low, node_id_high, index, remove_count, children, children, used_child_count);
  return checkedRef(symbols, runtime, result);
}

export function viewGridSetCell(symbols: ViewAbiSymbols, runtime: Pointer, base_grid_ref: number, node_id_low: number, node_id_high: number, row: number, column: number, child_ref: number): number {
  const result = symbols.viewGridSetCell(runtime, base_grid_ref, node_id_low, node_id_high, row, column, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewAxisSetChildPath(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, axis_index: number, track_word: number, child_ref: number): number {
  const result = symbols.viewAxisSetChildPath(runtime, base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, axis_index, track_word, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewGridCreateBuffer(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, column_gap: number, row_gap: number, words: NodeJS.TypedArray | DataView, used_word_count: number): number {
  const result = symbols.viewGridCreateBuffer(runtime, node_id_low, node_id_high, column_gap, row_gap, words, words, used_word_count);
  return checkedRef(symbols, runtime, result);
}

export function viewDiffCreateBuffer(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, words: NodeJS.TypedArray | DataView, used_word_count: number, bytes: NodeJS.TypedArray | DataView, used_byte_count: number): number {
  const result = symbols.viewDiffCreateBuffer(runtime, node_id_low, node_id_high, words, words, used_word_count, bytes, bytes, used_byte_count);
  return checkedRef(symbols, runtime, result);
}

export function viewGridSetCellPath(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, grid_row: number, grid_column: number, child_ref: number): number {
  const result = symbols.viewGridSetCellPath(runtime, base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, grid_row, grid_column, child_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewReleaseMany(symbols: ViewAbiSymbols, runtime: Pointer, refs: NodeJS.TypedArray | DataView, used_ref_count: number): number {
  const result = symbols.viewReleaseMany(runtime, refs, refs, used_ref_count);
  return result;
}

export function viewRefForNodeId(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number): number {
  const result = symbols.viewRefForNodeId(runtime, node_id_low, node_id_high);
  return checkedRef(symbols, runtime, result);
}

export function pathRoot(symbols: ViewAbiSymbols, runtime: Pointer): number {
  const result = symbols.pathRoot(runtime);
  return checkedRef(symbols, runtime, result);
}

export function pathChild(symbols: ViewAbiSymbols, runtime: Pointer, parent_path_ref: number, step_kind: number, expected_view_kind: number, selector: number): number {
  const result = symbols.pathChild(runtime, parent_path_ref, step_kind, expected_view_kind, selector);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPath(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number {
  const result = symbols.viewTextLayoutPatchPath(runtime, base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD1(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, wrap: number, align: number): number {
  const result = symbols.viewTextLayoutPatchPathD1(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD2(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, wrap: number, align: number): number {
  const result = symbols.viewTextLayoutPatchPathD2(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD3(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, wrap: number, align: number): number {
  const result = symbols.viewTextLayoutPatchPathD3(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextLayoutPatchPathD4(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, path_ref: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number {
  const result = symbols.viewTextLayoutPatchPathD4(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function editTxnBegin(symbols: ViewAbiSymbols, runtime: Pointer, base_root_ref: number, expected_edit_count: number): number {
  const result = symbols.editTxnBegin(runtime, base_root_ref, expected_edit_count);
  return checkedRef(symbols, runtime, result);
}

export function editTxnAddTextLayout(symbols: ViewAbiSymbols, runtime: Pointer, txn_ref: number, path_ref: number, path_depth: number, target_node_id_low: number, target_node_id_high: number, ancestor0_node_id_low: number, ancestor0_node_id_high: number, ancestor1_node_id_low: number, ancestor1_node_id_high: number, ancestor2_node_id_low: number, ancestor2_node_id_high: number, ancestor3_node_id_low: number, ancestor3_node_id_high: number, wrap: number, align: number): number {
  const result = symbols.editTxnAddTextLayout(runtime, txn_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align);
  return result;
}

export function editTxnCommitRender(symbols: ViewAbiSymbols, runtime: Pointer, host: Pointer, txn_ref: number): number {
  const result = symbols.editTxnCommitRender(runtime, host, txn_ref);
  return checkedRef(symbols, runtime, result);
}

export function editTxnAbort(symbols: ViewAbiSymbols, runtime: Pointer, txn_ref: number): number {
  const result = symbols.editTxnAbort(runtime, txn_ref);
  return result;
}

export function styleAtomCreateCstring(symbols: ViewAbiSymbols, runtime: Pointer, value: string): number {
  const result = symbols.styleAtomCreateCstring(runtime, value);
  return checkedRef(symbols, runtime, result);
}

export function styleCreateBits(symbols: ViewAbiSymbols, runtime: Pointer, flags: number, attribute_present: number, attribute_true: number, foreground_ref: number, background_ref: number, theme_atom_ref: number): number {
  const result = symbols.styleCreateBits(runtime, flags, attribute_present, attribute_true, foreground_ref, background_ref, theme_atom_ref);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, text: string, style_ref: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateCstring(runtime, node_id_low, node_id_high, text, style_ref, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf8(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, bytes: NodeJS.TypedArray | DataView, used_bytes: number, style_ref: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateUtf8(runtime, node_id_low, node_id_high, bytes, bytes, used_bytes, style_ref, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf82(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, bytes: NodeJS.TypedArray | DataView, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateUtf82(runtime, node_id_low, node_id_high, bytes, bytes, used_bytes, span0_bytes, style0, span1_bytes, style1, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf83(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, bytes: NodeJS.TypedArray | DataView, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, span2_bytes: number, style2: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateUtf83(runtime, node_id_low, node_id_high, bytes, bytes, used_bytes, span0_bytes, style0, span1_bytes, style1, span2_bytes, style2, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateUtf84(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, bytes: NodeJS.TypedArray | DataView, used_bytes: number, span0_bytes: number, style0: number, span1_bytes: number, style1: number, span2_bytes: number, style2: number, span3_bytes: number, style3: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateUtf84(runtime, node_id_low, node_id_high, bytes, bytes, used_bytes, span0_bytes, style0, span1_bytes, style1, span2_bytes, style2, span3_bytes, style3, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring2(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateCstring2(runtime, node_id_low, node_id_high, text0, style0, text1, style1, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring3(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, text2: string, style2: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateCstring3(runtime, node_id_low, node_id_high, text0, style0, text1, style1, text2, style2, wrap, align);
  return checkedRef(symbols, runtime, result);
}

export function viewTextCreateCstring4(symbols: ViewAbiSymbols, runtime: Pointer, node_id_low: number, node_id_high: number, text0: string, style0: number, text1: string, style1: number, text2: string, style2: number, text3: string, style3: number, wrap: number, align: number): number {
  const result = symbols.viewTextCreateCstring4(runtime, node_id_low, node_id_high, text0, style0, text1, style1, text2, style2, text3, style3, wrap, align);
  return checkedRef(symbols, runtime, result);
}

