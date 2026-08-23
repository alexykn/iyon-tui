/**
 * PERF-12 T13.1 Step 3 — internal monomorphic semantic compose helpers
 * (§12/§17/§18/§19).
 *
 * These are the functions the Step 4 source transform lowers recognized
 * `View.*` factories and modifier chains to:
 *
 *   View.text(t)            ->  composeText(moduleId, siteId, t)
 *   base.fillWidth()        ->  composeFillWidth(moduleId, siteId, base)
 *   View.vertical(build)    ->  composeVertical(moduleId, siteId, build)
 *
 * Contract for every helper:
 *   1. no active composition pass -> ordinary construction, identical
 *      semantics (§19 fall-through);
 *   2. active pass -> resolve this lexical site's slot, compare the new
 *      operation's raw arguments against the previous committed node's
 *      immediate semantic fields BEFORE allocating anything (§19), return the
 *      exact previous View on match (§17), otherwise construct one new
 *      immutable View;
 *   3. children compare by BridgeViewNode identity — they were themselves
 *      composed, so identity IS the shallow equality proof (§17.1);
 *   4. no closures/rest arrays/string dispatch/reflection dispatch on the hot
 *      path (§52.8); modifier deltas are tagged with small integer constants
 *      that are compile-time constants at every lowered call site.
 *
 * Comparators operate on frozen normalized bridge records (§18.5). The only
 * property-name loops are bounded comparisons over tiny normalized style or
 * style-state records inside comparators (a handful of keys), never tree
 * walks or generic reflection.
 */

import {
  activeCompositionPass,
  noteExactViewReuse,
  noteNewView,
  slotReuseCandidate,
  stageSlotValue,
} from "./composition.ts";
import {
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
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeViewNode,
  type ColorNode,
  type DecorationNode,
  type DiffHunkNode,
  type StyleNode,
} from "./ir.ts";
import { ChildrenBuilder, View, nodeForBridge, type OverflowIndicator } from "./values/view.ts";
import { insets } from "./values/geometry.ts";
import type { Insets } from "./values/geometry.ts";
import type { HorizontalAlign, TextSpan, WrapMode } from "./values/text.ts";
import type { StyleSpec } from "./values/style.ts";
import type { NativeHandleId } from "./types.ts";

type ViewValue = View;

/** Component-handle contract mirror of View.component's parameter. */
interface ComponentHandleLike {
  readonly id: NativeHandleId;
  nativeComponentId?: () => number | undefined;
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
      if (!colorEqual(dec.style.theme, inherited.style.theme)) return false;
      if (!colorEqual(dec.style.foreground, inherited.style.foreground)) return false;
      if (!colorEqual(dec.style.background, inherited.style.background)) return false;
      const inheritedAttributes = inherited.style.attributes;
      const actual = dec.style.attributes;
      let inheritedCount = 0;
      for (const key in inheritedAttributes) {
        if (actual[key] !== inheritedAttributes[key]) return false;
        inheritedCount += 1;
      }
      let actualCount = 0;
      for (const _key in actual) actualCount += 1;
      return actualCount === inheritedCount + 1 && actual[name] === true;
    }
    case MOD_STYLE_STATE: {
      const key = a as string;
      const value = b as string;
      const inheritedStates = inherited.styleStates;
      const actual = dec.styleStates;
      if (actual === undefined || actual[key] !== value) return false;
      let inheritedCount = inheritedStates === undefined ? 0 : 0;
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
 * Shared modifier engine. `applyDirect` uses ONLY public View methods so T9
 * derivation hints and validation behave identically to uncomposed code.
 */
function applyDecoration(
  moduleId: number,
  siteId: number,
  base: ViewValue,
  tag: number,
  a?: unknown,
  b?: unknown,
): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return applyDecorationDirect(base, tag, a, b);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const previousNode = nodeForBridge(previous);
    if (previousNode.kind === BRIDGE_VIEW_KIND.decorated) {
      const baseNode = nodeForBridge(base);
      // decorate() flattens decorated bases: the result wraps the INNER child
      // and clones the base's accumulated decoration as the merge input.
      const inner = baseNode.kind === BRIDGE_VIEW_KIND.decorated ? baseNode.child : baseNode;
      const inherited = baseNode.kind === BRIDGE_VIEW_KIND.decorated ? baseNode.decoration : emptyDecoration();
      if (previousNode.child === inner && decorationDeltaMatches(previousNode.decoration, inherited, tag, a, b)) {
        stageSlotValue(slot, previous);
        noteExactViewReuse();
        return previous;
      }
    }
  }
  const view = applyDecorationDirect(base, tag, a, b);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

function applyDecorationDirect(base: ViewValue, tag: number, a: unknown, b: unknown): ViewValue {
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
    case MOD_TEXT_ATTRIBUTE: return base.textAttribute(a as string);
    case MOD_STYLE_STATE: return base.styleState(a as string, b as string);
    case MOD_BORDER: return base.border(a as BorderNode);
    default:
      throw new RangeError(`unknown decoration modifier ${tag}`);
  }
}

