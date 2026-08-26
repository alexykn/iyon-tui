/**
 * PERF-12 T13.1 R1 — internal monomorphic compose helpers over retained
 * execution scopes (handoff §11, AMENDMENT-C §10/§17.3).
 *
 * Call shape is final since R0: value-addressed, no module/site ids. Each
 * helper resolves its slot through the ACTIVE EXECUTION SCOPE's dense cursor:
 *
 *   1. no active scope -> ordinary construction, identical semantics
 *      (§19 fall-through; measured within noise by the R0 gate);
 *   2. active scope -> take the scope's next dense semantic slot, compare the
 *      operation's raw arguments against the slot's committed View BEFORE
 *      allocating anything (§19), return the exact previous View on match,
 *      otherwise construct one new immutable View;
 *   3. children compare by BridgeViewNode identity — they were themselves
 *      composed inside their own scopes/slots, so identity IS the shallow
 *      equality proof (§17.1);
 *   4. no rest arrays/string dispatch/reflection on the hot path (§52.8);
 *      modifier deltas use small integer tags.
 *
 * Control-flow shifts realign the dense cursor: they may reduce local reuse
 * but can never select another component instance or produce stale semantics
 * — immediate equality still authorizes every hit (AMENDMENT-C §10.1).
 *
 * Comparators operate on frozen normalized bridge records (§18). The only
 * property-name loops are bounded comparisons over tiny normalized style /
 * style-state records, never tree walks or generic reflection.
 */

import { executionContext, executionCounters } from "./execution.ts";
import { withoutRetainedComposition } from "./execution-context.ts";
import {
  BRIDGE_GRID_TRACK_KIND,
  BRIDGE_HORIZONTAL_ALIGN,
  BRIDGE_LAYOUT_CHILD_KIND,
  BRIDGE_OVERFLOW_KIND,
  BRIDGE_VIEW_KIND,
  BRIDGE_WRAP_MODE,
  emptyDecoration,
  emptyStyle,
  mergeStyles,
  peekBridgeSequenceOverride,
  type BorderNode,
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeViewNode,
  type ColorNode,
  type DecorationNode,
  type DiffHunkNode,
  type StyleNode,
} from "./ir.ts";
import {
  ChildrenBuilder,
  GridBuilder,
  View,
  nodeForBridge,
  type GridSpec,
  type OverflowIndicator,
} from "./values/view.ts";
import { insets } from "./values/geometry.ts";
import type { Insets } from "./values/geometry.ts";
import type { HorizontalAlign, TextSpan, WrapMode } from "./values/text.ts";
import type { StyleSpec } from "./values/style.ts";
import type { NativeHandleId } from "./types.ts";

/** Component-handle contract mirror of View.component's parameter. */
interface ComponentHandleLike {
  readonly id: NativeHandleId;
  nativeComponentId?: () => number | undefined;
}

// --- Slot staging ------------------------------------------------------------

function stageReuse(slot: { pending: View | undefined }, previous: View): void {
  slot.pending = previous;
  executionCounters.composition_exact_view_reuses += 1;
}

function stageFresh(slot: { pending: View | undefined }, view: View): void {
  slot.pending = view;
  executionCounters.composition_new_views += 1;
}

// --- Modifier tags (integer constants; never strings on the hot path). ------

const MOD_FILL_WIDTH = 1;
const MOD_FIT_WIDTH = 2;
const MOD_FILL_HEIGHT = 3;
const MOD_FIT_HEIGHT = 4;
const MOD_MIN_WIDTH = 5;
const MOD_MAX_WIDTH = 6;
const MOD_MIN_HEIGHT = 7;
const MOD_MAX_HEIGHT = 8;
const MOD_PADDING = 9;
const MOD_FOREGROUND = 10;
const MOD_BACKGROUND = 11;
const MOD_STYLE_SPEC = 12;
const MOD_TEXT_ATTRIBUTE = 13;
const MOD_STYLE_STATE = 14;
const MOD_BORDER = 15;

// --- Field comparators over normalized bridge records (§18). ----------------

function colorEqual(a: ColorNode | undefined, b: ColorNode | undefined): boolean {
  if (a === b) return true;
  if (typeof a === "string" || typeof b === "string") return false;
  if (a === undefined || b === undefined) return false;
  return a.type === b.type && a.value === b.value;
}

