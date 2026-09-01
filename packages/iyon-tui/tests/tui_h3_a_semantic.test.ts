import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import { DiffHunk, DiffLine, DiffRange, TextSpan, View } from "../src/index.ts";
import type { ComponentId } from "../src/api/extensions/traits/component.ts";
import type { HandleId } from "../src/api/controls/framework-handle.ts";
import type { OverflowIndicator } from "../src/api/view/view.ts";
import { Insets } from "../src/api/view/geometry.ts";
import { StyleRef, StyleSpec } from "../src/api/presentation/style.ts";
import { themeColor } from "../src/api/presentation/theme.ts";
import type { AnsiColor } from "../src/api/presentation/theme.ts";
import {
  semanticBorderFor,
  semanticColorFor,
  semanticDecorationFor,
  semanticEmptyStyle,
  semanticOverflowFor,
  semanticStyleFor,
  semanticTextSpanFor,
} from "../src/api/presentation/semantic-style.ts";
import { PersistentSeq } from "../src/composition/persistent-seq.ts";
import {
  createSemanticViewNode,
  installSemanticNode,
  peekSemanticDerivation,
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  SEMANTIC_VIEW_KIND,
  semanticNodeOf,
  setSemanticDerivation,
  setSemanticGridSequenceOverride,
  setSemanticSequenceOverride,
  type SemanticAxisTrack,
  type SemanticBorder,
  type SemanticColor,
  type SemanticDecoration,
  type SemanticDerivation,
  type SemanticGridTrack,
  type SemanticLayoutChild,
  type SemanticOverflowIndicator,
  type SemanticSequence,
  type SemanticStyle,
  type SemanticViewNode,
  type SemanticViewNodeDraft,
} from "../src/api/view/semantic-node.ts";
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
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeViewNode,
  type ColorNode,
  type DecorationNode,
  type StyleNode,
} from "../src/transport/structural/ir.ts";
import { colorNodeFor, borderNodeFor, styleNodeFor, textSpanNodeFor } from "../src/transport/structural/style-lowering.ts";
import { lowerColdView } from "../src/transport/structural/cold-lowering.ts";
import {
  axisSetChildForTransport,
  axisSpliceForTransport,
  gridSetCellForTransport,
} from "../src/api/view/view.ts";

const ANSI_COLORS = new Set([
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "gray",
  "darkGray",
  "lightRed",
  "lightGreen",
  "lightYellow",
  "lightBlue",
  "lightMagenta",
  "lightCyan",
  "white",
]);

/**
 * Test-only bridge oracle. It intentionally lives under tests: H3-A proves
 * the semantic vocabulary against the complete cold bridge fallback but does
 * not make this compatibility translation part of the runtime route.
 */
function semanticFromBridge(root: BridgeViewNode): SemanticViewNode {
  const seen = new WeakMap<BridgeViewNode, SemanticViewNode>();

  const visit = (node: BridgeViewNode): SemanticViewNode => {
    const existing = seen.get(node);
    if (existing !== undefined) return existing;

    let draft: SemanticViewNodeDraft;
    switch (node.kind) {
      case BRIDGE_VIEW_KIND.text:
        draft = {
          kind: SEMANTIC_VIEW_KIND.text,
          spans: node.spans.map((span) => Object.freeze({
            text: span.text,
            ...(span.style === undefined ? {} : { style: semanticStyleFromBridge(span.style) }),
          })),
          wrap: semanticWrapFromBridge(node.wrap),
          align: semanticHorizontalAlignFromBridge(node.align),
        };
        break;
      case BRIDGE_VIEW_KIND.diff:
        draft = {
          kind: SEMANTIC_VIEW_KIND.diff,
          hunks: node.hunks.map((hunk) => Object.freeze({
            oldRange: Object.freeze({ ...hunk.oldRange }),
            newRange: Object.freeze({ ...hunk.newRange }),
            lines: Object.freeze(hunk.lines.map((line) => Object.freeze({
              kind: semanticDiffLineKindFromBridge(line.kind),
              text: line.text,
              termination: semanticDiffTerminationFromBridge(line.termination),
              ...(line.oldLine === undefined ? {} : { oldLine: line.oldLine }),
              ...(line.newLine === undefined ? {} : { newLine: line.newLine }),
            }))),
          })),
        };
        break;
      case BRIDGE_VIEW_KIND.spacer:
        draft = { kind: SEMANTIC_VIEW_KIND.spacer, rows: node.rows };
        break;
      case BRIDGE_VIEW_KIND.row:
      case BRIDGE_VIEW_KIND.column:
        draft = {
          kind: node.kind === BRIDGE_VIEW_KIND.row ? SEMANTIC_VIEW_KIND.row : SEMANTIC_VIEW_KIND.column,
          children: node.children.map((child) => semanticLayoutChildFromBridge(child, visit)),
          gap: node.gap,
        };
        break;
      case BRIDGE_VIEW_KIND.hanging:
        draft = {
          kind: SEMANTIC_VIEW_KIND.hanging,
          prefix: visit(node.prefix),
          continuation: visit(node.continuation),
          body: visit(node.body),
        };
        break;
      case BRIDGE_VIEW_KIND.grid:
        draft = {
          kind: SEMANTIC_VIEW_KIND.grid,
          columns: node.columns.map(semanticGridTrackFromBridge),
          rows: node.rows.map((row) => Object.freeze({
            track: semanticGridTrackFromBridge(row.track),
            cells: Object.freeze(row.cells.map((cell) => Object.freeze({
              view: visit(cell.view),
              columnSpan: cell.columnSpan,
              rowSpan: cell.rowSpan,
              horizontalAlign: semanticHorizontalAlignFromBridge(cell.horizontalAlign),
              verticalAlign: semanticVerticalAlignFromBridge(cell.verticalAlign),
            }))),
          })),
          columnGap: node.columnGap,
          rowGap: node.rowGap,
        };
        break;
      case BRIDGE_VIEW_KIND.container:
        draft = { kind: SEMANTIC_VIEW_KIND.container, child: visit(node.child) };
        break;
      case BRIDGE_VIEW_KIND.clamp:
        if (node.overflow === undefined) throw new TypeError("valid clamp bridge node must carry overflow semantics");
        draft = {
          kind: SEMANTIC_VIEW_KIND.clamp,
          child: visit(node.child),
          maxRows: requireBridgeMaxRows(node.maxRows),
          overflow: semanticOverflowFromBridge(node.overflow),
        };
        break;
      case BRIDGE_VIEW_KIND.contentMax:
        draft = {
          kind: SEMANTIC_VIEW_KIND.contentMax,
          child: visit(node.child),
          maxRows: node.maxRows,
        };
        break;
      case BRIDGE_VIEW_KIND.component:
        draft = {
          kind: SEMANTIC_VIEW_KIND.component,
          handleId: node.handle as unknown as HandleId,
        };
        break;
      case BRIDGE_VIEW_KIND.decorated:
        draft = {
          kind: SEMANTIC_VIEW_KIND.decorated,
          child: visit(node.child),
          decoration: semanticDecorationFromBridge(node.decoration),
        };
        break;
      case BRIDGE_VIEW_KIND.contentHost:
        draft = {
          kind: SEMANTIC_VIEW_KIND.contentHost,
          contentAttachment: node.contentPortId as unknown as HandleId,
        };
        break;
      default:
        return unexpectedBridgeKind(node);
    }

    const semantic = createSemanticViewNode(node.id, draft);
    seen.set(node, semantic);
    return semantic;
  };

  return visit(root);
}

