/**
 * Backend-neutral semantic representation of an immutable View.
 *
 * This module deliberately has no knowledge of the structural bridge, native
 * handles, ABI schema, or generated calls. It is the private semantic model
 * that API/view and composition will share during the H3 cutover.
 *
 * H3-C keeps bridge records as derived cold-fallback artifacts only. The
 * factories and sidecars here are private semantic infrastructure; retained
 * structural transport consumes these nodes directly.
 */

import type { HandleId } from "../controls/framework-handle.ts";
import type { TextAttribute } from "../presentation/style.ts";
import type { View } from "./view.ts";
import type { AnsiColor } from "../presentation/theme.ts";

// ---------------------------------------------------------------------------
// Semantic presentation values
// ---------------------------------------------------------------------------

export type SemanticColor =
  | { readonly kind: "theme"; readonly key: string }
  | { readonly kind: "named"; readonly value: AnsiColor }
  | { readonly kind: "indexed"; readonly value: number }
  | { readonly kind: "rgb"; readonly r: number; readonly g: number; readonly b: number };

export interface SemanticInsets {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

export interface SemanticStyle {
  readonly theme?: string;
  readonly foreground?: SemanticColor;
  readonly background?: SemanticColor;
  readonly attributes: Readonly<Partial<Record<TextAttribute, boolean>>>;
}

export type SemanticBorderStyle = "plain" | "rounded" | "double";
export type SemanticBorderEdges = "all" | "topBottom";

export interface SemanticBorderGlyphs {
  readonly top: string;
  readonly right: string;
  readonly bottom: string;
  readonly left: string;
  readonly topLeft: string;
  readonly topRight: string;
  readonly bottomLeft: string;
  readonly bottomRight: string;
}

export interface SemanticBorder {
  readonly glyphs?: SemanticBorderGlyphs;
  readonly style?: SemanticBorderStyle;
  readonly edges?: SemanticBorderEdges;
  readonly color?: SemanticColor;
}

export type SemanticSizeMode = "fit" | "fill";

export type SemanticOverflowIndicator =
  | { readonly kind: "none" }
  | { readonly kind: "ellipsis"; readonly style: SemanticStyle }
  | { readonly kind: "footer"; readonly prefix: string; readonly style: SemanticStyle };

export type SemanticStyleStates = Readonly<Record<string, string>>;

export interface SemanticDecoration {
  readonly padding?: SemanticInsets;
  readonly background?: SemanticColor;
  readonly foreground?: SemanticColor;
  readonly border?: SemanticBorder;
  readonly style: SemanticStyle;
  readonly styleStates?: SemanticStyleStates;
  readonly width?: SemanticSizeMode;
  readonly height?: SemanticSizeMode;
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}

export interface SemanticTextSpan {
  readonly text: string;
  readonly style?: SemanticStyle;
}

// ---------------------------------------------------------------------------
// Semantic content/layout values
// ---------------------------------------------------------------------------

export type SemanticWrapMode = "wordThenGrapheme" | "grapheme" | "noWrap";
export type SemanticHorizontalAlign = "start" | "center" | "end";
export type SemanticVerticalAlign = "top" | "center" | "bottom";

export type SemanticLayoutChild =
  | { readonly kind: "normal"; readonly child: SemanticViewNode }
  | { readonly kind: "fixed"; readonly size: number; readonly child: SemanticViewNode }
  | { readonly kind: "flex"; readonly child: SemanticViewNode }
  | { readonly kind: "flexMax"; readonly maxRows: number; readonly child: SemanticViewNode }
  | { readonly kind: "contentMax"; readonly maxRows: number; readonly child: SemanticViewNode };

/** Track data without the child occurrence, used by derivation hints. */
export type SemanticAxisTrack =
  | { readonly kind: "normal" }
  | { readonly kind: "fixed"; readonly size: number }
  | { readonly kind: "flex" }
  | { readonly kind: "flexMax"; readonly maxRows: number }
  | { readonly kind: "contentMax"; readonly maxRows: number };

export type SemanticGridTrack =
  | { readonly kind: "content" }
  | { readonly kind: "contentMax"; readonly max: number }
  | { readonly kind: "fixed"; readonly size: number }
  | { readonly kind: "flex" }
  | { readonly kind: "flexMax"; readonly max: number };

export interface SemanticGridCell {
  readonly view: SemanticViewNode;
  readonly columnSpan: number;
  readonly rowSpan: number;
  readonly horizontalAlign: SemanticHorizontalAlign;
  readonly verticalAlign: SemanticVerticalAlign;
}

export interface SemanticGridRow {
  readonly track: SemanticGridTrack;
  readonly cells: readonly SemanticGridCell[];
}

export interface SemanticDiffRange {
  readonly start: number;
  readonly count: number;
}

export type SemanticDiffLineKind = "context" | "addition" | "deletion";
export type SemanticDiffLineTermination = "terminated" | "unterminated";

export interface SemanticDiffLine {
  readonly kind: SemanticDiffLineKind;
  readonly text: string;
  readonly termination: SemanticDiffLineTermination;
  readonly oldLine?: number;
  readonly newLine?: number;
}

export interface SemanticDiffHunk {
  readonly oldRange: SemanticDiffRange;
  readonly newRange: SemanticDiffRange;
  readonly lines: readonly SemanticDiffLine[];
}

// ---------------------------------------------------------------------------
// Semantic View identity and node vocabulary
// ---------------------------------------------------------------------------

/** Private semantic identity; it is not a native ABI identifier. */
export type SemanticNodeId = number;

/** Private semantic discriminants. They are intentionally not bridge codes. */
export const SEMANTIC_VIEW_KIND = Object.freeze({
  text: 0,
  diff: 1,
  spacer: 2,
  row: 3,
  column: 4,
  grid: 5,
  hanging: 6,
  container: 7,
  clamp: 8,
  contentMax: 9,
  component: 10,
  decorated: 11,
} as const);

export type SemanticViewKind = (typeof SEMANTIC_VIEW_KIND)[keyof typeof SEMANTIC_VIEW_KIND];

export interface SemanticNodeBase {
  readonly id: SemanticNodeId;
  readonly kind: SemanticViewKind;
}

export interface SemanticTextNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.text;
  readonly spans: readonly SemanticTextSpan[];
  readonly wrap: SemanticWrapMode;
  readonly align: SemanticHorizontalAlign;
}