function insetsEqual(
  a: { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number } | undefined,
  b: { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number } | undefined,
): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined) return false;
  return a.top === b.top && a.right === b.right && a.bottom === b.bottom && a.left === b.left;
}

function attributesEqual(
  a: Readonly<Record<string, boolean>>,
  b: Readonly<Record<string, boolean>>,
): boolean {
  let aCount = 0;
  for (const key in a) {
    if (b[key] !== a[key]) return false;
    aCount += 1;
  }
  let bCount = 0;
  for (const _key in b) bCount += 1;
  return aCount === bCount;
}

function styleNodesEqual(a: StyleNode | undefined, b: StyleNode | undefined): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined) return false;
  return a.theme === b.theme
    && colorEqual(a.foreground, b.foreground)
    && colorEqual(a.background, b.background)
    && attributesEqual(a.attributes, b.attributes);
}

function styleStatesEqual(
  a: Readonly<Record<string, string>> | undefined,
  b: Readonly<Record<string, string>> | undefined,
): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined) return false;
  let aCount = 0;
  for (const key in a) {
    if (b[key] !== a[key]) return false;
    aCount += 1;
  }
  let bCount = 0;
  for (const _key in b) bCount += 1;
  return aCount === bCount;
}

function borderEqual(a: BorderNode | undefined, b: BorderNode | undefined): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined) return false;
  if (a.style !== b.style || a.edges !== b.edges || !colorEqual(a.color, b.color)) return false;
  const aGlyphs = a.glyphs;
  const bGlyphs = b.glyphs;
  if (aGlyphs === bGlyphs) return true;
  if (aGlyphs === undefined || bGlyphs === undefined) return false;
  let count = 0;
  for (const key in aGlyphs) {
    if (bGlyphs[key] !== aGlyphs[key]) return false;
    count += 1;
  }
  let bCount = 0;
  for (const _key in bGlyphs) bCount += 1;
  return count === bCount;
}

/**
 * Full structural equality of two decoration records (used when a modifier
 * inherits the ENTIRE previous chain state unchanged, e.g. layout patches
 * over decorated text).
 */
function decorationFullyEqual(a: DecorationNode, b: DecorationNode): boolean {
  return insetsEqual(a.padding, b.padding)
    && colorEqual(a.background, b.background)
    && colorEqual(a.foreground, b.foreground)
    && borderEqual(a.border, b.border)
    && styleNodesEqual(a.style, b.style)
    && styleStatesEqual(a.styleStates, b.styleStates)
    && a.width === b.width
    && a.height === b.height
    && a.minWidth === b.minWidth
    && a.maxWidth === b.maxWidth
    && a.minHeight === b.minHeight
    && a.maxHeight === b.maxHeight;
}

/**
 * Whether `dec` equals `inherited` except for the field(s) `tag` owns, which
 * are validated against the new arguments (`a`, `b`). This is exactly the
 * decorate() merge contract: untouched fields come from the base chain state,
 * touched fields come from this call site.
 */
