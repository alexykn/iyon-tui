/**
 * Transitional complete semantic-to-bridge lowering for H3-B.
 *
 * This module is the correctness path while retained-dag.ts still consumes
 * bridge records. It is intentionally derived and weakly cached: the semantic
 * node remains authoritative, and H3-C will remove bridge construction from
 * the warm retained route.
 */

import { semanticNodeOf } from "../../api/view/semantic-node.ts";
import {
  peekSemanticDerivation,
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  SEMANTIC_VIEW_KIND,
  type SemanticAxisTrack,
  type SemanticColor,
  type SemanticCommonScalarChanges,
  type SemanticDecoration,
  type SemanticDerivation,
  type SemanticGridCell,
  type SemanticGridTrack,
  type SemanticLayoutChild,
  type SemanticOverflowIndicator,
  type SemanticStyle,
  type SemanticViewNode,
} from "../../api/view/semantic-node.ts";
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
  setBridgeDerivation,
  setBridgeGridSequenceOverride,
  setBridgeSequenceOverride,
  VIEW_BRIDGE_SCHEMA_VERSION,
  type BorderNode,
  type BridgeDerivation,
  type BridgeGridCellNode,
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeSequence,
  type BridgeViewNode,
  type BridgeViewNodeDraft,
  type ColorNode,
  type DecorationNode,
  type StyleNode,
} from "./ir.ts";
import { componentIdForHandleId } from "./component-view.ts";
import type { View } from "../../api/view/view.ts";

const semanticToBridge = new WeakMap<SemanticViewNode, BridgeViewNode>();
/** Transitional reverse association for the generated bridge materializer. */
const bridgeToSemantic = new WeakMap<object, SemanticViewNode>();
const lowering = new WeakSet<SemanticViewNode>();
const coldCounters = { cold_bridge_objects_allocated: 0 };

export interface ColdLoweringCounters {
  readonly cold_bridge_objects_allocated: number;
}

export function coldLoweringCounterSnapshot(): ColdLoweringCounters {
  return { ...coldCounters };
}

export function resetColdLoweringCounters(): void {
  coldCounters.cold_bridge_objects_allocated = 0;
}

/** Lowers a public View through its private semantic association. */
export function lowerColdView(view: View): BridgeViewNode {
  return lowerSemanticView(semanticNodeOf(view));
}

/** Lowers a semantic node completely for the safe bridge fallback. */
export function lowerSemanticView(node: SemanticViewNode): BridgeViewNode {
  const cached = semanticToBridge.get(node);
  if (cached !== undefined) return cached;
  if (lowering.has(node)) throw new TypeError("semantic View graph contains a cycle");
  lowering.add(node);
  try {
    coldCounters.cold_bridge_objects_allocated += 1;
    const draft = semanticDraftFor(node);
    const bridge = freezeBridgeNode({
      id: node.id,
      schema: VIEW_BRIDGE_SCHEMA_VERSION,
      ...draft,
    } as BridgeViewNode);
    semanticToBridge.set(node, bridge);
    bridgeToSemantic.set(bridge, node);
    attachBridgeSidecars(node, bridge);
    return bridge;
  } finally {
    lowering.delete(node);
  }
}

/** @internal Resolves only bridge records produced by this cold lowerer. */
export function semanticNodeForBridge(bridge: object): SemanticViewNode | undefined {
  return bridgeToSemantic.get(bridge);
}

