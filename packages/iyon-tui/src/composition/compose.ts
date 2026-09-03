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
 *   3. children compare by SemanticViewNode identity — they were themselves
 *      composed inside their own scopes/slots, so identity IS the shallow
 *      equality proof (§17.1);
 *   4. no rest arrays/string dispatch/reflection on the hot path (§52.8);
 *      modifier deltas use small integer tags.
 *
 * Comparators operate on frozen normalized semantic records (§18). The only
 * property-name loops are bounded comparisons over tiny normalized style /
 * style-state records, never tree walks or generic reflection.
 */

import { executionContext, executionCounters } from "./execution.ts";
import { withoutRetainedComposition } from "./execution-context.ts";
import {
  semanticBorderFor,
  semanticColorFor,
  semanticDecorationFor,
  semanticEmptyStyle,
  semanticMergeStyles,
  semanticStyleFor,
} from "../api/presentation/semantic-style.ts";
import {
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  SEMANTIC_VIEW_KIND,
  semanticNodeOf,
  type SemanticBorder,
  type SemanticColor,
  type SemanticDecoration,
  type SemanticGridTrack,
  type SemanticLayoutChild,
  type SemanticOverflowIndicator,
  type SemanticStyle,
  type SemanticViewNode,
} from "../api/view/semantic-node.ts";
import {
  attachStateForComposition,
  ChildrenBuilder,
  componentViewForHandle,
  composedAxis,
  GridBuilder,
  gridBuilderFromSpecification,
  gridViewFromBuilder,
  View,
  type GridSpec,
  type GridTrack,
  type LayoutChild,
  type OverflowIndicator,
} from "../api/view/view.ts";
import { insets } from "../api/view/geometry.ts";
import type { Insets } from "../api/view/geometry.ts";
import type { HorizontalAlign, TextSpan, WrapMode } from "../api/content/text.ts";
import type { DiffHunk } from "../api/content/diff.ts";
import type { StyleRef, StyleSpec, BorderSpec, TextAttribute } from "../api/presentation/style.ts";
import type { ColorSpec } from "../api/presentation/theme.ts";
import type { VerticalAlign } from "../api/view/view.ts";
import type { ComponentHandle, HandleId } from "../api/controls/framework-handle.ts";
import type { ContentPort } from "../api/content/retained.ts";

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

// --- Field comparators over normalized semantic records (§18). -------------

function colorEqual(a: SemanticColor | undefined, b: SemanticColor | undefined): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined || a.kind !== b.kind) return false;
  switch (a.kind) {
    case "theme": return a.key === (b as Extract<SemanticColor, { kind: "theme" }>).key;
    case "named": return a.value === (b as Extract<SemanticColor, { kind: "named" }>).value;
    case "indexed": return a.value === (b as Extract<SemanticColor, { kind: "indexed" }>).value;
    case "rgb": {
      const other = b as Extract<SemanticColor, { kind: "rgb" }>;
      return a.r === other.r && a.g === other.g && a.b === other.b;
    }
  }
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

function styleNodesEqual(a: SemanticStyle | undefined, b: SemanticStyle | undefined): boolean {
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

function borderEqual(a: SemanticBorder | undefined, b: SemanticBorder | undefined): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined) return false;
  if (a.style !== b.style || a.edges !== b.edges || !colorEqual(a.color, b.color)) return false;
  const aGlyphs = a.glyphs;
  const bGlyphs = b.glyphs;
  if (aGlyphs === bGlyphs) return true;
  if (aGlyphs === undefined || bGlyphs === undefined) return false;
  return aGlyphs.top === bGlyphs.top
    && aGlyphs.right === bGlyphs.right
    && aGlyphs.bottom === bGlyphs.bottom
    && aGlyphs.left === bGlyphs.left
    && aGlyphs.topLeft === bGlyphs.topLeft
    && aGlyphs.topRight === bGlyphs.topRight
    && aGlyphs.bottomLeft === bGlyphs.bottomLeft
    && aGlyphs.bottomRight === bGlyphs.bottomRight;
}

/**
 * Full structural equality of two decoration records (used when a modifier
 * inherits the ENTIRE previous chain state unchanged, e.g. layout patches
 * over decorated text).
 */
