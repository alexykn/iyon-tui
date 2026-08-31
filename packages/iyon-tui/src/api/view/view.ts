/**
 * Immutable semantic View values.
 *
 * The View layer owns semantic identity, child relationships, normalized
 * presentation values, and retained derivation hints. Structural transport is
 * deliberately absent here; the retained path consumes these semantic values
 * directly and the cold path lowers them only at the physical boundary.
 */

import type { BorderSpec, TextAttribute } from "../presentation/style.ts";
import { StyleSpec, validateTextAttribute } from "../presentation/style.ts";
import type { StyleRef, StyleStateKey, StyleStateValue } from "../presentation/style.ts";
import type { HandleId } from "../controls/framework-handle.ts";
import type { ViewState } from "./retained-state.ts";
import type { ColorSpec } from "../presentation/theme.ts";
import { insets, Insets } from "./geometry.ts";
import type { DiffHunk } from "../content/diff.ts";
import { TextSpan } from "../content/text.ts";
import {
  semanticBorderFor,
  semanticCloneDecoration,
  semanticColorFor,
  semanticCloneStyle,
  semanticDecorationFor,
  semanticEmptyStyle,
  semanticMergeStyles,
  semanticOverflowFor,
  semanticStyleFor,
  semanticTextSpanFor,
} from "../presentation/semantic-style.ts";
import {
  createSemanticViewNode,
  installSemanticNode,
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  SEMANTIC_VIEW_KIND,
  semanticNodeOf,
  retainSemanticAttachmentReference,
  semanticNodeHasAttachments,
  setSemanticAttachmentPresence,
  setSemanticDerivation,
  setSemanticGridSequenceOverride,
  setSemanticSequenceOverride,
  type SemanticAxisSequenceEdit,
  type SemanticAxisTrack,
  type SemanticCommonScalarChanges,
  type SemanticDecoration,
  type SemanticDerivation,
  type SemanticDiffHunk,
  type SemanticDiffLine,
  type SemanticGridCell,
  type SemanticGridRow,
  type SemanticGridTrack,
  type SemanticHorizontalAlign,
  type SemanticVerticalAlign,
  type SemanticLayoutChild,
  type SemanticOverflowIndicator,
  type SemanticTextNode,
  type SemanticViewNode,
  type SemanticViewNodeDraft,
  type SemanticWrapMode,
} from "./semantic-node.ts";
import { PersistentSeq } from "../../composition/persistent-seq.ts";
import {
  activeChildOwnerOrThrow,
  executionContext,
  semanticConstruction,
  withKeyedChildOwner,
} from "../../composition/execution-context.ts";
import {
  composeBackground,
  composeBorder,
  composeClampRows,
  composeComponent,
  composeContainer,
  composeContentMax,
  composeDiff,
  composeFillHeight,
  composeFillWidth,
  composeFitHeight,
  composeFitWidth,
  composeForeground,
  composeGrid,
  composeHanging,
  composeHorizontal,
  composeMaxHeight,
  composeMaxWidth,
  composeMinHeight,
  composeMinWidth,
  composePadding,
  composeSpacer,
  composeStyle,
  composeStyleState,
  composeState,
  composeStyledText,
  composeText,
  composeTextAlign,
  composeTextAttribute,
  composeVertical,
  composeWrap,
} from "../../composition/compose.ts";

export type HorizontalAlign = "start" | "center" | "end";
export type VerticalAlign = "top" | "center" | "bottom";
export type WrapMode = "wordThenGrapheme" | "grapheme" | "noWrap";

export type LayoutChild =
  | { readonly kind: "normal"; readonly child: View }
  | { readonly kind: "fixed"; readonly size: number; readonly child: View }
  | { readonly kind: "flex"; readonly child: View }
  | { readonly kind: "flexMax"; readonly maxRows: number; readonly child: View }
  | { readonly kind: "contentMax"; readonly maxRows: number; readonly child: View };

export type GridTrack =
  | { readonly kind: "content" }
  | { readonly kind: "contentMax"; readonly max: number }
  | { readonly kind: "fixed"; readonly size: number }
  | { readonly kind: "flex" }
  | { readonly kind: "flexMax"; readonly max: number };

export interface GridCell {
  readonly view: View;
  readonly columnSpan?: number;
  readonly rowSpan?: number;
  readonly horizontalAlign?: HorizontalAlign;
  readonly verticalAlign?: VerticalAlign;
}

export interface GridRow {
  readonly track?: GridTrack;
  readonly cells: readonly GridCell[];
}

export interface GridSpec {
  readonly columns?: readonly GridTrack[];
  readonly rows: readonly GridRow[];
  readonly columnGap?: number;
  readonly rowGap?: number;
}

export type ViewChildren = readonly View[] | ((builder: ChildrenBuilder) => void);
type CounterBox = { next: number };
const NODE_ID_COUNTER = Symbol.for("iyon:tui:private-view-node-counter");
const globalRoot = globalThis as typeof globalThis & { [NODE_ID_COUNTER]?: CounterBox };
const nodeIdCounter = globalRoot[NODE_ID_COUNTER] ??= { next: 1 };
const WIDE_AXIS_SEQUENCE_THRESHOLD = 1_024;
const WIDE_GRID_SEQUENCE_THRESHOLD = 1_024;

function isRetainedConstruction(): boolean {
  return executionContext.top !== undefined && !semanticConstruction.raw;
}

function nextNodeId(): number {
  if (nodeIdCounter.next > Number.MAX_SAFE_INTEGER) throw new Error("TUI View node identity exhausted");
  return nodeIdCounter.next++;
}

export type OverflowIndicator =
  | { readonly kind: "none" }
  | { readonly kind: "ellipsis"; readonly style: StyleRef | StyleSpec }
  | { readonly kind: "footer"; readonly prefix: string; readonly style: StyleRef | StyleSpec };

export class GridRowBuilder {
  readonly cells: GridCell[] = [];
  cell(view: View): this { this.cells.push({ view }); return this; }
  cellWith(spec: Omit<GridCell, "view">, view: View): this { this.cells.push({ ...spec, view }); return this; }
}

