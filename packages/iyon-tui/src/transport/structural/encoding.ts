/**
 * Explicit semantic-to-structural encodings for the retained ABI.
 *
 * Semantic discriminants and values intentionally remain independent from the
 * bridge schema. This module is the only owner of the compact numeric forms
 * used by generated structural calls.
 */

import {
  BRIDGE_DIFF_LINE_KIND,
  BRIDGE_DIFF_LINE_TERMINATION,
  BRIDGE_GRID_TRACK_KIND,
  BRIDGE_HORIZONTAL_ALIGN,
  BRIDGE_OVERFLOW_KIND,
  BRIDGE_VIEW_KIND,
  BRIDGE_VERTICAL_ALIGN,
  BRIDGE_WRAP_MODE,
} from "./ir.ts";
import type {
  SemanticAxisTrack,
  SemanticColor,
  SemanticCommonScalarChanges,
  SemanticDiffLineKind,
  SemanticDiffLineTermination,
  SemanticGridTrack,
  SemanticHorizontalAlign,
  SemanticLayoutChild,
  SemanticOverflowIndicator,
  SemanticSizeMode,
  SemanticViewKind,
  SemanticVerticalAlign,
  SemanticWrapMode,
} from "../../api/view/semantic-node.ts";
import { SEMANTIC_VIEW_KIND } from "../../api/view/semantic-node.ts";

export function bridgeViewKind(kind: SemanticViewKind): number {
  switch (kind) {
    case SEMANTIC_VIEW_KIND.text: return BRIDGE_VIEW_KIND.text;
    case SEMANTIC_VIEW_KIND.diff: return BRIDGE_VIEW_KIND.diff;
    case SEMANTIC_VIEW_KIND.spacer: return BRIDGE_VIEW_KIND.spacer;
    case SEMANTIC_VIEW_KIND.row: return BRIDGE_VIEW_KIND.row;
    case SEMANTIC_VIEW_KIND.column: return BRIDGE_VIEW_KIND.column;
    case SEMANTIC_VIEW_KIND.grid: return BRIDGE_VIEW_KIND.grid;
    case SEMANTIC_VIEW_KIND.hanging: return BRIDGE_VIEW_KIND.hanging;
    case SEMANTIC_VIEW_KIND.container: return BRIDGE_VIEW_KIND.container;
    case SEMANTIC_VIEW_KIND.clamp: return BRIDGE_VIEW_KIND.clamp;
    case SEMANTIC_VIEW_KIND.contentMax: return BRIDGE_VIEW_KIND.contentMax;
    case SEMANTIC_VIEW_KIND.component: return BRIDGE_VIEW_KIND.component;
    case SEMANTIC_VIEW_KIND.decorated: return BRIDGE_VIEW_KIND.decorated;
  }
}

export function axisKind(kind: typeof SEMANTIC_VIEW_KIND.row | typeof SEMANTIC_VIEW_KIND.column): number {
  return kind === SEMANTIC_VIEW_KIND.row ? 1 : 2;
}

// Axis-create words intentionally use a different discriminant lane from the
// bridge layout-child records. Keep these physical ABI codes local to the
// structural encoder rather than reusing BRIDGE_LAYOUT_CHILD_KIND values.
const AXIS_TRACK_CONTENT_MAX = 2;
const AXIS_TRACK_FIXED = 3;
const AXIS_TRACK_FLEX = 4;
const AXIS_TRACK_FLEX_MAX = 5;

/** Encodes an eager axis child edge for row/column construction. */
export function layoutTrackWord(child: SemanticLayoutChild): number {
  switch (child.kind) {
    case "normal": return 0;
    case "fixed": return AXIS_TRACK_FIXED | (child.size << 8);
    // The ABI's axis-create family represents flex with a minimum of one.
    case "flex": return AXIS_TRACK_FLEX | (1 << 8);
    case "flexMax": return AXIS_TRACK_FLEX_MAX | (child.maxRows << 8);
    case "contentMax": return AXIS_TRACK_CONTENT_MAX | (child.maxRows << 8);
  }
}

/** Encodes the compact edit word; zero means preserve the existing track. */
export function axisTrackWord(track: SemanticAxisTrack | undefined): number {
  if (track === undefined || track.kind === "normal") return 0;
  switch (track.kind) {
    case "contentMax": return 2 | (track.maxRows << 8);
    case "fixed": return 3 | (track.size << 8);
    // The edit primitive defaults a flex minimum to one when the value lane is 0.
    case "flex": return 4;
    case "flexMax": return 5 | (track.maxRows << 8);
  }
}