function decorationDeltaMatches(
  dec: DecorationNode,
  inherited: DecorationNode,
  tag: number,
  a: unknown,
  b: unknown,
): boolean {
  if (tag !== MOD_PADDING && !insetsEqual(dec.padding, inherited.padding)) return false;
  if (tag !== MOD_BACKGROUND && !colorEqual(dec.background, inherited.background)) return false;
  if (tag !== MOD_FOREGROUND && !colorEqual(dec.foreground, inherited.foreground)) return false;
  if (tag !== MOD_BORDER && !borderEqual(dec.border, inherited.border)) return false;
  if (tag !== MOD_FILL_WIDTH && tag !== MOD_FIT_WIDTH && dec.width !== inherited.width) return false;
  if (tag !== MOD_FILL_HEIGHT && tag !== MOD_FIT_HEIGHT && dec.height !== inherited.height) return false;
  if (tag !== MOD_MIN_WIDTH && (dec.minWidth ?? -1) !== (inherited.minWidth ?? -1)) return false;
  if (tag !== MOD_MAX_WIDTH && (dec.maxWidth ?? -1) !== (inherited.maxWidth ?? -1)) return false;
  if (tag !== MOD_MIN_HEIGHT && (dec.minHeight ?? -1) !== (inherited.minHeight ?? -1)) return false;
  if (tag !== MOD_MAX_HEIGHT && (dec.maxHeight ?? -1) !== (inherited.maxHeight ?? -1)) return false;
  if (tag !== MOD_STYLE_SPEC && tag !== MOD_TEXT_ATTRIBUTE
    && !styleNodesEqual(dec.style, inherited.style)) return false;
  if (tag !== MOD_STYLE_STATE && !styleStatesEqual(dec.styleStates, inherited.styleStates)) return false;

  switch (tag) {
    case MOD_FILL_WIDTH: return dec.width === "fill";
    case MOD_FIT_WIDTH: return dec.width === "fit";
    case MOD_FILL_HEIGHT: return dec.height === "fill";
    case MOD_FIT_HEIGHT: return dec.height === "fit";
    case MOD_MIN_WIDTH: return dec.minWidth === a;
    case MOD_MAX_WIDTH: return dec.maxWidth === a;
    case MOD_MIN_HEIGHT: return dec.minHeight === a;
    case MOD_MAX_HEIGHT: return dec.maxHeight === a;
    case MOD_PADDING: return insetsEqual(dec.padding, insets(a as number | Insets));
    case MOD_FOREGROUND: return colorEqual(dec.foreground, a as ColorNode);
    case MOD_BACKGROUND: return colorEqual(dec.background, a as ColorNode);
    case MOD_BORDER: return borderEqual(dec.border, a as BorderNode);
    case MOD_STYLE_SPEC: {
      const expected = mergeStyles(inherited.style, mergeStyles(emptyStyle(), (a as StyleSpec).value));
      return styleNodesEqual(dec.style, expected);
    }
    case MOD_TEXT_ATTRIBUTE: {
      const name = a as string;
      const value = (b as boolean | undefined) ?? true;
      if (!colorEqual(dec.style.theme, inherited.style.theme)) return false;
      if (!colorEqual(dec.style.foreground, inherited.style.foreground)) return false;
      if (!colorEqual(dec.style.background, inherited.style.background)) return false;
      const inheritedAttributes = inherited.style.attributes;
      const actual = dec.style.attributes;
      let expectedCount = 0;
      for (const key in inheritedAttributes) {
        expectedCount += 1;
        if (actual[key] !== (key === name ? value : inheritedAttributes[key])) return false;
      }
      if (inheritedAttributes[name] === undefined) expectedCount += 1;
      let actualCount = 0;
      for (const _key in actual) actualCount += 1;
      return actualCount === expectedCount && actual[name] === value;
    }
    case MOD_STYLE_STATE: {
      const key = a as string;
      const value = b as string;
      const inheritedStates = inherited.styleStates;
      const actual = dec.styleStates;
      if (actual === undefined || actual[key] !== value) return false;
      let inheritedCount = 0;
      if (inheritedStates !== undefined) {
        for (const existing in inheritedStates) {
          if (existing === key) continue;
          if (actual[existing] !== inheritedStates[existing]) return false;
          inheritedCount += 1;
        }
      }
      let actualCount = 0;
      for (const _existing in actual) actualCount += 1;
      return actualCount === inheritedCount + 1;
    }
    default:
      return false;
  }
}

/**
 * Shared modifier engine. `applyDecorationDirect` uses ONLY public View
 * methods so T9 derivation hints and validation behave identically to
 * uncomposed code.
 */
function applyDecoration(base: View, tag: number, a?: unknown, b?: unknown): View {
  const scope = executionContext.top;
  if (scope === undefined) return applyDecorationDirect(base, tag, a, b);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const previousNode = nodeForBridge(previous);
    if (previousNode.kind === BRIDGE_VIEW_KIND.decorated) {
      const baseNode = nodeForBridge(base);
      // decorate() flattens decorated bases: the result wraps the INNER child
      // and clones the base's accumulated decoration as the merge input.
      const inner = baseNode.kind === BRIDGE_VIEW_KIND.decorated ? baseNode.child : baseNode;
      const inherited = baseNode.kind === BRIDGE_VIEW_KIND.decorated ? baseNode.decoration : emptyDecoration();
      if (previousNode.child === inner && decorationDeltaMatches(previousNode.decoration, inherited, tag, a, b)) {
        stageReuse(slot, previous);
        return previous;
      }
    }
  }
  const view = applyDecorationDirect(base, tag, a, b);
  stageFresh(slot, view);
  return view;
}