export class GridBuilder {
  columnsValue: GridTrack[] = [];
  rows: GridRow[] = [];
  columnGapValue = 0;
  rowGapValue = 0;
  columns(columns: readonly GridTrack[]): this { this.columnsValue = [...columns]; return this; }
  columnGap(value: number): this { this.columnGapValue = validateU16(value, "columnGap"); return this; }
  rowGap(value: number): this { this.rowGapValue = validateU16(value, "rowGap"); return this; }
  row(build: ((row: GridRowBuilder) => void) | GridRow): this {
    if (typeof build === "function") {
      const row = new GridRowBuilder();
      build(row);
      this.rows.push({ cells: row.cells });
    } else this.rows.push(build);
    return this;
  }
  rowWith(track: GridTrack, build: (row: GridRowBuilder) => void): this {
    const row = new GridRowBuilder();
    build(row);
    this.rows.push({ track, cells: row.cells });
    return this;
  }
}

export class ChildrenBuilder {
  private readonly entries: LayoutChild[] = [];
  private layoutGap = 0;
  get children(): LayoutChild[] { return this.entries; }
  child(view: View): this { this.entries.push({ kind: "normal", child: view }); return this; }
  childrenOf(views: readonly View[]): this { for (const view of views) this.child(view); return this; }
  gap(value: number): this { this.layoutGap = validateU16(value, "gap"); return this; }
  fixed(size: number, view: View): this {
    this.entries.push({ kind: "fixed", size: validateU16(size, "size"), child: view });
    return this;
  }
  flex(view: View): this { this.entries.push({ kind: "flex", child: view }); return this; }
  flexMax(maxRows: number, view: View): this {
    validateU16(maxRows, "maxRows");
    this.entries.push({ kind: "flexMax", maxRows, child: view });
    return this;
  }
  contentMax(maxRows: number, view: View): this {
    validateU16(maxRows, "maxRows");
    this.entries.push({ kind: "contentMax", maxRows, child: view });
    return this;
  }
  gapValue(): number { return this.layoutGap; }
}

function semanticDraftOf(node: SemanticViewNode): SemanticViewNodeDraft {
  const { id: _id, ...draft } = node;
  return draft as SemanticViewNodeDraft;
}

function withSemanticIdentity(node: SemanticViewNode | SemanticViewNodeDraft): SemanticViewNode {
  return createSemanticViewNode(
    nextNodeId(),
    "id" in node ? semanticDraftOf(node) : node,
  );
}

/** Applies a same-kind semantic replacement after the caller has narrowed it. */
function withSemanticUpdate(node: SemanticViewNode, update: object): SemanticViewNode {
  return withSemanticIdentity({ ...semanticDraftOf(node), ...update } as SemanticViewNodeDraft);
}

/** @internal Creates a new semantic node with a fresh View identity. */
export function updateSemanticViewNode(node: SemanticViewNode, update: object): SemanticViewNode {
  return withSemanticUpdate(node, update);
}

function createSemanticView(draft: SemanticViewNodeDraft): View {
  return createView(createSemanticViewNode(nextNodeId(), draft));
}

export class View {
  /**
   * PERF-12 T13.1 R8 (handoff §16/AMENDMENT-C §32.2.5): keyed child-owner
   * group. Component invocations inside `build` reconcile under a stable
   * identity namespace, so moved instances keep their execution scopes
   * without re-execution. Keys protect CHILD EXECUTION IDENTITY only — raw
   * Views built directly in the thunk still follow the enclosing scope's
   * ordinary semantic-slot behavior.
   *
   * Keyed groups do not consume unkeyed ordinals and are not independently
   * schedulable scopes: State reads inside `build` belong to the enclosing
   * execution scope (identity = View.key; execution = defineView;
   * invalidation = State<T>).
   */
  static key(key: string | number, build: () => View): View {
    return withKeyedChildOwner(activeChildOwnerOrThrow(), key, build);
  }

  readonly kind = "view" as const;

  private constructor(draft: SemanticViewNodeDraft) {
    installSemanticNode(this, createSemanticViewNode(nextNodeId(), draft));
    Object.freeze(this);
  }

  static contentMax(maxRows: number, child: View): View {
    validateU16(maxRows, "maxRows");
    if (isRetainedConstruction()) return composeContentMax(maxRows, child);
    return new View({ kind: SEMANTIC_VIEW_KIND.contentMax, child: semanticNodeOf(child), maxRows });
  }

  static diff(hunks: readonly DiffHunk[]): View {
    if (isRetainedConstruction()) return composeDiff(hunks);
    return new View({ kind: SEMANTIC_VIEW_KIND.diff, hunks: hunks.map(toSemanticHunk) });
  }

  static text(value: string): View {
    if (typeof value !== "string") throw new TypeError("View.text requires a string");
    if (isRetainedConstruction()) return composeText(value);
    return new View({
      kind: SEMANTIC_VIEW_KIND.text,
      spans: [{ text: value }],
      wrap: "wordThenGrapheme",
      align: "start",
    });
  }

  static styledText(spans: readonly TextSpan[]): View {
    if (isRetainedConstruction()) return composeStyledText(spans);
    return new View({
      kind: SEMANTIC_VIEW_KIND.text,
      spans: spans.map(semanticTextSpanFor),
      wrap: "wordThenGrapheme",
      align: "start",
    });
  }

  static spacer(rows: number): View {
    validateU16(rows, "rows");
    if (isRetainedConstruction()) return composeSpacer(rows);
    return new View({ kind: SEMANTIC_VIEW_KIND.spacer, rows });
  }

  static horizontal(children: ViewChildren): View {
    if (isRetainedConstruction()) {
      const build = typeof children === "function" ? children : (builder: ChildrenBuilder) => builder.childrenOf(children);
      return composeHorizontal(build);
    }
    const builder = buildChildren(children);
    const entries = semanticLayoutChildren(builder.children);
    const view = new View({ kind: SEMANTIC_VIEW_KIND.row, children: entries, gap: builder.gapValue() });
    seedWideAxisSequence(view, entries);
    return view;
  }

  static vertical(children: ViewChildren): View {
    if (isRetainedConstruction()) {
      const build = typeof children === "function" ? children : (builder: ChildrenBuilder) => builder.childrenOf(children);
      return composeVertical(build);
    }
    const builder = buildChildren(children);
    const entries = semanticLayoutChildren(builder.children);
    const view = new View({ kind: SEMANTIC_VIEW_KIND.column, children: entries, gap: builder.gapValue() });
    seedWideAxisSequence(view, entries);
    return view;
  }

