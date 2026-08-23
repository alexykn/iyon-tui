/**
 * PERF-12 T4: eager immutable semantic View DAG (PERF-12 handoff §13/§14/§84).
 *
 * Production has been restored to the historical 7v2 semantic model
 * (e5292d62c4011610850cbdc1ba4a35f296f78e4f), mechanically adapted to the
 * current schema. NodeId assignment, freezing, child identity sharing, and
 * the lookup-only nodeForBridge are exactly 7v2. The post-7v2 text-span
 * style pushdown was evaluated and dropped: under the eager model it is
 * slower than the 7v2 decorated wrapper at construction (measured 1,145 ns
 * vs 661 ns for one modifier) and is render-equivalent, so faithful 7v2
 * semantics win; a cheaper form can return as a PERF-12 §27 derivation hint
 * in the T9 tranche if it ever measures as a net win.
 *
 * The pending create/patch backings and the packed-transport metadata
 * coupling (registerPackedMeta at construction, *ForPackedTransport statics,
 * sequence overrides) have been removed: every transport candidate except
 * direct_7v2 and PERF-12 retained-DAG FFI was ruled out by PERF-11v4
 * category D, so their JS-side machinery is no longer kept alive. The native
 * packed decoders remain in iyon-native untouched (T4 is a no-native-change
 * tranche); they are simply unreachable from this module.
 *
 * The recipe reader functions (nativeAxisRecipe, nativeTextRecipe,
 * nativeSpacerRecipe, nativeScalarPatch, viewBackingState) are retained as
 * always-undefined stubs so the generated-route code in native_view_abi.ts
 * keeps compiling; under the eager DAG those routes never fire and every
 * render lands on the Direct decode path (measured faster on realistic
 * traces by PERF-11v4). Their removal happens with the route code in the
 * PERF-12 cleanup tranche.
 */

import type { NativeHandleId } from "../types.ts";
import {
  BRIDGE_DIFF_LINE_KIND,
  BRIDGE_DIFF_LINE_TERMINATION,
  BRIDGE_GRID_TRACK_KIND,
  BRIDGE_HORIZONTAL_ALIGN,
  BRIDGE_LAYOUT_CHILD_KIND,
  BRIDGE_OVERFLOW_KIND,
  BRIDGE_VIEW_KIND,
  BRIDGE_VERTICAL_ALIGN,
  BRIDGE_WRAP_MODE,
  cloneDecoration,
  cloneStyle,
  emptyDecoration,
  emptyStyle,
  mergeStyles,
  type BorderNode,
  type BridgeDiffHunkNode,
  type BridgeGridCellNode,
  type BridgeGridRowNode,
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeViewNode,
  type BridgeViewNodeDraft,
  type TextSpanNode,
  type ColorNode,
  type DecorationNode,
  type DiffHunkNode,
  type DiffLineNode,
  type GridTrackNode,
  type OverflowIndicatorNode,
  type StyleNode,
  VIEW_BRIDGE_SCHEMA_VERSION,
} from "../ir.ts";
import { insets, Insets } from "./geometry.ts";
import {
  setBridgeDerivation,
  setBridgeSequenceOverride,
  peekBridgeSequenceOverride,
  setBridgeGridSequenceOverride,
  peekBridgeGridSequenceOverride,
  type AxisSequenceEdit,
  type BridgeCommonScalarDerivation,
  type BridgeDerivation,
} from "../ir.ts";
import { PersistentSeq } from "../persistent_seq.ts";
import { StyleSpec } from "./style.ts";
import { TextSpan, type HorizontalAlign, type WrapMode } from "./text.ts";

type ChildBuilder = readonly View[] | ((builder: ChildrenBuilder) => void);
type CounterBox = { next: number };
const NODE_ID_COUNTER = Symbol.for("iyon:tui:private-view-node-counter");
const globalRoot = globalThis as typeof globalThis & { [NODE_ID_COUNTER]?: CounterBox };
const nodeIdCounter = globalRoot[NODE_ID_COUNTER] ??= { next: 1 };
const WIDE_AXIS_SEQUENCE_THRESHOLD = 1_024;
const WIDE_GRID_SEQUENCE_THRESHOLD = 1_024;

function nextNodeId(): number {
  if (nodeIdCounter.next > Number.MAX_SAFE_INTEGER) throw new Error("TUI View node identity exhausted");
  return nodeIdCounter.next++;
}

export interface NativeTextLayoutPatch {
  readonly kind: "textLayout";
  readonly base: View;
  readonly wrap: number;
  readonly align: number;
}

export interface NativeCommonPatch {
  readonly kind: "common";
  readonly base: View;
  readonly mask: number;
  readonly paddingTopRight: number;
  readonly paddingBottomLeft: number;
  readonly widthRule: number;
  readonly heightRule: number;
  readonly minWidth: number;
  readonly maxWidth: number;
  readonly minHeight: number;
  readonly maxHeight: number;
}

export type NativeScalarPatch = NativeTextLayoutPatch | NativeCommonPatch;

export type NativeStructuralEdit =
  | { readonly kind: "axisSet"; readonly base: View; readonly child: View; readonly index: number; readonly trackWord: number }
  | { readonly kind: "axisSplice"; readonly base: View; readonly children: readonly { readonly view: View; readonly trackWord: number }[]; readonly index: number; readonly removeCount: number }
  | { readonly kind: "gridCell"; readonly base: View; readonly child: View; readonly row: number; readonly column: number };

/** Native retained-path metadata; it stores selectors, never a View graph. */
export interface NativePathStep {
  readonly kind: number;
  readonly expectedViewKind: number;
  readonly selector: number;
}

/** Lineage metadata for retained-path constructions; references NodeIds only. */
export interface NativePathLineage {
  readonly baseNodeId: number;
  readonly parent?: NativePathLineage;
  readonly step?: NativePathStep;
  readonly depth: number;
}

export interface NativeTextLayoutTransactionEdit {
  readonly lineage: NativePathLineage;
  readonly nodeIds: readonly number[];
  readonly wrap: number;
  readonly align: number;
}

export const NATIVE_PATH_VIEW_KIND = Object.freeze({
  text: 1,
  row: 2,
  column: 3,
  grid: 4,
  hanging: 5,
  container: 6,
  clampRows: 7,
});

export const NATIVE_PATH_STEP = Object.freeze({
  containerChild: 1,
  clampChild: 2,
  rowViewportChild: 3,
  columnChild: 4,
  rowChild: 5,
  gridCell: 6,
  hangingPrefix: 7,
  hangingContinuation: 8,
  hangingBody: 9,
});

export type OverflowIndicator =
  | { readonly kind: "none" }
  | { readonly kind: "ellipsis"; readonly style: StyleSpec }
  | { readonly kind: "footer"; readonly prefix: string; readonly style: StyleSpec };

export type GridTrack = GridTrackNode;

export interface GridCell {
  readonly view: View;
  readonly columnSpan?: number;
  readonly rowSpan?: number;
  readonly horizontalAlign?: "start" | "center" | "end";
  readonly verticalAlign?: "top" | "center" | "bottom";
}

export interface GridRow {
  readonly track?: GridTrack;
  readonly cells: readonly GridCell[];
}

export interface GridSpec {
  readonly columns?: readonly GridTrack[];
  readonly rows: readonly GridRow[];
  readonly columnGap?: number;
  readonly rowGap?: number;
}

export class GridRowBuilder {
  readonly cells: GridCell[] = [];
  cell(view: View): this { this.cells.push({ view }); return this; }
  cellWith(spec: Omit<GridCell, "view">, view: View): this { this.cells.push({ ...spec, view }); return this; }
}

