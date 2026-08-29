import type { View } from "../api/view/view.ts";
import type { RetainedExecutionScope } from "./execution.ts";

/**
 * One prepared structural publication. All ordinary fallibility belongs to
 * preparation; commit only promotes already-prepared state and abort releases
 * staged resources without changing the committed frame.
 */
export interface PreparedStructuralPublication {
  commit(): void;
  abort(): void;
}

/**
 * Structural target owned by a runtime or control boundary. Composition knows
 * only the semantic View and the prepare/commit/abort protocol; native refs,
 * bridge records, leases, and host objects stay behind the target.
 */
export interface StructuralPublicationTarget {
  /** Returns undefined when the enclosing publication batch must abort. */
  preparePublication(output: View): PreparedStructuralPublication | undefined;
  /** Allows target-owned sideband state to request publication on identity hits. */
  needsPublication?(output: View): boolean;
}

/**
 * Stable semantic projection of a child execution scope into its parent.
 * Structural target ownership remains private to the projection creator.
 */
export interface StructuralScopeProjection {
  readonly view: View;
  readonly target: StructuralPublicationTarget;
  dispose(): void;
}

/** Optional factory consulted when a child scope mounts. */
export type StructuralScopeProjectionFactory = (
  scope: RetainedExecutionScope<never>,
) => StructuralScopeProjection | undefined;