  static hanging(prefix: View, continuation: View, body: View): View {
    if (isRetainedConstruction()) return composeHanging(prefix, continuation, body);
    return new View({
      kind: SEMANTIC_VIEW_KIND.hanging,
      prefix: semanticNodeOf(prefix),
      continuation: semanticNodeOf(continuation),
      body: semanticNodeOf(body),
    });
  }

  static grid(specification: readonly View[] | GridSpec | ((builder: GridBuilder) => void)): View {
    if (isRetainedConstruction()) return composeGrid(specification);
    return rawGrid(specification);
  }

  bold(): View { return this.textAttribute("bold"); }
  dim(): View { return this.textAttribute("dim"); }
  italic(): View { return this.textAttribute("italic"); }
  underline(): View { return this.textAttribute("underline"); }
  reversed(): View { return this.textAttribute("reversed"); }
  strikethrough(): View { return this.textAttribute("strikethrough"); }
  textAttribute(name: TextAttribute, enabled = true): View {
    validateTextAttribute(name);
    if (typeof enabled !== "boolean") throw new TypeError("text attribute value must be boolean");
    if (isRetainedConstruction()) return composeTextAttribute(this, name, enabled);
    return this.decorate({ style: { ...semanticEmptyStyle(), attributes: { [name]: enabled } } });
  }
  padding(value: number | Insets): View { return isRetainedConstruction() ? composePadding(this, value) : this.decorate({ padding: insets(value) }); }
  background(color: ColorSpec): View {
    if (isRetainedConstruction()) return composeBackground(this, color);
    return this.decorate({ background: semanticColorFor(color) });
  }
  foreground(color: ColorSpec): View {
    if (isRetainedConstruction()) return composeForeground(this, color);
    // Foreground is an inherited text-style patch in the Rust semantic API,
    // not a separate decoration field. Keeping it in the style record also
    // preserves fluent ordering with StyleRef named-style replacement.
    return this.decorate({ style: { ...semanticEmptyStyle(), foreground: semanticColorFor(color) } });
  }
  border(border: BorderSpec): View {
    if (isRetainedConstruction()) return composeBorder(this, border);
    return this.decorate({ border: semanticBorderFor(border) });
  }
  style(style: StyleRef | StyleSpec): View {
    if (isRetainedConstruction()) return composeStyle(this, style);
    const normalized = semanticStyleFor(style);
    // A named StyleRef replaces the current text-style identity; a direct
    // StyleSpec remains a sparse overlay. This mirrors Rust's View::style
    // distinction and keeps named-style selection from inheriting stale local
    // fields from an earlier style call.
    const replacesStyle = style.kind === "style-ref" && style.themeKey !== undefined;
    return this.decorate({
      style: replacesStyle
        ? normalized
        : semanticMergeStyles(semanticEmptyStyle(), normalized),
    }, replacesStyle);
  }

  /** Attaches one host-owned retained state identity to this occurrence. */
  state(state: ViewState): View {
    if (typeof state !== "object" || state === null || state.kind !== "state" || state.disposed) {
      throw new TypeError("View.state requires a live ViewState");
    }
    if (isRetainedConstruction()) return composeState(this, state);
    return attachStateDirect(this, state.id, state);
  }

  styleState(key: string | StyleStateKey, value: string | StyleStateValue): View {
    const stateKey = typeof key === "string" ? key : key.value;
    const stateValue = typeof value === "string" ? value : value.value;
    if (typeof stateKey !== "string" || stateKey.length === 0
      || typeof stateValue !== "string" || stateValue.length === 0) {
      throw new RangeError("style state key and value cannot be empty");
    }
    if (isRetainedConstruction()) return composeStyleState(this, stateKey, stateValue);
    const decorated = this.decoratedNode();
    const current = decorated === undefined ? semanticDecorationFor() : semanticCloneDecoration(decorated.decoration);
    const child = decorated?.child ?? semanticNodeOf(this);
    return new View({
      kind: SEMANTIC_VIEW_KIND.decorated,
      child,
      stateAttachment: decorated?.stateAttachment,
      decoration: {
        ...current,
        styleStates: { ...current.styleStates, [stateKey]: stateValue },
      },
    });
  }

  container(): View {
    return isRetainedConstruction()
      ? composeContainer(this)
      : new View({ kind: SEMANTIC_VIEW_KIND.container, child: semanticNodeOf(this) });
  }

  clampRows(maxRows: number, overflow: OverflowIndicator = { kind: "none" }): View {
    validateU16(maxRows, "maxRows");
    return isRetainedConstruction()
      ? composeClampRows(this, maxRows, overflow)
      : new View({
        kind: SEMANTIC_VIEW_KIND.clamp,
        child: semanticNodeOf(this),
        maxRows,
        overflow: semanticOverflowFor(overflow),
      });
  }

  fitWidth(): View { return isRetainedConstruction() ? composeFitWidth(this) : this.decorate({ width: "fit" }); }
  fillWidth(): View { return isRetainedConstruction() ? composeFillWidth(this) : this.decorate({ width: "fill" }); }
  fitHeight(): View { return isRetainedConstruction() ? composeFitHeight(this) : this.decorate({ height: "fit" }); }
  fillHeight(): View { return isRetainedConstruction() ? composeFillHeight(this) : this.decorate({ height: "fill" }); }
  minWidth(value: number): View { const validated = validateU16(value, "minWidth"); return isRetainedConstruction() ? composeMinWidth(this, validated) : this.decorate({ minWidth: validated }); }
  maxWidth(value: number): View { const validated = validateU16(value, "maxWidth"); return isRetainedConstruction() ? composeMaxWidth(this, validated) : this.decorate({ maxWidth: validated }); }
  minHeight(value: number): View { const validated = validateU16(value, "minHeight"); return isRetainedConstruction() ? composeMinHeight(this, validated) : this.decorate({ minHeight: validated }); }
  maxHeight(value: number): View { const validated = validateU16(value, "maxHeight"); return isRetainedConstruction() ? composeMaxHeight(this, validated) : this.decorate({ maxHeight: validated }); }
  wrap(mode: WrapMode): View {
    return isRetainedConstruction() ? composeWrap(this, mode) : this.textLayoutPatch(mode, undefined);
  }
  noWrap(): View { return this.wrap("noWrap"); }
  textAlign(align: HorizontalAlign): View {
    return isRetainedConstruction() ? composeTextAlign(this, align) : this.textLayoutPatch(undefined, align);
  }