export class GridBuilder {
  columnsValue: GridTrack[] = [];
  rows: GridRow[] = [];
  columnGapValue = 0;
  rowGapValue = 0;
  columns(columns: readonly GridTrack[]): this { this.columnsValue = [...columns]; return this; }
  columnGap(value: number): this { this.columnGapValue = validateU16(value, "columnGap"); return this; }
  rowGap(value: number): this { this.rowGapValue = validateU16(value, "rowGap"); return this; }
  row(build: ((row: GridRowBuilder) => void) | GridRow): this {
    if (typeof build === "function") {
      const row = new GridRowBuilder();
      build(row);
      this.rows.push({ cells: row.cells });
    } else this.rows.push(build);
    return this;
  }
  rowWith(track: GridTrack, build: (row: GridRowBuilder) => void): this {
    const row = new GridRowBuilder();
    build(row);
    this.rows.push({ track, cells: row.cells });
    return this;
  }
}

export class ChildrenBuilder {
  private readonly entries: BridgeLayoutChild[] = [];
  private layoutGap = 0;
  get children(): BridgeLayoutChild[] { return this.entries; }
  child(view: View): this { this.entries.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.normal, child: nodeForBridge(view) }); return this; }
  childrenOf(views: readonly View[]): this { for (const view of views) this.child(view); return this; }
  gap(value: number): this { this.layoutGap = validateU16(value, "gap"); return this; }
  fixed(size: number, view: View): this {
    this.entries.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.fixed, size: validateU16(size, "size"), child: nodeForBridge(view) });
    return this;
  }
  flex(view: View): this { this.entries.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.flex, child: nodeForBridge(view) }); return this; }
  flexMax(maxRows: number, view: View): this {
    validateU16(maxRows, "maxRows");
    this.entries.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.flexMax, maxRows, child: nodeForBridge(view) });
    return this;
  }
  contentMax(maxRows: number, view: View): this {
    validateU16(maxRows, "maxRows");
    this.entries.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.contentMax, maxRows, child: nodeForBridge(view) });
    return this;
  }
  gapValue(): number { return this.layoutGap; }
}

function withPrivateIdentity(node: BridgeViewNode | BridgeViewNodeDraft): BridgeViewNode {
  const { id: _oldId, schema: _oldSchema, ...draft } = node as BridgeViewNode;
  return freezeBridgeNode({ id: nextNodeId(), schema: VIEW_BRIDGE_SCHEMA_VERSION, ...draft } as BridgeViewNode);
}

function freezeColor(color: ColorNode | undefined): void {
  if (color !== undefined && typeof color === "object") Object.freeze(color);
}

function freezeStyle(style: StyleNode): void {
  freezeColor(style.foreground);
  freezeColor(style.background);
  Object.freeze(style.attributes);
  Object.freeze(style);
}

function freezeDecoration(decoration: DecorationNode): void {
  if (decoration.padding !== undefined) Object.freeze(decoration.padding);
  freezeColor(decoration.background);
  freezeColor(decoration.foreground);
  if (decoration.border !== undefined) {
    if (decoration.border.glyphs !== undefined) Object.freeze(decoration.border.glyphs);
    freezeColor(decoration.border.color);
    Object.freeze(decoration.border);
  }
  freezeStyle(decoration.style);
  if (decoration.styleStates !== undefined) Object.freeze(decoration.styleStates);
  Object.freeze(decoration);
}

function freezeOverflow(overflow: BridgeOverflowIndicatorNode | undefined): void {
  if (overflow === undefined) return;
  if (overflow.kind !== BRIDGE_OVERFLOW_KIND.none) freezeStyle(overflow.style);
  Object.freeze(overflow);
}

function freezeDiff(hunks: readonly BridgeDiffHunkNode[]): void {
  for (const hunk of hunks) {
    Object.freeze(hunk.oldRange);
    Object.freeze(hunk.newRange);
    for (const line of hunk.lines) Object.freeze(line);
    Object.freeze(hunk.lines);
    Object.freeze(hunk);
  }
  Object.freeze(hunks);
}

function freezeBridgeNode(node: BridgeViewNode): BridgeViewNode {
  switch (node.kind) {
    case BRIDGE_VIEW_KIND.text:
      for (const span of node.spans) {
        if (span.style !== undefined) freezeStyle(span.style);
        Object.freeze(span);
      }
      Object.freeze(node.spans);
      break;
    case BRIDGE_VIEW_KIND.diff:
      freezeDiff(node.hunks);
      break;
    case BRIDGE_VIEW_KIND.row:
    case BRIDGE_VIEW_KIND.column:
      for (const child of node.children) Object.freeze(child);
      Object.freeze(node.children);
      break;
    case BRIDGE_VIEW_KIND.grid:
      for (const track of node.columns) Object.freeze(track);
      Object.freeze(node.columns);
      for (const row of node.rows) {
        Object.freeze(row.track);
        for (const cell of row.cells) Object.freeze(cell);
        Object.freeze(row.cells);
        Object.freeze(row);
      }
      Object.freeze(node.rows);
      break;
    case BRIDGE_VIEW_KIND.container:
    case BRIDGE_VIEW_KIND.contentMax:
      break;
    case BRIDGE_VIEW_KIND.clamp:
      freezeOverflow(node.overflow);
      break;
    case BRIDGE_VIEW_KIND.decorated:
      freezeDecoration(node.decoration);
      break;
    case BRIDGE_VIEW_KIND.hanging:
    case BRIDGE_VIEW_KIND.spacer:
    case BRIDGE_VIEW_KIND.component:
      break;
  }
  return Object.freeze(node);
}

export class View {
  readonly kind = "view" as const;

  private constructor(node: BridgeViewNode | BridgeViewNodeDraft) {
    nodes.set(this, withPrivateIdentity(node));
    Object.freeze(this);
  }

  static contentMax(maxRows: number, child: View): View {
    validateU16(maxRows, "maxRows");
    return new View({ kind: BRIDGE_VIEW_KIND.contentMax, child: nodeForBridge(child), maxRows });
  }

  static diff(hunks: readonly DiffHunkNode[]): View {
    return new View({ kind: BRIDGE_VIEW_KIND.diff, hunks: hunks.map(toBridgeHunk) });
  }

  static text(value: string): View {
    if (typeof value !== "string") throw new TypeError("View.text requires a string");
    return new View({ kind: BRIDGE_VIEW_KIND.text, spans: [{ text: value }], wrap: BRIDGE_WRAP_MODE.wordThenGrapheme, align: BRIDGE_HORIZONTAL_ALIGN.start });
  }

  static styledText(spans: readonly TextSpan[]): View {
    return new View({ kind: BRIDGE_VIEW_KIND.text, spans: spans.map((span) => ({
      ...span.value,
      style: span.value.style === undefined ? undefined : cloneStyle(span.value.style),
    })), wrap: BRIDGE_WRAP_MODE.wordThenGrapheme, align: BRIDGE_HORIZONTAL_ALIGN.start });
  }

  static spacer(rows: number): View {
    validateU16(rows, "rows");
    return new View({ kind: BRIDGE_VIEW_KIND.spacer, rows });
  }

  static horizontal(children: ChildBuilder): View {
    const builder = buildChildren(children);
    const view = new View({ kind: BRIDGE_VIEW_KIND.row, children: builder.children, gap: builder.gapValue() });
    seedWideAxisSequence(view, builder.children);
    return view;
  }

