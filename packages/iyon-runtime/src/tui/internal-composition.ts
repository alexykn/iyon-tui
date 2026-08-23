/**
 * PERF-12 T13.1 — PRIVATE composition surface for transform-injected helpers.
 *
 * @internal This module is framework build-support infrastructure. It is NOT
 * part of the public `iyon-tui` API (§33): applications must never import it.
 * The T13.1 source transform (Step 4) emits imports from here into transformed
 * consumer modules, and the monomorphic compose helpers (Step 3) are layered
 * on top of these primitives.
 */

import {
  activeCompositionPass,
  noteExactViewReuse,
  noteNewView,
  notePredecessorHint,
  noteUntransformedFallback,
  popCompositionPass,
  pushCompositionPass,
  slotReuseCandidate,
  stageSlotValue,
  withCompositionScope,
  type CompositionSlot,
  type ViewCompositionPass,
} from "./composition.ts";

export { registerCompositionModule, compositionModuleSiteCount } from "./composition_registry.ts";
export {
  activeCompositionPass,
  noteExactViewReuse,
  noteNewView,
  notePredecessorHint,
  noteUntransformedFallback,
  popCompositionPass,
  pushCompositionPass,
  slotReuseCandidate,
  stageSlotValue,
  withCompositionScope,
};
export type { CompositionSlot, ViewCompositionPass };

/**
 * Resolves the active pass without pushing a scope: the entry helper used by
 * every lowered factory before any slot work (`undefined` means "no
 * composition pass is active — construct ordinarily", §19 fall-through).
 */
export function currentCompositionPass(): ViewCompositionPass | undefined {
  return activeCompositionPass();
}