function semanticDraftFor(node: SemanticViewNode): BridgeViewNodeDraft {
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.text:
      return {
        kind: BRIDGE_VIEW_KIND.text,
        spans: node.spans.map((span) => ({
          text: span.text,
          ...(span.style === undefined ? {} : { style: bridgeStyleFor(span.style) }),
        })),
        wrap: bridgeWrapMode(node.wrap),
        align: bridgeHorizontalAlign(node.align),
      };
    case SEMANTIC_VIEW_KIND.diff:
      return {
        kind: BRIDGE_VIEW_KIND.diff,
        hunks: node.hunks.map((hunk) => ({
          oldRange: { ...hunk.oldRange },
          newRange: { ...hunk.newRange },
          lines: hunk.lines.map((line) => ({
            kind: bridgeDiffLineKind(line.kind),
            text: line.text,
            termination: bridgeDiffTermination(line.termination),
            ...(line.oldLine === undefined ? {} : { oldLine: line.oldLine }),
            ...(line.newLine === undefined ? {} : { newLine: line.newLine }),
          })),
        })),
      };
    case SEMANTIC_VIEW_KIND.spacer:
      return { kind: BRIDGE_VIEW_KIND.spacer, rows: node.rows };
    case SEMANTIC_VIEW_KIND.row:
    case SEMANTIC_VIEW_KIND.column:
      return {
        kind: node.kind === SEMANTIC_VIEW_KIND.row ? BRIDGE_VIEW_KIND.row : BRIDGE_VIEW_KIND.column,
        children: node.children.map(bridgeLayoutChild),
        gap: node.gap,
      };
    case SEMANTIC_VIEW_KIND.hanging:
      return {
        kind: BRIDGE_VIEW_KIND.hanging,
        prefix: lowerSemanticView(node.prefix),
        continuation: lowerSemanticView(node.continuation),
        body: lowerSemanticView(node.body),
      };
    case SEMANTIC_VIEW_KIND.grid:
      return {
        kind: BRIDGE_VIEW_KIND.grid,
        columns: node.columns.map(bridgeGridTrack),
        rows: node.rows.map((row) => ({
          track: bridgeGridTrack(row.track),
          cells: row.cells.map((cell): BridgeGridCellNode => ({
            view: lowerSemanticView(cell.view),
            columnSpan: cell.columnSpan,
            rowSpan: cell.rowSpan,
            horizontalAlign: bridgeHorizontalAlign(cell.horizontalAlign),
            verticalAlign: bridgeVerticalAlign(cell.verticalAlign),
          })),
        })),
        columnGap: node.columnGap,
        rowGap: node.rowGap,
      };
    case SEMANTIC_VIEW_KIND.container:
      return { kind: BRIDGE_VIEW_KIND.container, child: lowerSemanticView(node.child) };
    case SEMANTIC_VIEW_KIND.clamp:
      return {
        kind: BRIDGE_VIEW_KIND.clamp,
        child: lowerSemanticView(node.child),
        maxRows: node.maxRows,
        overflow: bridgeOverflow(node.overflow),
      };
    case SEMANTIC_VIEW_KIND.contentMax:
      return {
        kind: BRIDGE_VIEW_KIND.contentMax,
        child: lowerSemanticView(node.child),
        maxRows: node.maxRows,
      };
    case SEMANTIC_VIEW_KIND.component:
      return {
        kind: BRIDGE_VIEW_KIND.component,
        handle: componentIdForHandleId(node.handleId),
      };
    case SEMANTIC_VIEW_KIND.decorated:
      return {
        kind: BRIDGE_VIEW_KIND.decorated,
        child: lowerSemanticView(node.child),
        decoration: bridgeDecoration(node.decoration),
      };
  }
}

function attachBridgeSidecars(node: SemanticViewNode, bridge: BridgeViewNode): void {
  const derivation = peekSemanticDerivation(node);
  if (derivation !== undefined) setBridgeDerivation(bridge, bridgeDerivation(derivation));

  const axis = peekSemanticSequenceOverride(node);
  if (axis !== undefined) {
    setBridgeSequenceOverride(bridge, {
      baseNode: lowerSemanticView(axis.baseNode),
      sequence: mapSequence(axis.sequence, bridgeLayoutChild),
      edit: axis.edit,
    });
  }

  const grid = peekSemanticGridSequenceOverride(node);
  if (grid !== undefined) {
    setBridgeGridSequenceOverride(bridge, {
      baseNode: lowerSemanticView(grid.baseNode),
      sequence: mapSequence(grid.sequence, bridgeGridCell),
      rowOffsets: grid.rowOffsets,
      rowTracks: grid.rowTracks.map(bridgeGridTrack),
      cellIndices: grid.cellIndices,
    });
  }
}

function mapSequence<T, U>(source: { readonly length: number; get(index: number): T | undefined }, map: (value: T) => U): BridgeSequence<U> {
  const mapped = new Map<number, U>();
  return {
    length: source.length,
    get(index: number): U | undefined {
      if (mapped.has(index)) return mapped.get(index);
      const value = source.get(index);
      if (value === undefined) return undefined;
      const result = map(value);
      mapped.set(index, result);
      return result;
    },
  };
}

