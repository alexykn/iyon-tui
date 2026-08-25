import {
  BRIDGE_DIFF_LINE_KIND,
  BRIDGE_VIEW_KIND,
  type BorderNode,
  type BridgeDiffHunkNode,
  type BridgeLayoutChild,
  type BridgeGridCellNode,
  type BridgeViewNode,
  type ColorNode,
  type DecorationNode,
  type StyleNode,
} from "./ir.ts";
import { PersistentSeq, type PersistentSeqNode } from "./persistent_seq.ts";

export type PackedLineage =
  | { readonly kind: "text"; readonly base: BridgeViewNode }
  | { readonly kind: "decoration"; readonly base: BridgeViewNode }
  | { readonly kind: "axis" | "grid"; readonly base: BridgeViewNode };

export type CanonicalColor =
  | { readonly kind: "string"; readonly value: string }
  | { readonly kind: "ansi"; readonly value: number };

export interface CanonicalStyle {
  readonly flags: number;
  readonly theme?: string;
  readonly foreground?: CanonicalColor;
  readonly background?: CanonicalColor;
  readonly attributePresent: number;
  readonly attributeTrue: number;
}

export interface CanonicalBorder {
  readonly flags: number;
  readonly glyphs?: readonly [string, string, string, string, string, string, string, string];
  readonly color?: CanonicalColor;
  readonly style?: number;
  readonly edges?: number;
}

export interface CanonicalDecoration {
  readonly flags: number;
  readonly padding?: readonly [number, number, number, number];
  readonly background?: CanonicalColor;
  readonly foreground?: CanonicalColor;
  readonly border?: CanonicalBorder;
  readonly style: CanonicalStyle;
  readonly styleStates?: readonly (readonly [string, string])[];
  readonly width?: "fit" | "fill";
  readonly height?: "fit" | "fill";
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}

export type PackedGridCell = BridgeGridCellNode & { readonly row: number; readonly column: number };

export type PackedU64 = readonly [number, number];
type CanonicalDiffLine = {
  readonly kind: number;
  readonly text: string;
  readonly termination: number;
  readonly oldLine?: PackedU64;
  readonly newLine?: PackedU64;
};
export type CanonicalDiff = readonly {
  readonly oldRange: PackedU64;
  readonly oldCount: PackedU64;
  readonly newRange: PackedU64;
  readonly newCount: PackedU64;
  readonly lines: readonly CanonicalDiffLine[];
}[];

export interface PackedMetaSeed {
  readonly sequence?: PersistentSeq<BridgeLayoutChild>;
  readonly gridCells?: PersistentSeq<PackedGridCell>;
  readonly gridCellOffsets?: ReadonlyMap<string, number>;
}

export interface PackedRecipe {
  readonly kind: number;
  readonly nodeIdLow: number;
  readonly nodeIdHigh: number;
  readonly componentHandle?: PackedU64;
}

export interface PackedMeta {
  readonly recipe: PackedRecipe;
  readonly nodeIdLow: number;
  readonly nodeIdHigh: number;
  readonly lineage?: PackedLineage;
  readonly canonicalStyles: readonly CanonicalStyle[];
  readonly textStyles?: readonly (CanonicalStyle | undefined)[];
  readonly diff?: CanonicalDiff;
  readonly overflowStyle?: CanonicalStyle;
  readonly canonicalDecoration?: CanonicalDecoration;
  sequence?: PersistentSeq<BridgeLayoutChild>;
  gridCells?: PersistentSeq<PackedGridCell>;
  readonly gridCellOffsets?: ReadonlyMap<string, number>;
  sequenceOverride: boolean;
  containsSequenceOverride: boolean;
  ref: number;
  refGeneration: number;
  publishedGeneration: number;
  visitEpoch: number;
  localDefIndex: number;
  state: "unseen" | "visiting" | "emitted";
}

export interface PackedSeqMeta {
  ref: number;
  refGeneration: number;
  publishedGeneration: number;
  visitEpoch: number;
  localDefIndex: number;
  state: "unseen" | "visiting" | "emitted";
}