function semanticDerivationComparable(derivation: SemanticDerivation): unknown {
  switch (derivation.kind) {
    case "textLayout":
      return { kind: derivation.kind, base: derivation.base.id, wrap: derivation.wrap, align: derivation.align };
    case "commonScalar":
      return { kind: derivation.kind, base: derivation.base.id, changes: derivation.changes };
    case "axisSet":
      return {
        kind: derivation.kind,
        base: derivation.base.id,
        index: derivation.index,
        track: derivation.track,
        child: derivation.child.id,
      };
    case "axisSplice":
      return {
        kind: derivation.kind,
        base: derivation.base.id,
        index: derivation.index,
        removeCount: derivation.removeCount,
        inserted: derivation.inserted.map((entry) => ({ track: entry.track, child: entry.child.id })),
      };
    case "gridCell":
      return { kind: derivation.kind, base: derivation.base.id, row: derivation.row, column: derivation.column, child: derivation.child.id };
  }
}

function semanticColorFromBridge(color: ColorNode): SemanticColor {
  if (typeof color !== "string") return Object.freeze({ kind: "indexed", value: color.value });
  if (color.startsWith("theme:")) return Object.freeze({ kind: "theme", key: color.slice("theme:".length) });
  if (/^#[0-9a-f]{6}$/u.test(color)) {
    return Object.freeze({
      kind: "rgb",
      r: Number.parseInt(color.slice(1, 3), 16),
      g: Number.parseInt(color.slice(3, 5), 16),
      b: Number.parseInt(color.slice(5, 7), 16),
    });
  }
  if (!ANSI_COLORS.has(color)) throw new TypeError(`unknown bridge color ${color}`);
  return Object.freeze({ kind: "named", value: color as AnsiColor });
}

function semanticStyleFromBridge(style: StyleNode): SemanticStyle {
  return Object.freeze({
    ...(style.theme === undefined ? {} : { theme: style.theme }),
    ...(style.foreground === undefined ? {} : { foreground: semanticColorFromBridge(style.foreground) }),
    ...(style.background === undefined ? {} : { background: semanticColorFromBridge(style.background) }),
    attributes: Object.freeze({ ...style.attributes }),
  });
}

function semanticBorderFromBridge(border: NonNullable<DecorationNode["border"]>): SemanticBorder {
  const glyphs = border.glyphs === undefined ? undefined : Object.freeze({
    top: border.glyphs.top,
    right: border.glyphs.right,
    bottom: border.glyphs.bottom,
    left: border.glyphs.left,
    topLeft: border.glyphs.topLeft,
    topRight: border.glyphs.topRight,
    bottomLeft: border.glyphs.bottomLeft,
    bottomRight: border.glyphs.bottomRight,
  });
  return Object.freeze({
    ...(glyphs === undefined ? {} : { glyphs }),
    ...(border.style === undefined ? {} : { style: border.style }),
    ...(border.edges === undefined ? {} : { edges: border.edges }),
    ...(border.color === undefined ? {} : { color: semanticColorFromBridge(border.color) }),
  });
}

function semanticDecorationFromBridge(decoration: DecorationNode): SemanticDecoration {
  return Object.freeze({
    ...(decoration.padding === undefined ? {} : { padding: Object.freeze({ ...decoration.padding }) }),
    ...(decoration.background === undefined ? {} : { background: semanticColorFromBridge(decoration.background) }),
    ...(decoration.foreground === undefined ? {} : { foreground: semanticColorFromBridge(decoration.foreground) }),
    ...(decoration.border === undefined ? {} : { border: semanticBorderFromBridge(decoration.border) }),
    style: semanticStyleFromBridge(decoration.style),
    ...(decoration.styleStates === undefined ? {} : { styleStates: Object.freeze({ ...decoration.styleStates }) }),
    ...(decoration.width === undefined ? {} : { width: decoration.width }),
    ...(decoration.height === undefined ? {} : { height: decoration.height }),
    ...(decoration.minWidth === undefined ? {} : { minWidth: decoration.minWidth }),
    ...(decoration.maxWidth === undefined ? {} : { maxWidth: decoration.maxWidth }),
    ...(decoration.minHeight === undefined ? {} : { minHeight: decoration.minHeight }),
    ...(decoration.maxHeight === undefined ? {} : { maxHeight: decoration.maxHeight }),
  });
}