  static vertical(children: ChildBuilder): View {
    const builder = buildChildren(children);
    const view = new View({ kind: BRIDGE_VIEW_KIND.column, children: builder.children, gap: builder.gapValue() });
    seedWideAxisSequence(view, builder.children);
    return view;
  }

  static hanging(prefix: View, continuation: View, body: View): View {
    return new View({ kind: BRIDGE_VIEW_KIND.hanging, prefix: nodeForBridge(prefix), continuation: nodeForBridge(continuation), body: nodeForBridge(body) });
  }

  static grid(specification: readonly View[] | GridSpec | ((builder: GridBuilder) => void)): View {
    const builder = new GridBuilder();
    if (Array.isArray(specification)) {
      builder.columns(specification.map(() => ({ kind: "content" as const })));
      builder.row((row) => specification.forEach((view) => row.cell(view)));
    } else if (typeof specification === "function") specification(builder);
    else {
      const spec = specification as GridSpec;
      builder.columns(spec.columns ?? []);
      for (const row of spec.rows) builder.row(row);
      builder.columnGap(spec.columnGap ?? 0).rowGap(spec.rowGap ?? 0);
    }
    const rows: BridgeGridRowNode[] = builder.rows.map((row) => ({
      track: bridgeGridTrack(row.track ?? { kind: "content" }),
      cells: row.cells.map((cell): BridgeGridCellNode => ({
        view: nodeForBridge(cell.view),
        columnSpan: validatePositiveU16(cell.columnSpan ?? 1, "columnSpan"),
        rowSpan: validatePositiveU16(cell.rowSpan ?? 1, "rowSpan"),
        horizontalAlign: horizontalAlignCode(cell.horizontalAlign ?? "start"),
        verticalAlign: verticalAlignCode(cell.verticalAlign ?? "top"),
      })),
    }));
    const view = new View({
      kind: BRIDGE_VIEW_KIND.grid,
      columns: builder.columnsValue.map(bridgeGridTrack),
      rows,
      columnGap: builder.columnGapValue,
      rowGap: builder.rowGapValue,
    });
    seedWideGridSequence(view, rows);
    return view;
  }

  /**
   * PERF-12 T10 (§34/§35): retained wide-edit constructors. Each builds
   * the complete new semantic node eagerly (fresh NodeId, frozen shape) but
   * stores the derived axis children as a PersistentSeq - O(log₃₂ N) JS work,
   * no flat array. The sequence override is authoritative; `children`
   * materializes the exact flat array lazily only when a consumer (Direct
   * fallback) asks. The derivation hint lets ensureNative run the exact
   * native retained edit (base ref + NodeId + index + child refs).
   * Not part of the public semantic API.
   */
  static axisSetChildForTransport(base: View, index: number, child: View, trackWord = 0): View {
    const baseNode = nodeForBridge(base);
    if (baseNode.kind !== BRIDGE_VIEW_KIND.row && baseNode.kind !== BRIDGE_VIEW_KIND.column) {
      throw new TypeError("retained axis edit base is not a row or column");
    }
    const baseSequence = bridgeAxisSequence(baseNode);
    if (!Number.isInteger(index) || index < 0 || index >= baseSequence.length) {
      throw new RangeError("retained axis edit index out of range");
    }
    const childNode = nodeForBridge(child);
    const current = baseSequence.get(index)!;
    const next = trackWord === 0 ? { ...current, child: childNode } : layoutChildFromTrackWord(trackWord, childNode);
    const sequence = baseSequence.set(index, next);
    return buildWideAxisNode(baseNode, sequence, { kind: "axisSet", index }, {
      kind: "axisSet",
      base: baseNode,
      index,
      trackWord,
      child: childNode,
    });
  }

  static axisSpliceForTransport(
    base: View,
    index: number,
    removeCount: number,
    inserted: readonly { readonly view: View; readonly trackWord?: number }[],
  ): View {
    const baseNode = nodeForBridge(base);
    if (baseNode.kind !== BRIDGE_VIEW_KIND.row && baseNode.kind !== BRIDGE_VIEW_KIND.column) {
      throw new TypeError("retained axis splice base is not a row or column");
    }
    const baseSequence = bridgeAxisSequence(baseNode);
    if (!Number.isInteger(index) || index < 0 || index > baseSequence.length) {
      throw new RangeError("retained axis splice index out of range");
    }
    if (!Number.isInteger(removeCount) || removeCount < 0 || index + removeCount > baseSequence.length) {
      throw new RangeError("retained axis splice count out of range");
    }
    const insertedChildren = inserted.map((entry) => ({
      node: nodeForBridge(entry.view),
      trackWord: entry.trackWord ?? 0,
    }));
    const insertedLayout = insertedChildren.map((entry) => layoutChildFromTrackWord(entry.trackWord, entry.node));
    const sequence = baseSequence.splice(index, removeCount, ...insertedLayout);
    return buildWideAxisNode(
      baseNode,
      sequence,
      { kind: "axisSplice", index, removeCount, insertedCount: insertedLayout.length },
      { kind: "axisSplice", base: baseNode, index, removeCount, inserted: insertedChildren },
    );
  }

  static gridSetCellForTransport(base: View, row: number, column: number, cellView: View): View {
    const gridNode = nodeForBridge(base);
    if (gridNode.kind !== BRIDGE_VIEW_KIND.grid) throw new TypeError("retained grid cell edit base is not a grid");
    const gridOverride = peekBridgeGridSequenceOverride(gridNode);
    const placement = gridOverride === undefined ? gridPlacement(gridNode.rows) : undefined;
    const rowCount = gridOverride?.rowTracks.length ?? gridNode.rows.length;
    if (!Number.isInteger(row) || row < 0 || row >= rowCount) throw new RangeError("retained grid cell row out of range");
    const childNode = nodeForBridge(cellView);
    if (gridOverride !== undefined) {
      const sequenceIndex = gridOverride.cellIndices[row]?.get(column);
      if (sequenceIndex === undefined) throw new RangeError("retained grid cell column out of range");
      const sequence = gridOverride.sequence.set(sequenceIndex, {
        ...gridOverride.sequence.get(sequenceIndex)!,
        view: childNode,
      });
      const derived = buildWideGridNode(gridNode, gridOverride, sequence);
      setBridgeDerivation(nodeForBridge(derived), {
        kind: "gridCell",
        base: gridNode,
        row,
        column,
        child: childNode,
      });
      return derived;
    }
    const sequenceIndex = placement!.cellIndices[row]?.get(column);
    if (sequenceIndex === undefined) throw new RangeError("retained grid cell column out of range");
    const rowStart = placement!.rowOffsets[row]!;
    const cellIndex = sequenceIndex - rowStart;
    // Narrow grids retain the ordinary eager semantic shape; only the wide
    // sidecar path above avoids copying the addressed row's cell array.
    const rows = gridNode.rows.map((current, rowIndex) =>
      rowIndex === row
        ? { ...current, cells: current.cells.map((cell, index) => index === cellIndex ? { ...cell, view: childNode } : cell) }
        : current,
    );
    const derived = new View({ kind: BRIDGE_VIEW_KIND.grid, columns: gridNode.columns, rows, columnGap: gridNode.columnGap, rowGap: gridNode.rowGap });
    setBridgeDerivation(nodeForBridge(derived), {
      kind: "gridCell",
      base: gridNode,
      row,
      column,
      child: childNode,
    });
    return derived;
  }

  static component(handle: { readonly id: NativeHandleId; nativeComponentId?: () => number | undefined }): View {
    const nativeId = handle.nativeComponentId?.();
    return new View({ kind: BRIDGE_VIEW_KIND.component, handle: (nativeId ?? handle.id) as NativeHandleId });
  }

