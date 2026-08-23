import type { NativeHandleId } from "./types.ts";
import bridgeSchema from "./bridge-schema.json";

type BridgeSchema = {
  readonly schemaVersion: 1;
  readonly viewText: 1;
  readonly viewDiff: 2;
  readonly viewSpacer: 3;
  readonly viewRow: 4;
  readonly viewColumn: 5;
  readonly viewHanging: 6;
  readonly viewGrid: 7;
  readonly viewContainer: 8;
  readonly viewClamp: 9;
  readonly viewContentMax: 10;
  readonly viewComponent: 11;
  readonly viewDecorated: 12;
  readonly layoutNormal: 1;
  readonly layoutFixed: 2;
  readonly layoutFlex: 3;
  readonly layoutFlexMax: 4;
  readonly layoutContentMax: 5;
  readonly trackContent: 1;
  readonly trackContentMax: 2;
  readonly trackFixed: 3;
  readonly trackFlex: 4;
  readonly trackFlexMax: 5;
  readonly overflowNone: 1;
  readonly overflowEllipsis: 2;
  readonly overflowFooter: 3;
  readonly wrapWordThenGrapheme: 1;
  readonly wrapGrapheme: 2;
  readonly wrapNoWrap: 3;
  readonly horizontalStart: 1;
  readonly horizontalCenter: 2;
  readonly horizontalEnd: 3;
  readonly verticalTop: 1;
  readonly verticalCenter: 2;
  readonly verticalBottom: 3;
  readonly diffContext: 1;
  readonly diffAddition: 2;
  readonly diffDeletion: 3;
  readonly terminationTerminated: 1;
  readonly terminationUnterminated: 2;
  readonly packedMagic: number;
  readonly packedProtocolVersion: 1;
  readonly packedRef: 0;
  readonly packedDef: 1;
  readonly packedColorNone: 0;
  readonly packedColorString: 1;
  readonly packedColorAnsi: 2;
  readonly packedOverflowNone: 1;
  readonly packedOverflowEllipsis: 2;
  readonly packedOverflowFooter: 3;
  readonly packedRuleAbsent: 0;
  readonly packedRuleFit: 1;
  readonly packedRuleFill: 2;
  readonly packedBorderStyleAbsent: 0;
  readonly packedBorderStylePlain: 1;
  readonly packedBorderStyleRounded: 2;
  readonly packedBorderStyleDouble: 3;
  readonly packedBorderEdgesAbsent: 0;
  readonly packedBorderEdgesAll: 1;
  readonly packedBorderEdgesTopBottom: 2;
  readonly packedStyleTheme: 1;
  readonly packedStyleForeground: 2;
  readonly packedStyleBackground: 4;
  readonly packedDecorationPadding: 1;
  readonly packedDecorationBackground: 2;
  readonly packedDecorationForeground: 4;
  readonly packedDecorationBorder: 8;
  readonly packedDecorationStyle: 16;
  readonly packedDecorationStates: 32;
  readonly packedDecorationWidth: 64;
  readonly packedDecorationHeight: 128;
  readonly packedDecorationMinWidth: 256;
  readonly packedDecorationMaxWidth: 512;
  readonly packedDecorationMinHeight: 1024;
  readonly packedDecorationMaxHeight: 2048;
  readonly packedBorderGlyphs: 1;
  readonly packedBorderColor: 2;
  readonly packedBorderStyle: 4;
  readonly packedBorderEdges: 8;
  readonly packedV3ProtocolVersion: 2;
  readonly packedV3ResetGeneration: 1;
  readonly packedV3ColdClosure: 2;
  readonly packedV3HasByteLane: 4;
  readonly packedV3HasStringLane: 8;
  readonly packedV3DefViewFull: 1;
  readonly packedV3PatchView: 2;
  readonly packedV3DefSeqLeaf: 3;
  readonly packedV3DefSeqBranch: 4;
  readonly packedV3DefGridCellLeaf: 5;
  readonly packedV3DefGridCellBranch: 6;
  readonly packedV3OpRender: 10;
  readonly packedV3OpRenderForest: 11;
  readonly packedV3PatchText: 1;
  readonly packedV3PatchDecoration: 2;
  readonly packedV3PatchAxis: 3;
  readonly packedV3PatchGrid: 4;
  readonly packedV3PatchWrap: 1;
  readonly packedV3PatchAlign: 2;
  readonly packedV3PatchPadding: 4;
  readonly packedV3PatchWidth: 8;
  readonly packedV3PatchHeight: 16;
  readonly packedV3PatchMinWidth: 32;
  readonly packedV3PatchMaxWidth: 64;
  readonly packedV3PatchMinHeight: 128;
  readonly packedV3PatchMaxHeight: 256;
  readonly packedV3PatchGap: 512;
  readonly packedV3PatchSequence: 1024;
  readonly packedV3PatchGridCells: 2048;
  readonly packedV3SeqColumn: 1;
  readonly packedV3SeqRow: 2;
  readonly packedV3SeqGrid: 3;
  readonly packedV3WireLocalBit: 2147483648;
  readonly packedV3SeqBranchFactor: 32;
  readonly packedV3SeqPageShift: 12;
  readonly packedV4ProtocolVersion: 4;
  readonly packedV4ResetGeneration: 1;
  readonly packedV4ColdClosure: 2;
  readonly packedV4HasUtf8: 4;
  readonly packedV4DefViewFull: 1;
  readonly packedV4PatchView: 2;
  readonly packedV4DefSeqLeaf: 3;
  readonly packedV4DefSeqBranch: 4;
  readonly packedV4DefGridCellLeaf: 5;
  readonly packedV4DefGridCellBranch: 6;
  readonly packedV4OpRender: 10;
  readonly packedV4OpRenderForest: 11;
  readonly packedV4PatchText: 1;
  readonly packedV4PatchDecoration: 2;
  readonly packedV4PatchAxis: 3;
  readonly packedV4PatchGrid: 4;
  readonly packedV4PatchWrap: 1;
  readonly packedV4PatchAlign: 2;
  readonly packedV4PatchPadding: 4;
  readonly packedV4PatchWidth: 8;
  readonly packedV4PatchHeight: 16;
  readonly packedV4PatchMinWidth: 32;
  readonly packedV4PatchMaxWidth: 64;
  readonly packedV4PatchMinHeight: 128;
  readonly packedV4PatchMaxHeight: 256;
  readonly packedV4PatchGap: 512;
  readonly packedV4PatchSequence: 1024;
  readonly packedV4PatchGridCells: 2048;
  readonly packedV4SeqColumn: 1;
  readonly packedV4SeqRow: 2;
  readonly packedV4SeqGrid: 3;
  readonly packedV4WireLocalBit: 2147483648;
  readonly packedV4SeqBranchFactor: 32;
  readonly packedV4SeqPageShift: 12;
};