function semanticOverflowFromBridge(overflow: BridgeOverflowIndicatorNode): SemanticOverflowIndicator {
  switch (overflow.kind) {
    case BRIDGE_OVERFLOW_KIND.none: return Object.freeze({ kind: "none" });
    case BRIDGE_OVERFLOW_KIND.ellipsis: return Object.freeze({ kind: "ellipsis", style: semanticStyleFromBridge(overflow.style) });
    case BRIDGE_OVERFLOW_KIND.footer: return Object.freeze({ kind: "footer", prefix: overflow.prefix, style: semanticStyleFromBridge(overflow.style) });
    default: return unexpectedBridgeOverflow(overflow);
  }
}

function semanticLayoutChildFromBridge(
  child: BridgeLayoutChild,
  visit: (node: BridgeViewNode) => SemanticViewNode,
): SemanticLayoutChild {
  switch (child.kind) {
    case BRIDGE_LAYOUT_CHILD_KIND.normal: return Object.freeze({ kind: "normal", child: visit(child.child) });
    case BRIDGE_LAYOUT_CHILD_KIND.fixed: return Object.freeze({ kind: "fixed", size: child.size, child: visit(child.child) });
    case BRIDGE_LAYOUT_CHILD_KIND.flex: return Object.freeze({ kind: "flex", child: visit(child.child) });
    case BRIDGE_LAYOUT_CHILD_KIND.flexMax: return Object.freeze({ kind: "flexMax", maxRows: child.maxRows, child: visit(child.child) });
    case BRIDGE_LAYOUT_CHILD_KIND.contentMax: return Object.freeze({ kind: "contentMax", maxRows: child.maxRows, child: visit(child.child) });
    default: return unexpectedBridgeLayoutChild(child);
  }
}

function semanticGridTrackFromBridge(track: BridgeGridTrackNode): SemanticGridTrack {
  switch (track.kind) {
    case BRIDGE_GRID_TRACK_KIND.content: return Object.freeze({ kind: "content" });
    case BRIDGE_GRID_TRACK_KIND.contentMax: return Object.freeze({ kind: "contentMax", max: track.max });
    case BRIDGE_GRID_TRACK_KIND.fixed: return Object.freeze({ kind: "fixed", size: track.size });
    case BRIDGE_GRID_TRACK_KIND.flex: return Object.freeze({ kind: "flex" });
    case BRIDGE_GRID_TRACK_KIND.flexMax: return Object.freeze({ kind: "flexMax", max: track.max });
    default: return unexpectedBridgeGridTrack(track);
  }
}

function semanticWrapFromBridge(value: number): "wordThenGrapheme" | "grapheme" | "noWrap" {
  switch (value) {
    case BRIDGE_WRAP_MODE.wordThenGrapheme: return "wordThenGrapheme";
    case BRIDGE_WRAP_MODE.grapheme: return "grapheme";
    case BRIDGE_WRAP_MODE.noWrap: return "noWrap";
    default: throw new TypeError(`unknown bridge wrap mode ${value}`);
  }
}

function semanticHorizontalAlignFromBridge(value: number): "start" | "center" | "end" {
  switch (value) {
    case BRIDGE_HORIZONTAL_ALIGN.start: return "start";
    case BRIDGE_HORIZONTAL_ALIGN.center: return "center";
    case BRIDGE_HORIZONTAL_ALIGN.end: return "end";
    default: throw new TypeError(`unknown bridge horizontal alignment ${value}`);
  }
}

function semanticVerticalAlignFromBridge(value: number): "top" | "center" | "bottom" {
  switch (value) {
    case BRIDGE_VERTICAL_ALIGN.top: return "top";
    case BRIDGE_VERTICAL_ALIGN.center: return "center";
    case BRIDGE_VERTICAL_ALIGN.bottom: return "bottom";
    default: throw new TypeError(`unknown bridge vertical alignment ${value}`);
  }
}

function semanticDiffLineKindFromBridge(value: number): "context" | "addition" | "deletion" {
  switch (value) {
    case BRIDGE_DIFF_LINE_KIND.context: return "context";
    case BRIDGE_DIFF_LINE_KIND.addition: return "addition";
    case BRIDGE_DIFF_LINE_KIND.deletion: return "deletion";
    default: throw new TypeError(`unknown bridge diff line kind ${value}`);
  }
}

function semanticDiffTerminationFromBridge(value: number): "terminated" | "unterminated" {
  switch (value) {
    case BRIDGE_DIFF_LINE_TERMINATION.terminated: return "terminated";
    case BRIDGE_DIFF_LINE_TERMINATION.unterminated: return "unterminated";
    default: throw new TypeError(`unknown bridge diff termination ${value}`);
  }
}

function requireBridgeMaxRows(value: number | undefined): number {
  if (value === undefined) throw new TypeError("valid clamp bridge node must carry maxRows");
  return value;
}

function unexpectedBridgeKind(node: never): never {
  throw new TypeError(`unknown bridge node kind ${(node as { kind?: unknown }).kind}`);
}
function unexpectedBridgeOverflow(value: never): never {
  throw new TypeError(`unknown bridge overflow kind ${(value as { kind?: unknown }).kind}`);
}
function unexpectedBridgeLayoutChild(value: never): never {
  throw new TypeError(`unknown bridge layout child kind ${(value as { kind?: unknown }).kind}`);
}
function unexpectedBridgeGridTrack(value: never): never {
  throw new TypeError(`unknown bridge grid track kind ${(value as { kind?: unknown }).kind}`);
}

function semanticKindName(kind: number): string {
  switch (kind) {
    case SEMANTIC_VIEW_KIND.text: return "text";
    case SEMANTIC_VIEW_KIND.diff: return "diff";
    case SEMANTIC_VIEW_KIND.spacer: return "spacer";
    case SEMANTIC_VIEW_KIND.row: return "row";
    case SEMANTIC_VIEW_KIND.column: return "column";
    case SEMANTIC_VIEW_KIND.grid: return "grid";
    case SEMANTIC_VIEW_KIND.hanging: return "hanging";
    case SEMANTIC_VIEW_KIND.container: return "container";
    case SEMANTIC_VIEW_KIND.clamp: return "clamp";
    case SEMANTIC_VIEW_KIND.contentMax: return "contentMax";
    case SEMANTIC_VIEW_KIND.component: return "component";
    case SEMANTIC_VIEW_KIND.decorated: return "decorated";
    default: throw new TypeError(`unknown semantic kind ${kind}`);
  }
}