function decorationFullyEqual(a: SemanticDecoration, b: SemanticDecoration): boolean {
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
  dec: SemanticDecoration,
  inherited: SemanticDecoration,
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
  if (tag !== MOD_STYLE_SPEC && tag !== MOD_TEXT_ATTRIBUTE && tag !== MOD_FOREGROUND
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
    case MOD_FOREGROUND: {
      const expected = semanticMergeStyles(inherited.style, { ...semanticEmptyStyle(), foreground: semanticColorFor(a as ColorSpec) });
      return styleNodesEqual(dec.style, expected);
    }
    case MOD_BACKGROUND: return colorEqual(dec.background, semanticColorFor(a as ColorSpec));
    case MOD_BORDER: return borderEqual(dec.border, semanticBorderFor(a as BorderSpec));
    case MOD_STYLE_SPEC: {
      const style = a as StyleRef | StyleSpec;
      const lowered = semanticMergeStyles(semanticEmptyStyle(), semanticStyleFor(style));
      const replacesStyle = style.kind === "style-ref" && style.themeKey !== undefined;
      const expected = replacesStyle
        ? lowered
        : semanticMergeStyles(inherited.style, lowered);
      return styleNodesEqual(dec.style, expected);
    }
    case MOD_TEXT_ATTRIBUTE: {
      const name = a as TextAttribute;
      const value = (b as boolean | undefined) ?? true;
      if (dec.style.theme !== inherited.style.theme) return false;
      if (!colorEqual(dec.style.foreground, inherited.style.foreground)) return false;
      if (!colorEqual(dec.style.background, inherited.style.background)) return false;
      const inheritedAttributes = inherited.style.attributes as Readonly<Record<string, boolean>>;
      const actual = dec.style.attributes as Readonly<Record<string, boolean>>;
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
    const previousNode = semanticNodeOf(previous);
    if (previousNode.kind === SEMANTIC_VIEW_KIND.decorated) {
      const baseNode = semanticNodeOf(base);
      // decorate() flattens decorated bases: the result wraps the INNER child
      // and clones the base's accumulated decoration as the merge input.
      const inner = baseNode.kind === SEMANTIC_VIEW_KIND.decorated ? baseNode.child : baseNode;
      const inherited = baseNode.kind === SEMANTIC_VIEW_KIND.decorated ? baseNode.decoration : emptySemanticDecoration();
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
      case MOD_FOREGROUND: return base.foreground(a as ColorSpec);
      case MOD_BACKGROUND: return base.background(a as ColorSpec);
      case MOD_STYLE_SPEC: return base.style(a as StyleRef | StyleSpec);
      case MOD_TEXT_ATTRIBUTE: return base.textAttribute(a as TextAttribute, (b as boolean | undefined) ?? true);
      case MOD_STYLE_STATE: return base.styleState(a as string, b as string);
      case MOD_BORDER: return base.border(a as BorderSpec);
      default:
        throw new RangeError(`unknown decoration modifier ${tag}`);
    }
  };
  return executionContext.top === undefined ? construct() : withoutRetainedComposition(construct);
}

function emptySemanticDecoration(): SemanticDecoration {
  return semanticDecorationFor();
}

// --- Factory helpers. -------------------------------------------------------

/** Constructs/reuses View.text(content). */
export function composeText(content: string): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.text(content);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (
      node.kind === SEMANTIC_VIEW_KIND.text
      && node.spans.length === 1
      && node.spans[0]!.style === undefined
      && node.spans[0]!.text === content
      && node.wrap === "wordThenGrapheme"
      && node.align === "start"
    ) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.text(content));
  stageFresh(slot, view);
  return view;
}

/** Constructs/reuses View.styledText(spans). */
export function composeStyledText(spans: readonly TextSpan[]): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.styledText(spans);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (node.kind === SEMANTIC_VIEW_KIND.text && styledSpansMatch(node.spans, spans)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.styledText(spans));
  stageFresh(slot, view);
  return view;
}