// --- Factory helpers. -------------------------------------------------------

/** Lowers View.text(content). */
export function composeText(moduleId: number, siteId: number, content: string): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return View.text(content);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
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
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.text(content);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/** Lowers View.styledText(spans). */
export function composeStyledText(moduleId: number, siteId: number, spans: readonly TextSpan[]): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return View.styledText(spans);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.text && styledSpansMatch(node.spans, spans)) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.styledText(spans);
  stageSlotValue(slot, view);
  noteNewView();
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
export function composeSpacer(moduleId: number, siteId: number, rows: number): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return View.spacer(rows);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.spacer && node.rows === rows) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.spacer(rows);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/** Lowers View.component(handle). */
export function composeComponent(moduleId: number, siteId: number, handle: ComponentHandleLike): ViewValue {
  const componentId = (handle.nativeComponentId?.() ?? handle.id) as NativeHandleId;
  const pass = activeCompositionPass();
  if (pass === undefined) return View.component(handle);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.component && node.handle === componentId) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.component(handle);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/** Lowers View.hanging(prefix, continuation, body). */
export function composeHanging(
  moduleId: number,
  siteId: number,
  prefix: ViewValue,
  continuation: ViewValue,
  body: ViewValue,
): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return View.hanging(prefix, continuation, body);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (
      node.kind === BRIDGE_VIEW_KIND.hanging
      && node.prefix === nodeForBridge(prefix)
      && node.continuation === nodeForBridge(continuation)
      && node.body === nodeForBridge(body)
    ) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.hanging(prefix, continuation, body);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/**
 * Lowers View.vertical(build)/View.horizontal(build) (§12.4). The builder
 * executes FIRST against our own builder instance so children flow through
 * their own composition slots; the container then compares immediate
 * semantics (entry count/kinds/scalars/child identities/gap) before
 * allocating a parent.
 */
export function composeAxis(
  moduleId: number,
  siteId: number,
  row: boolean,
  build: (children: ChildrenBuilder) => void,
): ViewValue {
  const builder = new ChildrenBuilder();
  const pass = activeCompositionPass();
  build(builder);
  const entries = builder.children;
  const gap = builder.gapValue();
  if (pass === undefined) return View.__composedAxis(row, entries, gap);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined && axisMatches(previous, row, entries, gap)) {
    stageSlotValue(slot, previous);
    noteExactViewReuse();
    return previous;
  }
  const view = View.__composedAxis(row, entries, gap);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

export function composeVertical(
  moduleId: number,
  siteId: number,
  build: (children: ChildrenBuilder) => void,
): ViewValue {
  return composeAxis(moduleId, siteId, false, build);
}

export function composeHorizontal(
  moduleId: number,
  siteId: number,
  build: (children: ChildrenBuilder) => void,
): ViewValue {
  return composeAxis(moduleId, siteId, true, build);
}

function axisMatches(previous: ViewValue, row: boolean, entries: readonly BridgeLayoutChild[], gap: number): boolean {
  const node = nodeForBridge(previous);
  const expectedKind = row ? BRIDGE_VIEW_KIND.row : BRIDGE_VIEW_KIND.column;
  if (node.kind !== expectedKind) return false;
  // Wide sequence-backed axes carry PersistentSeq overrides; comparing their
  // lazy children would flatten them (§22.3). Rebuild instead of guessing —
  // correctness first; production chrome never reaches widths here.
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

/** Lowers static View.contentMax(maxRows, child). */
export function composeContentMax(moduleId: number, siteId: number, maxRows: number, child: ViewValue): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return View.contentMax(maxRows, child);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.contentMax && node.maxRows === maxRows && node.child === nodeForBridge(child)) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.contentMax(maxRows, child);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/** Lowers base.container(). */
export function composeContainer(moduleId: number, siteId: number, base: ViewValue): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return base.container();
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.container && node.child === nodeForBridge(base)) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = base.container();
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/** Lowers base.clampRows(maxRows, overflow). */
export function composeClampRows(
  moduleId: number,
  siteId: number,
  base: ViewValue,
  maxRows: number,
  overflow: OverflowIndicator = { kind: "none" },
): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) return base.clampRows(maxRows, overflow);
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (
      node.kind === BRIDGE_VIEW_KIND.clamp
      && node.maxRows === maxRows
      && node.child === nodeForBridge(base)
      && node.overflow !== undefined
      && overflowIndicatorMatches(node.overflow, overflow)
    ) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = base.clampRows(maxRows, overflow);
  stageSlotValue(slot, view);
  noteNewView();
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

/**
 * Lowers View.diff(hunks) (§18.7): Diff payloads have no cheap immediate
 * equality, so composition stages a fresh immutable View every evaluation and
 * lets the specialized retained Diff lane absorb the cost.
 */