const schema = bridgeSchema as BridgeSchema;

/** Private semantic bridge schema shared by the retained TS DAG and native decoder. */
export const VIEW_BRIDGE_SCHEMA_VERSION = schema.schemaVersion;

export const BRIDGE_VIEW_KIND = {
  text: schema.viewText,
  diff: schema.viewDiff,
  spacer: schema.viewSpacer,
  row: schema.viewRow,
  column: schema.viewColumn,
  hanging: schema.viewHanging,
  grid: schema.viewGrid,
  container: schema.viewContainer,
  clamp: schema.viewClamp,
  contentMax: schema.viewContentMax,
  component: schema.viewComponent,
  decorated: schema.viewDecorated,
} as const;

export const BRIDGE_LAYOUT_CHILD_KIND = {
  normal: schema.layoutNormal,
  fixed: schema.layoutFixed,
  flex: schema.layoutFlex,
  flexMax: schema.layoutFlexMax,
  contentMax: schema.layoutContentMax,
} as const;

export const BRIDGE_GRID_TRACK_KIND = {
  content: schema.trackContent,
  contentMax: schema.trackContentMax,
  fixed: schema.trackFixed,
  flex: schema.trackFlex,
  flexMax: schema.trackFlexMax,
} as const;

export const BRIDGE_OVERFLOW_KIND = {
  none: schema.overflowNone,
  ellipsis: schema.overflowEllipsis,
  footer: schema.overflowFooter,
} as const;