function styledSpansMatch(
  semanticSpans: readonly { readonly text: string; readonly style?: SemanticStyle }[],
  spans: readonly TextSpan[],
): boolean {
  if (semanticSpans.length !== spans.length) return false;
  for (let index = 0; index < spans.length; index += 1) {
    const span = spans[index]!.value;
    const semantic = semanticSpans[index]!;
    if (semantic.text !== span.text) return false;
    const style = span.style === undefined ? undefined : semanticStyleFor(span.style);
    if (!styleNodesEqual(semantic.style, style)) return false;
  }
  return true;
}

/** Constructs/reuses View.spacer(rows). */
export function composeSpacer(rows: number): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.spacer(rows);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (node.kind === SEMANTIC_VIEW_KIND.spacer && node.rows === rows) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.spacer(rows));
  stageFresh(slot, view);
  return view;
}

/** Reuses/constructs a semantic component occurrence by local HandleId. */
export function composeComponent(handle: ComponentHandle): View {
  const handleId = handle.id;
  const scope = executionContext.top;
  if (scope === undefined) return componentViewForHandle(handleId, handle);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (node.kind === SEMANTIC_VIEW_KIND.component && node.handleId === handleId) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => componentViewForHandle(handleId, handle));
  stageFresh(slot, view);
  return view;
}

/** Constructs/reuses one retained ContentPort attachment. */
export function composeContent(port: ContentPort): View {
  const handleId = port.id;
  const scope = executionContext.top;
  if (scope === undefined) return View.content(port);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (node.kind === SEMANTIC_VIEW_KIND.contentHost && node.contentAttachment === handleId) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.content(port));
  stageFresh(slot, view);
  return view;
}

/** Constructs/reuses one retained state attachment without a wrapper node. */
export function composeState(base: View, state: { readonly id: HandleId }): View {
  const handleId = state.id;
  const scope = executionContext.top;
  if (scope === undefined) return attachStateForComposition(base, handleId, state);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const previousNode = semanticNodeOf(previous);
    const baseNode = semanticNodeOf(base);
    if (previousNode.stateAttachment === handleId && semanticNodeMatchesWithoutState(previousNode, baseNode)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => attachStateForComposition(base, handleId, state));
  stageFresh(slot, view);
  return view;
}

/** State attachment is the only changed top-level semantic field. */
function semanticNodeMatchesWithoutState(
  left: SemanticViewNode,
  right: SemanticViewNode,
): boolean {
  if (left.kind !== right.kind) return false;
  for (const key of Object.keys(right) as (keyof SemanticViewNode)[]) {
    if (key === "id" || key === "stateAttachment") continue;
    if (left[key] !== right[key]) return false;
  }
  return true;
}

/** Constructs/reuses View.hanging(prefix, continuation, body). */
export function composeHanging(prefix: View, continuation: View, body: View): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.hanging(prefix, continuation, body);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (
      node.kind === SEMANTIC_VIEW_KIND.hanging
      && node.prefix === semanticNodeOf(prefix)
      && node.continuation === semanticNodeOf(continuation)
      && node.body === semanticNodeOf(body)
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
 * Constructs/reuses View.vertical(build)/View.horizontal(build) (§12.4). The
 * builder executes FIRST so children flow through their own slots/scopes; the
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
  if (scope === undefined) return composedAxis(row, entries, gap);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined && axisMatches(previous, row, entries, gap)) {
    stageReuse(slot, previous);
    return previous;
  }
  const view = composedAxis(row, entries, gap);
  stageFresh(slot, view);
  return view;
}

export function composeVertical(build: (children: import("../api/view/view.ts").ChildrenBuilder) => void): View {
  return composeAxisImpl(false, build);
}

export function composeHorizontal(build: (children: import("../api/view/view.ts").ChildrenBuilder) => void): View {
  return composeAxisImpl(true, build);
}