const metas = new WeakMap<BridgeViewNode, PackedMeta>();
const sequenceMetas = new WeakMap<object, PackedSeqMeta>();
const styleMetas = new WeakMap<object, CanonicalStyle>();

const ATTRIBUTE_BITS = new Map([
  ["bold", 1],
  ["dim", 2],
  ["italic", 4],
  ["underline", 8],
  ["reversed", 16],
  ["strikethrough", 32],
]);

function canonicalColor(color: ColorNode): CanonicalColor {
  if (typeof color === "string") return Object.freeze({ kind: "string", value: color });
  if (!Number.isInteger(color.value) || color.value < 0 || color.value > 255) throw new RangeError("packed V3 ANSI color must fit in u8");
  return Object.freeze({ kind: "ansi", value: color.value });
}

function canonicalStyle(style: StyleNode): CanonicalStyle {
  const cached = styleMetas.get(style);
  if (cached !== undefined) return cached;
  let present = 0;
  let truth = 0;
  for (const [name, enabled] of Object.entries(style.attributes)) {
    const bit = ATTRIBUTE_BITS.get(name);
    if (bit === undefined) throw new TypeError(`unknown text attribute ${name}`);
    present |= bit;
    if (enabled) truth |= bit;
  }
  const result = Object.freeze({
    flags: (style.theme === undefined ? 0 : 1) | (style.foreground === undefined ? 0 : 2) | (style.background === undefined ? 0 : 4),
    theme: style.theme,
    foreground: style.foreground === undefined ? undefined : canonicalColor(style.foreground),
    background: style.background === undefined ? undefined : canonicalColor(style.background),
    attributePresent: present,
    attributeTrue: truth,
  });
  styleMetas.set(style, result);
  return result;
}

function splitU64(value: number, positive: boolean, name: string): PackedU64 {
  if (!Number.isSafeInteger(value) || (positive ? value <= 0 : value < 0) || value > Number.MAX_SAFE_INTEGER) {
    throw new RangeError(`packed V3 ${name} must be ${positive ? "a positive" : "a non-negative"} safe integer`);
  }
  return [value % 0x1_0000_0000, Math.floor(value / 0x1_0000_0000)];
}

function canonicalDiff(hunks: readonly BridgeDiffHunkNode[]): CanonicalDiff {
  return Object.freeze(hunks.map((hunk) => Object.freeze({
    oldRange: splitU64(hunk.oldRange.start, false, "diff range start"),
    oldCount: splitU64(hunk.oldRange.count, false, "diff range count"),
    newRange: splitU64(hunk.newRange.start, false, "diff range start"),
    newCount: splitU64(hunk.newRange.count, false, "diff range count"),
    lines: Object.freeze(hunk.lines.map((line) => Object.freeze({
      kind: line.kind,
      text: line.text,
      termination: line.termination,
      oldLine: line.kind === BRIDGE_DIFF_LINE_KIND.context || line.kind === BRIDGE_DIFF_LINE_KIND.deletion
        ? splitU64(line.oldLine ?? 0, true, "diff old line")
        : undefined,
      newLine: line.kind === BRIDGE_DIFF_LINE_KIND.context || line.kind === BRIDGE_DIFF_LINE_KIND.addition
        ? splitU64(line.newLine ?? 0, true, "diff new line")
        : undefined,
    }))),
  })));
}

function canonicalBorder(border: BorderNode): CanonicalBorder {
  let flags = 0;
  const glyphs = border.glyphs === undefined ? undefined : [
    border.glyphs.top, border.glyphs.right, border.glyphs.bottom, border.glyphs.left,
    border.glyphs.topLeft, border.glyphs.topRight, border.glyphs.bottomLeft, border.glyphs.bottomRight,
  ] as const;
  if (glyphs !== undefined) flags |= 1;
  if (border.color !== undefined) flags |= 2;
  const style = border.style === undefined ? undefined : border.style === "plain" ? 1 : border.style === "rounded" ? 2 : 3;
  if (style !== undefined) flags |= 4;
  const edges = border.edges === undefined ? undefined : border.edges === "all" ? 1 : 2;
  if (edges !== undefined) flags |= 8;
  return Object.freeze({ flags, glyphs: glyphs === undefined ? undefined : Object.freeze([...glyphs]) as unknown as typeof glyphs, color: border.color === undefined ? undefined : canonicalColor(border.color), style, edges });
}