function applyDecorationDirect(base: View, tag: number, a: unknown, b: unknown): View {
  const construct = (): View => {
    switch (tag) {
      case MOD_FILL_WIDTH: return base.fillWidth();
      case MOD_FIT_WIDTH: return base.fitWidth();
      case MOD_FILL_HEIGHT: return base.fillHeight();
      case MOD_FIT_HEIGHT: return base.fitHeight();
      case MOD_MIN_WIDTH: return base.minWidth(a as number);
      case MOD_MAX_WIDTH: return base.maxWidth(a as number);
      case MOD_MIN_HEIGHT: return base.minHeight(a as number);
      case MOD_MAX_HEIGHT: return base.maxHeight(a as number);
      case MOD_PADDING: return base.padding(a as number | Insets);
      case MOD_FOREGROUND: return base.foreground(a as ColorNode);
      case MOD_BACKGROUND: return base.background(a as ColorNode);
      case MOD_STYLE_SPEC: return base.style(a as StyleSpec);
      case MOD_TEXT_ATTRIBUTE: return base.textAttribute(a as string, (b as boolean | undefined) ?? true);
      case MOD_STYLE_STATE: return base.styleState(a as string, b as string);
      case MOD_BORDER: return base.border(a as BorderNode);
      default:
        throw new RangeError(`unknown decoration modifier ${tag}`);
    }
  };
  return executionContext.top === undefined ? construct() : withoutRetainedComposition(construct);
}

// --- Factory helpers. -------------------------------------------------------

/** Lowers View.text(content). */
export function composeText(content: string): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.text(content);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (
      node.kind === BRIDGE_VIEW_KIND.text
      && node.spans.length === 1
      && node.spans[0]!.style === undefined
      && node.spans[0]!.text === content
      && node.wrap === BRIDGE_WRAP_MODE.wordThenGrapheme
      && node.align === BRIDGE_HORIZONTAL_ALIGN.start
    ) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.text(content));
  stageFresh(slot, view);
  return view;
}

/** Lowers View.styledText(spans). */
export function composeStyledText(spans: readonly TextSpan[]): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.styledText(spans);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.text && styledSpansMatch(node.spans, spans)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.styledText(spans));
  stageFresh(slot, view);
  return view;
}

function styledSpansMatch(
  bridgeSpans: readonly { readonly text: string; readonly style?: StyleNode }[],
  spans: readonly TextSpan[],
): boolean {
  if (bridgeSpans.length !== spans.length) return false;
  for (let index = 0; index < spans.length; index += 1) {
    const span = spans[index]!.value;
    const bridged = bridgeSpans[index]!;
    if (bridged.text !== span.text) return false;
    if (!styleNodesEqual(bridged.style, span.style)) return false;
  }
  return true;
}

/** Lowers View.spacer(rows). */
export function composeSpacer(rows: number): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.spacer(rows);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.spacer && node.rows === rows) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.spacer(rows));
  stageFresh(slot, view);
  return view;
}

/** Lowers View.component(handle). */
export function composeComponent(handle: ComponentHandleLike): View {
  const componentId = (handle.nativeComponentId?.() ?? handle.id) as NativeHandleId;
  const scope = executionContext.top;
  if (scope === undefined) return View.component(handle);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.component && node.handle === componentId) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.component(handle));
  stageFresh(slot, view);
  return view;
}

/** Lowers View.hanging(prefix, continuation, body). */
export function composeHanging(prefix: View, continuation: View, body: View): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.hanging(prefix, continuation, body);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (
      node.kind === BRIDGE_VIEW_KIND.hanging
      && node.prefix === nodeForBridge(prefix)
      && node.continuation === nodeForBridge(continuation)
      && node.body === nodeForBridge(body)
    ) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.hanging(prefix, continuation, body));
  stageFresh(slot, view);
  return view;
}

/**
 * Lowers View.vertical(build)/View.horizontal(build) (§12.4). The builder
 * executes FIRST so children flow through their own slots/scopes; the
 * container then compares immediate semantics (entry count/kinds/scalars/
 * child identities/gap) before allocating a parent. Wide sequence-backed
 * axes bail out instead of being flattened (§22.3).
 */
function composeAxisImpl(row: boolean, build: (children: ChildrenBuilder) => void): View {
  const builderInstance = new ChildrenBuilder();
  build(builderInstance);
  const entries = builderInstance.children;
  const gap = builderInstance.gapValue();
  const scope = executionContext.top;
  if (scope === undefined) {
    return View.__composedAxis(row, entries, gap);
  }
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined && axisMatches(previous, row, entries, gap)) {
    stageReuse(slot, previous);
    return previous;
  }
  const view = View.__composedAxis(row, entries, gap);
  stageFresh(slot, view);
  return view;
}