  /**
   * PERF-12 T13.1 (@internal): construct an axis view from pre-composed
   * layout entries so the composition layer builds the builder callback
   * EXACTLY ONCE and reuses the entries for both reuse-check and construction
   * (§19). Equivalent to View.vertical/horizontal over a builder callback,
   * including the wide-sequence seed. Reinstituted in R1 for the scoped arm.
   */
  static __composedAxis(row: boolean, entries: BridgeLayoutChild[], gap: number): View {
    const view = new View({ kind: row ? BRIDGE_VIEW_KIND.row : BRIDGE_VIEW_KIND.column, children: entries, gap });
    seedWideAxisSequence(view, entries);
    return view;
  }

  /** Internal retained-path constructor; not part of the public semantic API. */
  static textLayoutAtNativePathForTransport(
    view: View,
    steps: readonly NativePathStep[],
    wrap: WrapMode,
    align: HorizontalAlign,
  ): View {
    if (steps.length > 4) throw new RangeError("native retained path depth must be at most 4");
    const nextNode = patchBridgeTextPath(nodeForBridge(view), steps, wrapCode(wrap), horizontalAlignCode(align));
    let lineage: NativePathLineage = Object.freeze({ baseNodeId: nodeForBridge(view).id, depth: 0 });
    for (const step of steps) lineage = nativePathChildLineage(view, lineage, step);
    const result = new View(nextNode);
    nativePathLineages.set(result, freezeNativePathLineage(lineage));
    return result;
  }

  /**
   * Internal retained-transport constructor for multiple independent text
   * edits. Builds complete eagerly-materialized path-patched nodes and
   * records typed transaction metadata for the generated transaction call.
   */
  static textLayoutTransactionForTransport(
    view: View,
    edits: readonly {
      readonly steps: readonly NativePathStep[];
      readonly wrap: WrapMode;
      readonly align: HorizontalAlign;
    }[],
  ): View {
    if (edits.length < 2 || edits.length > 256) {
      throw new RangeError("native text transaction must contain 2 through 256 edits");
    }
    const seen = new Set<string>();
    let node = nodeForBridge(view);
    for (const edit of edits) {
      if (edit.steps.length > 4) throw new RangeError("native retained transaction path depth must be at most 4");
      const key = edit.steps.map((step) => `${step.kind}:${step.expectedViewKind}:${step.selector}`).join("/");
      if (!seen.add(key)) throw new RangeError("native text transaction paths must be distinct");
      node = patchBridgeTextPath(node, edit.steps, wrapCode(edit.wrap), horizontalAlignCode(edit.align));
    }
    const result = new View(node);
    nativeTextLayoutTransactions.set(result, buildTransactionEdits(view, nodeForBridge(result), edits));
    return result;
  }

  bold(): View { return this.textAttribute("bold"); }
  dim(): View { return this.textAttribute("dim"); }
  italic(): View { return this.textAttribute("italic"); }
  underline(): View { return this.textAttribute("underline"); }
  reversed(): View { return this.textAttribute("reversed"); }
  strikethrough(): View { return this.textAttribute("strikethrough"); }
  textAttribute(name: string, enabled = true): View { return this.decorate({ style: { ...emptyStyle(), attributes: { [name]: enabled } } }); }
  padding(value: number | Insets): View { return this.decorate({ padding: insets(value) }); }
  background(color: ColorNode): View { return this.decorate({ background: color }); }
  foreground(color: ColorNode): View { return this.decorate({ foreground: color }); }
  border(border: BorderNode): View { return this.decorate({ border }); }
  style(style: StyleSpec): View { return this.decorate({ style: mergeStyles(emptyStyle(), style.value) }); }

  styleState(key: string, value: string): View {
    if (key.length === 0 || value.length === 0) throw new RangeError("style state key and value cannot be empty");
    const decorated = this.decoratedNode();
    const current = decorated === undefined ? emptyDecoration() : cloneDecoration(decorated.decoration);
    const child = decorated?.child ?? nodeForBridge(this);
    return new View({ kind: BRIDGE_VIEW_KIND.decorated, child, decoration: { ...current, styleStates: { ...current.styleStates, [key]: value } } });
  }

  container(): View { return new View({ kind: BRIDGE_VIEW_KIND.container, child: nodeForBridge(this) }); }
  clampRows(maxRows: number, overflow: OverflowIndicator = { kind: "none" }): View {
    validateU16(maxRows, "maxRows");
    return new View({ kind: BRIDGE_VIEW_KIND.clamp, child: nodeForBridge(this), maxRows, overflow: bridgeOverflow(overflow) });
  }
  fitWidth(): View { return this.decorate({ width: "fit" }); }
  fillWidth(): View { return this.decorate({ width: "fill" }); }
  fitHeight(): View { return this.decorate({ height: "fit" }); }
  fillHeight(): View { return this.decorate({ height: "fill" }); }
  minWidth(value: number): View { return this.decorate({ minWidth: validateU16(value, "minWidth") }); }
  maxWidth(value: number): View { return this.decorate({ maxWidth: validateU16(value, "maxWidth") }); }
  minHeight(value: number): View { return this.decorate({ minHeight: validateU16(value, "minHeight") }); }
  maxHeight(value: number): View { return this.decorate({ maxHeight: validateU16(value, "maxHeight") }); }
  wrap(mode: WrapMode): View { return this.textLayoutPatch(wrapCode(mode), undefined); }
  noWrap(): View { return this.wrap("noWrap"); }
  textAlign(align: HorizontalAlign): View { return this.textLayoutPatch(undefined, horizontalAlignCode(align)); }

  private decoratedNode(): Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated }> | undefined {
    const node = nodeForBridge(this);
    return node.kind === BRIDGE_VIEW_KIND.decorated ? node as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated }> : undefined;
  }

  private decorate(decoration: Partial<DecorationNode>): View {
    const decorated = this.decoratedNode();
    const current = decorated === undefined ? emptyDecoration() : cloneDecoration(decorated.decoration);
    const child = decorated?.child ?? nodeForBridge(this);
    const next: DecorationNode = { ...current, ...decoration, style: decoration.style === undefined ? current.style : mergeStyles(current.style, decoration.style) };
    const derived = new View({ kind: BRIDGE_VIEW_KIND.decorated, child, decoration: cloneDecoration(next) });
    // PERF-12 T9 (§27/§28): a scalar-only decoration is exactly
    // `base + masked modifiers`, which the retained common patch expresses
    // without re-materializing the base subtree. Mixed decorations stay
    // unhinted and route through normal materialization/fallback.
    const scalarDerivation = commonScalarDerivation(child, next);
    if (scalarDerivation !== undefined) setBridgeDerivation(nodeForBridge(derived), scalarDerivation);
    return derived;
  }

  private textLayoutPatch(wrap: number | undefined, align: number | undefined): View {
    const node = nodeForBridge(this);
    if (node.kind === BRIDGE_VIEW_KIND.text) {
      const derived = new View({ ...node, ...(wrap === undefined ? {} : { wrap }), ...(align === undefined ? {} : { align }) });
      // PERF-12 T9 (§27/§38): a wrap/align-only text change is derivable —
      // the retained path may clone the base's native text payload with new
      // layout scalars instead of re-importing the payload.
      setBridgeDerivation(nodeForBridge(derived), {
        kind: "textLayout",
        base: node,
        wrap: wrap ?? node.wrap,
        align: align ?? node.align,
      });
      return derived;
    }
    if (node.kind === BRIDGE_VIEW_KIND.decorated && node.child.kind === BRIDGE_VIEW_KIND.text) {
      const decorated = node as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated }>;
      const baseText = decorated.child as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }>;
      const child = { ...baseText, ...(wrap === undefined ? {} : { wrap }), ...(align === undefined ? {} : { align }) };
      const derived = new View({ ...decorated, child });
      const derivedNode = nodeForBridge(derived);
      if (derivedNode.kind === BRIDGE_VIEW_KIND.decorated) {
        setBridgeDerivation(derivedNode.child, {
          kind: "textLayout",
          base: baseText,
          wrap: wrap ?? baseText.wrap,
          align: align ?? baseText.align,
        });
      }
      return derived;
    }
    return this;
  }
}