function semanticComparable(node: SemanticViewNode): unknown {
  const base = { id: node.id, kind: semanticKindName(node.kind) };
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.text:
      return {
        ...base,
        spans: node.spans.map((span) => ({ text: span.text, style: semanticStyleComparable(span.style) })),
        wrap: node.wrap,
        align: node.align,
      };
    case SEMANTIC_VIEW_KIND.diff:
      return {
        ...base,
        hunks: node.hunks.map((hunk) => ({
          oldRange: hunk.oldRange,
          newRange: hunk.newRange,
          lines: hunk.lines,
        })),
      };
    case SEMANTIC_VIEW_KIND.spacer:
      return { ...base, rows: node.rows };
    case SEMANTIC_VIEW_KIND.row:
    case SEMANTIC_VIEW_KIND.column:
      return { ...base, children: node.children.map(semanticLayoutChildComparable), gap: node.gap };
    case SEMANTIC_VIEW_KIND.hanging:
      return { ...base, prefix: semanticComparable(node.prefix), continuation: semanticComparable(node.continuation), body: semanticComparable(node.body) };
    case SEMANTIC_VIEW_KIND.grid:
      return {
        ...base,
        columns: node.columns,
        rows: node.rows.map((row) => ({ track: row.track, cells: row.cells.map((cell) => ({
          view: semanticComparable(cell.view),
          columnSpan: cell.columnSpan,
          rowSpan: cell.rowSpan,
          horizontalAlign: cell.horizontalAlign,
          verticalAlign: cell.verticalAlign,
        })) })),
        columnGap: node.columnGap,
        rowGap: node.rowGap,
      };
    case SEMANTIC_VIEW_KIND.container:
      return { ...base, child: semanticComparable(node.child) };
    case SEMANTIC_VIEW_KIND.clamp:
      return { ...base, child: semanticComparable(node.child), maxRows: node.maxRows, overflow: semanticOverflowComparable(node.overflow) };
    case SEMANTIC_VIEW_KIND.contentMax:
      return { ...base, child: semanticComparable(node.child), maxRows: node.maxRows };
    case SEMANTIC_VIEW_KIND.component:
      return { ...base, handle: node.handleId };
    case SEMANTIC_VIEW_KIND.decorated:
      return { ...base, child: semanticComparable(node.child), decoration: semanticDecorationComparable(node.decoration) };
  }
}

function bridgeComparable(node: BridgeViewNode): unknown {
  const base = { id: node.id, kind: bridgeKindName(node.kind) };
  switch (node.kind) {
    case BRIDGE_VIEW_KIND.text:
      return {
        ...base,
        spans: node.spans.map((span) => ({ text: span.text, style: bridgeStyleComparable(span.style) })),
        wrap: semanticWrapFromBridge(node.wrap),
        align: semanticHorizontalAlignFromBridge(node.align),
      };
    case BRIDGE_VIEW_KIND.diff:
      return { ...base, hunks: node.hunks.map((hunk) => ({ oldRange: hunk.oldRange, newRange: hunk.newRange, lines: hunk.lines.map((line) => ({
        kind: semanticDiffLineKindFromBridge(line.kind),
        text: line.text,
        termination: semanticDiffTerminationFromBridge(line.termination),
        ...(line.oldLine === undefined ? {} : { oldLine: line.oldLine }),
        ...(line.newLine === undefined ? {} : { newLine: line.newLine }),
      })) })) };
    case BRIDGE_VIEW_KIND.spacer:
      return { ...base, rows: node.rows };
    case BRIDGE_VIEW_KIND.row:
    case BRIDGE_VIEW_KIND.column:
      return { ...base, children: node.children.map(bridgeLayoutChildComparable), gap: node.gap };
    case BRIDGE_VIEW_KIND.hanging:
      return { ...base, prefix: bridgeComparable(node.prefix), continuation: bridgeComparable(node.continuation), body: bridgeComparable(node.body) };
    case BRIDGE_VIEW_KIND.grid:
      return {
        ...base,
        columns: node.columns.map(bridgeGridTrackComparable),
        rows: node.rows.map((row) => ({ track: bridgeGridTrackComparable(row.track), cells: row.cells.map((cell) => ({
          view: bridgeComparable(cell.view),
          columnSpan: cell.columnSpan,
          rowSpan: cell.rowSpan,
          horizontalAlign: semanticHorizontalAlignFromBridge(cell.horizontalAlign),
          verticalAlign: semanticVerticalAlignFromBridge(cell.verticalAlign),
        })) })),
        columnGap: node.columnGap,
        rowGap: node.rowGap,
      };
    case BRIDGE_VIEW_KIND.container:
      return { ...base, child: bridgeComparable(node.child) };
    case BRIDGE_VIEW_KIND.clamp:
      return {
        ...base,
        child: bridgeComparable(node.child),
        maxRows: node.maxRows,
        overflow: node.overflow === undefined ? undefined : bridgeOverflowComparable(node.overflow),
      };
    case BRIDGE_VIEW_KIND.contentMax:
      return { ...base, child: bridgeComparable(node.child), maxRows: node.maxRows };
    case BRIDGE_VIEW_KIND.component:
      return { ...base, handle: node.handle };
    case BRIDGE_VIEW_KIND.decorated:
      return { ...base, child: bridgeComparable(node.child), decoration: bridgeDecorationComparable(node.decoration) };
  }
}

function bridgeKindName(kind: number): string {
  switch (kind) {
    case BRIDGE_VIEW_KIND.text: return "text";
    case BRIDGE_VIEW_KIND.diff: return "diff";
    case BRIDGE_VIEW_KIND.spacer: return "spacer";
    case BRIDGE_VIEW_KIND.row: return "row";
    case BRIDGE_VIEW_KIND.column: return "column";
    case BRIDGE_VIEW_KIND.grid: return "grid";
    case BRIDGE_VIEW_KIND.hanging: return "hanging";
    case BRIDGE_VIEW_KIND.container: return "container";
    case BRIDGE_VIEW_KIND.clamp: return "clamp";
    case BRIDGE_VIEW_KIND.contentMax: return "contentMax";
    case BRIDGE_VIEW_KIND.component: return "component";
    case BRIDGE_VIEW_KIND.decorated: return "decorated";
    default: throw new TypeError(`unknown bridge kind ${kind}`);
  }
}