export function composeDiff(moduleId: number, siteId: number, hunks: readonly DiffHunkNode[]): ViewValue {
  const view = View.diff(hunks);
  const pass = activeCompositionPass();
  if (pass !== undefined) {
    const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
    stageSlotValue(slot, view);
  }
  noteNewView();
  return view;
}

// --- Modifier helpers. ------------------------------------------------------

/** Lowers base.fillWidth(). */
export function composeFillWidth(moduleId: number, siteId: number, base: ViewValue): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_FILL_WIDTH);
}

/** Lowers base.fitWidth(). */
export function composeFitWidth(moduleId: number, siteId: number, base: ViewValue): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_FIT_WIDTH);
}

/** Lowers base.fillHeight(). */
export function composeFillHeight(moduleId: number, siteId: number, base: ViewValue): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_FILL_HEIGHT);
}

/** Lowers base.fitHeight(). */
export function composeFitHeight(moduleId: number, siteId: number, base: ViewValue): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_FIT_HEIGHT);
}

/** Lowers base.minWidth(value). */
export function composeMinWidth(moduleId: number, siteId: number, base: ViewValue, value: number): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_MIN_WIDTH, value);
}

/** Lowers base.maxWidth(value). */
export function composeMaxWidth(moduleId: number, siteId: number, base: ViewValue, value: number): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_MAX_WIDTH, value);
}

/** Lowers base.minHeight(value). */
export function composeMinHeight(moduleId: number, siteId: number, base: ViewValue, value: number): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_MIN_HEIGHT, value);
}

/** Lowers base.maxHeight(value). */
export function composeMaxHeight(moduleId: number, siteId: number, base: ViewValue, value: number): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_MAX_HEIGHT, value);
}

/** Lowers base.padding(value). */
export function composePadding(
  moduleId: number,
  siteId: number,
  base: ViewValue,
  value: number | Insets,
): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_PADDING, value);
}

/** Lowers base.foreground(color). */
export function composeForeground(moduleId: number, siteId: number, base: ViewValue, color: ColorNode): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_FOREGROUND, color);
}

/** Lowers base.background(color). */
export function composeBackground(moduleId: number, siteId: number, base: ViewValue, color: ColorNode): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_BACKGROUND, color);
}

/** Lowers base.style(spec). */
export function composeStyle(moduleId: number, siteId: number, base: ViewValue, spec: StyleSpec): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_STYLE_SPEC, spec);
}

/** Lowers base.styleState(key, value). */
export function composeStyleState(
  moduleId: number,
  siteId: number,
  base: ViewValue,
  key: string,
  value: string,
): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_STYLE_STATE, key, value);
}

/** Lowers base.textAttribute(name) — bold/dim/italic/strikethrough family. */
export function composeTextAttribute(moduleId: number, siteId: number, base: ViewValue, name: string): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_TEXT_ATTRIBUTE, name);
}

/** Lowers base.border(spec). */
export function composeBorder(moduleId: number, siteId: number, base: ViewValue, border: BorderNode): ViewValue {
  return applyDecoration(moduleId, siteId, base, MOD_BORDER, border);
}

/**
 * Lowers base.wrap(mode)/base.noWrap(): a text-layout patch. Mirrors the
 * public method's three shapes — plain text, decorated-wrapped text, and the
 * pass-through for non-text bases.
 */
export function composeWrap(moduleId: number, siteId: number, base: ViewValue, mode: WrapMode): ViewValue {
  return composeLayoutPatch(moduleId, siteId, base, mode, undefined);
}

/** Lowers base.textAlign(align). */
export function composeTextAlign(moduleId: number, siteId: number, base: ViewValue, align: HorizontalAlign): ViewValue {
  return composeLayoutPatch(moduleId, siteId, base, undefined, align);
}

function composeLayoutPatch(
  moduleId: number,
  siteId: number,
  base: ViewValue,
  wrapMode: WrapMode | undefined,
  alignMode: HorizontalAlign | undefined,
): ViewValue {
  const pass = activeCompositionPass();
  if (pass === undefined) {
    if (wrapMode !== undefined) return base.wrap(wrapMode);
    if (alignMode !== undefined) return base.textAlign(alignMode);
    return base;
  }
  // Comparator works on canonical numeric codes; the build path passes the
  // original string modes to the public methods unchanged.
  const wrap = wrapMode === undefined ? undefined : BRIDGE_WRAP_MODE[wrapMode];
  const align = alignMode === undefined ? undefined : BRIDGE_HORIZONTAL_ALIGN[alignMode];
  const slot = pass.root.currentPositionalSlot(pass, moduleId, siteId);
  const baseNode = nodeForBridge(base);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined && layoutPatchMatches(previous, baseNode, wrap, align)) {
    stageSlotValue(slot, previous);
    noteExactViewReuse();
    return previous;
  }
  if (wrapMode !== undefined) {
    const view = base.wrap(wrapMode);
    stageSlotValue(slot, view);
    noteNewView();
    return view;
  }
  if (alignMode === undefined) return base;
  const view = base.textAlign(alignMode);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

function layoutPatchMatches(
  previous: ViewValue,
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
