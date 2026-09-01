import type { ComponentId } from "../../api/extensions/traits/component.ts";
import bridgeSchema from "../abi/structural/schema/bridge-schema.json";

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
  readonly viewContentHost: 13;
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
  contentHost: schema.viewContentHost,
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

export interface TextSpanNode {
  readonly text: string;
  readonly style?: StyleNode;
}

export interface DiffRangeNode {
  readonly start: number;
  readonly count: number;
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

type BridgeViewNodeData = (
  | { readonly kind: typeof BRIDGE_VIEW_KIND.text; readonly spans: readonly TextSpanNode[]; readonly wrap: number; readonly align: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.diff; readonly hunks: readonly BridgeDiffHunkNode[] }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.spacer; readonly rows: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.row | typeof BRIDGE_VIEW_KIND.column; readonly children: readonly BridgeLayoutChild[]; readonly gap: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.hanging; readonly prefix: BridgeViewNode; readonly continuation: BridgeViewNode; readonly body: BridgeViewNode }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.grid; readonly columns: readonly BridgeGridTrackNode[]; readonly rows: readonly BridgeGridRowNode[]; readonly columnGap: number; readonly rowGap: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.container | typeof BRIDGE_VIEW_KIND.clamp; readonly child: BridgeViewNode; readonly maxRows?: number; readonly overflow?: BridgeOverflowIndicatorNode }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.contentMax; readonly child: BridgeViewNode; readonly maxRows: number }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.component; readonly handle: ComponentId }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.decorated; readonly child: BridgeViewNode; readonly decoration: DecorationNode }
  | { readonly kind: typeof BRIDGE_VIEW_KIND.contentHost; readonly contentPortId: number }
) & {
  /** Native state identity in the derived cold transport only. */
  readonly stateAttachment?: number;
};

export type BridgeViewNodeDraft = BridgeViewNodeData;
export type BridgeViewNode = BridgeViewNodeData & {
  readonly id: number;
  readonly schema: typeof VIEW_BRIDGE_SCHEMA_VERSION;
};