function canonicalDecoration(decoration: DecorationNode): CanonicalDecoration {
  let flags = 16;
  if (decoration.padding !== undefined) flags |= 1;
  if (decoration.background !== undefined) flags |= 2;
  if (decoration.foreground !== undefined) flags |= 4;
  if (decoration.border !== undefined) flags |= 8;
  if (decoration.styleStates !== undefined) flags |= 32;
  if (decoration.width !== undefined) flags |= 64;
  if (decoration.height !== undefined) flags |= 128;
  if (decoration.minWidth !== undefined) flags |= 256;
  if (decoration.maxWidth !== undefined) flags |= 512;
  if (decoration.minHeight !== undefined) flags |= 1024;
  if (decoration.maxHeight !== undefined) flags |= 2048;
  return Object.freeze({
    flags,
    padding: decoration.padding === undefined ? undefined : [decoration.padding.top, decoration.padding.right, decoration.padding.bottom, decoration.padding.left] as const,
    background: decoration.background === undefined ? undefined : canonicalColor(decoration.background),
    foreground: decoration.foreground === undefined ? undefined : canonicalColor(decoration.foreground),
    border: decoration.border === undefined ? undefined : canonicalBorder(decoration.border),
    style: canonicalStyle(decoration.style),
    styleStates: decoration.styleStates === undefined ? undefined : Object.freeze(Object.entries(decoration.styleStates)),
    width: decoration.width,
    height: decoration.height,
    minWidth: decoration.minWidth,
    maxWidth: decoration.maxWidth,
    minHeight: decoration.minHeight,
    maxHeight: decoration.maxHeight,
  });
}

function packedGridCells(node: Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.grid }>): { readonly cells: PackedGridCell[]; readonly offsets: ReadonlyMap<string, number> } {
  const occupiedUntil: number[] = [];
  const cells: PackedGridCell[] = [];
  const offsets = new Map<string, number>();
  for (let row = 0; row < node.rows.length; row += 1) {
    for (let cellIndex = 0; cellIndex < node.rows[row]!.cells.length; cellIndex += 1) {
      const cell = node.rows[row]!.cells[cellIndex]!;
      let column = 0;
      while (Array.from({ length: cell.columnSpan }, (_, offset) => occupiedUntil[column + offset] ?? 0).some((until) => until > row)) column += 1;
      while (occupiedUntil.length < column + cell.columnSpan) occupiedUntil.push(0);
      for (let offset = 0; offset < cell.columnSpan; offset += 1) occupiedUntil[column + offset] = row + cell.rowSpan;
      offsets.set(`${row}:${column}`, cells.length);
      cells.push(Object.freeze({ ...cell, row, column }));
    }
  }
  return { cells, offsets };
}

function viewAggregateFlags(node: BridgeViewNode): number {
  switch (node.kind) {
    case BRIDGE_VIEW_KIND.component: return 1;
    case BRIDGE_VIEW_KIND.row:
    case BRIDGE_VIEW_KIND.column: {
      const sequence = packedMeta(node).sequence;
      if (sequence !== undefined) return sequence.aggregate;
      return node.children.reduce((flags, child) => flags | viewAggregateFlags(child.child), 0);
    }
    case BRIDGE_VIEW_KIND.hanging: return viewAggregateFlags(node.prefix) | viewAggregateFlags(node.continuation) | viewAggregateFlags(node.body);
    case BRIDGE_VIEW_KIND.grid: return node.rows.reduce((flags, row) => flags | row.cells.reduce((rowFlags, cell) => rowFlags | viewAggregateFlags(cell.view), 0), 0);
    case BRIDGE_VIEW_KIND.container:
    case BRIDGE_VIEW_KIND.clamp:
    case BRIDGE_VIEW_KIND.contentMax:
    case BRIDGE_VIEW_KIND.decorated: return viewAggregateFlags(node.child);
    default: return 0;
  }
}