/**
 * PERF-12 T9 (§27/§28): encodes a scalar-only decoration as a common-scalar
 * derivation over `base`. Returns undefined for any decoration carrying
 * non-scalar content (color, border, non-empty style, styleStates) or no
 * scalar modifier at all — those have no exact retained primitive and stay
 * unhinted (§27: the hint must not guess).
 *
 * Mask bit values mirror the native view_common_patch_root implementation.
 */
const PATCH_PADDING = 4;
const PATCH_WIDTH = 8;
const PATCH_HEIGHT = 16;
const PATCH_MIN_WIDTH = 32;
const PATCH_MAX_WIDTH = 64;
const PATCH_MIN_HEIGHT = 128;
const PATCH_MAX_HEIGHT = 256;

function isEmptyStyle(style: StyleNode): boolean {
  return style.theme === undefined && style.foreground === undefined && style.background === undefined
    && Object.keys(style.attributes).length === 0;
}

function commonScalarDerivation(base: BridgeViewNode, decoration: DecorationNode): BridgeCommonScalarDerivation | undefined {
  if (decoration.background !== undefined || decoration.foreground !== undefined || decoration.border !== undefined) return undefined;
  if (decoration.styleStates !== undefined && Object.keys(decoration.styleStates).length > 0) return undefined;
  if (!isEmptyStyle(decoration.style)) return undefined;
  let mask = 0;
  let paddingTopRight = 0;
  let paddingBottomLeft = 0;
  if (decoration.padding !== undefined) {
    mask |= PATCH_PADDING;
    paddingTopRight = (decoration.padding.top & 0xffff) | ((decoration.padding.right & 0xffff) << 16);
    paddingBottomLeft = (decoration.padding.bottom & 0xffff) | ((decoration.padding.left & 0xffff) << 16);
  }
  let widthRule = 0;
  if (decoration.width !== undefined) {
    mask |= PATCH_WIDTH;
    widthRule = decoration.width === "fit" ? 1 : 2;
  }
  let heightRule = 0;
  if (decoration.height !== undefined) {
    mask |= PATCH_HEIGHT;
    heightRule = decoration.height === "fit" ? 1 : 2;
  }
  const minWidth = decoration.minWidth ?? 0;
  if (decoration.minWidth !== undefined) mask |= PATCH_MIN_WIDTH;
  const maxWidth = decoration.maxWidth ?? 0;
  if (decoration.maxWidth !== undefined) mask |= PATCH_MAX_WIDTH;
  const minHeight = decoration.minHeight ?? 0;
  if (decoration.minHeight !== undefined) mask |= PATCH_MIN_HEIGHT;
  const maxHeight = decoration.maxHeight ?? 0;
  if (decoration.maxHeight !== undefined) mask |= PATCH_MAX_HEIGHT;
  if (mask === 0) return undefined;
  return {
    kind: "commonScalar",
    base,
    mask,
    paddingTopRight,
    paddingBottomLeft,
    widthRule,
    heightRule,
    minWidth,
    maxWidth,
    minHeight,
    maxHeight,
  };
}

function seedWideAxisSequence(view: View, children: readonly BridgeLayoutChild[]): void {
  if (children.length <= WIDE_AXIS_SEQUENCE_THRESHOLD) return;
  const node = nodeForBridge(view);
  setBridgeSequenceOverride(node, {
    baseNode: node,
    sequence: PersistentSeq.from(children),
  });
}

function gridPlacement(rows: readonly BridgeGridRowNode[]): {
  readonly rowOffsets: readonly number[];
  readonly cellIndices: readonly ReadonlyMap<number, number>[];
} {
  const occupiedUntil: number[] = [];
  const rowOffsets = [0];
  const cellIndices: Map<number, number>[] = [];
  let sequenceIndex = 0;
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex]!;
    const rowIndexMap = new Map<number, number>();
    cellIndices.push(rowIndexMap);
    for (const cell of row.cells) {
      const columnSpan = cell.columnSpan;
      let column = 0;
      for (;;) {
        while (occupiedUntil.length < column + columnSpan) occupiedUntil.push(0);
        let available = true;
        for (let index = column; index < column + columnSpan; index += 1) {
          if (occupiedUntil[index]! > rowIndex) {
            available = false;
            break;
          }
        }
        if (available) break;
        column += 1;
      }
      rowIndexMap.set(column, sequenceIndex);
      sequenceIndex += 1;
      const occupiedThrough = rowIndex + cell.rowSpan;
      for (let index = column; index < column + columnSpan; index += 1) occupiedUntil[index] = occupiedThrough;
    }
    rowOffsets.push(sequenceIndex);
  }
  return {
    rowOffsets: Object.freeze(rowOffsets),
    cellIndices: Object.freeze(cellIndices.map((indices) => indices)),
  };
}

function seedWideGridSequence(view: View, rows: readonly BridgeGridRowNode[]): void {
  const totalCells = rows.reduce((total, row) => total + row.cells.length, 0);
  if (totalCells <= WIDE_GRID_SEQUENCE_THRESHOLD) return;
  const placement = gridPlacement(rows);
  const rowTracks: BridgeGridTrackNode[] = rows.map((row) => row.track);
  const cells: BridgeGridCellNode[] = [];
  for (const row of rows) cells.push(...row.cells);
  const node = nodeForBridge(view);
  setBridgeGridSequenceOverride(node, {
    baseNode: node,
    sequence: PersistentSeq.from(cells),
    rowOffsets: placement.rowOffsets,
    rowTracks: Object.freeze(rowTracks),
    cellIndices: placement.cellIndices,
  });
}

function buildWideGridNode(
  baseNode: BridgeViewNode,
  override: {
    readonly rowOffsets: readonly number[];
    readonly rowTracks: readonly BridgeGridTrackNode[];
    readonly cellIndices: readonly ReadonlyMap<number, number>[];
  },
  sequence: PersistentSeq<BridgeGridCellNode>,
): View {
  let flatRows: readonly BridgeGridRowNode[] | undefined;
  const node = Object.freeze({
    id: nextNodeId(),
    schema: VIEW_BRIDGE_SCHEMA_VERSION,
    kind: BRIDGE_VIEW_KIND.grid,
    columns: baseNode.kind === BRIDGE_VIEW_KIND.grid ? baseNode.columns : [],
    get rows(): readonly BridgeGridRowNode[] {
      if (flatRows === undefined) {
        const rows: BridgeGridRowNode[] = [];
        for (let rowIndex = 0; rowIndex < override.rowTracks.length; rowIndex += 1) {
          const start = override.rowOffsets[rowIndex]!;
          const end = override.rowOffsets[rowIndex + 1]!;
          const cells: BridgeGridCellNode[] = [];
          for (let index = start; index < end; index += 1) cells.push(Object.freeze({ ...sequence.get(index)! }));
          rows.push(Object.freeze({ track: override.rowTracks[rowIndex]!, cells: Object.freeze(cells) }));
        }
        flatRows = Object.freeze(rows);
      }
      return flatRows;
    },
    columnGap: baseNode.kind === BRIDGE_VIEW_KIND.grid ? baseNode.columnGap : 0,
    rowGap: baseNode.kind === BRIDGE_VIEW_KIND.grid ? baseNode.rowGap : 0,
  }) as unknown as BridgeViewNode;
  setBridgeGridSequenceOverride(node, {
    baseNode,
    sequence,
    rowOffsets: override.rowOffsets,
    rowTracks: override.rowTracks,
    cellIndices: override.cellIndices,
  });
  return wrapFrozenBridgeNode(node);
}