  private decoratedNode(): Extract<SemanticViewNode, { kind: typeof SEMANTIC_VIEW_KIND.decorated }> | undefined {
    const node = semanticNodeOf(this);
    return node.kind === SEMANTIC_VIEW_KIND.decorated ? node : undefined;
  }

  private decorate(decoration: Partial<SemanticDecoration>, replaceStyle = false): View {
    const decorated = this.decoratedNode();
    const current = decorated === undefined ? semanticDecorationFor() : semanticCloneDecoration(decorated.decoration);
    const child = decorated?.child ?? semanticNodeOf(this);
    const next: SemanticDecoration = {
      ...current,
      ...decoration,
      style: decoration.style === undefined
        ? current.style
        : replaceStyle ? semanticCloneStyle(decoration.style) : semanticMergeStyles(current.style, decoration.style),
    };
    const derived = new View({
      kind: SEMANTIC_VIEW_KIND.decorated,
      child,
      stateAttachment: decorated?.stateAttachment,
      decoration: semanticCloneDecoration(next),
    });
    // PERF-12 T9 (§27/§28): a scalar-only decoration is exactly
    // `base + masked modifiers`, which the retained common patch expresses
    // without re-materializing the base subtree. Mixed decorations stay
    // unhinted and route through normal materialization/fallback.
    const scalarDerivation = commonScalarDerivation(child, next);
    if (scalarDerivation !== undefined) setSemanticDerivation(semanticNodeOf(derived), scalarDerivation);
    return derived;
  }

  private textLayoutPatch(wrap: WrapMode | undefined, align: HorizontalAlign | undefined): View {
    if (wrap !== undefined) validateWrapMode(wrap);
    if (align !== undefined) validateHorizontalAlign(align);
    const node = semanticNodeOf(this);
    if (node.kind === SEMANTIC_VIEW_KIND.text) {
      const derived = new View({
        kind: SEMANTIC_VIEW_KIND.text,
        spans: node.spans,
        wrap: wrap ?? node.wrap,
        align: align ?? node.align,
        stateAttachment: node.stateAttachment,
      });
      setSemanticDerivation(semanticNodeOf(derived), {
        kind: "textLayout",
        base: node,
        wrap: wrap ?? node.wrap,
        align: align ?? node.align,
      });
      return derived;
    }
    if (node.kind === SEMANTIC_VIEW_KIND.decorated && node.child.kind === SEMANTIC_VIEW_KIND.text) {
      const baseText = node.child;
      // The patched text child is a semantic mutation in its own right. Give
      // it a fresh NodeId before placing it under the preserved decoration;
      // reusing baseText.id would let NodeId promotion return the old layout.
      const child = withSemanticUpdate(baseText, {
        wrap: wrap ?? baseText.wrap,
        align: align ?? baseText.align,
      });
      const derived = new View({
        kind: SEMANTIC_VIEW_KIND.decorated,
        child,
        stateAttachment: node.stateAttachment,
        decoration: node.decoration,
      });
      setSemanticDerivation(child, {
        kind: "textLayout",
        base: baseText,
        wrap: wrap ?? baseText.wrap,
        align: align ?? baseText.align,
      });
      return derived;
    }
    return this;
  }
}

/** @internal Creates a View from an already-owned semantic node. */
function createView(node: SemanticViewNode): View {
  const view = Object.create(View.prototype) as View;
  Object.defineProperty(view, "kind", {
    configurable: true,
    enumerable: true,
    value: "view",
    writable: true,
  });
  installSemanticNode(view, node);
  return Object.freeze(view) as View;
}

/** @internal Re-wraps an already-owned semantic node at the API boundary. */
export function createViewFromSemanticNode(node: SemanticViewNode): View {
  return createView(node);
}

/** @internal Adds a retained state attachment without changing topology. */
function attachStateDirect(view: View, handleId: HandleId, reference: object): View {
  const node = updateSemanticViewNode(semanticNodeOf(view), {
    stateAttachment: handleId,
  });
  retainSemanticAttachmentReference(node, reference);
  return createViewFromSemanticNode(node);
}

/** @internal Composition-only state attachment constructor. */
export function attachStateForComposition(
  view: View,
  handleId: HandleId,
  reference: object,
): View {
  return attachStateDirect(view, handleId, reference);
}

/** @internal Creates a semantic component occurrence from a local HandleId. */
export function componentViewForHandle(handleId: HandleId, reference?: object): View {
  const view = createSemanticView({ kind: SEMANTIC_VIEW_KIND.component, handleId });
  if (reference !== undefined) retainSemanticAttachmentReference(semanticNodeOf(view), reference);
  return view;
}

/** @internal Adds an H3 attachment without exposing the plane to public View APIs. */
export function attachSemanticResourceForTesting(
  view: View,
  slot: "stateAttachment" | "contentAttachment",
  handleId: HandleId,
  reference: object,
): View {
  const node = updateSemanticViewNode(semanticNodeOf(view), { [slot]: handleId });
  retainSemanticAttachmentReference(node, reference);
  return createViewFromSemanticNode(node);
}

/** @internal Raw grid construction shared by public and retained composition paths. */
export function rawGrid(specification: readonly View[] | GridSpec | ((builder: GridBuilder) => void)): View {
  return gridViewFromBuilder(gridBuilderFromSpecification(specification));
}

/** @internal Builds the public grid input once, without allocating a semantic View. */
export function gridBuilderFromSpecification(
  specification: readonly View[] | GridSpec | ((builder: GridBuilder) => void),
): GridBuilder {
  const builder = new GridBuilder();
  if (Array.isArray(specification)) {
    builder.columns(specification.map(() => ({ kind: "content" as const })));
    builder.row((row) => specification.forEach((view) => row.cell(view)));
  } else if (typeof specification === "function") specification(builder);
  else {
    const spec = specification as GridSpec;
    builder.columns(spec.columns ?? []);
    for (const row of spec.rows) builder.row(row);
    builder.columnGap(spec.columnGap ?? 0).rowGap(spec.rowGap ?? 0);
  }
  return builder;
}