function sequenceAggregate(child: BridgeLayoutChild): number {
  return viewAggregateFlags(child.child);
}

function allCanonicalStyles(node: BridgeViewNode): readonly CanonicalStyle[] {
  const styles: CanonicalStyle[] = [];
  const visitStyle = (style: StyleNode | undefined) => { if (style !== undefined) styles.push(canonicalStyle(style)); };
  switch (node.kind) {
    case BRIDGE_VIEW_KIND.text:
      for (const span of node.spans) visitStyle(span.style);
      break;
    case BRIDGE_VIEW_KIND.clamp:
      if (node.overflow !== undefined && node.overflow.kind !== 1) visitStyle(node.overflow.style);
      break;
    case BRIDGE_VIEW_KIND.decorated:
      styles.push(canonicalStyle(node.decoration.style));
      if (node.decoration.styleStates !== undefined) {
        // Force validation/canonicalization of every immutable style state key/value now.
        for (const [key, value] of Object.entries(node.decoration.styleStates)) {
          if (key.length === 0 || value.length === 0) throw new RangeError("style state key and value cannot be empty");
        }
      }
      break;
    case BRIDGE_VIEW_KIND.diff:
    case BRIDGE_VIEW_KIND.spacer:
    case BRIDGE_VIEW_KIND.row:
    case BRIDGE_VIEW_KIND.column:
    case BRIDGE_VIEW_KIND.hanging:
    case BRIDGE_VIEW_KIND.grid:
    case BRIDGE_VIEW_KIND.container:
    case BRIDGE_VIEW_KIND.contentMax:
    case BRIDGE_VIEW_KIND.component:
      break;
  }
  return Object.freeze(styles);
}

export function registerPackedMeta(node: BridgeViewNode, lineage?: PackedLineage, seed?: PackedMetaSeed): void {
  if (metas.has(node)) return;
  if (!Number.isSafeInteger(node.id) || node.id <= 0 || node.id > Number.MAX_SAFE_INTEGER) {
    throw new RangeError("packed V3 NodeId must be a positive safe integer");
  }
  const nodeIdLow = node.id % 0x1_0000_0000;
  const nodeIdHigh = Math.floor(node.id / 0x1_0000_0000);
  let sequence: PersistentSeq<BridgeLayoutChild> | undefined;
  let gridCells: PersistentSeq<PackedGridCell> | undefined;
  if (node.kind === BRIDGE_VIEW_KIND.row || node.kind === BRIDGE_VIEW_KIND.column) {
    sequence = seed?.sequence ?? PersistentSeq.from(node.children, sequenceAggregate);
  }
  let gridCellOffsets: ReadonlyMap<string, number> | undefined;
  if (node.kind === BRIDGE_VIEW_KIND.grid) {
    if (seed?.gridCells !== undefined) {
      gridCells = seed.gridCells;
      gridCellOffsets = seed.gridCellOffsets;
    } else {
      const packed = packedGridCells(node);
      gridCells = PersistentSeq.from(packed.cells, (cell) => viewAggregateFlags(cell.view));
      gridCellOffsets = packed.offsets;
    }
  }
  const textStyles = node.kind === BRIDGE_VIEW_KIND.text
    ? Object.freeze(node.spans.map((span) => span.style === undefined ? undefined : canonicalStyle(span.style)))
    : undefined;
  const overflowStyle = node.kind === BRIDGE_VIEW_KIND.clamp && node.overflow !== undefined && node.overflow.kind !== 1
    ? canonicalStyle(node.overflow.style)
    : undefined;
  const sequenceOverride = seed?.sequence !== undefined || seed?.gridCells !== undefined;
  const containsSequenceOverride = sequenceOverride || sequenceOverrideInChildren(node);
  metas.set(node, {
    recipe: Object.freeze({
      kind: node.kind,
      nodeIdLow,
      nodeIdHigh,
      componentHandle: node.kind === BRIDGE_VIEW_KIND.component
        ? splitU64(node.handle, true, "component handle")
        : undefined,
    }),
    nodeIdLow,
    nodeIdHigh,
    lineage,
    canonicalStyles: allCanonicalStyles(node),
    textStyles,
    diff: node.kind === BRIDGE_VIEW_KIND.diff ? canonicalDiff(node.hunks) : undefined,
    overflowStyle,
    canonicalDecoration: node.kind === BRIDGE_VIEW_KIND.decorated ? canonicalDecoration(node.decoration) : undefined,
    sequence,
    gridCells,
    gridCellOffsets,
    sequenceOverride,
    containsSequenceOverride,
    ref: 0,
    refGeneration: 0xffff_ffff,
    publishedGeneration: 0xffff_ffff,
    visitEpoch: 0,
    localDefIndex: 0xffff_ffff,
    state: "unseen",
  });
}