export const BRIDGE_WRAP_MODE = {
  wordThenGrapheme: schema.wrapWordThenGrapheme,
  grapheme: schema.wrapGrapheme,
  noWrap: schema.wrapNoWrap,
} as const;

export const BRIDGE_HORIZONTAL_ALIGN = {
  start: schema.horizontalStart,
  center: schema.horizontalCenter,
  end: schema.horizontalEnd,
} as const;

export const BRIDGE_VERTICAL_ALIGN = {
  top: schema.verticalTop,
  center: schema.verticalCenter,
  bottom: schema.verticalBottom,
} as const;

export const BRIDGE_DIFF_LINE_KIND = {
  context: schema.diffContext,
  addition: schema.diffAddition,
  deletion: schema.diffDeletion,
} as const;

export const BRIDGE_DIFF_LINE_TERMINATION = {
  terminated: schema.terminationTerminated,
  unterminated: schema.terminationUnterminated,
} as const;

/** Canonical constants for the benchmark-only packed View transaction. */
export const PACKED_VIEW = {
  magic: schema.packedMagic,
  version: schema.packedProtocolVersion,
  ref: schema.packedRef,
  def: schema.packedDef,
  colorNone: schema.packedColorNone,
  colorString: schema.packedColorString,
  colorAnsi: schema.packedColorAnsi,
  overflowNone: schema.packedOverflowNone,
  overflowEllipsis: schema.packedOverflowEllipsis,
  overflowFooter: schema.packedOverflowFooter,
  ruleAbsent: schema.packedRuleAbsent,
  ruleFit: schema.packedRuleFit,
  ruleFill: schema.packedRuleFill,
  borderStyleAbsent: schema.packedBorderStyleAbsent,
  borderStylePlain: schema.packedBorderStylePlain,
  borderStyleRounded: schema.packedBorderStyleRounded,
  borderStyleDouble: schema.packedBorderStyleDouble,
  borderEdgesAbsent: schema.packedBorderEdgesAbsent,
  borderEdgesAll: schema.packedBorderEdgesAll,
  borderEdgesTopBottom: schema.packedBorderEdgesTopBottom,
  styleTheme: schema.packedStyleTheme,
  styleForeground: schema.packedStyleForeground,
  styleBackground: schema.packedStyleBackground,
  decorationPadding: schema.packedDecorationPadding,
  decorationBackground: schema.packedDecorationBackground,
  decorationForeground: schema.packedDecorationForeground,
  decorationBorder: schema.packedDecorationBorder,
  decorationStyle: schema.packedDecorationStyle,
  decorationStates: schema.packedDecorationStates,
  decorationWidth: schema.packedDecorationWidth,
  decorationHeight: schema.packedDecorationHeight,
  decorationMinWidth: schema.packedDecorationMinWidth,
  decorationMaxWidth: schema.packedDecorationMaxWidth,
  decorationMinHeight: schema.packedDecorationMinHeight,
  decorationMaxHeight: schema.packedDecorationMaxHeight,
  borderGlyphs: schema.packedBorderGlyphs,
  borderColor: schema.packedBorderColor,
  borderStyle: schema.packedBorderStyle,
  borderEdges: schema.packedBorderEdges,
} as const;

