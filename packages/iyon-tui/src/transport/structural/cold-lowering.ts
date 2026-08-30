/**
 * Complete derived semantic-to-bridge lowering for the cold structural path.
 *
 * The semantic node remains authoritative. This module is used only when a
 * caller needs a complete bridge object for safe N-API decoding; retained
 * materialization consumes semantic nodes directly.
 */

import { semanticNodeOf } from "../../api/view/semantic-node.ts";
import { nativeResourceForHandleId } from "../native/resources.ts";
import type { NativeViewStateContract } from "../native/addon.ts";
import {
  SEMANTIC_VIEW_KIND,
  type SemanticColor,
  type SemanticDecoration,
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
  VIEW_BRIDGE_SCHEMA_VERSION,
  type BorderNode,
  type BridgeGridCellNode,
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeViewNode,
  type BridgeViewNodeDraft,
  type ColorNode,
  type DecorationNode,
  type StyleNode,
} from "./ir.ts";
import { componentIdForHandleId } from "./component-id.ts";
import type { View } from "../../api/view/view.ts";

const semanticToBridge = new WeakMap<SemanticViewNode, BridgeViewNode>();
/** Semantic subtrees containing physical component identities are rebuilt on
 * every cold lowering so HandleId resolution cannot become stale in a cached
 * bridge artifact. */
const componentBearing = new WeakSet<SemanticViewNode>();
const lowering = new WeakSet<SemanticViewNode>();
const loweringStack: SemanticViewNode[] = [];
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
  // A cached bridge component embeds a physical ComponentId. Do not reuse
  // that artifact after the HandleId's resource is disposed or recreated;
  // component-bearing subtrees are marked while they are lowered and rebuilt
  // on every later cold request. Component-free subtrees retain the normal
  // weak cache without a preflight tree walk.
  const cached = semanticToBridge.get(node);
  if (cached !== undefined && !componentBearing.has(node)) return cached;
  if (lowering.has(node)) throw new TypeError("semantic View graph contains a cycle");
  lowering.add(node);
  loweringStack.push(node);
  try {
    if (node.kind === SEMANTIC_VIEW_KIND.component) {
      for (const active of loweringStack) componentBearing.add(active);
    }
    coldCounters.cold_bridge_objects_allocated += 1;
    const draft = statefulBridgeDraft(node, semanticDraftFor(node));
    const bridge = freezeBridgeNode({
      id: node.id,
      schema: VIEW_BRIDGE_SCHEMA_VERSION,
      ...draft,
    } as BridgeViewNode);
    if (!componentBearing.has(node)) semanticToBridge.set(node, bridge);
    return bridge;
  } finally {
    loweringStack.pop();
    lowering.delete(node);
  }
}

function statefulBridgeDraft(
  node: SemanticViewNode,
  draft: BridgeViewNodeDraft,
): BridgeViewNodeDraft {
  if (node.stateAttachment === undefined) return draft;
  const resource = nativeResourceForHandleId<NativeViewStateContract>(node.stateAttachment, "state");
  if (typeof (resource as { readonly stateId?: unknown }).stateId !== "function") {
    // API-H3 internal fixtures use validation-only resources and do not have
    // a native retained presentation identity to encode in the cold bridge.
    return draft;
  }
  const stateId = resource.stateId();
  if (!Number.isSafeInteger(stateId) || stateId <= 0) {
    throw new RangeError("ViewState native identity must be a positive safe integer");
  }
  return { ...draft, stateAttachment: stateId };
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

function bridgeLayoutChild(child: SemanticLayoutChild): BridgeLayoutChild {
  switch (child.kind) {
    case "normal": return { kind: BRIDGE_LAYOUT_CHILD_KIND.normal, child: lowerSemanticView(child.child) };
    case "fixed": return { kind: BRIDGE_LAYOUT_CHILD_KIND.fixed, size: child.size, child: lowerSemanticView(child.child) };
    case "flex": return { kind: BRIDGE_LAYOUT_CHILD_KIND.flex, child: lowerSemanticView(child.child) };
    case "flexMax": return { kind: BRIDGE_LAYOUT_CHILD_KIND.flexMax, maxRows: child.maxRows, child: lowerSemanticView(child.child) };
    case "contentMax": return { kind: BRIDGE_LAYOUT_CHILD_KIND.contentMax, maxRows: child.maxRows, child: lowerSemanticView(child.child) };
  }
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