function semanticStyleComparable(style: SemanticStyle | undefined): unknown {
  if (style === undefined) return undefined;
  return {
    theme: style.theme,
    foreground: style.foreground,
    background: style.background,
    attributes: style.attributes,
  };
}

function bridgeStyleComparable(style: StyleNode | undefined): unknown {
  if (style === undefined) return undefined;
  return {
    theme: style.theme,
    foreground: style.foreground === undefined ? undefined : semanticColorFromBridge(style.foreground),
    background: style.background === undefined ? undefined : semanticColorFromBridge(style.background),
    attributes: style.attributes,
  };
}

function semanticLayoutChildComparable(child: SemanticLayoutChild): unknown {
  return {
    kind: child.kind,
    ...(child.kind === "fixed" ? { size: child.size } : {}),
    ...(child.kind === "flexMax" || child.kind === "contentMax" ? { maxRows: child.maxRows } : {}),
    child: semanticComparable(child.child),
  };
}

function bridgeLayoutChildComparable(child: BridgeLayoutChild): unknown {
  switch (child.kind) {
    case BRIDGE_LAYOUT_CHILD_KIND.normal: return { kind: "normal", child: bridgeComparable(child.child) };
    case BRIDGE_LAYOUT_CHILD_KIND.fixed: return { kind: "fixed", size: child.size, child: bridgeComparable(child.child) };
    case BRIDGE_LAYOUT_CHILD_KIND.flex: return { kind: "flex", child: bridgeComparable(child.child) };
    case BRIDGE_LAYOUT_CHILD_KIND.flexMax: return { kind: "flexMax", maxRows: child.maxRows, child: bridgeComparable(child.child) };
    case BRIDGE_LAYOUT_CHILD_KIND.contentMax: return { kind: "contentMax", maxRows: child.maxRows, child: bridgeComparable(child.child) };
  }
}

function bridgeGridTrackComparable(track: BridgeGridTrackNode): unknown {
  switch (track.kind) {
    case BRIDGE_GRID_TRACK_KIND.content: return { kind: "content" };
    case BRIDGE_GRID_TRACK_KIND.contentMax: return { kind: "contentMax", max: track.max };
    case BRIDGE_GRID_TRACK_KIND.fixed: return { kind: "fixed", size: track.size };
    case BRIDGE_GRID_TRACK_KIND.flex: return { kind: "flex" };
    case BRIDGE_GRID_TRACK_KIND.flexMax: return { kind: "flexMax", max: track.max };
  }
}

function semanticOverflowComparable(overflow: SemanticOverflowIndicator): unknown {
  if (overflow.kind === "none") return { kind: "none" };
  return {
    kind: overflow.kind,
    ...(overflow.kind === "footer" ? { prefix: overflow.prefix } : {}),
    style: semanticStyleComparable(overflow.style),
  };
}

function bridgeOverflowComparable(overflow: BridgeOverflowIndicatorNode): unknown {
  switch (overflow.kind) {
    case BRIDGE_OVERFLOW_KIND.none: return { kind: "none" };
    case BRIDGE_OVERFLOW_KIND.ellipsis: return { kind: "ellipsis", style: bridgeStyleComparable(overflow.style) };
    case BRIDGE_OVERFLOW_KIND.footer: return { kind: "footer", prefix: overflow.prefix, style: bridgeStyleComparable(overflow.style) };
  }
}

function semanticDecorationComparable(decoration: SemanticDecoration): unknown {
  return {
    padding: decoration.padding,
    background: decoration.background,
    foreground: decoration.foreground,
    border: decoration.border,
    style: semanticStyleComparable(decoration.style),
    styleStates: decoration.styleStates,
    width: decoration.width,
    height: decoration.height,
    minWidth: decoration.minWidth,
    maxWidth: decoration.maxWidth,
    minHeight: decoration.minHeight,
    maxHeight: decoration.maxHeight,
  };
}

function bridgeDecorationComparable(decoration: DecorationNode): unknown {
  return {
    padding: decoration.padding,
    background: decoration.background === undefined ? undefined : semanticColorFromBridge(decoration.background),
    foreground: decoration.foreground === undefined ? undefined : semanticColorFromBridge(decoration.foreground),
    border: decoration.border === undefined ? undefined : {
      glyphs: decoration.border.glyphs,
      style: decoration.border.style,
      edges: decoration.border.edges,
      color: decoration.border.color === undefined ? undefined : semanticColorFromBridge(decoration.border.color),
    },
    style: bridgeStyleComparable(decoration.style),
    styleStates: decoration.styleStates,
    width: decoration.width,
    height: decoration.height,
    minWidth: decoration.minWidth,
    maxWidth: decoration.maxWidth,
    minHeight: decoration.minHeight,
    maxHeight: decoration.maxHeight,
  };
}

function completeBorder() {
  return {
    glyphs: {
      top: "─",
      right: "│",
      bottom: "─",
      left: "│",
      topLeft: "┌",
      topRight: "┐",
      bottomLeft: "└",
      bottomRight: "┘",
    },
    style: "double" as const,
    edges: "topBottom" as const,
    color: themeColor("border"),
  };
}