/** Packed V3 retained graph protocol constants. */
export const PACKED_V3 = {
  version: schema.packedV3ProtocolVersion,
  resetGeneration: schema.packedV3ResetGeneration,
  coldClosure: schema.packedV3ColdClosure,
  hasByteLane: schema.packedV3HasByteLane,
  hasStringLane: schema.packedV3HasStringLane,
  defViewFull: schema.packedV3DefViewFull,
  patchView: schema.packedV3PatchView,
  defSeqLeaf: schema.packedV3DefSeqLeaf,
  defSeqBranch: schema.packedV3DefSeqBranch,
  defGridCellLeaf: schema.packedV3DefGridCellLeaf,
  defGridCellBranch: schema.packedV3DefGridCellBranch,
  opRender: schema.packedV3OpRender,
  opRenderForest: schema.packedV3OpRenderForest,
  patchText: schema.packedV3PatchText,
  patchDecoration: schema.packedV3PatchDecoration,
  patchAxis: schema.packedV3PatchAxis,
  patchGrid: schema.packedV3PatchGrid,
  patchWrap: schema.packedV3PatchWrap,
  patchAlign: schema.packedV3PatchAlign,
  patchPadding: schema.packedV3PatchPadding,
  patchWidth: schema.packedV3PatchWidth,
  patchHeight: schema.packedV3PatchHeight,
  patchMinWidth: schema.packedV3PatchMinWidth,
  patchMaxWidth: schema.packedV3PatchMaxWidth,
  patchMinHeight: schema.packedV3PatchMinHeight,
  patchMaxHeight: schema.packedV3PatchMaxHeight,
  patchGap: schema.packedV3PatchGap,
  patchSequence: schema.packedV3PatchSequence,
  patchGridCells: schema.packedV3PatchGridCells,
  seqColumn: schema.packedV3SeqColumn,
  seqRow: schema.packedV3SeqRow,
  seqGrid: schema.packedV3SeqGrid,
  wireLocalBit: schema.packedV3WireLocalBit,
  seqBranchFactor: schema.packedV3SeqBranchFactor,
  seqPageShift: schema.packedV3SeqPageShift,
} as const;

/** Packed V4 dual-lane retained graph protocol constants. */
export const PACKED_V4 = {
  version: schema.packedV4ProtocolVersion,
  resetGeneration: schema.packedV4ResetGeneration,
  coldClosure: schema.packedV4ColdClosure,
  hasUtf8: schema.packedV4HasUtf8,
  defViewFull: schema.packedV4DefViewFull,
  patchView: schema.packedV4PatchView,
  defSeqLeaf: schema.packedV4DefSeqLeaf,
  defSeqBranch: schema.packedV4DefSeqBranch,
  defGridCellLeaf: schema.packedV4DefGridCellLeaf,
  defGridCellBranch: schema.packedV4DefGridCellBranch,
  opRender: schema.packedV4OpRender,
  opRenderForest: schema.packedV4OpRenderForest,
  patchText: schema.packedV4PatchText,
  patchDecoration: schema.packedV4PatchDecoration,
  patchAxis: schema.packedV4PatchAxis,
  patchGrid: schema.packedV4PatchGrid,
  patchWrap: schema.packedV4PatchWrap,
  patchAlign: schema.packedV4PatchAlign,
  patchPadding: schema.packedV4PatchPadding,
  patchWidth: schema.packedV4PatchWidth,
  patchHeight: schema.packedV4PatchHeight,
  patchMinWidth: schema.packedV4PatchMinWidth,
  patchMaxWidth: schema.packedV4PatchMaxWidth,
  patchMinHeight: schema.packedV4PatchMinHeight,
  patchMaxHeight: schema.packedV4PatchMaxHeight,
  patchGap: schema.packedV4PatchGap,
  patchSequence: schema.packedV4PatchSequence,
  patchGridCells: schema.packedV4PatchGridCells,
  seqColumn: schema.packedV4SeqColumn,
  seqRow: schema.packedV4SeqRow,
  seqGrid: schema.packedV4SeqGrid,
  wireLocalBit: schema.packedV4WireLocalBit,
  seqBranchFactor: schema.packedV4SeqBranchFactor,
  seqPageShift: schema.packedV4SeqPageShift,
} as const;

