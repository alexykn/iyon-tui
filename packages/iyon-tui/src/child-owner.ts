/**
 * PERF-12 T13.1 R8 — child-owner bookkeeping (handoff §32.2.5/§32.2.8).
 *
 * One reusable ownership record shared by TWO kinds of owners:
 *
 *   - RetainedExecutionScope (real execution: slots, dependencies, scheduling);
 *   - KeyGroup (identity namespace ONLY — never execution, never scheduling).
 *
 * The WIP model: evaluation participates via `beginChildPass()` (wipActive =
 * true); pending structures collect the next version; commit promotes pending
 * over committed; abort discards pending leaving committed untouched. An owner
 * that evaluated with ZERO keyed children (wipActive && pendingKeyed empty)
 * unmounts all committed groups; an owner that never evaluated preserves them.
 */

/** Local, scope-limited composition key. Not a NodeId, not global. */
export type ViewKey = string | number;

/**
 * Marker interface so child-owner walkers can treat scopes and key groups
 * uniformly without a common base class. Satisfied structurally.
 */
export interface OwnsChildren {
  readonly owner: ChildOwnerState;
}

export class ChildOwnerState {
  /** Committed UNKEYED children, strictly positional. */
  committedChildren: ChildRecord[] = [];
  /** Pending UNKEYED children for the active pass (dense, 0..cursor-1). */
  pendingChildren: ChildRecord[] = [];
  /** Unkeyed ordinal cursor for the active pass. */
  cursor = 0;
  /** Committed keyed groups. Literally untouched during evaluation. */
  committedKeyed: Map<ViewKey, KeyGroup> | undefined;
  /** Keyed groups touched during the active pass. */
  pendingKeyed: Map<ViewKey, KeyGroup> | undefined;
  /**
   * Whether THIS owner participated in the active pass. Distinguishes
   * "evaluated with zero keyed children" (⇒ committed groups unmount) from
   * "never evaluated" (⇒ committed groups preserved).
   */
  wipActive = false;

  /** Marks participation and opens fresh WIP. Called by the walker ONLY —
   * a KeyGroup is not an execution scope; nothing here schedules anything. */
  beginChildPass(): void {
    this.pendingChildren = [];
    this.cursor = 0;
    this.pendingKeyed = undefined;
    this.wipActive = true;
  }

  /** Aborted pass: discard WIP; committed state untouched. */
  dropPending(): void {
    this.pendingChildren = [];
    this.cursor = 0;
    this.pendingKeyed = undefined;
    this.wipActive = false;
  }

  release(): void {
    this.committedChildren.length = 0;
    this.pendingChildren = [];
    this.cursor = 0;
    this.committedKeyed = undefined;
    this.pendingKeyed = undefined;
    this.wipActive = false;
  }
}

export interface ChildRecord {
  /** Component token (reference identity decides remount, §9.1). */
  readonly type: object;
  readonly key: ViewKey | undefined;
  readonly scope: RetainedExecutionScope;
}

/**
 * Identity namespace for one keyed logical instance (§32.2.5). Deliberately
 * carries NO dependencies, NO dirty flag, NO scheduler access, and NO output:
 * it owns child identity only. Independent invalidation belongs to a
 * defineView component mounted INSIDE the group.
 */
export class KeyGroup implements OwnsChildren {
  readonly owner = new ChildOwnerState();

  constructor(readonly key: ViewKey) {}
}

// Type-only reference (erased at runtime — keeps this module dependency-free).
import type { RetainedExecutionScope } from "./execution.ts";