export interface SemanticDiffNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.diff;
  readonly hunks: readonly SemanticDiffHunk[];
}

export interface SemanticSpacerNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.spacer;
  readonly rows: number;
}

export interface SemanticAxisNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.row | typeof SEMANTIC_VIEW_KIND.column;
  readonly children: readonly SemanticLayoutChild[];
  readonly gap: number;
}

export interface SemanticHangingNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.hanging;
  readonly prefix: SemanticViewNode;
  readonly continuation: SemanticViewNode;
  readonly body: SemanticViewNode;
}

export interface SemanticGridNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.grid;
  readonly columns: readonly SemanticGridTrack[];
  readonly rows: readonly SemanticGridRow[];
  readonly columnGap: number;
  readonly rowGap: number;
}

export interface SemanticContainerNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.container;
  readonly child: SemanticViewNode;
}

export interface SemanticClampNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.clamp;
  readonly child: SemanticViewNode;
  readonly maxRows: number;
  readonly overflow: SemanticOverflowIndicator;
}

export interface SemanticContentMaxNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.contentMax;
  readonly child: SemanticViewNode;
  readonly maxRows: number;
}

export interface SemanticComponentNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.component;
  /** JS-local framework identity; never a native ComponentId. */
  readonly handleId: HandleId;
}

export interface SemanticDecoratedNode extends SemanticNodeBase {
  readonly kind: typeof SEMANTIC_VIEW_KIND.decorated;
  readonly child: SemanticViewNode;
  readonly decoration: SemanticDecoration;
}

export type SemanticViewNode =
  | SemanticTextNode
  | SemanticDiffNode
  | SemanticSpacerNode
  | SemanticAxisNode
  | SemanticHangingNode
  | SemanticGridNode
  | SemanticContainerNode
  | SemanticClampNode
  | SemanticContentMaxNode
  | SemanticComponentNode
  | SemanticDecoratedNode;

type WithoutId<T> = T extends SemanticViewNode ? Omit<T, "id"> : never;
export type SemanticViewNodeDraft = WithoutId<SemanticViewNode>;

/**
 * Installs a semantic identity on an already-owned semantic draft.
 * Construction is private and intentionally receives the identity from the
 * current API/view owner.
 */
export function createSemanticViewNode(id: SemanticNodeId, draft: SemanticViewNodeDraft): SemanticViewNode {
  if (!Number.isSafeInteger(id) || id < 1) throw new RangeError("semantic View node identity must be a positive safe integer");
  const node = freezeSemanticViewNode({ id, ...draft } as SemanticViewNode);
  semanticNodeBrand.add(node);
  return node;
}

/** Private association used when H3-B makes semantic nodes View-authoritative. */
const semanticNodes = new WeakMap<View, SemanticViewNode>();
const semanticNodeBrand = new WeakSet<object>();

