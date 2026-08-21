// Benchmark-only reconstruction copied from PERF-7v2 source SHA
// e5292d62c4011610850cbdc1ba4a35f296f78e4f.
// Adaptations are mechanical: import paths, the class/export name, and the
// current schema type imports. The eager frozen DAG and WeakMap lookup are
// intentionally unchanged; no production View/native transport code is used.
import type { NativeHandleId } from "../../src/tui/types.ts";
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
  type ColorNode,
  type DecorationNode,
  type DiffHunkNode,
  type DiffLineNode,
  type GridTrackNode,
  type InsetsNode,
  type OverflowIndicatorNode,
  type StyleNode,
  VIEW_BRIDGE_SCHEMA_VERSION,
} from "../../src/tui/ir.ts";
import { insets, Insets } from "../../src/tui/values/geometry.ts";
import { StyleSpec } from "../../src/tui/values/style.ts";
import { TextSpan, type HorizontalAlign, type WrapMode } from "../../src/tui/values/text.ts";

type ChildBuilder = readonly Perf7v2View[] | ((builder: ChildrenBuilder) => void);
type CounterBox = { next: number };
const NODE_ID_COUNTER = Symbol.for("iyon:tui:private-view-node-counter");
const globalRoot = globalThis as typeof globalThis & { [NODE_ID_COUNTER]?: CounterBox };
const nodeIdCounter = globalRoot[NODE_ID_COUNTER] ??= { next: 1 };

function nextNodeId(): number {
  if (nodeIdCounter.next > Number.MAX_SAFE_INTEGER) throw new Error("TUI Perf7v2View node identity exhausted");
  return nodeIdCounter.next++;
}

export type OverflowIndicator =
  | { readonly kind: "none" }
  | { readonly kind: "ellipsis"; readonly style: StyleSpec }
  | { readonly kind: "footer"; readonly prefix: string; readonly style: StyleSpec };

export type GridTrack = GridTrackNode;