/**
 * PERF-12 T10 (§34): authoritative children sequence of an axis node - the
 * wide override when present, else a one-time snapshot of the flat array.
 */
function bridgeAxisSequence(node: BridgeViewNode): PersistentSeq<BridgeLayoutChild> {
  const override = peekBridgeSequenceOverride(node);
  if (override !== undefined) return override.sequence;
  if (node.kind !== BRIDGE_VIEW_KIND.row && node.kind !== BRIDGE_VIEW_KIND.column) {
    throw new TypeError("axis sequence requested from a non-axis node");
  }
  return PersistentSeq.from(node.children);
}

/** Inverse of the generated layoutTrackWord encoding (axis track words). */
function layoutChildFromTrackWord(trackWord: number, child: BridgeViewNode): BridgeLayoutChild {
  const kind = trackWord & 0xff;
  const amount = trackWord >>> 8;
  switch (kind) {
    case 0: return { kind: BRIDGE_LAYOUT_CHILD_KIND.normal, child };
    case 2:
      if (amount === 0 || amount > 0xffff) throw new RangeError("retained track word contentMax rows out of range");
      return { kind: BRIDGE_LAYOUT_CHILD_KIND.contentMax, maxRows: amount, child };
    case 3:
      if (amount === 0 || amount > 0xffff) throw new RangeError("retained track word fixed size out of range");
      return { kind: BRIDGE_LAYOUT_CHILD_KIND.fixed, size: amount, child };
    case 4: return { kind: BRIDGE_LAYOUT_CHILD_KIND.flex, child };
    case 5:
      if (amount === 0 || amount > 0xffff) throw new RangeError("retained track word flexMax rows out of range");
      return { kind: BRIDGE_LAYOUT_CHILD_KIND.flexMax, maxRows: amount, child };
    default:
      throw new RangeError(`retained track word kind ${kind} is invalid`);
  }
}

/**
 * Builds the frozen derived axis node with sequence-backed lazy children
 * (§34): construction performs O(log₃₂ N) work; the exact flat array is
 * materialized once on first access and cached in the closure.
 */
function buildWideAxisNode(
  baseNode: BridgeViewNode,
  sequence: PersistentSeq<BridgeLayoutChild>,
  edit: AxisSequenceEdit,
  derivation: BridgeDerivation,
): View {
  let flat: readonly BridgeLayoutChild[] | undefined;
  const kind = baseNode.kind as typeof BRIDGE_VIEW_KIND.row | typeof BRIDGE_VIEW_KIND.column;
  const node = Object.freeze({
    id: nextNodeId(),
    schema: VIEW_BRIDGE_SCHEMA_VERSION,
    kind,
    gap: (baseNode as { gap: number }).gap,
    get children(): readonly BridgeLayoutChild[] {
      if (flat === undefined) {
        flat = Object.freeze(sequence.toArray().map((entry) => Object.freeze({ ...entry })));
      }
      return flat;
    },
  }) as unknown as BridgeViewNode;
  setBridgeSequenceOverride(node, { baseNode, sequence, edit });
  setBridgeDerivation(node, derivation);
  // The node is already frozen with its final identity; the normal
  // constructor path would re-assign a NodeId and eagerly walk children
  // (materializing the flat array), defeating §34's laziness.
  return wrapFrozenBridgeNode(node as unknown as BridgeViewNode);
}

/** Installs an already-frozen bridge node under a fresh View wrapper. */
function wrapFrozenBridgeNode(node: BridgeViewNode): View {
  const view = Object.create(View.prototype) as View;
  Object.defineProperty(view, "kind", {
    configurable: true,
    enumerable: true,
    value: "view",
    writable: true,
  });
  nodes.set(view, node);
  return Object.freeze(view) as View;
}

const nodes = new WeakMap<View, BridgeViewNode>();
const nativePathLineages = new WeakMap<View, NativePathLineage>();
const nativeTextLayoutTransactions = new WeakMap<View, readonly NativeTextLayoutTransactionEdit[]>();

function freezeNativePathLineage(lineage: NativePathLineage): NativePathLineage {
  const parent = lineage.parent === undefined ? undefined : freezeNativePathLineage(lineage.parent);
  const step = lineage.step === undefined ? undefined : Object.freeze({ ...lineage.step });
  return Object.freeze({ baseNodeId: lineage.baseNodeId, parent, step, depth: lineage.depth });
}

/** Returns the one-time retained path lineage attached during construction. */
export function nativePathLineage(view: View): NativePathLineage | undefined {
  return nativePathLineages.get(view);
}

/** Internal construction helper used by path-aware retained tests/builders. */
export function nativePathChildLineage(
  base: View,
  parent: NativePathLineage | undefined,
  step: NativePathStep,
): NativePathLineage {
  const baseNodeId = viewNodeId(base);
  if (parent !== undefined && parent.baseNodeId !== baseNodeId) throw new Error("native path lineage base mismatch");
  const immutableStep = Object.freeze({ ...step });
  return Object.freeze({ baseNodeId, parent, step: immutableStep, depth: (parent?.depth ?? 0) + 1 });
}

/** Attaches a root/child path lineage without retaining any child View. */
export function attachNativePathLineage(view: View, lineage: NativePathLineage): void {
  if (lineage.baseNodeId === viewNodeId(view)) throw new Error("native path lineage base must be the previous root");
  nativePathLineages.set(view, freezeNativePathLineage(lineage));
}

/** Returns the full semantic NodeId of the View's frozen semantic node. */
export function viewNodeId(view: View): number {
  return nodeForBridge(view).id;
}

/** Diagnostic placeholder: the eager DAG has no backing states; always 0. */
export function viewBackingState(_view: View): 0 | 1 | 2 {
  return 0;
}

/** Always undefined under the eager DAG; retained until the cleanup tranche removes the dead native-builder routes. */
export function nativeAxisRecipe(_view: View): {
  readonly horizontal: boolean;
  readonly gap: number;
  readonly children: readonly { readonly view: View; readonly trackWord: number }[];
} | undefined {
  return undefined;
}

/** Always undefined under the eager DAG; see nativeAxisRecipe. */
export function nativeTextRecipe(_view: View): {
  readonly spans: readonly TextSpanNode[];
  readonly wrap: number;
  readonly align: number;
} | undefined {
  return undefined;
}

/** Always undefined under the eager DAG; see nativeAxisRecipe. */
export function nativeSpacerRecipe(_view: View): number | undefined {
  return undefined;
}

/** Always undefined under the eager DAG; see nativeAxisRecipe. */
export function nativeScalarPatch(_view: View): NativeScalarPatch | undefined {
  return undefined;
}

/** Always undefined under the eager DAG; see nativeAxisRecipe. */
export function nativeStructuralEdit(_view: View): NativeStructuralEdit | undefined {
  return undefined;
}

/** Returns construction-time typed transaction metadata without rebuilding it. */
export function nativeTextLayoutTransaction(view: View): readonly NativeTextLayoutTransactionEdit[] | undefined {
  return nativeTextLayoutTransactions.get(view);
}