export function installSemanticNode(view: View, node: SemanticViewNode): void {
  semanticNodeBrand.add(node);
  semanticNodes.set(view, node);
}

export function semanticNodeOf(view: View): SemanticViewNode {
  const node = semanticNodes.get(view);
  if (node === undefined) throw new TypeError("view is not a framework semantic value");
  return node;
}

/** @internal Brand check for transport entrypoints that already hold a node. */
export function isSemanticViewNode(value: unknown): value is SemanticViewNode {
  return typeof value === "object" && value !== null && semanticNodeBrand.has(value);
}

/** Freezes semantic records without introducing a transport-shaped copy. */
export function freezeSemanticViewNode<T extends SemanticViewNode>(node: T): T {
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.text:
      for (const span of node.spans) {
        if (span.style !== undefined) freezeSemanticStyle(span.style);
        Object.freeze(span);
      }
      Object.freeze(node.spans);
      break;
    case SEMANTIC_VIEW_KIND.diff:
      for (const hunk of node.hunks) {
        Object.freeze(hunk.oldRange);
        Object.freeze(hunk.newRange);
        for (const line of hunk.lines) Object.freeze(line);
        Object.freeze(hunk.lines);
        Object.freeze(hunk);
      }
      Object.freeze(node.hunks);
      break;
    case SEMANTIC_VIEW_KIND.row:
    case SEMANTIC_VIEW_KIND.column:
      for (const child of node.children) Object.freeze(child);
      Object.freeze(node.children);
      break;
    case SEMANTIC_VIEW_KIND.grid:
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
    case SEMANTIC_VIEW_KIND.clamp:
      freezeSemanticOverflow(node.overflow);
      break;
    case SEMANTIC_VIEW_KIND.decorated:
      freezeSemanticDecoration(node.decoration);
      break;
    case SEMANTIC_VIEW_KIND.hanging:
    case SEMANTIC_VIEW_KIND.container:
    case SEMANTIC_VIEW_KIND.contentMax:
    case SEMANTIC_VIEW_KIND.spacer:
    case SEMANTIC_VIEW_KIND.component:
      break;
  }
  return Object.freeze(node);
}

function freezeSemanticColor(color: SemanticColor): void {
  Object.freeze(color);
}

function freezeSemanticStyle(style: SemanticStyle): void {
  if (style.foreground !== undefined) freezeSemanticColor(style.foreground);
  if (style.background !== undefined) freezeSemanticColor(style.background);
  Object.freeze(style.attributes);
  Object.freeze(style);
}

function freezeSemanticOverflow(overflow: SemanticOverflowIndicator): void {
  if (overflow.kind !== "none") freezeSemanticStyle(overflow.style);
  Object.freeze(overflow);
}

function freezeSemanticDecoration(decoration: SemanticDecoration): void {
  if (decoration.padding !== undefined) Object.freeze(decoration.padding);
  if (decoration.background !== undefined) Object.freeze(decoration.background);
  if (decoration.foreground !== undefined) Object.freeze(decoration.foreground);
  if (decoration.border !== undefined) {
    if (decoration.border.glyphs !== undefined) Object.freeze(decoration.border.glyphs);
    if (decoration.border.color !== undefined) Object.freeze(decoration.border.color);
    Object.freeze(decoration.border);
  }
  freezeSemanticStyle(decoration.style);
  if (decoration.styleStates !== undefined) Object.freeze(decoration.styleStates);
  Object.freeze(decoration);
}

// ---------------------------------------------------------------------------
// Semantic derivation hints
// ---------------------------------------------------------------------------

export interface SemanticTextLayoutDerivation {
  readonly kind: "textLayout";
  readonly base: SemanticViewNode;
  readonly wrap: SemanticWrapMode;
  readonly align: SemanticHorizontalAlign;
}

export interface SemanticCommonScalarChanges {
  readonly padding?: SemanticInsets;
  readonly width?: SemanticSizeMode;
  readonly height?: SemanticSizeMode;
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}

export interface SemanticCommonScalarDerivation {
  readonly kind: "commonScalar";
  readonly base: SemanticViewNode;
  readonly changes: SemanticCommonScalarChanges;
}

export interface SemanticAxisSetDerivation {
  readonly kind: "axisSet";
  readonly base: SemanticViewNode;
  readonly index: number;
  /** Undefined preserves the base occurrence's existing track. */
  readonly track: SemanticAxisTrack | undefined;
  readonly child: SemanticViewNode;
}

export interface SemanticAxisSpliceDerivation {
  readonly kind: "axisSplice";
  readonly base: SemanticViewNode;
  readonly index: number;
  readonly removeCount: number;
  readonly inserted: readonly { readonly track: SemanticAxisTrack; readonly child: SemanticViewNode }[];
}