/** @internal Materializes a normalized grid after retained equality has been checked. */
export function gridViewFromBuilder(builder: GridBuilder): View {
  const rows: SemanticGridRow[] = builder.rows.map((row) => ({
    track: semanticGridTrack(row.track ?? { kind: "content" }),
    cells: row.cells.map((cell): SemanticGridCell => ({
      view: semanticNodeOf(cell.view),
      columnSpan: validatePositiveU16(cell.columnSpan ?? 1, "columnSpan"),
      rowSpan: validatePositiveU16(cell.rowSpan ?? 1, "rowSpan"),
      horizontalAlign: semanticHorizontalAlign(cell.horizontalAlign ?? "start"),
      verticalAlign: semanticVerticalAlign(cell.verticalAlign ?? "top"),
    })),
  }));
  const view = createSemanticView({
    kind: SEMANTIC_VIEW_KIND.grid,
    columns: builder.columnsValue.map(semanticGridTrack),
    rows,
    columnGap: builder.columnGapValue,
    rowGap: builder.rowGapValue,
  });
  seedWideGridSequence(view, rows);
  return view;
}

/** @internal PERF-12 wide retained axis replacement. */
export function axisSetChildForTransport(base: View, index: number, child: View, track?: SemanticAxisTrack): View {
  const baseNode = semanticNodeOf(base);
  if (baseNode.kind !== SEMANTIC_VIEW_KIND.row && baseNode.kind !== SEMANTIC_VIEW_KIND.column) {
    throw new TypeError("retained axis edit base is not a row or column");
  }
  const baseSequence = semanticAxisSequence(baseNode);
  if (!Number.isInteger(index) || index < 0 || index >= baseSequence.length) {
    throw new RangeError("retained axis edit index out of range");
  }
  const childNode = semanticNodeOf(child);
  const semanticTrack = track === undefined ? undefined : semanticAxisTrackFor(track);
  const current = baseSequence.get(index)!;
  const next = semanticTrack === undefined ? { ...current, child: childNode } : layoutChildFromAxisTrack(semanticTrack, childNode);
  const sequence = baseSequence.set(index, next);
  return buildWideAxisNode(baseNode, sequence, { kind: "axisSet", index }, {
    kind: "axisSet",
    base: baseNode,
    index,
    track: semanticTrack,
    child: childNode,
  });
}

/** @internal PERF-12 wide retained axis splice. */
export function axisSpliceForTransport(
  base: View,
  index: number,
  removeCount: number,
  inserted: readonly { readonly view: View; readonly track?: SemanticAxisTrack }[],
): View {
  const baseNode = semanticNodeOf(base);
  if (baseNode.kind !== SEMANTIC_VIEW_KIND.row && baseNode.kind !== SEMANTIC_VIEW_KIND.column) {
    throw new TypeError("retained axis edit base is not a row or column");
  }
  const baseSequence = semanticAxisSequence(baseNode);
  if (!Number.isInteger(index) || index < 0 || index > baseSequence.length) {
    throw new RangeError("retained axis splice index out of range");
  }
  if (!Number.isInteger(removeCount) || removeCount < 0 || index + removeCount > baseSequence.length) {
    throw new RangeError("retained axis splice count out of range");
  }
  const insertedChildren = inserted.map((entry) => ({
    child: semanticNodeOf(entry.view),
    track: semanticAxisTrackFor(entry.track ?? { kind: "normal" }),
  }));
  const insertedLayout = insertedChildren.map((entry) => layoutChildFromAxisTrack(entry.track, entry.child));
  const sequence = baseSequence.splice(index, removeCount, ...insertedLayout);
  return buildWideAxisNode(
    baseNode,
    sequence,
    { kind: "axisSplice", index, removeCount, insertedCount: insertedLayout.length },
    { kind: "axisSplice", base: baseNode, index, removeCount, inserted: insertedChildren },
  );
}

/** @internal PERF-12 retained grid-cell replacement. */
export function gridSetCellForTransport(base: View, row: number, column: number, cellView: View): View {
  const gridNode = semanticNodeOf(base);
  if (gridNode.kind !== SEMANTIC_VIEW_KIND.grid) throw new TypeError("retained grid cell edit base is not a grid");
  const gridOverride = peekSemanticGridSequenceOverride(gridNode);
  const placement = gridOverride === undefined ? gridPlacement(gridNode.rows) : undefined;
  const rowCount = gridOverride?.rowTracks.length ?? gridNode.rows.length;
  if (!Number.isInteger(row) || row < 0 || row >= rowCount) throw new RangeError("retained grid cell row out of range");
  const childNode = semanticNodeOf(cellView);
  if (gridOverride !== undefined) {
    const sequenceIndex = gridOverride.cellIndices[row]?.get(column);
    if (sequenceIndex === undefined) throw new RangeError("retained grid cell column out of range");
    const sequence = (gridOverride.sequence as PersistentSeq<SemanticGridCell>).set(sequenceIndex, {
      ...gridOverride.sequence.get(sequenceIndex)!,
      view: childNode,
    });
    const derived = buildWideGridNode(gridNode, gridOverride, sequence);
    const derivedNode = semanticNodeOf(derived);
    setSemanticDerivation(derivedNode, {
      kind: "gridCell",
      base: gridNode,
      row,
      column,
      child: childNode,
    });
    setSemanticAttachmentPresence(
      derivedNode,
      semanticNodeHasAttachments(gridNode) || semanticNodeHasAttachments(childNode),
    );
    return derived;
  }
  const sequenceIndex = placement!.cellIndices[row]?.get(column);
  if (sequenceIndex === undefined) throw new RangeError("retained grid cell column out of range");
  const rowStart = placement!.rowOffsets[row]!;
  const cellIndex = sequenceIndex - rowStart;
  // Narrow grids retain the ordinary eager semantic shape; only the wide
  // sidecar path above avoids copying the addressed row's cell array.
  const rows = gridNode.rows.map((current, rowIndex) =>
    rowIndex === row
      ? { ...current, cells: current.cells.map((cell, index) => index === cellIndex ? { ...cell, view: childNode } : cell) }
      : current,
  );
  const derived = createSemanticView({
    kind: SEMANTIC_VIEW_KIND.grid,
    columns: gridNode.columns,
    rows,
    columnGap: gridNode.columnGap,
    rowGap: gridNode.rowGap,
    stateAttachment: gridNode.stateAttachment,
  });
  setSemanticDerivation(semanticNodeOf(derived), {
    kind: "gridCell",
    base: gridNode,
    row,
    column,
    child: childNode,
  });
  return derived;
}

