import kindCodes from "../abi/structural/schema/view-kind-codes.json";

type KindCodes = {
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

const schema = kindCodes as KindCodes;

/** Private numeric kind codes shared by the retained TS DAG and native constructors. */
export const NATIVE_VIEW_KIND = {
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

export const NATIVE_LAYOUT_CHILD_KIND = {
  normal: schema.layoutNormal,
  fixed: schema.layoutFixed,
  flex: schema.layoutFlex,
  flexMax: schema.layoutFlexMax,
  contentMax: schema.layoutContentMax,
} as const;

export const NATIVE_GRID_TRACK_KIND = {
  content: schema.trackContent,
  contentMax: schema.trackContentMax,
  fixed: schema.trackFixed,
  flex: schema.trackFlex,
  flexMax: schema.trackFlexMax,
} as const;

export const NATIVE_WRAP_MODE = {
  wordThenGrapheme: schema.wrapWordThenGrapheme,
  grapheme: schema.wrapGrapheme,
  noWrap: schema.wrapNoWrap,
} as const;

export const NATIVE_HORIZONTAL_ALIGN = {
  start: schema.horizontalStart,
  center: schema.horizontalCenter,
  end: schema.horizontalEnd,
} as const;

export const NATIVE_VERTICAL_ALIGN = {
  top: schema.verticalTop,
  center: schema.verticalCenter,
  bottom: schema.verticalBottom,
} as const;

export const NATIVE_DIFF_LINE_KIND = {
  context: schema.diffContext,
  addition: schema.diffAddition,
  deletion: schema.diffDeletion,
} as const;

export const NATIVE_DIFF_LINE_TERMINATION = {
  terminated: schema.terminationTerminated,
  unterminated: schema.terminationUnterminated,
} as const;

export type ColorNode = string | { readonly type: "ansi"; readonly value: number };

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

export interface BorderNode {
  readonly glyphs?: Readonly<Record<string, string>>;
  readonly style?: "plain" | "rounded" | "double";
  readonly edges?: "all" | "topBottom";
  readonly color?: ColorNode;
}