export function composeVertical(build: (children: import("./values/view.ts").ChildrenBuilder) => void): View {
  return composeAxisImpl(false, build);
}

export function composeHorizontal(build: (children: import("./values/view.ts").ChildrenBuilder) => void): View {
  return composeAxisImpl(true, build);
}

function axisMatches(previous: View, row: boolean, entries: readonly BridgeLayoutChild[], gap: number): boolean {
  const node = nodeForBridge(previous);
  const expectedKind = row ? BRIDGE_VIEW_KIND.row : BRIDGE_VIEW_KIND.column;
  if (node.kind !== expectedKind) return false;
  // Wide sequence-backed axes carry PersistentSeq overrides; comparing their
  // lazy children would flatten them (§22.3). Rebuild instead of guessing.
  if (peekBridgeSequenceOverride(node) !== undefined) return false;
  if (node.gap !== gap) return false;
  const previousEntries = node.children;
  if (previousEntries.length !== entries.length) return false;
  for (let index = 0; index < entries.length; index += 1) {
    const past = previousEntries[index]!;
    const next = entries[index]!;
    if (past.kind !== next.kind) return false;
    if (past.child !== next.child) return false;
    switch (next.kind) {
      case BRIDGE_LAYOUT_CHILD_KIND.fixed: {
        const previousFixed = past as typeof next;
        if (previousFixed.size !== next.size) return false;
        break;
      }
      case BRIDGE_LAYOUT_CHILD_KIND.flexMax:
      case BRIDGE_LAYOUT_CHILD_KIND.contentMax: {
        const previousCapped = past as typeof next;
        if (previousCapped.maxRows !== next.maxRows) return false;
        break;
      }
    }
  }
  return true;
}

// --- Factory helpers (axis entries below). ---------------------------------

/** Lowers static View.contentMax(maxRows, child). */
export function composeContentMax(maxRows: number, child: View): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.contentMax(maxRows, child);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.contentMax && node.maxRows === maxRows && node.child === nodeForBridge(child)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.contentMax(maxRows, child));
  stageFresh(slot, view);
  return view;
}

/** Lowers base.container(). */
export function composeContainer(base: View): View {
  const scope = executionContext.top;
  if (scope === undefined) return base.container();
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.container && node.child === nodeForBridge(base)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => base.container());
  stageFresh(slot, view);
  return view;
}

/** Lowers base.clampRows(maxRows, overflow). */
export function composeClampRows(
  base: View,
  maxRows: number,
  overflow: OverflowIndicator = { kind: "none" },
): View {
  const scope = executionContext.top;
  if (scope === undefined) return base.clampRows(maxRows, overflow);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (
      node.kind === BRIDGE_VIEW_KIND.clamp
      && node.maxRows === maxRows
      && node.child === nodeForBridge(base)
      && node.overflow !== undefined
      && overflowIndicatorMatches(node.overflow, overflow)
    ) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => base.clampRows(maxRows, overflow));
  stageFresh(slot, view);
  return view;
}

function overflowIndicatorMatches(bridged: BridgeOverflowIndicatorNode, overflow: OverflowIndicator): boolean {
  if (overflow.kind === "none") return bridged.kind === BRIDGE_OVERFLOW_KIND.none;
  if (overflow.kind === "ellipsis") {
    return bridged.kind === BRIDGE_OVERFLOW_KIND.ellipsis
      && styleNodesEqual(bridged.style, overflow.style.value as unknown as StyleNode);
  }
  return bridged.kind === BRIDGE_OVERFLOW_KIND.footer
    && bridged.prefix === overflow.prefix
    && styleNodesEqual(bridged.style, overflow.style.value as unknown as StyleNode);
}

/** Lowers View.grid(specification) with immediate child/track equality. */
export function composeGrid(
  specification: readonly View[] | GridSpec | ((builder: GridBuilder) => void),
): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.grid(specification);
  const slot = scope.nextSemanticSlot();
  const view = View.__rawGrid(specification);
  const previous = slot.current;
  if (previous !== undefined && gridMatches(previous, view)) {
    stageReuse(slot, previous);
    return previous;
  }
  stageFresh(slot, view);
  return view;
}