function bridgeLayoutChild(child: SemanticLayoutChild): BridgeLayoutChild {
  switch (child.kind) {
    case "normal": return { kind: BRIDGE_LAYOUT_CHILD_KIND.normal, child: lowerSemanticView(child.child) };
    case "fixed": return { kind: BRIDGE_LAYOUT_CHILD_KIND.fixed, size: child.size, child: lowerSemanticView(child.child) };
    case "flex": return { kind: BRIDGE_LAYOUT_CHILD_KIND.flex, child: lowerSemanticView(child.child) };
    case "flexMax": return { kind: BRIDGE_LAYOUT_CHILD_KIND.flexMax, maxRows: child.maxRows, child: lowerSemanticView(child.child) };
    case "contentMax": return { kind: BRIDGE_LAYOUT_CHILD_KIND.contentMax, maxRows: child.maxRows, child: lowerSemanticView(child.child) };
  }
}

function bridgeGridCell(cell: SemanticGridCell): BridgeGridCellNode {
  return {
    view: lowerSemanticView(cell.view),
    columnSpan: cell.columnSpan,
    rowSpan: cell.rowSpan,
    horizontalAlign: bridgeHorizontalAlign(cell.horizontalAlign),
    verticalAlign: bridgeVerticalAlign(cell.verticalAlign),
  };
}

function bridgeGridTrack(track: SemanticGridTrack): BridgeGridTrackNode {
  switch (track.kind) {
    case "content": return { kind: BRIDGE_GRID_TRACK_KIND.content };
    case "contentMax": return { kind: BRIDGE_GRID_TRACK_KIND.contentMax, max: track.max };
    case "fixed": return { kind: BRIDGE_GRID_TRACK_KIND.fixed, size: track.size };
    case "flex": return { kind: BRIDGE_GRID_TRACK_KIND.flex };
    case "flexMax": return { kind: BRIDGE_GRID_TRACK_KIND.flexMax, max: track.max };
  }
}

function bridgeColor(color: SemanticColor): ColorNode {
  switch (color.kind) {
    case "theme": return `theme:${color.key}`;
    case "named": return color.value;
    case "indexed": return { type: "ansi", value: color.value };
    case "rgb": return `#${hex(color.r)}${hex(color.g)}${hex(color.b)}`;
  }
}

function bridgeStyleFor(style: SemanticStyle): StyleNode {
  return {
    ...(style.theme === undefined ? {} : { theme: style.theme }),
    ...(style.foreground === undefined ? {} : { foreground: bridgeColor(style.foreground) }),
    ...(style.background === undefined ? {} : { background: bridgeColor(style.background) }),
    attributes: { ...style.attributes },
  };
}

function bridgeBorder(border: NonNullable<SemanticDecoration["border"]>): BorderNode {
  return {
    ...(border.glyphs === undefined ? {} : { glyphs: { ...border.glyphs } }),
    ...(border.style === undefined ? {} : { style: border.style }),
    ...(border.edges === undefined ? {} : { edges: border.edges }),
    ...(border.color === undefined ? {} : { color: bridgeColor(border.color) }),
  };
}

function bridgeDecoration(decoration: SemanticDecoration): DecorationNode {
  return {
    ...(decoration.padding === undefined ? {} : { padding: { ...decoration.padding } }),
    ...(decoration.background === undefined ? {} : { background: bridgeColor(decoration.background) }),
    ...(decoration.foreground === undefined ? {} : { foreground: bridgeColor(decoration.foreground) }),
    ...(decoration.border === undefined ? {} : { border: bridgeBorder(decoration.border) }),
    style: bridgeStyleFor(decoration.style),
    ...(decoration.styleStates === undefined ? {} : { styleStates: { ...decoration.styleStates } }),
    ...(decoration.width === undefined ? {} : { width: decoration.width }),
    ...(decoration.height === undefined ? {} : { height: decoration.height }),
    ...(decoration.minWidth === undefined ? {} : { minWidth: decoration.minWidth }),
    ...(decoration.maxWidth === undefined ? {} : { maxWidth: decoration.maxWidth }),
    ...(decoration.minHeight === undefined ? {} : { minHeight: decoration.minHeight }),
    ...(decoration.maxHeight === undefined ? {} : { maxHeight: decoration.maxHeight }),
  };
}