export interface SemanticGridCellDerivation {
  readonly kind: "gridCell";
  readonly base: SemanticViewNode;
  readonly row: number;
  readonly column: number;
  readonly child: SemanticViewNode;
}

export type SemanticDerivation =
  | SemanticTextLayoutDerivation
  | SemanticCommonScalarDerivation
  | SemanticAxisSetDerivation
  | SemanticAxisSpliceDerivation
  | SemanticGridCellDerivation;

/** Semantic sidecars are weak and die with the immutable node. */
const SEMANTIC_DERIVATION = new WeakMap<SemanticViewNode, SemanticDerivation>();

export function setSemanticDerivation(node: SemanticViewNode, derivation: SemanticDerivation): void {
  SEMANTIC_DERIVATION.set(node, freezeSemanticDerivation(derivation));
}

function freezeSemanticDerivation(derivation: SemanticDerivation): SemanticDerivation {
  switch (derivation.kind) {
    case "textLayout":
      return Object.freeze(derivation);
    case "commonScalar":
      if (derivation.changes.padding !== undefined) Object.freeze(derivation.changes.padding);
      Object.freeze(derivation.changes);
      return Object.freeze(derivation);
    case "axisSet":
      if (derivation.track !== undefined) Object.freeze(derivation.track);
      return Object.freeze(derivation);
    case "axisSplice":
      for (const entry of derivation.inserted) {
        Object.freeze(entry.track);
        Object.freeze(entry);
      }
      Object.freeze(derivation.inserted);
      return Object.freeze(derivation);
    case "gridCell":
      return Object.freeze(derivation);
  }
}

export function peekSemanticDerivation(node: SemanticViewNode): SemanticDerivation | undefined {
  return SEMANTIC_DERIVATION.get(node);
}

// ---------------------------------------------------------------------------
// Semantic sequence contracts and wide-structure sidecars
// ---------------------------------------------------------------------------

/** Read-only view of a semantic persistent sequence. */
export interface SemanticSequence<T> {
  readonly length: number;
  get(index: number): T | undefined;
  values(): IterableIterator<T>;
}

export type SemanticAxisSequenceEdit =
  | { readonly kind: "axisSet"; readonly index: number }
  | { readonly kind: "axisSplice"; readonly index: number; readonly removeCount: number; readonly insertedCount: number };

export interface SemanticAxisSequenceOverride {
  readonly baseNode: SemanticViewNode;
  readonly sequence: SemanticSequence<SemanticLayoutChild>;
  readonly edit?: SemanticAxisSequenceEdit;
}

export interface SemanticGridSequenceOverride {
  readonly baseNode: SemanticViewNode;
  readonly sequence: SemanticSequence<SemanticGridCell>;
  readonly rowOffsets: readonly number[];
  readonly rowTracks: readonly SemanticGridTrack[];
  readonly cellIndices: readonly ReadonlyMap<number, number>[];
}

const SEMANTIC_SEQUENCE = new WeakMap<SemanticViewNode, SemanticAxisSequenceOverride>();
const SEMANTIC_GRID_SEQUENCE = new WeakMap<SemanticViewNode, SemanticGridSequenceOverride>();

export function setSemanticSequenceOverride(node: SemanticViewNode, override: SemanticAxisSequenceOverride): void {
  const edit = override.edit === undefined ? undefined : Object.freeze({ ...override.edit });
  SEMANTIC_SEQUENCE.set(node, Object.freeze({
    baseNode: override.baseNode,
    sequence: override.sequence,
    ...(edit === undefined ? {} : { edit }),
  }));
}

export function peekSemanticSequenceOverride(node: SemanticViewNode): SemanticAxisSequenceOverride | undefined {
  return SEMANTIC_SEQUENCE.get(node);
}

export function setSemanticGridSequenceOverride(node: SemanticViewNode, override: SemanticGridSequenceOverride): void {
  const rowOffsets = Object.freeze([...override.rowOffsets]);
  const rowTracks = Object.freeze(override.rowTracks.map((track) => Object.freeze({ ...track })));
  const cellIndices = Object.freeze(override.cellIndices.map((indices) => new Map(indices)));
  SEMANTIC_GRID_SEQUENCE.set(node, Object.freeze({
    baseNode: override.baseNode,
    sequence: override.sequence,
    rowOffsets,
    rowTracks,
    cellIndices,
  }));
}

export function peekSemanticGridSequenceOverride(node: SemanticViewNode): SemanticGridSequenceOverride | undefined {
  return SEMANTIC_GRID_SEQUENCE.get(node);
}