export type ColorNode = string | { readonly type: "ansi"; readonly value: number };

export interface InsetsNode {
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
  readonly left: number;
}

export interface StyleNode {
  readonly theme?: string;
  readonly foreground?: ColorNode;
  readonly background?: ColorNode;
  readonly attributes: Readonly<Record<string, boolean>>;
}

export type OverflowIndicatorNode =
  | { readonly kind: "none" }
  | { readonly kind: "ellipsis"; readonly style: StyleNode }
  | { readonly kind: "footer"; readonly prefix: string; readonly style: StyleNode };

export type ViewNode =
  | { readonly type: "text"; readonly spans: readonly TextSpanNode[]; readonly wrap: string; readonly align: string }
  | { readonly type: "diff"; readonly hunks: readonly DiffHunkNode[] }
  | { readonly type: "spacer"; readonly rows: number }
  | { readonly type: "row" | "column"; readonly children: readonly LayoutChild[]; readonly gap: number }
  | { readonly type: "hanging"; readonly prefix: ViewNode; readonly continuation: ViewNode; readonly body: ViewNode }
  | { readonly type: "grid"; readonly columns: readonly GridTrackNode[]; readonly rows: readonly GridRowNode[]; readonly columnGap: number; readonly rowGap: number }
  | { readonly type: "container" | "clamp"; readonly child: ViewNode; readonly maxRows?: number; readonly overflow?: OverflowIndicatorNode }
  | { readonly type: "contentMax"; readonly child: ViewNode; readonly maxRows: number }
  | { readonly type: "component"; readonly handle: NativeHandleId }
  | { readonly type: "decorated"; readonly child: ViewNode; readonly decoration: DecorationNode };

export interface TextSpanNode {
  readonly text: string;
  readonly style?: StyleNode;
}

export interface DiffRangeNode {
  readonly start: number;
  readonly count: number;
}

export interface DiffLineNode {
  readonly kind: "context" | "addition" | "deletion";
  readonly text: string;
  readonly termination: "terminated" | "unterminated";
  readonly oldLine?: number;
  readonly newLine?: number;
}

export interface DiffHunkNode {
  readonly oldRange: DiffRangeNode;
  readonly newRange: DiffRangeNode;
  readonly lines: readonly DiffLineNode[];
}

export type LayoutChild =
  | { readonly kind: "normal"; readonly child: ViewNode }
  | { readonly kind: "fixed"; readonly size: number; readonly child: ViewNode }
  | { readonly kind: "flex"; readonly child: ViewNode }
  | { readonly kind: "flexMax"; readonly maxRows: number; readonly child: ViewNode }
  | { readonly kind: "contentMax"; readonly maxRows: number; readonly child: ViewNode };

export type GridTrackNode =
  | { readonly kind: "content" }
  | { readonly kind: "contentMax"; readonly max: number }
  | { readonly kind: "fixed"; readonly size: number }
  | { readonly kind: "flex" }
  | { readonly kind: "flexMax"; readonly max: number };

export interface GridCellNode {
  readonly view: ViewNode;
  readonly columnSpan: number;
  readonly rowSpan: number;
  readonly horizontalAlign: "start" | "center" | "end";
  readonly verticalAlign: "top" | "center" | "bottom";
}

export interface GridRowNode {
  readonly track: GridTrackNode;
  readonly cells: readonly GridCellNode[];
}

export interface DecorationNode {
  readonly padding?: InsetsNode;
  readonly background?: ColorNode;
  readonly foreground?: ColorNode;
  readonly border?: BorderNode;
  readonly style: StyleNode;
  readonly styleStates?: Readonly<Record<string, string>>;
  readonly width?: "fit" | "fill";
  readonly height?: "fit" | "fill";
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}