function gridMatches(previous: View, next: View): boolean {
  const past = nodeForBridge(previous);
  const current = nodeForBridge(next);
  if (past.kind !== BRIDGE_VIEW_KIND.grid || current.kind !== BRIDGE_VIEW_KIND.grid) return false;
  if (past.columnGap !== current.columnGap || past.rowGap !== current.rowGap) return false;
  if (past.columns.length !== current.columns.length || past.rows.length !== current.rows.length) return false;
  for (let index = 0; index < current.columns.length; index += 1) {
    if (!gridTrackMatches(past.columns[index]!, current.columns[index]!)) return false;
  }
  for (let rowIndex = 0; rowIndex < current.rows.length; rowIndex += 1) {
    const oldRow = past.rows[rowIndex]!;
    const newRow = current.rows[rowIndex]!;
    if (!gridTrackMatches(oldRow.track, newRow.track) || oldRow.cells.length !== newRow.cells.length) return false;
    for (let cellIndex = 0; cellIndex < newRow.cells.length; cellIndex += 1) {
      const oldCell = oldRow.cells[cellIndex]!;
      const newCell = newRow.cells[cellIndex]!;
      if (oldCell.view !== newCell.view
        || oldCell.columnSpan !== newCell.columnSpan
        || oldCell.rowSpan !== newCell.rowSpan
        || oldCell.horizontalAlign !== newCell.horizontalAlign
        || oldCell.verticalAlign !== newCell.verticalAlign) return false;
    }
  }
  return true;
}

function gridTrackMatches(a: BridgeGridTrackNode, b: BridgeGridTrackNode): boolean {
  if (a.kind !== b.kind) return false;
  switch (a.kind) {
    case BRIDGE_GRID_TRACK_KIND.content: return true;
    case BRIDGE_GRID_TRACK_KIND.contentMax:
    case BRIDGE_GRID_TRACK_KIND.flexMax:
      return a.max === (b as Extract<BridgeGridTrackNode, { kind: typeof a.kind }>).max;
    case BRIDGE_GRID_TRACK_KIND.fixed:
      return a.size === (b as Extract<BridgeGridTrackNode, { kind: typeof a.kind }>).size;
    case BRIDGE_GRID_TRACK_KIND.flex: return true;
  }
}

/**
 * Lowers View.diff(hunks) (§18.7): diff payloads have no cheap immediate
 * equality, so composition stages a fresh immutable View every evaluation —
 * but ALWAYS consumes a slot to keep the scope cursor aligned across renders.
 */
export function composeDiff(hunks: readonly DiffHunkNode[]): View {
  const scope = executionContext.top;
  const view = scope === undefined ? View.diff(hunks) : withoutRetainedComposition(() => View.diff(hunks));
  if (scope !== undefined) {
    const slot = scope.nextSemanticSlot();
    stageFresh(slot, view);
  }
  return view;
}

// --- Modifier helpers. ------------------------------------------------------

/** Lowers base.fillWidth(). */
export function composeFillWidth(base: View): View {
  return applyDecoration(base, MOD_FILL_WIDTH);
}

/** Lowers base.fitWidth(). */
export function composeFitWidth(base: View): View {
  return applyDecoration(base, MOD_FIT_WIDTH);
}

/** Lowers base.fillHeight(). */
export function composeFillHeight(base: View): View {
  return applyDecoration(base, MOD_FILL_HEIGHT);
}

/** Lowers base.fitHeight(). */
export function composeFitHeight(base: View): View {
  return applyDecoration(base, MOD_FIT_HEIGHT);
}

/** Lowers base.minWidth(value). */
export function composeMinWidth(base: View, value: number): View {
  return applyDecoration(base, MOD_MIN_WIDTH, value);
}

/** Lowers base.maxWidth(value). */
export function composeMaxWidth(base: View, value: number): View {
  return applyDecoration(base, MOD_MAX_WIDTH, value);
}

/** Lowers base.minHeight(value). */
export function composeMinHeight(base: View, value: number): View {
  return applyDecoration(base, MOD_MIN_HEIGHT, value);
}

/** Lowers base.maxHeight(value). */
export function composeMaxHeight(base: View, value: number): View {
  return applyDecoration(base, MOD_MAX_HEIGHT, value);
}

/** Lowers base.padding(value). */
export function composePadding(base: View, value: number | Insets): View {
  return applyDecoration(base, MOD_PADDING, value);
}