export function gridTrackWord(track: SemanticGridTrack): number {
  switch (track.kind) {
    case "content": return BRIDGE_GRID_TRACK_KIND.content;
    case "contentMax": return BRIDGE_GRID_TRACK_KIND.contentMax | (track.max << 8);
    case "fixed": return BRIDGE_GRID_TRACK_KIND.fixed | (track.size << 8);
    case "flex": return BRIDGE_GRID_TRACK_KIND.flex;
    case "flexMax": return BRIDGE_GRID_TRACK_KIND.flexMax | (track.max << 8);
  }
}

export function wrapModeCode(mode: SemanticWrapMode): number {
  switch (mode) {
    case "wordThenGrapheme": return BRIDGE_WRAP_MODE.wordThenGrapheme;
    case "grapheme": return BRIDGE_WRAP_MODE.grapheme;
    case "noWrap": return BRIDGE_WRAP_MODE.noWrap;
  }
}

export function horizontalAlignCode(align: SemanticHorizontalAlign): number {
  switch (align) {
    case "start": return BRIDGE_HORIZONTAL_ALIGN.start;
    case "center": return BRIDGE_HORIZONTAL_ALIGN.center;
    case "end": return BRIDGE_HORIZONTAL_ALIGN.end;
  }
}

export function verticalAlignCode(align: SemanticVerticalAlign): number {
  switch (align) {
    case "top": return BRIDGE_VERTICAL_ALIGN.top;
    case "center": return BRIDGE_VERTICAL_ALIGN.center;
    case "bottom": return BRIDGE_VERTICAL_ALIGN.bottom;
  }
}

export function diffLineKindCode(kind: SemanticDiffLineKind): number {
  switch (kind) {
    case "context": return BRIDGE_DIFF_LINE_KIND.context;
    case "addition": return BRIDGE_DIFF_LINE_KIND.addition;
    case "deletion": return BRIDGE_DIFF_LINE_KIND.deletion;
  }
}

export function diffTerminationCode(termination: SemanticDiffLineTermination): number {
  return termination === "terminated"
    ? BRIDGE_DIFF_LINE_TERMINATION.terminated
    : BRIDGE_DIFF_LINE_TERMINATION.unterminated;
}

export function overflowKindCode(overflow: SemanticOverflowIndicator): number {
  switch (overflow.kind) {
    case "none": return BRIDGE_OVERFLOW_KIND.none;
    case "ellipsis": return BRIDGE_OVERFLOW_KIND.ellipsis;
    case "footer": return BRIDGE_OVERFLOW_KIND.footer;
  }
}

export function sizeModeCode(mode: SemanticSizeMode | undefined): number {
  if (mode === undefined) return 0;
  return mode === "fit" ? 1 : 2;
}

export interface CommonScalarEncoding {
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

export function commonScalarEncoding(changes: SemanticCommonScalarChanges): CommonScalarEncoding {
  return {
    mask: (changes.padding === undefined ? 0 : 4)
      | (changes.width === undefined ? 0 : 8)
      | (changes.height === undefined ? 0 : 16)
      | (changes.minWidth === undefined ? 0 : 32)
      | (changes.maxWidth === undefined ? 0 : 64)
      | (changes.minHeight === undefined ? 0 : 128)
      | (changes.maxHeight === undefined ? 0 : 256),
    paddingTopRight: changes.padding === undefined ? 0 : (changes.padding.top & 0xffff) | ((changes.padding.right & 0xffff) << 16),
    paddingBottomLeft: changes.padding === undefined ? 0 : (changes.padding.bottom & 0xffff) | ((changes.padding.left & 0xffff) << 16),
    widthRule: sizeModeCode(changes.width),
    heightRule: sizeModeCode(changes.height),
    minWidth: changes.minWidth ?? 0,
    maxWidth: changes.maxWidth ?? 0,
    minHeight: changes.minHeight ?? 0,
    maxHeight: changes.maxHeight ?? 0,
  };
}

export function colorAtomValue(color: SemanticColor): string {
  switch (color.kind) {
    case "theme": return `theme:${color.key}`;
    case "named": return color.value;
    case "indexed": return `ansi:${color.value}`;
    case "rgb": return `#${hex(color.r)}${hex(color.g)}${hex(color.b)}`;
  }
}

function hex(value: number): string {
  return value.toString(16).padStart(2, "0");
}
