import type { ComponentId } from "../../types.ts";
import bridgeSchema from "../abi/structural/schema/bridge-schema.json";
import { PersistentSeq } from "../../composition/persistent-seq.ts";

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
  | { readonly type: "component"; readonly handle: ComponentId }
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
  | { readonly kind: typeof BRIDGE_VIEW_KIND.component; readonly handle: ComponentId }
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

// --- PERF-12 T10 (§34/§28): wide retained edits --------------------------

/** One-child replacement on a retained row/column axis. */
export interface BridgeAxisSetDerivation {
  readonly kind: "axisSet";
  readonly base: BridgeViewNode;
  readonly index: number;
  /** 0 preserves the existing track; else the compact track word encoding. */
  readonly trackWord: number;
  readonly child: BridgeViewNode;
}

/** Insert/remove/splice on a retained row/column axis. */
export interface BridgeAxisSpliceDerivation {
  readonly kind: "axisSplice";
  readonly base: BridgeViewNode;
  readonly index: number;
  readonly removeCount: number;
  /** Only the INSERTED children cross the native boundary (§35); order matches the splice. */
  readonly inserted: readonly { readonly node: BridgeViewNode; readonly trackWord: number }[];
}

/** One-cell replacement on a retained grid. */
export interface BridgeGridCellDerivation {
  readonly kind: "gridCell";
  readonly base: BridgeViewNode;
  readonly row: number;
  readonly column: number;
  readonly child: BridgeViewNode;
}

export type BridgeDerivation =
  | BridgeTextLayoutDerivation
  | BridgeCommonScalarDerivation
  | BridgeAxisSetDerivation
  | BridgeAxisSpliceDerivation
  | BridgeGridCellDerivation;

/** Axis sequence edit descriptor carried alongside the authoritative seq. */
export type AxisSequenceEdit =
  | { readonly kind: "axisSet"; readonly index: number }
  | { readonly kind: "axisSplice"; readonly index: number; readonly removeCount: number; readonly insertedCount: number };

export interface BridgeSequenceOverride {
  readonly baseNode: BridgeViewNode;
  readonly sequence: PersistentSeq<BridgeLayoutChild>;
  /** Undefined for the initial wide-axis sequence seed. */
  readonly edit?: AxisSequenceEdit;
}

export interface BridgeGridSequenceOverride {
  readonly baseNode: BridgeViewNode;
  readonly sequence: PersistentSeq<BridgeGridCellNode>;
  /** Prefix cell offsets; length = row count + 1. */
  readonly rowOffsets: readonly number[];
  readonly rowTracks: readonly BridgeGridTrackNode[];
  /** Native grid coordinates to flat sequence indexes, per source row. */
  readonly cellIndices: readonly ReadonlyMap<number, number>[];
}


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

/** §34: sequence overrides die with their semantic node. */
const BRIDGE_SEQUENCE = new WeakMap<BridgeViewNode, BridgeSequenceOverride>();
const BRIDGE_GRID_SEQUENCE = new WeakMap<BridgeViewNode, BridgeGridSequenceOverride>();

export function setBridgeSequenceOverride(
  node: BridgeViewNode,
  override: BridgeSequenceOverride,
): void {
  BRIDGE_SEQUENCE.set(node, override);
}

export function peekBridgeSequenceOverride(node: BridgeViewNode): BridgeSequenceOverride | undefined {
  return BRIDGE_SEQUENCE.get(node);
}

export function setBridgeGridSequenceOverride(node: BridgeViewNode, override: BridgeGridSequenceOverride): void {
  BRIDGE_GRID_SEQUENCE.set(node, override);
}

export function peekBridgeGridSequenceOverride(node: BridgeViewNode): BridgeGridSequenceOverride | undefined {
  return BRIDGE_GRID_SEQUENCE.get(node);
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