function bridgeOverflow(overflow: SemanticOverflowIndicator): BridgeOverflowIndicatorNode {
  switch (overflow.kind) {
    case "none": return { kind: BRIDGE_OVERFLOW_KIND.none };
    case "ellipsis": return { kind: BRIDGE_OVERFLOW_KIND.ellipsis, style: bridgeStyleFor(overflow.style) };
    case "footer": return { kind: BRIDGE_OVERFLOW_KIND.footer, prefix: overflow.prefix, style: bridgeStyleFor(overflow.style) };
  }
}

function bridgeDerivation(derivation: SemanticDerivation): BridgeDerivation {
  switch (derivation.kind) {
    case "textLayout":
      return {
        kind: "textLayout",
        base: lowerSemanticView(derivation.base),
        wrap: bridgeWrapMode(derivation.wrap),
        align: bridgeHorizontalAlign(derivation.align),
      };
    case "commonScalar":
      return bridgeCommonScalarDerivation(derivation.base, derivation.changes);
    case "axisSet":
      return {
        kind: "axisSet",
        base: lowerSemanticView(derivation.base),
        index: derivation.index,
        trackWord: bridgeTrackWord(derivation.track),
        child: lowerSemanticView(derivation.child),
      };
    case "axisSplice":
      return {
        kind: "axisSplice",
        base: lowerSemanticView(derivation.base),
        index: derivation.index,
        removeCount: derivation.removeCount,
        inserted: derivation.inserted.map((entry) => ({
          node: lowerSemanticView(entry.child),
          trackWord: bridgeTrackWord(entry.track),
        })),
      };
    case "gridCell":
      return {
        kind: "gridCell",
        base: lowerSemanticView(derivation.base),
        row: derivation.row,
        column: derivation.column,
        child: lowerSemanticView(derivation.child),
      };
  }
}

function bridgeCommonScalarDerivation(
  base: SemanticViewNode,
  changes: SemanticCommonScalarChanges,
): Extract<BridgeDerivation, { kind: "commonScalar" }> {
  const mask =
    (changes.padding === undefined ? 0 : 4)
    | (changes.width === undefined ? 0 : 8)
    | (changes.height === undefined ? 0 : 16)
    | (changes.minWidth === undefined ? 0 : 32)
    | (changes.maxWidth === undefined ? 0 : 64)
    | (changes.minHeight === undefined ? 0 : 128)
    | (changes.maxHeight === undefined ? 0 : 256);
  return {
    kind: "commonScalar",
    base: lowerSemanticView(base),
    mask,
    paddingTopRight: changes.padding === undefined ? 0 : (changes.padding.top & 0xffff) | ((changes.padding.right & 0xffff) << 16),
    paddingBottomLeft: changes.padding === undefined ? 0 : (changes.padding.bottom & 0xffff) | ((changes.padding.left & 0xffff) << 16),
    widthRule: changes.width === undefined ? 0 : changes.width === "fit" ? 1 : 2,
    heightRule: changes.height === undefined ? 0 : changes.height === "fit" ? 1 : 2,
    minWidth: changes.minWidth ?? 0,
    maxWidth: changes.maxWidth ?? 0,
    minHeight: changes.minHeight ?? 0,
    maxHeight: changes.maxHeight ?? 0,
  };
}

function bridgeTrackWord(track: SemanticAxisTrack | undefined): number {
  // The compact retained edit word is a distinct encoding from
  // BRIDGE_LAYOUT_CHILD_KIND: zero=normal, 2=contentMax, 3=fixed,
  // 4=flex, and 5=flexMax. Keep this mapping explicit at the boundary.
  if (track === undefined || track.kind === "normal") return 0;
  switch (track.kind) {
    case "contentMax": return 2 | (track.maxRows << 8);
    case "fixed": return 3 | (track.size << 8);
    case "flex": return 4;
    case "flexMax": return 5 | (track.maxRows << 8);
  }
}