function axisMatches(previous: View, row: boolean, entries: readonly LayoutChild[], gap: number): boolean {
  const node = semanticNodeOf(previous);
  const expectedKind = row ? SEMANTIC_VIEW_KIND.row : SEMANTIC_VIEW_KIND.column;
  if (node.kind !== expectedKind) return false;
  // Wide sequence-backed axes carry semantic sequence overrides; comparing
  // their lazy children would flatten them (§22.3). Rebuild instead of guessing.
  if (peekSemanticSequenceOverride(node) !== undefined) return false;
  if (node.gap !== gap) return false;
  const previousEntries = node.children;
  if (previousEntries.length !== entries.length) return false;
  for (let index = 0; index < entries.length; index += 1) {
    const past = previousEntries[index]!;
    const next = entries[index]!;
    if (past.kind !== next.kind) return false;
    if (past.child !== semanticNodeOf(next.child)) return false;
    switch (next.kind) {
      case "fixed":
        if (past.kind !== "fixed" || past.size !== next.size) return false;
        break;
      case "flexMax":
        if (past.kind !== "flexMax" || past.maxRows !== next.maxRows) return false;
        break;
      case "contentMax":
        if (past.kind !== "contentMax" || past.maxRows !== next.maxRows) return false;
        break;
    }
  }
  return true;
}

// --- Factory helpers (axis entries below). ---------------------------------

/** Constructs/reuses static View.contentMax(maxRows, child). */
export function composeContentMax(maxRows: number, child: View): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.contentMax(maxRows, child);
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (node.kind === SEMANTIC_VIEW_KIND.contentMax && node.maxRows === maxRows && node.child === semanticNodeOf(child)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => View.contentMax(maxRows, child));
  stageFresh(slot, view);
  return view;
}

/** Constructs/reuses base.container(). */
export function composeContainer(base: View): View {
  const scope = executionContext.top;
  if (scope === undefined) return base.container();
  const slot = scope.nextSemanticSlot();
  const previous = slot.current;
  if (previous !== undefined) {
    const node = semanticNodeOf(previous);
    if (node.kind === SEMANTIC_VIEW_KIND.container && node.child === semanticNodeOf(base)) {
      stageReuse(slot, previous);
      return previous;
    }
  }
  const view = withoutRetainedComposition(() => base.container());
  stageFresh(slot, view);
  return view;
}