export interface BorderNode {
  readonly glyphs?: Readonly<Record<string, string>>;
  readonly style?: "plain" | "rounded" | "double";
  readonly edges?: "all" | "topBottom";
  readonly color?: ColorNode;
}

/** Internal numeric representation. It is never exported from the public TUI entrypoint. */
export type BridgeDiffLineNode = {
  readonly kind: (typeof BRIDGE_DIFF_LINE_KIND)[keyof typeof BRIDGE_DIFF_LINE_KIND];
  readonly text: string;
  readonly termination: (typeof BRIDGE_DIFF_LINE_TERMINATION)[keyof typeof BRIDGE_DIFF_LINE_TERMINATION];
  readonly oldLine?: number;
  readonly newLine?: number;
};

export interface BridgeDiffHunkNode {
  readonly oldRange: DiffRangeNode;
  readonly newRange: DiffRangeNode;
  readonly lines: readonly BridgeDiffLineNode[];
}

export type BridgeOverflowIndicatorNode =
  | { readonly kind: typeof BRIDGE_OVERFLOW_KIND.none }
  | { readonly kind: typeof BRIDGE_OVERFLOW_KIND.ellipsis; readonly style: StyleNode }
  | { readonly kind: typeof BRIDGE_OVERFLOW_KIND.footer; readonly prefix: string; readonly style: StyleNode };

export type BridgeLayoutChild =
  | { readonly kind: typeof BRIDGE_LAYOUT_CHILD_KIND.normal; readonly child: BridgeViewNode }
  | { readonly kind: typeof BRIDGE_LAYOUT_CHILD_KIND.fixed; readonly size: number; readonly child: BridgeViewNode }
  | { readonly kind: typeof BRIDGE_LAYOUT_CHILD_KIND.flex; readonly child: BridgeViewNode }
  | { readonly kind: typeof BRIDGE_LAYOUT_CHILD_KIND.flexMax; readonly maxRows: number; readonly child: BridgeViewNode }
  | { readonly kind: typeof BRIDGE_LAYOUT_CHILD_KIND.contentMax; readonly maxRows: number; readonly child: BridgeViewNode };

export type BridgeGridTrackNode =
  | { readonly kind: typeof BRIDGE_GRID_TRACK_KIND.content }
  | { readonly kind: typeof BRIDGE_GRID_TRACK_KIND.contentMax; readonly max: number }
  | { readonly kind: typeof BRIDGE_GRID_TRACK_KIND.fixed; readonly size: number }
  | { readonly kind: typeof BRIDGE_GRID_TRACK_KIND.flex }
  | { readonly kind: typeof BRIDGE_GRID_TRACK_KIND.flexMax; readonly max: number };

export interface BridgeGridCellNode {
  readonly view: BridgeViewNode;
  readonly columnSpan: number;
  readonly rowSpan: number;
  readonly horizontalAlign: number;
  readonly verticalAlign: number;
}

export interface BridgeGridRowNode {
  readonly track: BridgeGridTrackNode;
  readonly cells: readonly BridgeGridCellNode[];
}

type BridgeViewNodeData =
  | { readonly kind: typeof BRIDGE_VIEW_KIND.text; readonly spans: readonly TextSpanNode[]; readonly wrap: number; readonly align: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.diff; readonly hunks: readonly BridgeDiffHunkNode[] }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.spacer; readonly rows: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.row | typeof BRIDGE_VIEW_KIND.column; readonly children: readonly BridgeLayoutChild[]; readonly gap: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.hanging; readonly prefix: BridgeViewNode; readonly continuation: BridgeViewNode; readonly body: BridgeViewNode }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.grid; readonly columns: readonly BridgeGridTrackNode[]; readonly rows: readonly BridgeGridRowNode[]; readonly columnGap: number; readonly rowGap: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.container | typeof BRIDGE_VIEW_KIND.clamp; readonly child: BridgeViewNode; readonly maxRows?: number; readonly overflow?: BridgeOverflowIndicatorNode }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.contentMax; readonly child: BridgeViewNode; readonly maxRows: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.component; readonly handle: NativeHandleId }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.decorated; readonly child: BridgeViewNode; readonly decoration: DecorationNode };

