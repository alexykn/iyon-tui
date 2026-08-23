/**
 * PERF-12 T13.1 R0 — internal monomorphic compose helpers in their final
 * active-scope call shape (AMENDMENT-C §17.3, handoff §11).
 *
 * These helpers are the stable call targets for scope-local semantic
 * construction. Per AMENDMENT-C §17.4/§11 the lexical SiteId source transform
 * was abandoned: component execution scopes (`defineView`, R1+) provide the
 * composition boundary directly at runtime, so these helpers are addressed by
 * the ACTIVE EXECUTION SCOPE's dense slot cursor rather than module/site ids.
 *
 * R0 posture (handoff §32.1): no retained-execution runtime exists yet, so
 * every helper below is a pure §19 fall-through to the ordinary eager public
 * constructor/modifier. Semantics are byte-for-byte identical to uncomposed
 * construction; the only cost is one non-inlined call frame (measured by
 * bench/perf12_t13_1_r0_cold_fallthrough.ts against the ≤3% gate).
 *
 * R1 will insert the scoped arm into each helper without changing call
 * shapes: resolve the active scope's next semantic slot, compare the raw
 * arguments against the previous committed node's immediate fields BEFORE
 * allocating (§19), and return the exact previous View on match. The
 * comparator knowledge from the original Step 3 implementation lives in git
 * history (`dad92b5`) and is summarized in handoff §12.
 *
 * Contract preserved from Step 3:
 *   - fall-through must be semantically IDENTICAL to ordinary construction;
 *   - no rest arrays/string dispatch/reflection on the hot path;
 *   - modifier helpers go through ONLY public View methods so T9 derivation
 *     hints and validation behave exactly like uncomposed code;
 *   - Diff payloads stage fresh every evaluation (§18.7) — a pure delegate.
 */

import type { BorderNode, ColorNode, DiffHunkNode } from "./ir.ts";
import { View } from "./values/view.ts";
import type { Insets } from "./values/geometry.ts";
import type { HorizontalAlign, TextSpan, WrapMode } from "./values/text.ts";
import type { StyleSpec } from "./values/style.ts";
import type { NativeHandleId } from "./types.ts";

/** Component-handle contract mirror of View.component's parameter. */
interface ComponentHandleLike {
  readonly id: NativeHandleId;
  nativeComponentId?: () => number | undefined;
}

// --- Factory helpers. -------------------------------------------------------

/** Lowers View.text(content). */
export function composeText(content: string): View {
  return View.text(content);
}

/** Lowers View.styledText(spans). */
export function composeStyledText(spans: readonly TextSpan[]): View {
  return View.styledText(spans);
}

/** Lowers View.spacer(rows). */
export function composeSpacer(rows: number): View {
  return View.spacer(rows);
}

/** Lowers View.component(handle). */
export function composeComponent(handle: ComponentHandleLike): View {
  return View.component(handle);
}

/** Lowers View.hanging(prefix, continuation, body). */
export function composeHanging(prefix: View, continuation: View, body: View): View {
  return View.hanging(prefix, continuation, body);
}

/** Lowers View.vertical(build) (§12.4 container form). */
export function composeVertical(build: (children: import("./values/view.ts").ChildrenBuilder) => void): View {
  return View.vertical(build);
}

/** Lowers View.horizontal(build) (§12.4 container form). */
export function composeHorizontal(build: (children: import("./values/view.ts").ChildrenBuilder) => void): View {
  return View.horizontal(build);
}

/** Lowers static View.contentMax(maxRows, child). */
export function composeContentMax(maxRows: number, child: View): View {
  return View.contentMax(maxRows, child);
}

/** Lowers base.container(). */
export function composeContainer(base: View): View {
  return base.container();
}

/** Lowers base.clampRows(maxRows, overflow). */
export function composeClampRows(
  base: View,
  maxRows: number,
  overflow: import("./values/view.ts").OverflowIndicator = { kind: "none" },
): View {
  return base.clampRows(maxRows, overflow);
}

/**
 * Lowers View.diff(hunks) (§18.7): diff payloads have no cheap immediate
 * equality; the specialized retained Diff lane owns their cost.
 */
export function composeDiff(hunks: readonly DiffHunkNode[]): View {
  return View.diff(hunks);
}

// --- Modifier helpers (public-View-method delegation only). -----------------

/** Lowers base.fillWidth(). */
export function composeFillWidth(base: View): View {
  return base.fillWidth();
}

/** Lowers base.fitWidth(). */
export function composeFitWidth(base: View): View {
  return base.fitWidth();
}

/** Lowers base.fillHeight(). */
export function composeFillHeight(base: View): View {
  return base.fillHeight();
}

/** Lowers base.fitHeight(). */
export function composeFitHeight(base: View): View {
  return base.fitHeight();
}

/** Lowers base.minWidth(value). */
export function composeMinWidth(base: View, value: number): View {
  return base.minWidth(value);
}

/** Lowers base.maxWidth(value). */
export function composeMaxWidth(base: View, value: number): View {
  return base.maxWidth(value);
}

/** Lowers base.minHeight(value). */
export function composeMinHeight(base: View, value: number): View {
  return base.minHeight(value);
}

/** Lowers base.maxHeight(value). */
export function composeMaxHeight(base: View, value: number): View {
  return base.maxHeight(value);
}

/** Lowers base.padding(value). */
export function composePadding(base: View, value: number | Insets): View {
  return base.padding(value);
}

/** Lowers base.foreground(color). */
export function composeForeground(base: View, color: ColorNode): View {
  return base.foreground(color);
}

/** Lowers base.background(color). */
export function composeBackground(base: View, color: ColorNode): View {
  return base.background(color);
}

/** Lowers base.style(spec). */
export function composeStyle(base: View, spec: StyleSpec): View {
  return base.style(spec);
}

/** Lowers base.styleState(key, value). */
export function composeStyleState(base: View, key: string, value: string): View {
  return base.styleState(key, value);
}

/** Lowers base.textAttribute(name) — bold/dim/italic/strikethrough family. */
export function composeTextAttribute(base: View, name: string): View {
  return base.textAttribute(name);
}

/** Lowers base.border(spec). */
export function composeBorder(base: View, border: BorderNode): View {
  return base.border(border);
}

/** Lowers base.wrap(mode) / base.noWrap(). */
export function composeWrap(base: View, mode: WrapMode): View {
  return base.wrap(mode);
}

/** Lowers base.textAlign(align). */
export function composeTextAlign(base: View, align: HorizontalAlign): View {
  return base.textAlign(align);
}