/** @internal Retained axis construction used by compose.ts. */
export function composedAxis(row: boolean, entries: readonly LayoutChild[], gap: number): View {
  const semanticEntries = semanticLayoutChildren(entries);
  const view = createSemanticView({ kind: row ? SEMANTIC_VIEW_KIND.row : SEMANTIC_VIEW_KIND.column, children: semanticEntries, gap });
  seedWideAxisSequence(view, semanticEntries);
  return view;
}

/**
 * PERF-12 T9 (§27/§28): encodes a scalar-only decoration as a common-scalar
 * derivation over `base`. Returns undefined for any decoration carrying
 * non-scalar content (color, border, non-empty style, styleStates) or no
 * scalar modifier at all — those have no exact retained primitive and stay
 * unhinted (§27: the hint must not guess).
 */
function commonScalarDerivation(
  base: SemanticViewNode,
  decoration: SemanticDecoration,
): Extract<SemanticDerivation, { kind: "commonScalar" }> | undefined {
  if (decoration.background !== undefined || decoration.foreground !== undefined || decoration.border !== undefined) return undefined;
  if (decoration.styleStates !== undefined && Object.keys(decoration.styleStates).length > 0) return undefined;
  if (!isEmptyStyle(decoration.style)) return undefined;
  const changes: {
    padding?: SemanticCommonScalarChanges["padding"];
    width?: SemanticCommonScalarChanges["width"];
    height?: SemanticCommonScalarChanges["height"];
    minWidth?: SemanticCommonScalarChanges["minWidth"];
    maxWidth?: SemanticCommonScalarChanges["maxWidth"];
    minHeight?: SemanticCommonScalarChanges["minHeight"];
    maxHeight?: SemanticCommonScalarChanges["maxHeight"];
  } = {};
  if (decoration.padding !== undefined) changes.padding = { ...decoration.padding };
  if (decoration.width !== undefined) changes.width = decoration.width;
  if (decoration.height !== undefined) changes.height = decoration.height;
  if (decoration.minWidth !== undefined) changes.minWidth = decoration.minWidth;
  if (decoration.maxWidth !== undefined) changes.maxWidth = decoration.maxWidth;
  if (decoration.minHeight !== undefined) changes.minHeight = decoration.minHeight;
  if (decoration.maxHeight !== undefined) changes.maxHeight = decoration.maxHeight;
  if (Object.keys(changes).length === 0) return undefined;
  return { kind: "commonScalar", base, changes };
}

function isEmptyStyle(style: { readonly theme?: string; readonly foreground?: unknown; readonly background?: unknown; readonly attributes: Readonly<Record<string, boolean>> }): boolean {
  return style.theme === undefined && style.foreground === undefined && style.background === undefined
    && Object.keys(style.attributes).length === 0;
}

function seedWideAxisSequence(view: View, children: readonly SemanticLayoutChild[]): void {
  if (children.length <= WIDE_AXIS_SEQUENCE_THRESHOLD) return;
  const node = semanticNodeOf(view);
  setSemanticSequenceOverride(node, {
    baseNode: node,
    sequence: PersistentSeq.from(children),
  });
}

function gridPlacement(rows: readonly SemanticGridRow[]): {
  readonly rowOffsets: readonly number[];
  readonly cellIndices: readonly ReadonlyMap<number, number>[];
} {
  const occupiedUntil: number[] = [];
  const rowOffsets = [0];
  const cellIndices: Map<number, number>[] = [];
  let sequenceIndex = 0;
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const row = rows[rowIndex]!;
    const rowIndexMap = new Map<number, number>();
    cellIndices.push(rowIndexMap);
    for (const cell of row.cells) {
      const columnSpan = cell.columnSpan;
      let column = 0;
      for (;;) {
        while (occupiedUntil.length < column + columnSpan) occupiedUntil.push(0);
        let available = true;
        for (let index = column; index < column + columnSpan; index += 1) {
          if (occupiedUntil[index]! > rowIndex) {
            available = false;
            break;
          }
        }
        if (available) break;
        column += 1;
      }
      rowIndexMap.set(column, sequenceIndex);
      sequenceIndex += 1;
      const occupiedThrough = rowIndex + cell.rowSpan;
      for (let index = column; index < column + columnSpan; index += 1) occupiedUntil[index] = occupiedThrough;
    }
    rowOffsets.push(sequenceIndex);
  }
  return {
    rowOffsets: Object.freeze(rowOffsets),
    cellIndices: Object.freeze(cellIndices.map((indices) => indices)),
  };
}

function seedWideGridSequence(view: View, rows: readonly SemanticGridRow[]): void {
  const totalCells = rows.reduce((total, row) => total + row.cells.length, 0);
  if (totalCells <= WIDE_GRID_SEQUENCE_THRESHOLD) return;
  const placement = gridPlacement(rows);
  const rowTracks = rows.map((row) => row.track);
  const cells: SemanticGridCell[] = [];
  for (const row of rows) cells.push(...row.cells);
  const node = semanticNodeOf(view);
  setSemanticGridSequenceOverride(node, {
    baseNode: node,
    sequence: PersistentSeq.from(cells),
    rowOffsets: placement.rowOffsets,
    rowTracks: Object.freeze(rowTracks),
    cellIndices: placement.cellIndices,
  });
}