function sampleBridgeNodes(): BridgeViewNode[] {
  const style = new StyleSpec()
    .foreground({ type: "rgb", r: 1, g: 2, b: 3 })
    .background({ type: "indexed", value: 17 })
    .bold()
    .underline();
  const styled = View.styledText([
    TextSpan.plain("plain"),
    TextSpan.styled("styled", StyleRef.theme("body", style)),
  ]).noWrap().textAlign("end");
  const diff = View.diff([
    new DiffHunk(new DiffRange(0, 2), new DiffRange(0, 2), [
      DiffLine.context(1, 1, "same", "crlf"),
      DiffLine.deletion(2, "old", "none"),
      DiffLine.addition(2, "new"),
    ]),
  ]);
  const axis = View.horizontal((builder) => {
    builder
      .child(View.text("normal"))
      .fixed(2, View.spacer(1))
      .flex(View.spacer(2))
      .flexMax(3, View.spacer(3))
      .contentMax(4, View.spacer(4))
      .gap(1);
  });
  const grid = View.grid((builder) => {
    builder
      .columns([
        { kind: "content" },
        { kind: "contentMax", max: 2 },
        { kind: "fixed", size: 3 },
        { kind: "flex" },
        { kind: "flexMax", max: 5 },
      ])
      .columnGap(1)
      .rowGap(2)
      .rowWith({ kind: "contentMax", max: 6 }, (row) => {
        row.cellWith({ columnSpan: 2, rowSpan: 2, horizontalAlign: "center", verticalAlign: "bottom" }, View.text("cell"));
        row.cell(View.spacer(1));
        row.cellWith({ horizontalAlign: "end", verticalAlign: "center" }, View.text("aligned"));
      });
  });
  const decorated = View.text("decorated")
    .padding(Insets.of(1, 2, 3, 4))
    .background(themeColor("panel"))
    .foreground({ type: "named", value: "cyan" })
    .border(completeBorder())
    .style(style)
    .styleState("mode", "active")
    .fillWidth()
    .maxHeight(5);
  const component = {
    id: 900_001,
    schema: VIEW_BRIDGE_SCHEMA_VERSION,
    kind: BRIDGE_VIEW_KIND.component,
    handle: 77 as ComponentId,
  } as BridgeViewNode;

  return [
    lowerColdView(View.text("text").noWrap().textAlign("center")),
    lowerColdView(View.text("grapheme").wrap("grapheme").textAlign("end")),
    lowerColdView(styled),
    lowerColdView(diff),
    lowerColdView(View.spacer(3)),
    lowerColdView(axis),
    lowerColdView(View.vertical([View.text("column")])),
    lowerColdView(View.hanging(View.text("> "), View.text("  "), View.text("body"))),
    lowerColdView(grid),
    lowerColdView(View.text("container").container()),
    lowerColdView(View.text("clamp").clampRows(2, { kind: "ellipsis", style })),
    lowerColdView(View.text("footer").clampRows(2, { kind: "footer", prefix: "more ", style })),
    lowerColdView(View.contentMax(4, View.text("content"))),
    component,
    lowerColdView(decorated),
  ];
}