function sequenceOverrideInChildren(node: BridgeViewNode): boolean {
  const childHasOverride = (child: BridgeViewNode): boolean => packedMeta(child).containsSequenceOverride;
  switch (node.kind) {
    case BRIDGE_VIEW_KIND.row:
    case BRIDGE_VIEW_KIND.column:
      return node.children.some((child) => childHasOverride(child.child));
    case BRIDGE_VIEW_KIND.hanging:
      return childHasOverride(node.prefix) || childHasOverride(node.continuation) || childHasOverride(node.body);
    case BRIDGE_VIEW_KIND.grid:
      return node.rows.some((row) => row.cells.some((cell) => childHasOverride(cell.view)));
    case BRIDGE_VIEW_KIND.container:
    case BRIDGE_VIEW_KIND.clamp:
    case BRIDGE_VIEW_KIND.contentMax:
    case BRIDGE_VIEW_KIND.decorated:
      return childHasOverride(node.child);
    default:
      return false;
  }
}

export function packedMeta(node: BridgeViewNode): PackedMeta {
  const existing = metas.get(node);
  if (existing !== undefined) return existing;
  registerPackedMeta(node);
  return metas.get(node)!;
}

export function registerPackedSequenceMeta<T>(node: PersistentSeqNode<T>): PackedSeqMeta {
  const existing = sequenceMetas.get(node);
  if (existing !== undefined) return existing;
  const meta: PackedSeqMeta = {
    ref: 0,
    refGeneration: 0xffff_ffff,
    publishedGeneration: 0xffff_ffff,
    visitEpoch: 0,
    localDefIndex: 0xffff_ffff,
    state: "unseen",
  };
  sequenceMetas.set(node, meta);
  return meta;
}

export function packedSequenceMeta<T>(node: PersistentSeqNode<T>): PackedSeqMeta {
  return registerPackedSequenceMeta(node);
}

export function setPackedSequence(node: BridgeViewNode, sequence: PersistentSeq<BridgeLayoutChild>): void {
  const meta = packedMeta(node);
  meta.sequence = sequence;
  meta.sequenceOverride = true;
}

export function setPackedGridCells(node: BridgeViewNode, sequence: PersistentSeq<PackedGridCell>): void {
  const meta = packedMeta(node);
  meta.gridCells = sequence;
  meta.sequenceOverride = true;
}

export function canonicalStyleFor(style: StyleNode): CanonicalStyle { return canonicalStyle(style); }

export function effectiveTextNode(node: BridgeViewNode): Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }> | undefined {
  if (node.kind === BRIDGE_VIEW_KIND.text) return node;
  if (node.kind === BRIDGE_VIEW_KIND.decorated && node.child.kind === BRIDGE_VIEW_KIND.text) return node.child;
  return undefined;
}

export function effectiveDecoration(node: BridgeViewNode): CanonicalDecoration | undefined {
  if (node.kind !== BRIDGE_VIEW_KIND.decorated) return undefined;
  return packedMeta(node).canonicalDecoration;
}