function buildWideGridNode(
  baseNode: Extract<SemanticViewNode, { kind: typeof SEMANTIC_VIEW_KIND.grid }>,
  override: {
    readonly rowOffsets: readonly number[];
    readonly rowTracks: readonly SemanticGridTrack[];
    readonly cellIndices: readonly ReadonlyMap<number, number>[];
  },
  sequence: PersistentSeq<SemanticGridCell>,
): View {
  let flatRows: readonly SemanticGridRow[] | undefined;
  const node = Object.freeze({
    id: nextNodeId(),
    kind: SEMANTIC_VIEW_KIND.grid,
    columns: baseNode.columns,
    stateAttachment: baseNode.stateAttachment,
    get rows(): readonly SemanticGridRow[] {
      if (flatRows === undefined) {
        const rows: SemanticGridRow[] = [];
        for (let rowIndex = 0; rowIndex < override.rowTracks.length; rowIndex += 1) {
          const start = override.rowOffsets[rowIndex]!;
          const end = override.rowOffsets[rowIndex + 1]!;
          const cells: SemanticGridCell[] = [];
          for (let index = start; index < end; index += 1) cells.push(Object.freeze({ ...sequence.get(index)! }));
          rows.push(Object.freeze({ track: override.rowTracks[rowIndex]!, cells: Object.freeze(cells) }));
        }
        flatRows = Object.freeze(rows);
      }
      return flatRows;
    },
    columnGap: baseNode.columnGap,
    rowGap: baseNode.rowGap,
  }) as SemanticViewNode;
  setSemanticGridSequenceOverride(node, {
    baseNode,
    sequence,
    rowOffsets: override.rowOffsets,
    rowTracks: override.rowTracks,
    cellIndices: override.cellIndices,
  });
  setSemanticAttachmentPresence(node, semanticNodeHasAttachments(baseNode));
  return wrapFrozenSemanticNode(node);
}

/**
 * PERF-12 T10 (§34): authoritative children sequence of an axis node - the
 * wide override when present, else a one-time snapshot of the flat array.
 */
function semanticAxisSequence(node: Extract<SemanticViewNode, { kind: typeof SEMANTIC_VIEW_KIND.row | typeof SEMANTIC_VIEW_KIND.column }>): PersistentSeq<SemanticLayoutChild> {
  const override = peekSemanticSequenceOverride(node);
  if (override !== undefined) return override.sequence as PersistentSeq<SemanticLayoutChild>;
  return PersistentSeq.from(node.children);
}

function semanticAxisTrackFor(track: SemanticAxisTrack): SemanticAxisTrack {
  switch (track.kind) {
    case "normal": return Object.freeze({ kind: "normal" });
    case "fixed": return Object.freeze({ kind: "fixed", size: validateU16(track.size, "size") });
    case "flex": return Object.freeze({ kind: "flex" });
    case "flexMax": return Object.freeze({ kind: "flexMax", maxRows: validateU16(track.maxRows, "maxRows") });
    case "contentMax": return Object.freeze({ kind: "contentMax", maxRows: validateU16(track.maxRows, "maxRows") });
    default: throw new TypeError("unknown semantic axis track kind");
  }
}

function layoutChildFromAxisTrack(track: SemanticAxisTrack, child: SemanticViewNode): SemanticLayoutChild {
  switch (track.kind) {
    case "normal": return { kind: "normal", child };
    case "fixed": return { kind: "fixed", size: validateU16(track.size, "size"), child };
    case "flex": return { kind: "flex", child };
    case "flexMax": return { kind: "flexMax", maxRows: validateU16(track.maxRows, "maxRows"), child };
    case "contentMax": return { kind: "contentMax", maxRows: validateU16(track.maxRows, "maxRows"), child };
    default: throw new TypeError("unknown semantic axis track kind");
  }
}

function buildWideAxisNode(
  baseNode: Extract<SemanticViewNode, { kind: typeof SEMANTIC_VIEW_KIND.row | typeof SEMANTIC_VIEW_KIND.column }>,
  sequence: PersistentSeq<SemanticLayoutChild>,
  edit: SemanticAxisSequenceEdit,
  derivation: Extract<SemanticDerivation, { kind: "axisSet" | "axisSplice" }>,
): View {
  let flat: readonly SemanticLayoutChild[] | undefined;
  const node = Object.freeze({
    id: nextNodeId(),
    kind: baseNode.kind,
    stateAttachment: baseNode.stateAttachment,
    gap: baseNode.gap,
    get children(): readonly SemanticLayoutChild[] {
      if (flat === undefined) flat = Object.freeze(sequence.toArray().map((entry) => Object.freeze({ ...entry })));
      return flat;
    },
  }) as SemanticViewNode;
  setSemanticSequenceOverride(node, { baseNode, sequence, edit });
  setSemanticDerivation(node, derivation);
  const insertedAttachments = derivation.kind === "axisSet"
    ? semanticNodeHasAttachments(derivation.child)
    : derivation.inserted.some((entry) => semanticNodeHasAttachments(entry.child));
  setSemanticAttachmentPresence(node, semanticNodeHasAttachments(baseNode) || insertedAttachments);
  // The node is already frozen with its final identity; evaluating children
  // here would materialize the lazy wide sequence and defeat §34.
  return wrapFrozenSemanticNode(node);
}

/** Internal component-free identity helper used by semantic construction. */
function wrapFrozenSemanticNode(node: SemanticViewNode): View {
  const view = Object.create(View.prototype) as View;
  Object.defineProperty(view, "kind", {
    configurable: true,
    enumerable: true,
    value: "view",
    writable: true,
  });
  installSemanticNode(view, node);
  return Object.freeze(view) as View;
}

/** Returns the full semantic NodeId of the View's frozen semantic node. */
export function viewNodeId(view: View): number {
  return semanticNodeOf(view).id;
}

