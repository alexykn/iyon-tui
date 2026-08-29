import { semanticNodeOf } from "../../src/api/view/semantic-node.ts";
import {
  lowerColdView,
  lowerSemanticView,
} from "../../src/transport/structural/cold-lowering.ts";
import {
  peekSemanticDerivation,
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  SEMANTIC_VIEW_KIND,
  type SemanticAxisTrack,
  type SemanticDerivation,
  type SemanticGridCell,
  type SemanticGridTrack,
  type SemanticLayoutChild,
  type SemanticViewNode,
} from "../../src/api/view/semantic-node.ts";
import {
  BRIDGE_GRID_TRACK_KIND,
  BRIDGE_HORIZONTAL_ALIGN,
  BRIDGE_LAYOUT_CHILD_KIND,
  BRIDGE_VERTICAL_ALIGN,
  BRIDGE_WRAP_MODE,
  type BridgeGridCellNode,
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeViewNode,
} from "../../src/transport/structural/ir.ts";
import type { View } from "../../src/api/view/view.ts";

/**
 * Direct FFI is an oracle benchmark, not production transport. Its retained
 * candidate still consumes bridge records, so it owns this local metadata
 * adapter rather than reviving bridge sidecars in the framework source.
 */
export type BridgeDerivation =
  | {
      readonly kind: "textLayout";
      readonly base: BridgeViewNode;
      readonly wrap: number;
      readonly align: number;
    }
  | {
      readonly kind: "commonScalar";
      readonly base: BridgeViewNode;
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
  | {
      readonly kind: "axisSet";
      readonly base: BridgeViewNode;
      readonly index: number;
      readonly trackWord: number;
      readonly child: BridgeViewNode;
    }
  | {
      readonly kind: "axisSplice";
      readonly base: BridgeViewNode;
      readonly index: number;
      readonly removeCount: number;
      readonly inserted: readonly { readonly node: BridgeViewNode; readonly trackWord: number }[];
    }
  | {
      readonly kind: "gridCell";
      readonly base: BridgeViewNode;
      readonly row: number;
      readonly column: number;
      readonly child: BridgeViewNode;
    };

export interface BridgeSequence<T> {
  readonly length: number;
  get(index: number): T | undefined;
}

type AxisSequenceEdit =
  | { readonly kind: "axisSet"; readonly index: number }
  | { readonly kind: "axisSplice"; readonly index: number; readonly removeCount: number; readonly insertedCount: number };

interface BridgeSequenceOverride {
  readonly baseNode: BridgeViewNode;
  readonly sequence: BridgeSequence<BridgeLayoutChild>;
  readonly edit?: AxisSequenceEdit;
}

interface BridgeGridSequenceOverride {
  readonly baseNode: BridgeViewNode;
  readonly sequence: BridgeSequence<BridgeGridCellNode>;
  readonly rowOffsets: readonly number[];
  readonly rowTracks: readonly BridgeGridTrackNode[];
  readonly cellIndices: readonly ReadonlyMap<number, number>[];
}

const derivations = new WeakMap<BridgeViewNode, BridgeDerivation>();
const axisSequences = new WeakMap<BridgeViewNode, BridgeSequenceOverride>();
const gridSequences = new WeakMap<BridgeViewNode, BridgeGridSequenceOverride>();

export function lowerColdViewForDirect(view: View): BridgeViewNode {
  const bridge = lowerColdView(view);
  installMetadata(semanticNodeOf(view), bridge, new WeakSet());
  return bridge;
}

export function peekBridgeDerivation(node: BridgeViewNode): BridgeDerivation | undefined {
  return derivations.get(node);
}

export function peekBridgeSequenceOverride(node: BridgeViewNode): BridgeSequenceOverride | undefined {
  return axisSequences.get(node);
}

export function peekBridgeGridSequenceOverride(node: BridgeViewNode): BridgeGridSequenceOverride | undefined {
  return gridSequences.get(node);
}

function installMetadata(
  node: SemanticViewNode,
  bridge: BridgeViewNode,
  seen: WeakSet<SemanticViewNode>,
): void {
  if (seen.has(node)) return;
  seen.add(node);
  const derivation = peekSemanticDerivation(node);
  if (derivation !== undefined) derivations.set(bridge, bridgeDerivation(derivation));

  const axis = peekSemanticSequenceOverride(node);
  if (axis !== undefined) {
    axisSequences.set(bridge, {
      baseNode: lowerSemanticView(axis.baseNode),
      sequence: mapSequence(axis.sequence, bridgeLayoutChild),
      edit: axis.edit,
    });
  }
  const grid = peekSemanticGridSequenceOverride(node);
  if (grid !== undefined) {
    gridSequences.set(bridge, {
      baseNode: lowerSemanticView(grid.baseNode),
      sequence: mapSequence(grid.sequence, bridgeGridCell),
      rowOffsets: grid.rowOffsets,
      rowTracks: grid.rowTracks.map(bridgeGridTrack),
      cellIndices: grid.cellIndices,
    });
  }

  for (const child of semanticChildren(node)) {
    installMetadata(child, lowerSemanticView(child), seen);
  }
}

function semanticChildren(node: SemanticViewNode): SemanticViewNode[] {
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.row:
    case SEMANTIC_VIEW_KIND.column:
      return [...node.children.map((entry) => entry.child)];
    case SEMANTIC_VIEW_KIND.hanging:
      return [node.prefix, node.continuation, node.body];
    case SEMANTIC_VIEW_KIND.grid:
      return node.rows.flatMap((row) => row.cells.map((cell) => cell.view));
    case SEMANTIC_VIEW_KIND.container:
    case SEMANTIC_VIEW_KIND.clamp:
    case SEMANTIC_VIEW_KIND.contentMax:
    case SEMANTIC_VIEW_KIND.decorated:
      return [node.child];
    case SEMANTIC_VIEW_KIND.text:
    case SEMANTIC_VIEW_KIND.diff:
    case SEMANTIC_VIEW_KIND.spacer:
    case SEMANTIC_VIEW_KIND.component:
      return [];
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

function bridgeDerivation(derivation: SemanticDerivation): BridgeDerivation {
  switch (derivation.kind) {
    case "textLayout":
      return {
        kind: "textLayout",
        base: lowerSemanticView(derivation.base),
        wrap: bridgeWrapMode(derivation.wrap),
        align: bridgeHorizontalAlign(derivation.align),
      };
    case "commonScalar": {
      const changes = derivation.changes;
      return {
        kind: "commonScalar",
        base: lowerSemanticView(derivation.base),
        mask: (changes.padding === undefined ? 0 : 4)
          | (changes.width === undefined ? 0 : 8)
          | (changes.height === undefined ? 0 : 16)
          | (changes.minWidth === undefined ? 0 : 32)
          | (changes.maxWidth === undefined ? 0 : 64)
          | (changes.minHeight === undefined ? 0 : 128)
          | (changes.maxHeight === undefined ? 0 : 256),
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

function bridgeTrackWord(track: SemanticAxisTrack | undefined): number {
  // Axis-set reserves zero for preserving the existing track; an explicit
  // semantic normal track must use the nonzero content code.
  if (track === undefined) return 0;
  switch (track.kind) {
    case "normal": return 1;
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