/** Constructs/reuses base.clampRows(maxRows, overflow). */
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
    const node = semanticNodeOf(previous);
    if (
      node.kind === SEMANTIC_VIEW_KIND.clamp
      && node.maxRows === maxRows
      && node.child === semanticNodeOf(base)
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

function overflowIndicatorMatches(semantic: SemanticOverflowIndicator, overflow: OverflowIndicator): boolean {
  if (overflow.kind === "none") return semantic.kind === "none";
  if (overflow.kind === "ellipsis") {
    return semantic.kind === "ellipsis" && styleNodesEqual(semantic.style, semanticStyleFor(overflow.style));
  }
  return semantic.kind === "footer"
    && semantic.prefix === overflow.prefix
    && styleNodesEqual(semantic.style, semanticStyleFor(overflow.style));
}

/** Constructs/reuses View.grid(specification) with immediate semantic equality. */
export function composeGrid(
  specification: readonly View[] | GridSpec | ((builder: GridBuilder) => void),
): View {
  const scope = executionContext.top;
  if (scope === undefined) return View.grid(specification);
  const slot = scope.nextSemanticSlot();
  // Normalize only the public grid input. The previous implementation built a
  // complete node and View before comparing it, which defeated the
  // retained no-op path for every grid update.
  const builder = gridBuilderFromSpecification(specification);
  const previous = slot.current;
  if (previous !== undefined && gridBuilderMatches(previous, builder)) {
    stageReuse(slot, previous);
    return previous;
  }
  const view = gridViewFromBuilder(builder);
  stageFresh(slot, view);
  return view;
}

function gridBuilderMatches(previous: View, builder: GridBuilder): boolean {
  const past = semanticNodeOf(previous);
  if (past.kind !== SEMANTIC_VIEW_KIND.grid) return false;
  // Do not force a wide grid's lazy row view to flatten merely to prove a
  // composition hit. Wide grids are intentionally rebuilt through their
  // PersistentSeq sidecar rather than scanned on the retained hot path.
  if (peekSemanticGridSequenceOverride(past) !== undefined) return false;
  if (past.columnGap !== builder.columnGapValue || past.rowGap !== builder.rowGapValue) return false;
  if (past.columns.length !== builder.columnsValue.length || past.rows.length !== builder.rows.length) return false;
  for (let index = 0; index < builder.columnsValue.length; index += 1) {
    if (!gridTrackMatchesPublic(past.columns[index]!, builder.columnsValue[index]!)) return false;
  }
  for (let rowIndex = 0; rowIndex < builder.rows.length; rowIndex += 1) {
    const oldRow = past.rows[rowIndex]!;
    const newRow = builder.rows[rowIndex]!;
    if (!gridTrackMatchesPublic(oldRow.track, newRow.track ?? { kind: "content" })
      || oldRow.cells.length !== newRow.cells.length) return false;
    for (let cellIndex = 0; cellIndex < newRow.cells.length; cellIndex += 1) {
      const oldCell = oldRow.cells[cellIndex]!;
      const newCell = newRow.cells[cellIndex]!;
      if (oldCell.view !== semanticNodeOf(newCell.view)
        || oldCell.columnSpan !== validatePositiveU16(newCell.columnSpan ?? 1, "columnSpan")
        || oldCell.rowSpan !== validatePositiveU16(newCell.rowSpan ?? 1, "rowSpan")
        || oldCell.horizontalAlign !== (newCell.horizontalAlign ?? "start")
        || oldCell.verticalAlign !== (newCell.verticalAlign ?? "top")) return false;
    }
  }
  return true;
}

function gridTrackMatchesPublic(
  semantic: SemanticGridTrack,
  track: GridTrack,
): boolean {
  switch (track.kind) {
    case "content": return semantic.kind === "content";
    case "contentMax":
      return semantic.kind === "contentMax"
        && semantic.max === validateU16(track.max, "grid track max");
    case "fixed":
      return semantic.kind === "fixed"
        && semantic.size === validateU16(track.size, "grid track size");
    case "flex": return semantic.kind === "flex";
    case "flexMax":
      return semantic.kind === "flexMax"
        && semantic.max === validateU16(track.max, "grid track max");
    default: throw new TypeError("unknown grid track kind");
  }
}

/**
 * View.diff has no cheap immediate equality, so composition stages a fresh
 * immutable View every evaluation — but ALWAYS consumes a slot to keep the
 * scope cursor aligned across renders.
 */
export function composeDiff(hunks: readonly DiffHunk[]): View {
  const scope = executionContext.top;
  const view = scope === undefined ? View.diff(hunks) : withoutRetainedComposition(() => View.diff(hunks));
  if (scope !== undefined) {
    const slot = scope.nextSemanticSlot();
    stageFresh(slot, view);
  }
  return view;
}

// --- Modifier helpers. ------------------------------------------------------

export function composeFillWidth(base: View): View { return applyDecoration(base, MOD_FILL_WIDTH); }
export function composeFitWidth(base: View): View { return applyDecoration(base, MOD_FIT_WIDTH); }
export function composeFillHeight(base: View): View { return applyDecoration(base, MOD_FILL_HEIGHT); }
export function composeFitHeight(base: View): View { return applyDecoration(base, MOD_FIT_HEIGHT); }
export function composeMinWidth(base: View, value: number): View { return applyDecoration(base, MOD_MIN_WIDTH, value); }
export function composeMaxWidth(base: View, value: number): View { return applyDecoration(base, MOD_MAX_WIDTH, value); }
export function composeMinHeight(base: View, value: number): View { return applyDecoration(base, MOD_MIN_HEIGHT, value); }
export function composeMaxHeight(base: View, value: number): View { return applyDecoration(base, MOD_MAX_HEIGHT, value); }
export function composePadding(base: View, value: number | Insets): View { return applyDecoration(base, MOD_PADDING, value); }
export function composeForeground(base: View, color: ColorSpec): View { return applyDecoration(base, MOD_FOREGROUND, color); }
export function composeBackground(base: View, color: ColorSpec): View { return applyDecoration(base, MOD_BACKGROUND, color); }
export function composeStyle(base: View, spec: StyleRef | StyleSpec): View { return applyDecoration(base, MOD_STYLE_SPEC, spec); }
export function composeStyleState(base: View, key: string, value: string): View { return applyDecoration(base, MOD_STYLE_STATE, key, value); }
export function composeTextAttribute(base: View, name: TextAttribute, enabled = true): View { return applyDecoration(base, MOD_TEXT_ATTRIBUTE, name, enabled); }
export function composeBorder(base: View, border: BorderSpec): View { return applyDecoration(base, MOD_BORDER, border); }

/** Constructs/reuses base.wrap(mode). */
export function composeWrap(base: View, mode: WrapMode): View {
  return composeLayoutPatch(base, mode, undefined);
}

/** Constructs/reuses base.textAlign(align). */
export function composeTextAlign(base: View, align: HorizontalAlign): View {
  return composeLayoutPatch(base, undefined, align);
}

function composeLayoutPatch(base: View, wrapMode: WrapMode | undefined, alignMode: HorizontalAlign | undefined): View {
  if (wrapMode !== undefined) validateWrapMode(wrapMode);
  if (alignMode !== undefined) validateHorizontalAlign(alignMode);
  const scope = executionContext.top;
  if (scope === undefined) {
    if (wrapMode !== undefined) return base.wrap(wrapMode);
    if (alignMode !== undefined) return base.textAlign(alignMode);
    return base;
  }
  const slot = scope.nextSemanticSlot();
  const baseNode = semanticNodeOf(base);
  const previous = slot.current;
  if (previous !== undefined && layoutPatchMatches(previous, baseNode, wrapMode, alignMode)) {
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
  baseNode: SemanticViewNode,
  wrap: WrapMode | undefined,
  align: HorizontalAlign | undefined,
): boolean {
  if (baseNode.kind === SEMANTIC_VIEW_KIND.text) {
    // Patch spreads the base text node: payload identity (the frozen spans
    // array) plus the untouched layout scalar prove equality.
    const previousNode = semanticNodeOf(previous);
    if (previousNode.kind !== SEMANTIC_VIEW_KIND.text) return false;
    return previousNode.spans === baseNode.spans
      && previousNode.align === (align ?? baseNode.align)
      && previousNode.wrap === (wrap ?? baseNode.wrap);
  }
  if (baseNode.kind === SEMANTIC_VIEW_KIND.decorated && baseNode.child.kind === SEMANTIC_VIEW_KIND.text) {
    const previousNode = semanticNodeOf(previous);
    if (previousNode.kind !== SEMANTIC_VIEW_KIND.decorated) return false;
    if (previousNode.child === baseNode.child) {
      // A flattened decorated base can keep the same text child while its
      // decoration changes. The layout patch preserves that decoration, so
      // child identity alone is not enough to authorize reuse here.
      return decorationFullyEqual(previousNode.decoration, baseNode.decoration)
        && (wrap === undefined || baseNode.child.wrap === wrap)
        && (align === undefined || baseNode.child.align === align);
    }
    if (previousNode.child.kind !== SEMANTIC_VIEW_KIND.text) return false;
    return previousNode.child.spans === baseNode.child.spans
      && previousNode.child.align === baseNode.child.align
      && previousNode.child.wrap === (wrap ?? baseNode.child.wrap)
      && decorationFullyEqual(previousNode.decoration, baseNode.decoration);
  }
  // Non-text bases pass through unchanged: the composed result IS the base.
  return semanticNodeOf(previous) === baseNode;
}

function validateWrapMode(mode: WrapMode): void {
  switch (mode) {
    case "wordThenGrapheme":
    case "grapheme":
    case "noWrap":
      return;
    default:
      throw new RangeError(`unknown wrap mode ${JSON.stringify(mode)}`);
  }
}

function validateHorizontalAlign(align: HorizontalAlign): void {
  switch (align) {
    case "start":
    case "center":
    case "end":
      return;
    default:
      throw new RangeError(`unknown horizontal alignment ${JSON.stringify(align)}`);
  }
}

function validateU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 65535) {
    throw new RangeError(`${name} must be an integer from 0 to 65535`);
  }
  return value;
}

function validatePositiveU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 1 || value > 65535) {
    throw new RangeError(`${name} must be an integer from 1 to 65535`);
  }
  return value;
}