/** Returns the u32 halves of a View's full safe-integer NodeId. */
export function nodeIdPair(view: View): readonly [number, number] {
  const id = nodeForBridge(view).id;
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

/**
 * Current high-water of the private monotonic NodeId allocator (§18).
 * View-bearing boundaries capture this after each successful commit as
 * `nativeLookupCeiling`: only NodeIds at or below it may already exist in the
 * native semantic cache and are eligible for NodeId→NativeRef promotion.
 */
export function viewNodeIdHighWater(): number {
  return nodeIdCounter.next - 1;
}

/** Private bridge access; the retained DAG is never part of the public API. */
export function nodeForBridge(view: View): BridgeViewNode {
  const node = nodes.get(view);
  if (node === undefined) throw new TypeError("view is not a runtime semantic value");
  return node;
}

/**
 * Builds a path-aware immutable value for retained-path differential tests and
 * future structural builders. Construction assigns a fresh NodeId to the
 * changed leaf and every rebuilt ancestor.
 */
export function textLayoutAtNativePathForTransport(
  view: View,
  steps: readonly NativePathStep[],
  wrap: WrapMode,
  align: HorizontalAlign,
): View {
  return View.textLayoutAtNativePathForTransport(view, steps, wrap, align);
}

function buildTransactionEdits(
  base: View,
  finalNode: BridgeViewNode,
  edits: readonly {
    readonly steps: readonly NativePathStep[];
    readonly wrap: WrapMode;
    readonly align: HorizontalAlign;
  }[],
): readonly NativeTextLayoutTransactionEdit[] {
  return Object.freeze(edits.map((edit) => {
    const nodes_ = bridgePathNodesForTransaction(finalNode, edit.steps);
    if (nodes_ === undefined || nodes_[nodes_.length - 1]?.kind !== BRIDGE_VIEW_KIND.text) {
      throw new TypeError("native text transaction path does not terminate at text");
    }
    let lineage: NativePathLineage = Object.freeze({ baseNodeId: viewNodeId(base), depth: 0 });
    for (const step of edit.steps) lineage = nativePathChildLineage(base, lineage, step);
    return Object.freeze({
      lineage,
      nodeIds: Object.freeze(nodes_.slice().reverse().map((entry) => entry.id)),
      wrap: wrapCode(edit.wrap),
      align: horizontalAlignCode(edit.align),
    });
  }));
}

function patchBridgeTextPath(
  node: BridgeViewNode,
  steps: readonly NativePathStep[],
  wrap: number,
  align: number,
): BridgeViewNode {
  const step = steps[0];
  if (step === undefined) {
    if (node.kind !== BRIDGE_VIEW_KIND.text) throw new TypeError("native retained text path must terminate at text");
    return withPrivateIdentity({ ...node, wrap, align });
  }
  if (bridgePathViewKind(node.kind) !== step.expectedViewKind) {
    throw new TypeError("native retained path expected view kind does not match bridge node");
  }
  const tail = steps.slice(1);
  switch (step.kind) {
    case NATIVE_PATH_STEP.containerChild:
    case NATIVE_PATH_STEP.clampChild: {
      if (step.selector !== 0 || (node.kind !== BRIDGE_VIEW_KIND.container && node.kind !== BRIDGE_VIEW_KIND.clamp && node.kind !== BRIDGE_VIEW_KIND.contentMax)) {
        throw new RangeError("native retained single-child path is invalid");
      }
      return withPrivateIdentity({ ...node, child: patchBridgeTextPath(node.child, tail, wrap, align) });
    }
    case NATIVE_PATH_STEP.columnChild: {
      if (node.kind !== BRIDGE_VIEW_KIND.column) throw new TypeError("native retained column path kind is invalid");
      if (!Number.isInteger(step.selector) || step.selector < 0 || step.selector >= node.children.length) throw new RangeError("native retained column path selector is out of range");
      const children = node.children.map((child, index) => index === step.selector
        ? { ...child, child: patchBridgeTextPath(child.child, tail, wrap, align) }
        : child);
      return withPrivateIdentity({ ...node, children });
    }
    case NATIVE_PATH_STEP.rowChild: {
      if (node.kind !== BRIDGE_VIEW_KIND.row) throw new TypeError("native retained row path kind is invalid");
      if (!Number.isInteger(step.selector) || step.selector < 0 || step.selector >= node.children.length) throw new RangeError("native retained row path selector is out of range");
      const children = node.children.map((child, index) => index === step.selector
        ? { ...child, child: patchBridgeTextPath(child.child, tail, wrap, align) }
        : child);
      return withPrivateIdentity({ ...node, children });
    }
    case NATIVE_PATH_STEP.gridCell: {
      if (node.kind !== BRIDGE_VIEW_KIND.grid || !Number.isInteger(step.selector) || step.selector < 0) throw new TypeError("native retained grid path kind is invalid");
      let remaining = step.selector;
      let changed = false;
      const rows = node.rows.map((row) => ({
        ...row,
        cells: row.cells.map((cell) => {
          if (changed || remaining !== 0) {
            if (!changed) remaining -= 1;
            return cell;
          }
          changed = true;
          return { ...cell, view: patchBridgeTextPath(cell.view, tail, wrap, align) };
        }),
      }));
      if (!changed || remaining !== 0) throw new RangeError("native retained grid path selector is out of range");
      return withPrivateIdentity({ ...node, rows });
    }
    case NATIVE_PATH_STEP.hangingPrefix:
    case NATIVE_PATH_STEP.hangingContinuation:
    case NATIVE_PATH_STEP.hangingBody: {
      if (node.kind !== BRIDGE_VIEW_KIND.hanging || step.selector !== 0) throw new TypeError("native retained hanging path is invalid");
      const key = step.kind === NATIVE_PATH_STEP.hangingPrefix ? "prefix" : step.kind === NATIVE_PATH_STEP.hangingContinuation ? "continuation" : "body";
      return withPrivateIdentity({ ...node, [key]: patchBridgeTextPath(node[key], tail, wrap, align) });
    }
    default: throw new TypeError("unknown native retained path step");
  }
}

function bridgePathNodesForTransaction(
  root: BridgeViewNode,
  steps: readonly NativePathStep[],
): BridgeViewNode[] | undefined {
  const collected = [root];
  let current = root;
  for (const step of steps) {
    if (bridgePathViewKind(current.kind) !== step.expectedViewKind) return undefined;
    switch (step.kind) {
      case NATIVE_PATH_STEP.containerChild:
      case NATIVE_PATH_STEP.clampChild:
      case NATIVE_PATH_STEP.rowViewportChild:
        if (step.selector !== 0 || (current.kind !== BRIDGE_VIEW_KIND.container && current.kind !== BRIDGE_VIEW_KIND.clamp && current.kind !== BRIDGE_VIEW_KIND.contentMax)) return undefined;
        current = current.child;
        break;
      case NATIVE_PATH_STEP.columnChild:
      case NATIVE_PATH_STEP.rowChild: {
        if (current.kind !== (step.kind === NATIVE_PATH_STEP.columnChild ? BRIDGE_VIEW_KIND.column : BRIDGE_VIEW_KIND.row)) return undefined;
        const child = current.children[step.selector];
        if (child === undefined) return undefined;
        current = child.child;
        break;
      }
      case NATIVE_PATH_STEP.gridCell: {
        if (current.kind !== BRIDGE_VIEW_KIND.grid || step.selector < 0) return undefined;
        let remaining = step.selector;
        let found: BridgeViewNode | undefined;
        for (const row of current.rows) {
          for (const cell of row.cells) {
            if (remaining === 0) found = cell.view;
            remaining -= 1;
          }
        }
        if (found === undefined || remaining >= 0) return undefined;
        current = found;
        break;
      }
      case NATIVE_PATH_STEP.hangingPrefix:
      case NATIVE_PATH_STEP.hangingContinuation:
      case NATIVE_PATH_STEP.hangingBody:
        if (current.kind !== BRIDGE_VIEW_KIND.hanging || step.selector !== 0) return undefined;
        current = step.kind === NATIVE_PATH_STEP.hangingPrefix ? current.prefix : step.kind === NATIVE_PATH_STEP.hangingContinuation ? current.continuation : current.body;
        break;
      default: return undefined;
    }
    collected.push(current);
  }
  return collected;
}

function bridgePathViewKind(kind: number): number {
  switch (kind) {
    case BRIDGE_VIEW_KIND.text: return NATIVE_PATH_VIEW_KIND.text;
    case BRIDGE_VIEW_KIND.row: return NATIVE_PATH_VIEW_KIND.row;
    case BRIDGE_VIEW_KIND.column: return NATIVE_PATH_VIEW_KIND.column;
    case BRIDGE_VIEW_KIND.grid: return NATIVE_PATH_VIEW_KIND.grid;
    case BRIDGE_VIEW_KIND.hanging: return NATIVE_PATH_VIEW_KIND.hanging;
    case BRIDGE_VIEW_KIND.container: return NATIVE_PATH_VIEW_KIND.container;
    case BRIDGE_VIEW_KIND.clamp:
    case BRIDGE_VIEW_KIND.contentMax: return NATIVE_PATH_VIEW_KIND.clampRows;
    default: return 0;
  }
}

export function textRowsForHarness(view: View): string[] { return rows(nodeForBridge(view)); }

function rows(node: BridgeViewNode): string[] {
  switch (node.kind) {
    case BRIDGE_VIEW_KIND.text: return [node.spans.map((span) => span.text).join("")];
    case BRIDGE_VIEW_KIND.diff: return node.hunks.flatMap((hunk) => [
      `@@ -${displayDiffRange(hunk.oldRange)} +${displayDiffRange(hunk.newRange)} @@`,
      ...hunk.lines.flatMap((line) => [
        `${line.kind === BRIDGE_DIFF_LINE_KIND.addition ? "+" : line.kind === BRIDGE_DIFF_LINE_KIND.deletion ? "-" : " "}${line.text}`,
        ...(line.termination === BRIDGE_DIFF_LINE_TERMINATION.unterminated ? ["\\ No newline at end of file"] : []),
      ]),
    ]);
    case BRIDGE_VIEW_KIND.spacer: return Array.from({ length: node.rows }, () => "");
    case BRIDGE_VIEW_KIND.row: return [node.children.flatMap((child) => rows(child.child)).join("")];
    case BRIDGE_VIEW_KIND.column: return node.children.flatMap((child) => rows(child.child));
    case BRIDGE_VIEW_KIND.grid: return node.rows.flatMap((row) => row.cells.flatMap((cell) => rows(cell.view)));
    case BRIDGE_VIEW_KIND.hanging: return rows(node.prefix).map((prefix, index) => `${prefix}${index === 0 ? rows(node.body)[0] ?? "" : rows(node.body)[index] ?? ""}`);
    case BRIDGE_VIEW_KIND.container: case BRIDGE_VIEW_KIND.clamp: return rows(node.child).slice(0, node.maxRows);
    case BRIDGE_VIEW_KIND.contentMax: return rows(node.child).slice(0, node.maxRows);
    case BRIDGE_VIEW_KIND.component: return [""];
    case BRIDGE_VIEW_KIND.decorated: return rows(node.child);
  }
}

function toBridgeHunk(hunk: DiffHunkNode): BridgeDiffHunkNode {
  let oldLine = hunk.oldRange.start + 1;
  let newLine = hunk.newRange.start + 1;
  const lines = hunk.lines.map((line: DiffLineNode) => {
    const node = {
      kind: BRIDGE_DIFF_LINE_KIND[line.kind],
      text: line.text,
      termination: line.termination === "unterminated" ? BRIDGE_DIFF_LINE_TERMINATION.unterminated : BRIDGE_DIFF_LINE_TERMINATION.terminated,
      ...(line.kind === "context" ? { oldLine, newLine } : {}),
      ...(line.kind === "addition" ? { newLine } : {}),
      ...(line.kind === "deletion" ? { oldLine } : {}),
    } as const;
    if (line.kind !== "addition") oldLine += 1;
    if (line.kind !== "deletion") newLine += 1;
    return node;
  });
  return { oldRange: { ...hunk.oldRange }, newRange: { ...hunk.newRange }, lines };
}

function bridgeOverflow(overflow: OverflowIndicator): BridgeOverflowIndicatorNode {
  if (overflow.kind === "none") return { kind: BRIDGE_OVERFLOW_KIND.none };
  if (overflow.kind === "ellipsis") return { kind: BRIDGE_OVERFLOW_KIND.ellipsis, style: cloneStyle(overflow.style.value) };
  return { kind: BRIDGE_OVERFLOW_KIND.footer, prefix: overflow.prefix, style: cloneStyle(overflow.style.value) };
}

function bridgeGridTrack(track: GridTrackNode): BridgeGridTrackNode {
  switch (track.kind) {
    case "content": return { kind: BRIDGE_GRID_TRACK_KIND.content };
    case "contentMax": return { kind: BRIDGE_GRID_TRACK_KIND.contentMax, max: track.max };
    case "fixed": return { kind: BRIDGE_GRID_TRACK_KIND.fixed, size: track.size };
    case "flex": return { kind: BRIDGE_GRID_TRACK_KIND.flex };
    case "flexMax": return { kind: BRIDGE_GRID_TRACK_KIND.flexMax, max: track.max };
  }
}

function buildChildren(children: ChildBuilder): ChildrenBuilder {
  const builder = new ChildrenBuilder();
  if (typeof children === "function") children(builder);
  else builder.childrenOf(children);
  return builder;
}

function displayDiffRange(range: { readonly start: number; readonly count: number }): string {
  if (range.count === 0) return `${range.start},0`;
  const start = range.start + 1;
  return range.count === 1 ? `${start}` : `${start},${range.count}`;
}

function wrapCode(mode: WrapMode): number {
  if (mode === "wordThenGrapheme") return BRIDGE_WRAP_MODE.wordThenGrapheme;
  if (mode === "grapheme") return BRIDGE_WRAP_MODE.grapheme;
  return BRIDGE_WRAP_MODE.noWrap;
}

function horizontalAlignCode(align: HorizontalAlign): number {
  if (align === "start") return BRIDGE_HORIZONTAL_ALIGN.start;
  if (align === "center") return BRIDGE_HORIZONTAL_ALIGN.center;
  return BRIDGE_HORIZONTAL_ALIGN.end;
}

function verticalAlignCode(align: "top" | "center" | "bottom"): number {
  if (align === "top") return BRIDGE_VERTICAL_ALIGN.top;
  if (align === "center") return BRIDGE_VERTICAL_ALIGN.center;
  return BRIDGE_VERTICAL_ALIGN.bottom;
}

function validateU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 65535) throw new RangeError(`${name} must be an integer from 0 to 65535`);
  return value;
}

function validatePositiveU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 1 || value > 65535) throw new RangeError(`${name} must be an integer from 1 to 65535`);
  return value;
}