describe("API-H3 H3-A semantic foundation", () => {
  test("semantic oracle covers every current View family and preserves all fields", () => {
    const samples = sampleBridgeNodes();
    const kinds = new Set(samples.map((bridge) => (semanticFromBridge(bridge).kind)));
    expect([...kinds].sort((a, b) => a - b)).toEqual([
      SEMANTIC_VIEW_KIND.text,
      SEMANTIC_VIEW_KIND.diff,
      SEMANTIC_VIEW_KIND.spacer,
      SEMANTIC_VIEW_KIND.row,
      SEMANTIC_VIEW_KIND.column,
      SEMANTIC_VIEW_KIND.grid,
      SEMANTIC_VIEW_KIND.hanging,
      SEMANTIC_VIEW_KIND.container,
      SEMANTIC_VIEW_KIND.clamp,
      SEMANTIC_VIEW_KIND.contentMax,
      SEMANTIC_VIEW_KIND.component,
      SEMANTIC_VIEW_KIND.decorated,
    ].sort((a, b) => a - b));

    for (const bridge of samples) {
      expect(semanticComparable(semanticFromBridge(bridge))).toEqual(bridgeComparable(bridge));
    }
  });

  test("semantic conversion preserves shared child identity and excludes bridge metadata", () => {
    const child = View.text("shared");
    const bridge = lowerColdView(View.horizontal([child, child]));
    const semantic = semanticFromBridge(bridge);
    if (semantic.kind !== SEMANTIC_VIEW_KIND.row) throw new Error("expected semantic row");

    expect(semantic.children[0]!.child).toBe(semantic.children[1]!.child);
    expect(semantic.children[0]!.child.id).toBe(lowerColdView(child).id);
    expect(semantic.id).toBe(bridge.id);
    expect(Object.isFrozen(semantic)).toBe(true);
    expect(Object.isFrozen(semantic.children)).toBe(true);
    expect("schema" in semantic).toBe(false);
    expect("handle" in semantic).toBe(false);

    const associated = View.text("associated");
    const associatedNode = semanticFromBridge(lowerColdView(associated));
    installSemanticNode(associated, associatedNode);
    expect(semanticNodeOf(associated)).toBe(associatedNode);
    expect(() => semanticNodeOf({} as View)).toThrow(/semantic value/);
  });

  test("semantic normalizers match the current bridge lowering for every public presentation form", () => {
    const colors = [
      themeColor("accent"),
      { type: "named", value: "magenta" as const },
      { type: "indexed", value: 200 },
      { type: "rgb", r: 12, g: 34, b: 56 },
    ] as const;
    for (const color of colors) {
      expect(semanticColorFor(color)).toEqual(semanticColorFromBridge(colorNodeFor(color)));
    }

    const styles = [
      new StyleSpec(),
      new StyleSpec().foreground(colors[0]!).background(colors[1]!).bold().italic(),
      StyleRef.theme("named", new StyleSpec().background(colors[2]!)),
      { attributes: { reversed: false, strikethrough: true } },
    ] as const;
    for (const style of styles) {
      expect(semanticStyleFor(style)).toEqual(semanticStyleFromBridge(styleNodeFor(style)));
    }

    const border = completeBorder();
    expect(semanticBorderFor(border)).toEqual(semanticBorderFromBridge(borderNodeFor(border)));

    const span = TextSpan.styled("span", StyleRef.theme("span", new StyleSpec().foreground(colors[3]!)));
    expect(semanticTextSpanFor(span)).toEqual({
      text: textSpanNodeFor(span).text,
      style: semanticStyleFromBridge(textSpanNodeFor(span).style!),
    });

    const overflowValues: readonly OverflowIndicator[] = [
      { kind: "none" },
      { kind: "ellipsis", style: styles[1]! },
      { kind: "footer", prefix: "more ", style: styles[2]! },
    ];
    for (const overflow of overflowValues) {
      const semantic = semanticOverflowFor(overflow);
      expect(semantic).toEqual(semanticOverflowFromBridge(
        overflow.kind === "none"
          ? { kind: BRIDGE_OVERFLOW_KIND.none }
          : overflow.kind === "ellipsis"
            ? { kind: BRIDGE_OVERFLOW_KIND.ellipsis, style: styleNodeFor(overflow.style) }
            : { kind: BRIDGE_OVERFLOW_KIND.footer, prefix: overflow.prefix, style: styleNodeFor(overflow.style) },
      ));
    }

    const decoration = semanticDecorationFor({
      padding: Insets.of(1, 2, 3, 4),
      background: colors[0],
      foreground: colors[1],
      border,
      style: styles[2],
      styleStates: { mode: "active" },
      width: "fill",
      height: "fit",
      minWidth: 1,
      maxWidth: 20,
      minHeight: 2,
      maxHeight: 30,
    });
    expect(decoration).toEqual({
      padding: { top: 1, right: 2, bottom: 3, left: 4 },
      background: semanticColorFromBridge(colorNodeFor(colors[0])),
      foreground: semanticColorFromBridge(colorNodeFor(colors[1])),
      border: semanticBorderFromBridge(borderNodeFor(border)),
      style: semanticStyleFromBridge(styleNodeFor(styles[2]!)),
      styleStates: { mode: "active" },
      width: "fill",
      height: "fit",
      minWidth: 1,
      maxWidth: 20,
      minHeight: 2,
      maxHeight: 30,
    });

    expect(() => semanticColorFor({ type: "indexed", value: 256 })).toThrow(/indexed ANSI color/);
    expect(() => semanticStyleFor({ attributes: { invalid: true } as never })).toThrow(/unknown text attribute/);
    expect(() => semanticBorderFor({ glyphs: { top: "─" } } as never)).toThrow(/border glyph/);
    expect(() => semanticDecorationFor({ padding: { top: 65_536, right: 0, bottom: 0, left: 0 } })).toThrow(/inset top/);
  });

  test("semantic normalizers snapshot caller values and freeze owned records", () => {
    const mutableRgb = { type: "rgb" as const, r: 10, g: 20, b: 30 };
    const attributes = { bold: true };
    const styleValue = { foreground: mutableRgb, attributes };
    const style = semanticStyleFor(styleValue);
    mutableRgb.r = 99;
    attributes.bold = false;
    expect(style.foreground).toEqual({ kind: "rgb", r: 10, g: 20, b: 30 });
    expect(style.attributes).toEqual({ bold: true });
    expect(Object.isFrozen(style)).toBe(true);
    expect(Object.isFrozen(style.foreground)).toBe(true);
    expect(Object.isFrozen(style.attributes)).toBe(true);

    const glyphs = {
      top: "─", right: "│", bottom: "─", left: "│",
      topLeft: "┌", topRight: "┐", bottomLeft: "└", bottomRight: "┘",
    };
    const states = { mode: "active" };
    const border = semanticBorderFor({ glyphs, color: mutableRgb });
    const decoration = semanticDecorationFor({ border: { glyphs, color: mutableRgb }, styleStates: states });
    glyphs.top = "x";
    states.mode = "changed";
    mutableRgb.g = 88;
    expect(border.glyphs?.top).toBe("─");
    expect(border.color).toEqual({ kind: "rgb", r: 99, g: 20, b: 30 });
    expect(decoration.border?.glyphs?.top).toBe("─");
    expect(decoration.styleStates).toEqual({ mode: "active" });
    expect(Object.isFrozen(border)).toBe(true);
    expect(Object.isFrozen(border.glyphs)).toBe(true);
    expect(Object.isFrozen(decoration)).toBe(true);
    expect(Object.isFrozen(decoration.border)).toBe(true);
    expect(Object.isFrozen(decoration.style)).toBe(true);
    expect(semanticEmptyStyle()).toEqual({ attributes: {} });
  });

  test("semantic derivations retain exact semantic facts and weak sidecars", () => {
    const textBase = createSemanticViewNode(1, {
      kind: SEMANTIC_VIEW_KIND.text,
      spans: Object.freeze([{ text: "base" }]),
      wrap: "wordThenGrapheme",
      align: "start",
    });
    const textDerived = createSemanticViewNode(2, {
      kind: SEMANTIC_VIEW_KIND.text,
      spans: Object.freeze([{ text: "base" }]),
      wrap: "noWrap",
      align: "end",
    });
    const decorated = createSemanticViewNode(3, {
      kind: SEMANTIC_VIEW_KIND.decorated,
      child: textDerived,
      decoration: semanticDecorationFor({ padding: 1 }),
    });
    const axis = createSemanticViewNode(4, {
      kind: SEMANTIC_VIEW_KIND.row,
      children: Object.freeze([]),
      gap: 0,
    });
    const grid = createSemanticViewNode(5, {
      kind: SEMANTIC_VIEW_KIND.grid,
      columns: Object.freeze([{ kind: "content" }]),
      rows: Object.freeze([]),
      columnGap: 0,
      rowGap: 0,
    });
    const child = createSemanticViewNode(6, {
      kind: SEMANTIC_VIEW_KIND.spacer,
      rows: 1,
    });
    const track: SemanticAxisTrack = { kind: "fixed", size: 2 };
    const derivations: SemanticDerivation[] = [
      { kind: "textLayout", base: textBase, wrap: "noWrap", align: "end" },
      { kind: "commonScalar", base: textBase, changes: { padding: { top: 1, right: 1, bottom: 1, left: 1 }, width: "fill" } },
      { kind: "axisSet", base: axis, index: 0, track, child },
      { kind: "axisSplice", base: axis, index: 0, removeCount: 1, inserted: [{ track: { kind: "normal" }, child }] },
      { kind: "gridCell", base: grid, row: 0, column: 0, child },
    ];
    const targets = [textDerived, decorated, axis, createSemanticViewNode(7, { ...axis }), createSemanticViewNode(8, { ...grid })];
    for (let index = 0; index < derivations.length; index += 1) {
      setSemanticDerivation(targets[index]!, derivations[index]!);
      expect(peekSemanticDerivation(targets[index]!)).toBe(derivations[index]);
      expect("trackWord" in derivations[index]!).toBe(false);
      expect("schema" in derivations[index]!).toBe(false);
    }
    expect(new Set(derivations.map((derivation) => derivation.kind))).toEqual(new Set([
      "textLayout", "commonScalar", "axisSet", "axisSplice", "gridCell",
    ]));
  });

  test("the semantic derivation oracle preserves every current retained fast-path fact", () => {
    const text = View.text("text").noWrap();
    const scalar = View.text("scalar").padding(1);
    const axisBase = View.horizontal([View.text("a"), View.text("b")]);
    const axisSet = axisSetChildForTransport(axisBase, 0, View.text("replacement"), { kind: "fixed", size: 4 });
    const axisSplice = axisSpliceForTransport(axisBase, 1, 1, [{ view: View.text("inserted"), track: { kind: "flexMax", maxRows: 3 } }]);
    const gridBase = View.grid([View.text("cell")]);
    const gridCell = gridSetCellForTransport(gridBase, 0, 0, View.text("new cell"));
    const semanticDerivations = [
      peekSemanticDerivation(semanticNodeOf(text)),
      peekSemanticDerivation(semanticNodeOf(scalar)),
      peekSemanticDerivation(semanticNodeOf(axisSet)),
      peekSemanticDerivation(semanticNodeOf(axisSplice)),
      peekSemanticDerivation(semanticNodeOf(gridCell)),
    ];
    if (semanticDerivations.some((derivation) => derivation === undefined)) {
      throw new Error("retained derivation fixture did not produce every derivation family");
    }
    const textDerivation = semanticDerivations[0] as Extract<SemanticDerivation, { kind: "textLayout" }>;
    const scalarDerivation = semanticDerivations[1] as Extract<SemanticDerivation, { kind: "commonScalar" }>;
    const axisSetDerivation = semanticDerivations[2] as Extract<SemanticDerivation, { kind: "axisSet" }>;
    const axisSpliceDerivation = semanticDerivations[3] as Extract<SemanticDerivation, { kind: "axisSplice" }>;
    const gridCellDerivation = semanticDerivations[4] as Extract<SemanticDerivation, { kind: "gridCell" }>;
    expect(semanticDerivationComparable(textDerivation)).toEqual({
      kind: "textLayout",
      base: textDerivation.base.id,
      wrap: "noWrap",
      align: "start",
    });
    expect(semanticDerivationComparable(scalarDerivation)).toEqual({
      kind: "commonScalar",
      base: scalarDerivation.base.id,
      changes: { padding: { top: 1, right: 1, bottom: 1, left: 1 } },
    });
    expect(semanticDerivationComparable(axisSetDerivation)).toEqual({
      kind: "axisSet",
      base: axisSetDerivation.base.id,
      index: 0,
      track: { kind: "fixed", size: 4 },
      child: axisSetDerivation.child.id,
    });
    expect(semanticDerivationComparable(axisSpliceDerivation)).toEqual({
      kind: "axisSplice",
      base: axisSpliceDerivation.base.id,
      index: 1,
      removeCount: 1,
      inserted: [{ track: { kind: "flexMax", maxRows: 3 }, child: axisSpliceDerivation.inserted[0]!.child.id }],
    });
    expect(semanticDerivationComparable(gridCellDerivation)).toEqual({
      kind: "gridCell",
      base: gridCellDerivation.base.id,
      row: 0,
      column: 0,
      child: gridCellDerivation.child.id,
    });
  });

  test("PersistentSeq satisfies the read-only semantic sequence contract without flattening at the boundary", () => {
    const sequence: SemanticSequence<number> = PersistentSeq.from([1, 2, 3]);
    expect(sequence.length).toBe(3);
    expect(sequence.get(1)).toBe(2);
    expect([...sequence.values()]).toEqual([1, 2, 3]);

    const child = createSemanticViewNode(10, { kind: SEMANTIC_VIEW_KIND.spacer, rows: 1 });
    const axis = createSemanticViewNode(11, { kind: SEMANTIC_VIEW_KIND.row, children: [], gap: 0 });
    const layout: SemanticLayoutChild = { kind: "normal", child };
    const axisSequence: SemanticSequence<SemanticLayoutChild> = PersistentSeq.from([layout]);
    setSemanticSequenceOverride(axis, { baseNode: axis, sequence: axisSequence });
    expect(peekSemanticSequenceOverride(axis)?.sequence.get(0)).toBe(layout);

    const grid = createSemanticViewNode(12, { kind: SEMANTIC_VIEW_KIND.grid, columns: [], rows: [], columnGap: 0, rowGap: 0 });
    setSemanticGridSequenceOverride(grid, {
      baseNode: grid,
      sequence: PersistentSeq.from([]),
      rowOffsets: [0],
      rowTracks: [],
      cellIndices: [],
    });
    expect(peekSemanticGridSequenceOverride(grid)?.rowOffsets).toEqual([0]);
  });

  test("semantic foundation source has no structural bridge or native-retention dependency", () => {
    const semanticNodeSource = readFileSync(new URL("../src/api/view/semantic-node.ts", import.meta.url), "utf8");
    const semanticStyleSource = readFileSync(new URL("../src/api/presentation/semantic-style.ts", import.meta.url), "utf8");
    const forbidden = /\b(?:BridgeViewNode|BRIDGE_VIEW_KIND|VIEW_BRIDGE_SCHEMA_VERSION|NativeRef|trackWord|pathRef|viewRefForNodeId)\b/u;
    expect(semanticNodeSource).not.toMatch(forbidden);
    expect(semanticStyleSource).not.toMatch(forbidden);
    expect(semanticNodeSource).not.toContain("../transport/");
    expect(semanticStyleSource).not.toContain("../transport/");
  });
});