function bridgeWrapMode(mode: "wordThenGrapheme" | "grapheme" | "noWrap"): number {
  switch (mode) {
    case "wordThenGrapheme": return BRIDGE_WRAP_MODE.wordThenGrapheme;
    case "grapheme": return BRIDGE_WRAP_MODE.grapheme;
    case "noWrap": return BRIDGE_WRAP_MODE.noWrap;
  }
}

function bridgeHorizontalAlign(align: "start" | "center" | "end"): number {
  switch (align) {
    case "start": return BRIDGE_HORIZONTAL_ALIGN.start;
    case "center": return BRIDGE_HORIZONTAL_ALIGN.center;
    case "end": return BRIDGE_HORIZONTAL_ALIGN.end;
  }
}

function bridgeVerticalAlign(align: "top" | "center" | "bottom"): number {
  switch (align) {
    case "top": return BRIDGE_VERTICAL_ALIGN.top;
    case "center": return BRIDGE_VERTICAL_ALIGN.center;
    case "bottom": return BRIDGE_VERTICAL_ALIGN.bottom;
  }
}

function bridgeDiffLineKind(kind: "context" | "addition" | "deletion"): (typeof BRIDGE_DIFF_LINE_KIND)[keyof typeof BRIDGE_DIFF_LINE_KIND] {
  switch (kind) {
    case "context": return BRIDGE_DIFF_LINE_KIND.context;
    case "addition": return BRIDGE_DIFF_LINE_KIND.addition;
    case "deletion": return BRIDGE_DIFF_LINE_KIND.deletion;
  }
}

function bridgeDiffTermination(termination: "terminated" | "unterminated"): (typeof BRIDGE_DIFF_LINE_TERMINATION)[keyof typeof BRIDGE_DIFF_LINE_TERMINATION] {
  return termination === "terminated" ? BRIDGE_DIFF_LINE_TERMINATION.terminated : BRIDGE_DIFF_LINE_TERMINATION.unterminated;
}

function hex(value: number): string {
  return value.toString(16).padStart(2, "0");
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
      for (const hunk of node.hunks) {
        Object.freeze(hunk.oldRange);
        Object.freeze(hunk.newRange);
        for (const line of hunk.lines) Object.freeze(line);
        Object.freeze(hunk.lines);
        Object.freeze(hunk);
      }
      Object.freeze(node.hunks);
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
    case BRIDGE_VIEW_KIND.clamp:
      if (node.overflow !== undefined && node.overflow.kind !== BRIDGE_OVERFLOW_KIND.none) freezeStyle(node.overflow.style);
      if (node.overflow !== undefined) Object.freeze(node.overflow);
      break;
    case BRIDGE_VIEW_KIND.decorated:
      freezeDecoration(node.decoration);
      break;
    case BRIDGE_VIEW_KIND.hanging:
    case BRIDGE_VIEW_KIND.spacer:
    case BRIDGE_VIEW_KIND.container:
    case BRIDGE_VIEW_KIND.contentMax:
    case BRIDGE_VIEW_KIND.component:
      break;
  }
  return Object.freeze(node);
}

function freezeStyle(style: StyleNode): void {
  if (style.foreground !== undefined && typeof style.foreground === "object") Object.freeze(style.foreground);
  if (style.background !== undefined && typeof style.background === "object") Object.freeze(style.background);
  Object.freeze(style.attributes);
  Object.freeze(style);
}

function freezeDecoration(decoration: DecorationNode): void {
  if (decoration.padding !== undefined) Object.freeze(decoration.padding);
  if (decoration.background !== undefined && typeof decoration.background === "object") Object.freeze(decoration.background);
  if (decoration.foreground !== undefined && typeof decoration.foreground === "object") Object.freeze(decoration.foreground);
  if (decoration.border !== undefined) {
    if (decoration.border.glyphs !== undefined) Object.freeze(decoration.border.glyphs);
    if (decoration.border.color !== undefined && typeof decoration.border.color === "object") Object.freeze(decoration.border.color);
    Object.freeze(decoration.border);
  }
  freezeStyle(decoration.style);
  if (decoration.styleStates !== undefined) Object.freeze(decoration.styleStates);
  Object.freeze(decoration);
}