/** Returns the u32 halves of a View's full safe-integer NodeId. */
export function nodeIdPair(view: View): readonly [number, number] {
  const id = viewNodeId(view);
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

/**
 * Current high-water of the private monotonic NodeId allocator (§18).
 * View-bearing boundaries capture this after each successful commit as
 * `nativeLookupCeiling`: only NodeIds at or below it may already exist in the
 * native semantic cache and are eligible for generation-scoped promotion.
 */
export function viewNodeIdHighWater(): number {
  return nodeIdCounter.next - 1;
}

export function textRowsForHarness(view: View): string[] { return rows(semanticNodeOf(view)); }

function rows(node: SemanticViewNode): string[] {
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.text: return [node.spans.map((span) => span.text).join("")];
    case SEMANTIC_VIEW_KIND.diff: return node.hunks.flatMap((hunk) => [
      `@@ -${displayDiffRange(hunk.oldRange)} +${displayDiffRange(hunk.newRange)} @@`,
      ...hunk.lines.flatMap((line) => [
        `${line.kind === "addition" ? "+" : line.kind === "deletion" ? "-" : " "}${line.text}`,
        ...(line.termination === "unterminated" ? ["\\ No newline at end of file"] : []),
      ]),
    ]);
    case SEMANTIC_VIEW_KIND.spacer: return Array.from({ length: node.rows }, () => "");
    case SEMANTIC_VIEW_KIND.row: return [node.children.flatMap((child) => rows(child.child)).join("")];
    case SEMANTIC_VIEW_KIND.column: return node.children.flatMap((child) => rows(child.child));
    case SEMANTIC_VIEW_KIND.grid: return node.rows.flatMap((row) => row.cells.flatMap((cell) => rows(cell.view)));
    case SEMANTIC_VIEW_KIND.hanging: return rows(node.prefix).map((prefix, index) => `${prefix}${index === 0 ? rows(node.body)[0] ?? "" : rows(node.body)[index] ?? ""}`);
    case SEMANTIC_VIEW_KIND.container: return rows(node.child);
    case SEMANTIC_VIEW_KIND.clamp: return rows(node.child).slice(0, node.maxRows);
    case SEMANTIC_VIEW_KIND.contentMax: return rows(node.child).slice(0, node.maxRows);
    case SEMANTIC_VIEW_KIND.component: return [""];
    case SEMANTIC_VIEW_KIND.decorated: return rows(node.child);
  }
}

function toSemanticHunk(hunk: DiffHunk): SemanticDiffHunk {
  let oldLine = hunk.oldRange.start + 1;
  let newLine = hunk.newRange.start + 1;
  const lines: SemanticDiffLine[] = hunk.lines.map((line) => {
    const lineKind = semanticDiffLineKind(line.lineKind);
    const termination = semanticDiffTermination(line.termination);
    const node = {
      kind: lineKind,
      text: line.text,
      termination,
      ...(lineKind === "context" ? { oldLine, newLine } : {}),
      ...(lineKind === "addition" ? { newLine } : {}),
      ...(lineKind === "deletion" ? { oldLine } : {}),
    } as SemanticDiffLine;
    if (lineKind !== "addition") oldLine += 1;
    if (lineKind !== "deletion") newLine += 1;
    return node;
  });
  return {
    oldRange: { start: hunk.oldRange.start, count: hunk.oldRange.lineCount },
    newRange: { start: hunk.newRange.start, count: hunk.newRange.lineCount },
    lines,
  };
}

function semanticDiffLineKind(kind: string): SemanticDiffLine["kind"] {
  switch (kind) {
    case "context": return "context";
    case "addition": return "addition";
    case "deletion": return "deletion";
    default: throw new TypeError(`unknown diff line kind ${JSON.stringify(kind)}`);
  }
}

function semanticDiffTermination(termination: string): SemanticDiffLine["termination"] {
  switch (termination) {
    case "lf":
    case "crlf": return "terminated";
    case "none": return "unterminated";
    default: throw new TypeError(`unknown diff line termination ${JSON.stringify(termination)}`);
  }
}

function semanticHorizontalAlign(align: HorizontalAlign): SemanticHorizontalAlign {
  switch (align) {
    case "start": return "start";
    case "center": return "center";
    case "end": return "end";
    default: throw new RangeError(`unknown horizontal alignment ${JSON.stringify(align)}`);
  }
}

function semanticVerticalAlign(align: VerticalAlign): SemanticVerticalAlign {
  switch (align) {
    case "top": return "top";
    case "center": return "center";
    case "bottom": return "bottom";
    default: throw new RangeError(`unknown vertical alignment ${JSON.stringify(align)}`);
  }
}

function validateWrapMode(mode: WrapMode): WrapMode {
  switch (mode) {
    case "wordThenGrapheme":
    case "grapheme":
    case "noWrap": return mode;
    default: throw new RangeError(`unknown wrap mode ${JSON.stringify(mode)}`);
  }
}

function validateHorizontalAlign(align: HorizontalAlign): HorizontalAlign {
  switch (align) {
    case "start":
    case "center":
    case "end": return align;
    default: throw new RangeError(`unknown horizontal alignment ${JSON.stringify(align)}`);
  }
}

function semanticGridTrack(track: GridTrack): SemanticGridTrack {
  switch (track.kind) {
    case "content": return { kind: "content" };
    case "contentMax": return { kind: "contentMax", max: validateU16(track.max, "grid track max") };
    case "fixed": return { kind: "fixed", size: validateU16(track.size, "grid track size") };
    case "flex": return { kind: "flex" };
    case "flexMax": return { kind: "flexMax", max: validateU16(track.max, "grid track max") };
    default: throw new TypeError("unknown grid track kind");
  }
}

function semanticLayoutChildren(children: readonly LayoutChild[]): SemanticLayoutChild[] {
  return children.map((entry) => {
    switch (entry.kind) {
      case "normal": return { kind: "normal", child: semanticNodeOf(entry.child) };
      case "fixed": return { kind: "fixed", size: validateU16(entry.size, "size"), child: semanticNodeOf(entry.child) };
      case "flex": return { kind: "flex", child: semanticNodeOf(entry.child) };
      case "flexMax": return { kind: "flexMax", maxRows: validateU16(entry.maxRows, "maxRows"), child: semanticNodeOf(entry.child) };
      case "contentMax": return { kind: "contentMax", maxRows: validateU16(entry.maxRows, "maxRows"), child: semanticNodeOf(entry.child) };
      default: throw new TypeError("unknown layout child kind");
    }
  });
}

function buildChildren(children: ViewChildren): ChildrenBuilder {
  const builder = new ChildrenBuilder();
  if (typeof children === "function") children(builder);
  else builder.childrenOf(children);
  return builder;
}

function displayDiffRange(range: { readonly start: number; readonly count: number }): string {
  if (range.count === 0) return `${range.start},0`;
  const start = range.start + 1;
  return range.count === 1 ? `${start}` : `${start},${range.count}`;
}

function validateU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 65535) throw new RangeError(`${name} must be an integer from 0 to 65535`);
  return value;
}

function validatePositiveU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 1 || value > 65535) throw new RangeError(`${name} must be an integer from 1 to 65535`);
  return value;
}