/** Lowers base.foreground(color). */
export function composeForeground(base: View, color: ColorNode): View {
  return applyDecoration(base, MOD_FOREGROUND, color);
}

/** Lowers base.background(color). */
export function composeBackground(base: View, color: ColorNode): View {
  return applyDecoration(base, MOD_BACKGROUND, color);
}

/** Lowers base.style(spec). */
export function composeStyle(base: View, spec: StyleSpec): View {
  return applyDecoration(base, MOD_STYLE_SPEC, spec);
}

/** Lowers base.styleState(key, value). */
export function composeStyleState(base: View, key: string, value: string): View {
  return applyDecoration(base, MOD_STYLE_STATE, key, value);
}

/** Lowers base.textAttribute(name) — bold/dim/italic/strikethrough family. */
export function composeTextAttribute(base: View, name: string, enabled = true): View {
  return applyDecoration(base, MOD_TEXT_ATTRIBUTE, name, enabled);
}

/** Lowers base.border(spec). */
export function composeBorder(base: View, border: BorderNode): View {
  return applyDecoration(base, MOD_BORDER, border);
}

/**
 * Lowers base.wrap(mode)/base.noWrap(): a text-layout patch mirroring the
 * public method's three shapes — plain text, decorated-wrapped text, and the
 * pass-through for non-text bases.
 */
export function composeWrap(base: View, mode: WrapMode): View {
  return composeLayoutPatch(base, mode, undefined);
}

/** Lowers base.textAlign(align). */
export function composeTextAlign(base: View, align: HorizontalAlign): View {
  return composeLayoutPatch(base, undefined, align);
}

function composeLayoutPatch(base: View, wrapMode: WrapMode | undefined, alignMode: HorizontalAlign | undefined): View {
  const scope = executionContext.top;
  if (scope === undefined) {
    if (wrapMode !== undefined) return base.wrap(wrapMode);
    if (alignMode !== undefined) return base.textAlign(alignMode);
    return base;
  }
  // Comparators work on canonical numeric codes; the build path passes the
  // original string modes to the public methods unchanged.
  const wrap = wrapMode === undefined ? undefined : BRIDGE_WRAP_MODE[wrapMode];
  const align = alignMode === undefined ? undefined : BRIDGE_HORIZONTAL_ALIGN[alignMode];
  const slot = scope.nextSemanticSlot();
  const baseNode = nodeForBridge(base);
  const previous = slot.current;
  if (previous !== undefined && layoutPatchMatches(previous, baseNode, wrap, align)) {
    stageReuse(slot, previous);
    return previous;
  }
  let view: View;
  if (wrapMode !== undefined) {
    view = withoutRetainedComposition(() => base.wrap(wrapMode));
  } else if (alignMode !== undefined) {
    view = withoutRetainedComposition(() => base.textAlign(alignMode));
  } else {
    view = base;
  }
  stageFresh(slot, view);
  return view;
}

function layoutPatchMatches(
  previous: View,
  baseNode: BridgeViewNode,
  wrap: number | undefined,
  align: number | undefined,
): boolean {
  if (baseNode.kind === BRIDGE_VIEW_KIND.text) {
    // Patch spreads the base text node: payload identity (the frozen spans
    // array) plus the untouched layout scalar prove equality.
    const previousNode = nodeForBridge(previous);
    if (previousNode.kind !== BRIDGE_VIEW_KIND.text) return false;
    return previousNode.spans === baseNode.spans
      && previousNode.align === (align ?? baseNode.align)
      && previousNode.wrap === (wrap ?? baseNode.wrap);
  }
  if (baseNode.kind === BRIDGE_VIEW_KIND.decorated && baseNode.child.kind === BRIDGE_VIEW_KIND.text) {
    const previousNode = nodeForBridge(previous);
    if (previousNode.kind !== BRIDGE_VIEW_KIND.decorated) return false;
    if (previousNode.child === baseNode.child) return wrap === undefined || baseNode.child.wrap === wrap;
    if (previousNode.child.kind !== BRIDGE_VIEW_KIND.text) return false;
    return previousNode.child.spans === baseNode.child.spans
      && previousNode.child.align === baseNode.child.align
      && previousNode.child.wrap === (wrap ?? baseNode.child.wrap)
      && decorationFullyEqual(previousNode.decoration, baseNode.decoration);
  }
  // Non-text bases pass through unchanged: the composed result IS the base.
  return nodeForBridge(previous) === baseNode;
}