export interface GridCell {
  readonly view: Perf7v2View;
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
  cell(view: Perf7v2View): this { this.cells.push({ view }); return this; }
  cellWith(spec: Omit<GridCell, "view">, view: Perf7v2View): this { this.cells.push({ ...spec, view }); return this; }
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
  readonly children: BridgeLayoutChild[] = [];
  private layoutGap = 0;
  child(view: Perf7v2View): this { this.children.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.normal, child: nodeForPerf7v2Bridge(view) }); return this; }
  childrenOf(views: readonly Perf7v2View[]): this { for (const view of views) this.child(view); return this; }
  gap(value: number): this { this.layoutGap = validateU16(value, "gap"); return this; }
  fixed(size: number, view: Perf7v2View): this {
    this.children.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.fixed, size: validateU16(size, "size"), child: nodeForPerf7v2Bridge(view) });
    return this;
  }
  flex(view: Perf7v2View): this { this.children.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.flex, child: nodeForPerf7v2Bridge(view) }); return this; }
  flexMax(maxRows: number, view: Perf7v2View): this {
    validateU16(maxRows, "maxRows");
    this.children.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.flexMax, maxRows, child: nodeForPerf7v2Bridge(view) });
    return this;
  }
  contentMax(maxRows: number, view: Perf7v2View): this {
    validateU16(maxRows, "maxRows");
    this.children.push({ kind: BRIDGE_LAYOUT_CHILD_KIND.contentMax, maxRows, child: nodeForPerf7v2Bridge(view) });
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

export class Perf7v2View {
  readonly kind = "view" as const;
  private constructor(node: BridgeViewNode | BridgeViewNodeDraft) {
    perf7v2Nodes.set(this, withPrivateIdentity(node));
    Object.freeze(this);
  }

  static contentMax(maxRows: number, child: Perf7v2View): Perf7v2View {
    validateU16(maxRows, "maxRows");
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.contentMax, child: nodeForPerf7v2Bridge(child), maxRows });
  }

  static diff(hunks: readonly DiffHunkNode[]): Perf7v2View {
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.diff, hunks: hunks.map(toBridgeHunk) });
  }

  static text(value: string): Perf7v2View {
    if (typeof value !== "string") throw new TypeError("Perf7v2View.text requires a string");
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.text, spans: [{ text: value }], wrap: BRIDGE_WRAP_MODE.wordThenGrapheme, align: BRIDGE_HORIZONTAL_ALIGN.start });
  }

  static styledText(spans: readonly TextSpan[]): Perf7v2View {
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.text, spans: spans.map((span) => ({
      ...span.value,
      style: span.value.style === undefined ? undefined : cloneStyle(span.value.style),
    })), wrap: BRIDGE_WRAP_MODE.wordThenGrapheme, align: BRIDGE_HORIZONTAL_ALIGN.start });
  }

  static spacer(rows: number): Perf7v2View {
    validateU16(rows, "rows");
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.spacer, rows });
  }

  static horizontal(children: ChildBuilder): Perf7v2View {
    const builder = buildChildren(children);
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.row, children: builder.children, gap: builder.gapValue() });
  }

  static vertical(children: ChildBuilder): Perf7v2View {
    const builder = buildChildren(children);
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.column, children: builder.children, gap: builder.gapValue() });
  }

  static hanging(prefix: Perf7v2View, continuation: Perf7v2View, body: Perf7v2View): Perf7v2View {
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.hanging, prefix: nodeForPerf7v2Bridge(prefix), continuation: nodeForPerf7v2Bridge(continuation), body: nodeForPerf7v2Bridge(body) });
  }

  static grid(specification: readonly Perf7v2View[] | GridSpec | ((builder: GridBuilder) => void)): Perf7v2View {
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
        view: nodeForPerf7v2Bridge(cell.view),
        columnSpan: validatePositiveU16(cell.columnSpan ?? 1, "columnSpan"),
        rowSpan: validatePositiveU16(cell.rowSpan ?? 1, "rowSpan"),
        horizontalAlign: horizontalAlignCode(cell.horizontalAlign ?? "start"),
        verticalAlign: verticalAlignCode(cell.verticalAlign ?? "top"),
      })),
    }));
    return new Perf7v2View({
      kind: BRIDGE_VIEW_KIND.grid,
      columns: builder.columnsValue.map(bridgeGridTrack),
      rows,
      columnGap: builder.columnGapValue,
      rowGap: builder.rowGapValue,
    });
  }

  static component(handle: { readonly id: NativeHandleId; nativeComponentId?: () => number | undefined }): Perf7v2View {
    const nativeId = handle.nativeComponentId?.();
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.component, handle: (nativeId ?? handle.id) as NativeHandleId });
  }

  bold(): Perf7v2View { return this.textAttribute("bold"); }
  dim(): Perf7v2View { return this.textAttribute("dim"); }
  italic(): Perf7v2View { return this.textAttribute("italic"); }
  underline(): Perf7v2View { return this.textAttribute("underline"); }
  reversed(): Perf7v2View { return this.textAttribute("reversed"); }
  strikethrough(): Perf7v2View { return this.textAttribute("strikethrough"); }
  textAttribute(name: string, enabled = true): Perf7v2View { return this.decorate({ style: { ...emptyStyle(), attributes: { [name]: enabled } } }); }
  padding(value: number | Insets): Perf7v2View { return this.decorate({ padding: insets(value) }); }
  background(color: ColorNode): Perf7v2View { return this.decorate({ background: color }); }
  foreground(color: ColorNode): Perf7v2View { return this.decorate({ foreground: color }); }
  border(border: BorderNode): Perf7v2View { return this.decorate({ border }); }
  style(style: StyleSpec): Perf7v2View { return this.decorate({ style: mergeStyles(emptyStyle(), style.value) }); }

  styleState(key: string, value: string): Perf7v2View {
    if (key.length === 0 || value.length === 0) throw new RangeError("style state key and value cannot be empty");
    const decorated = this.decoratedNode();
    const current = decorated === undefined ? emptyDecoration() : cloneDecoration(decorated.decoration);
    const child = decorated?.child ?? nodeForPerf7v2Bridge(this);
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.decorated, child, decoration: { ...current, styleStates: { ...current.styleStates, [key]: value } } });
  }

  container(): Perf7v2View { return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.container, child: nodeForPerf7v2Bridge(this) }); }
  clampRows(maxRows: number, overflow: OverflowIndicator = { kind: "none" }): Perf7v2View {
    validateU16(maxRows, "maxRows");
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.clamp, child: nodeForPerf7v2Bridge(this), maxRows, overflow: bridgeOverflow(overflow) });
  }
  fitWidth(): Perf7v2View { return this.decorate({ width: "fit" }); }
  fillWidth(): Perf7v2View { return this.decorate({ width: "fill" }); }
  fitHeight(): Perf7v2View { return this.decorate({ height: "fit" }); }
  fillHeight(): Perf7v2View { return this.decorate({ height: "fill" }); }
  minWidth(value: number): Perf7v2View { return this.decorate({ minWidth: validateU16(value, "minWidth") }); }
  maxWidth(value: number): Perf7v2View { return this.decorate({ maxWidth: validateU16(value, "maxWidth") }); }
  minHeight(value: number): Perf7v2View { return this.decorate({ minHeight: validateU16(value, "minHeight") }); }
  maxHeight(value: number): Perf7v2View { return this.decorate({ maxHeight: validateU16(value, "maxHeight") }); }
  wrap(mode: WrapMode): Perf7v2View { return this.mapText((text) => ({ ...text, wrap: wrapCode(mode) })); }
  noWrap(): Perf7v2View { return this.wrap("noWrap"); }
  textAlign(align: HorizontalAlign): Perf7v2View { return this.mapText((text) => ({ ...text, align: horizontalAlignCode(align) })); }

  private decoratedNode(): Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated }> | undefined {
    const node = nodeForPerf7v2Bridge(this);
    return node.kind === BRIDGE_VIEW_KIND.decorated ? node as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated }> : undefined;
  }

  private decorate(decoration: Partial<DecorationNode>): Perf7v2View {
    const decorated = this.decoratedNode();
    const current = decorated === undefined ? emptyDecoration() : cloneDecoration(decorated.decoration);
    const child = decorated?.child ?? nodeForPerf7v2Bridge(this);
    const next: DecorationNode = { ...current, ...decoration, style: decoration.style === undefined ? current.style : mergeStyles(current.style, decoration.style) };
    return new Perf7v2View({ kind: BRIDGE_VIEW_KIND.decorated, child, decoration: cloneDecoration(next) });
  }

  private mapText(map: (text: Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }>) => Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }>): Perf7v2View {
    const node = nodeForPerf7v2Bridge(this);
    if (node.kind === BRIDGE_VIEW_KIND.text) return new Perf7v2View(map(node as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }>));
    if (node.kind === BRIDGE_VIEW_KIND.decorated && node.child.kind === BRIDGE_VIEW_KIND.text) {
      const decorated = node as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated }>;
      return new Perf7v2View({ ...decorated, child: map(decorated.child as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }>) });
    }
    return this;
  }
}

const perf7v2Nodes = new WeakMap<Perf7v2View, BridgeViewNode>();

/** Private bridge access; the retained DAG is never part of the public API. */
export function nodeForPerf7v2Bridge(view: Perf7v2View): BridgeViewNode {
  const node = perf7v2Nodes.get(view);
  if (node === undefined) throw new TypeError("view is not a runtime semantic value");
  return node;
}

export function textRowsForPerf7v2Harness(view: Perf7v2View): string[] { return rows(nodeForPerf7v2Bridge(view)); }

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