export type BridgeViewNodeDraft = BridgeViewNodeData;
export type BridgeViewNode = BridgeViewNodeData & {
  readonly id: number;
  readonly schema: typeof VIEW_BRIDGE_SCHEMA_VERSION;
};

export function emptyStyle(): StyleNode {
  return { attributes: {} };
}

export function emptyDecoration(): DecorationNode {
  return { style: emptyStyle() };
}

// ---------------------------------------------------------------------------
// PERF-12 T9 (§15/§27): derivation hints.
//
// The semantic DAG is authoritative; a derivation records how a new immutable
// node was derived from an old one so the retained path can use an exact
// native clone/edit primitive instead of re-materializing from fields. It is
// an optimization hint, never a second representation: `ensureNative` uses it
// only when the base carries a same-generation NativeRef and an exact native
// retained primitive exists; otherwise it is ignored and the node
// materializes from its semantic fields (§27/§38).

export interface BridgeTextLayoutDerivation {
  readonly kind: "textLayout";
  /** The complete prior semantic node whose retained text payload is reused. */
  readonly base: BridgeViewNode;
  /** Final BRIDGE_WRAP_MODE code for the derived text node. */
  readonly wrap: number;
  /** Final BRIDGE_HORIZONTAL_ALIGN code for the derived text node. */
  readonly align: number;
}

/**
 * Scalar-only decorated node derivable through the retained common patch:
 * the derived node renders identically to the native `base` view with the
 * masked modifiers applied (padding/width/height/min/max only — any color,
 * border, style, or styleState content makes the hint inexpressible and it
 * is not attached).
 */
export interface BridgeCommonScalarDerivation {
  readonly kind: "commonScalar";
  readonly base: BridgeViewNode;
  /** view_common_patch_root mask: exactly the modifiers this derivation carries. */
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

export type BridgeDerivation = BridgeTextLayoutDerivation | BridgeCommonScalarDerivation;

/** Derivation hints die with their semantic node (§15). */
const BRIDGE_DERIVATION = new WeakMap<BridgeViewNode, BridgeDerivation>();

/** Attaches a derivation hint to a freshly constructed semantic node. */
export function setBridgeDerivation(node: BridgeViewNode, derivation: BridgeDerivation): void {
  BRIDGE_DERIVATION.set(node, derivation);
}

/** Retained-path read access (ensureNative's tryDerivation step). */
export function peekBridgeDerivation(node: BridgeViewNode): BridgeDerivation | undefined {
  return BRIDGE_DERIVATION.get(node);
}

function cloneColor(color: ColorNode | undefined): ColorNode | undefined {
  return color !== undefined && typeof color === "object" ? { ...color } : color;
}

export function cloneStyle(style: StyleNode): StyleNode {
  return {
    ...style,
    foreground: cloneColor(style.foreground),
    background: cloneColor(style.background),
    attributes: { ...style.attributes },
  };
}

export function cloneDecoration(decoration: DecorationNode): DecorationNode {
  return {
    ...decoration,
    padding: decoration.padding === undefined ? undefined : { ...decoration.padding },
    background: cloneColor(decoration.background),
    foreground: cloneColor(decoration.foreground),
    style: cloneStyle(decoration.style),
    border: decoration.border === undefined ? undefined : {
      ...decoration.border,
      color: cloneColor(decoration.border.color),
      glyphs: decoration.border.glyphs && { ...decoration.border.glyphs },
    },
  };
}

export function mergeStyles(left: StyleNode, right: StyleNode): StyleNode {
  return {
    theme: right.theme ?? left.theme,
    foreground: cloneColor(right.foreground ?? left.foreground),
    background: cloneColor(right.background ?? left.background),
    attributes: { ...left.attributes, ...right.attributes },
  };
}
