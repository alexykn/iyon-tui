/**
 * PERF-12 T13.1 R8 — active execution/child-owner context
 * (handoff §32.2.8).
 *
 * Lower-level than `execution.ts` on purpose: this module must never import
 * `View`, so `values/view.ts` can implement `View.key` through
 * {@link withKeyedChildOwner} without import cycles or monkey-patching.
 *
 * Two contexts are tracked:
 *
 *   ACTIVE_EXECUTION_SCOPE — whose semantic slots / dependencies apply
 *                            (unchanged inside View.key thunks);
 *   ACTIVE_CHILD_OWNER     — where component invocations reconcile
 *                            (= the scope's child-owner normally; swapped to
 *                            a keyed group's child-owner inside View.key).
 *
 * Also hosts the protocol reentrancy flag: builder-boundary mutations
 * (setView/setContent/render-builder) from inside a running retained
 * protocol reject deterministically (§32.2.7).
 */

import type { RetainedExecutionScope } from "./execution.ts";
import { KeyGroup, type ChildOwnerState, type ViewKey } from "./child-owner.ts";

interface ActiveFrame {
  readonly scope: RetainedExecutionScope;
  owner: ChildOwnerState;
}

const frameStack: ActiveFrame[] = [];

/** Stable cell for hot-path reads (same pattern as the R1 gate requires). */
export const executionContext: {
  top: RetainedExecutionScope | undefined;
  childOwner: ChildOwnerState | undefined;
} = { top: undefined, childOwner: undefined };

/**
 * True while the retained protocol (evaluate/prepare/commit) is running.
 * Builder-boundary user mutations consult this and reject deterministically.
 */
export const protocolState = { mutating: false };

export function activeExecutionScope(): RetainedExecutionScope | undefined {
  return executionContext.top;
}

export function activeChildOwner(): ChildOwnerState | undefined {
  return executionContext.childOwner;
}

export function pushActiveFrame(scope: RetainedExecutionScope): void {
  frameStack.push({ scope, owner: scope.owner });
  syncTop();
}

function syncTop(): void {
  const top = frameStack[frameStack.length - 1];
  executionContext.top = top?.scope;
  executionContext.childOwner = top?.owner;
}

export function popActiveFrame(scope: RetainedExecutionScope): void {
  const popped = frameStack.pop();
  if (popped === undefined || popped.scope !== scope) {
    throw new Error("TUI_EXECUTION_CONTEXT_CORRUPTED");
  }
  syncTop();
}

/**
 * Resolves (or creates) the keyed group for `key` under `owner`, rejecting
 * duplicate use within the same pass, then runs `build` with the ACTIVE
 * CHILD OWNER pointed at the group. The active EXECUTION scope is untouched:
 * State reads and semantic slots still belong to the enclosing scope — key
 * groups own identity only (AMENDMENT-C §32.2.5 / handoff §16).
 */
export function withKeyedChildOwner<T>(owner: ChildOwnerState, key: ViewKey, build: () => T): T {
  const frame = frameStack[frameStack.length - 1];
  if (frame === undefined) {
    throw new Error("TUI_EXECUTION_NO_ACTIVE_SCOPE");
  }
  // Duplicate detection lives in the WIP map: presence in pendingKeyed means
  // "already seen this pass" \u2014 committed groups never enter it implicitly.
  if (owner.pendingKeyed?.has(key)) {
    throw new Error(`TUI_EXECUTION_DUPLICATE_KEY: key ${String(key)} already used in this pass`);
  }
  const existing = owner.committedKeyed?.get(key);
  const group = existing ?? new KeyGroup(key);
  (owner.pendingKeyed ??= new Map()).set(key, group);
  // Fresh WIP for the group's child stream on every entry (reused groups too).
  group.owner.beginChildPass();

  const previous = frame.owner;
  frame.owner = group.owner;
  syncTop();
  try {
    return build();
  } finally {
    frame.owner = previous;
    syncTop();
  }
}
