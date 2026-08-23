/**
 * PERF-12 T13.1 R1 — retained execution substrate (handoff §10, AMENDMENT-C
 * §4/§10/§18 Step 4R).
 *
 * Persistent execution scopes over independently retained immutable View DAG
 * roots: each scope owns one logical component instance (parent-local
 * type/position identity), its tracked dependencies (R4), its child scopes,
 * and a dense table of scope-local semantic slots that the compose helpers
 * (compose.ts) address through the ACTIVE EXECUTION SCOPE context.
 *
 * Hard boundaries (handoff §23/§4.4):
 *   - scopes store execution/lifecycle state and pointers to immutable View
 *     outputs only — never payload copies, NodeIds as identity, NativeRefs,
 *     or transport state (NOT a second semantic graph);
 *   - clean scopes never execute; replay cost is bounded by the invalidated
 *     scope (AMENDMENT-C §10.2);
 *   - evaluation is pure and synchronous: a returned Promise is rejected;
 *   - the batch protocol PREPARES all work, then COMMITS ONCE via pointer
 *     swaps/truncations; any failure aborts leaving committed state
 *     authoritative (handoff §17/§21/§27).
 *
 * Slot-lifetime rules (scoped form of handoff §22):
 *   - one committed View per visited slot plus one pending during evaluation;
 *   - commit truncates the slot tail beyond this pass's cursor (§25.3);
 *   - abort rewinds growth and drops staged pendings; committed slots are
 *     untouched;
 *   - control-flow shifts realign the dense cursor, reducing local reuse —
 *     they can never select another component instance or produce stale
 *     semantics, because immediate semantic equality authorizes reuse
 *     (AMENDMENT-C §10.1).
 *
 * R1 posture (handoff §32.1): positional child reconciliation only (keyed
 * dynamics arrive in R8); no State<T> dependency tracking (R4); no native
 * projection — a scope's committed output View IS its observable artifact
 * until R3 introduces stable ScopeRef projections. Body-execution isolation
 * across scopes IS proven here; cross-scope composite splicing arrives with
 * the projection (R3/R6a).
 */

import type { View } from "./values/view.ts";
import type { TrackedStateSource } from "./tracked-state.ts";

// --- Public-machinery types --------------------------------------------------

/** Local, scope-limited composition key. Not a NodeId, not global. */
export type ViewKey = string | number;

/**
 * Core component abstraction: a stable identity token carrying its pure,
 * synchronous render body. Identity is object REFERENCE equality —
 * `defineView` (define-view.ts) returns one object per component definition;
 * synthetic drivers create bare literals. Two structurally identical
 * literals are two DIFFERENT component types (mismatch ⇒ remount, §9.1).
 *
 * `render` uses PROPERTY (arrow) syntax deliberately: parameter types are
 * checked contravariantly, so `ViewComponent<A>` is not silently assignable
 * to `ViewComponent<B>`.
 */
export interface ViewComponentType<P = unknown> {
  readonly render: (props: P) => View;
}

/**
 * The PUBLIC component value returned by `defineView` (handoff §8): callable
 * so idiomatic usage reads `column.child(Footer({ status }))`. The call
 * performs reconciliation + scheduling inside the currently evaluating
 * parent scope; the `.render` entry is what the runtime calls when THIS
 * scope itself executes.
 */
export interface ViewComponent<P = unknown> extends ViewComponentType<P> {
  (props: P): View;
}

/**
 * Independently retained sub-DAG root for one live execution scope
 * (AMENDMENT-C §5/§14, handoff §19). The projection VIEW is created once at
 * mount and embedded in the parent — its identity is FIXED for the scope's
 * lifetime; content swaps happen behind it via {@link ScopeProjection.install}.
 *
 * Implementations own their native slot/boundary/lease (production uses the
 * existing ViewSlot primitive; a slim private equivalent may replace it per
 * §31.6 measurements — architecture first, per AMENDMENT-C §14.1).
 */
export interface ScopeProjection {
  /** Stable component/ref view shown to the parent. Never rebuilt. */
  readonly view: View;
  /** Swaps the independently retained sub-DAG root. Failure keeps the old content. */
  install(output: View): void;
  /** Releases the sub-root lease and native slot. Must be idempotent. */
  dispose(): void;
}

/** Optional factory the runtime consults at child-scope mount. */
export type ScopeProjectionFactory = (scope: RetainedExecutionScope<never>) => ScopeProjection | undefined;

interface SemanticSlot {
  /** Last committed View for this dense slot (strong, by design). */
  current: View | undefined;
  /** Staged View for the active evaluation; dropped on abort. */
  pending: View | undefined;
}

class ScopeSemanticTable {
  slots: SemanticSlot[] = [];
  /** Dense cursor for the active evaluation of the owning scope. */
  cursor = 0;
  /** Slot-table length when the active evaluation began (abort rewind). */
  beginLength = 0;

  begin(): void {
    this.beginLength = this.slots.length;
    this.cursor = 0;
  }

  next(): SemanticSlot {
    const index = this.cursor;
    this.cursor += 1;
    let slot = this.slots[index];
    if (slot === undefined) {
      slot = { current: undefined, pending: undefined };
      this.slots[index] = slot;
    }
    return slot;
  }

  /** Commit: visited slots promote pending -> current; tail truncates. */
  commit(): void {
    for (let index = 0; index < this.cursor; index += 1) {
      const slot = this.slots[index]!;
      if (slot.pending !== undefined) {
        slot.current = slot.pending;
        slot.pending = undefined;
      }
    }
    this.slots.length = this.cursor;
  }

  /** Abort: drop staged pendings, rewind growth; committed state untouched. */
  rollback(): void {
    for (let index = 0; index < this.slots.length; index += 1) {
      this.slots[index]!.pending = undefined;
    }
    this.slots.length = Math.min(this.slots.length, this.beginLength);
    this.cursor = 0;
  }

  release(): void {
    this.slots.length = 0;
    this.cursor = 0;
  }
}

interface ChildRecord {
  readonly type: ViewComponentType<never>;
  readonly key: ViewKey | undefined;
  readonly scope: RetainedExecutionScope;
}

export type ExecutionScopeState =
  | "clean"
  | "evaluating"
  | "aborted";

/**
 * One logical component instance. Continuity boundary between successive
 * evaluations: the instance survives while its immutable output changes
 * (handoff §6: ExecutionScope identity ≠ NodeId ≠ NativeRef).
 */
export class RetainedExecutionScope<P = unknown> {
  /** Dense process-local id (diagnostics only — NOT semantic identity). */
  readonly id: number;
  readonly runtime: RetainedExecutionRuntime;
  readonly parent: RetainedExecutionScope | null;
  readonly depth: number;
  readonly key: ViewKey | undefined;
  readonly type: ViewComponentType<never>;

  /** Positional ordinal among the parent's children (-1 for roots). */
  ordinal: number;
  currentProps: P | undefined;
  /** Last committed immutable output — the scope's observable artifact in R1. */
  currentOutput: View | undefined;
  pendingOutput: View | undefined;

  state: ExecutionScopeState = "clean";
  /** Set on first successful commit; aborted fresh mounts never set it. */
  mounted = false;
  dirty = false;
  disposed = false;

  /** Committed child instances (positional order). */
  readonly children: ChildRecord[] = [];
  /** Children reconciled during the active evaluation (dense, 0..n-1). */
  pendingChildren: ChildRecord[] = [];

  readonly table = new ScopeSemanticTable();

  /** Committed tracked-state subscriptions (AMENDMENT-C §7.1). */
  readonly dependencies = new Set<TrackedStateSource>();
  /** Subscriptions collected during the active evaluation; promoted on commit. */
  pendingDependencies = new Set<TrackedStateSource>();

  /** Records one tracked read against the active evaluation (state.ts calls this). */
  linkDependency(source: TrackedStateSource): void {
    if (!this.pendingDependencies.has(source)) this.pendingDependencies.add(source);
  }

  /**
   * Independently retained sub-DAG projection (R3). `undefined` in detached
   * mode (no factory): the scope's raw output is embedded directly, which is
   * the documented R1 fallback — parent composites then see content changes
   * and rebuild along the changed path.
   */
  projection: ScopeProjection | undefined = undefined;
  /** Last output installed into the projection (dedupes no-op installs). */
  projectedOutput: View | undefined = undefined;

  constructor(
    runtime: RetainedExecutionRuntime,
    parent: RetainedExecutionScope | null,
    type: ViewComponentType<P>,
    props: P | undefined,
    ordinal: number,
    key: ViewKey | undefined,
    id: number,
  ) {
    this.id = id;
    this.runtime = runtime;
    this.parent = parent;
    this.depth = parent === null ? 0 : parent.depth + 1;
    this.ordinal = ordinal;
    this.key = key;
    this.type = type as ViewComponentType<never>;
    this.currentProps = props;
  }

  /**
   * Resolves the scope's next dense semantic slot for the active evaluation.
   * Called by compose helpers via the active-scope context.
   */
  nextSemanticSlot(): SemanticSlot {
    return this.table.next();
  }

  /** Committed slot-table size (diagnostics/tests). */
  get committedSlotCount(): number {
    return this.table.slots.length;
  }

  /**
   * Releases every strong reference held by this scope and its subtree.
   * Called on unmounted/replaced scopes after successful commit and on
   * runtime shutdown / aborted fresh mounts.
   */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.dirty = false;
    try {
      this.projection?.dispose();
    } finally {
      this.projection = undefined;
      this.projectedOutput = undefined;
    }
    for (const dep of this.dependencies) dep.unsubscribe(this);
    this.dependencies.clear();
    this.pendingDependencies.clear();
    this.currentOutput = undefined;
    this.pendingOutput = undefined;
    this.currentProps = undefined;
    this.state = "clean";
    this.table.release();
    for (const record of this.children) record.scope.dispose();
    this.children.length = 0;
    this.pendingChildren.length = 0;
    this.runtime.noteUnmount(this);
  }
}

// --- Active-scope context (handoff §10): synchronous, nesting-safe. ----------

const contextStack: RetainedExecutionScope[] = [];

/**
 * Hot-path cell for the active execution scope. Compose helpers read
 * `executionContext.top` DIRECTLY (a property load on a stable shape) instead
 * of paying a cross-module call per helper invocation — the R0 ≤3% cold gate
 * is measured through exactly this path.
 */
export const executionContext: { top: RetainedExecutionScope | undefined } = {
  top: undefined,
};

/** The scope whose semantic slots compose helpers currently address. */
export function activeExecutionScope(): RetainedExecutionScope | undefined {
  return executionContext.top;
}

function pushActive(scope: RetainedExecutionScope): void {
  contextStack.push(scope);
  executionContext.top = scope;
}

function popActive(scope: RetainedExecutionScope): void {
  const popped = contextStack.pop();
  if (popped !== scope) {
    throw new ExecutionError("TUI_EXECUTION_CONTEXT", "execution context stack corrupted");
  }
  executionContext.top = contextStack[contextStack.length - 1];
}

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  if ((typeof value !== "object" || value === null) && typeof value !== "function") return false;
  return typeof (value as { then?: unknown }).then === "function";
}

// --- Errors ------------------------------------------------------------------

export const EXECUTION_ASYNC_BODY = "TUI_EXECUTION_ASYNC_BODY";

export class ExecutionError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ExecutionError";
  }
}

// --- Counters (handoff §28 execution layer; R1 subset) -----------------------

export interface ExecutionCounters {
  execution_scope_mounts: number;
  execution_scope_unmounts: number;
  execution_scope_body_calls: number;
  execution_scope_prop_skips: number;
  execution_scope_state_invalidations: number;
  execution_scope_dirty_enqueues: number;
  execution_scope_duplicate_invalidations: number;
  execution_scope_noop_outputs: number;
  execution_scope_changed_outputs: number;
  execution_flush_passes: number;
  execution_commit_batches: number;
  execution_commit_aborts: number;
  /** Existing semantic-layer counters (handoff §28: these remain). */
  composition_exact_view_reuses: number;
  composition_new_views: number;
}

export const executionCounters: ExecutionCounters = {
  execution_scope_mounts: 0,
  execution_scope_unmounts: 0,
  execution_scope_body_calls: 0,
  execution_scope_prop_skips: 0,
  execution_scope_state_invalidations: 0,
  execution_scope_dirty_enqueues: 0,
  execution_scope_duplicate_invalidations: 0,
  execution_scope_noop_outputs: 0,
  execution_scope_changed_outputs: 0,
  execution_flush_passes: 0,
  execution_commit_batches: 0,
  execution_commit_aborts: 0,
  composition_exact_view_reuses: 0,
  composition_new_views: 0,
};

const COUNTER_KEYS = Object.keys(executionCounters) as Array<keyof ExecutionCounters>;

export function executionCounterSnapshot(): ExecutionCounters {
  return structuredClone(executionCounters);
}

export function resetExecutionCounters(): void {
  for (const key of COUNTER_KEYS) executionCounters[key] = 0;
}

// --- Shallow props comparison (Review Addendum §33.6) ------------------------

/**
 * Default skip check: same own prop-key set + `Object.is` per value.
 * Objects/functions compare by identity — deep comparison is prohibited.
 */
export function propsShallowEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (typeof a !== "object" || a === null || typeof b !== "object" || b === null) return false;
  const keysA = Object.keys(a);
  const keysB = Object.keys(b);
  if (keysA.length !== keysB.length) return false;
  const recordA = a as Record<string, unknown>;
  const recordB = b as Record<string, unknown>;
  for (const key of keysA) {
    if (!Object.is(recordA[key], recordB[key])) return false;
  }
  return true;
}

// --- Runtime -----------------------------------------------------------------

let NEXT_SCOPE_ID = 1;

/**
 * Owns the dirty queue and the transactional batch protocol (handoff §17):
 * prepare ALL dirty scopes (parents before children, §12.2), then COMMIT
 * ONCE; any failure aborts the whole batch leaving committed state
 * authoritative. Further invalidations during preparation join later passes
 * of the same flush (§22.3).
 */
export interface RetainedExecutionRuntimeOptions {
  /**
   * Factory consulted when a child scope mounts. Return a projection backed
   * by the existing ViewSlot/component primitives (or a slim equivalent) to
   * give every scope an independently retained sub-DAG root whose stable
   * component view is embedded in the parent. Return `undefined` to run the
   * scope DETACHED (R1 raw-output embedding).
   */
  createScopeProjection?: ScopeProjectionFactory;
  /**
   * Auto-scheduling (AMENDMENT-C §12.1): the FIRST invalidation in a turn
   * schedules one flush at the end of the current microtask turn; later
   * invalidations join the same dirty set. Explicit `flush()` always runs
   * immediately and pre-empts the scheduled one. Production hosts may wire
   * their frame loop instead (handoff flush-integration rule); disable with
   * `autoFlush: false` for fully manual driving. Default: `true`.
   */
  autoFlush?: boolean;
}

export class RetainedExecutionRuntime {
  private queue: RetainedExecutionScope[] = [];
  private flushing = false;
  private readonly roots: RetainedExecutionScope[] = [];
  private readonly projectionFactory: ScopeProjectionFactory | undefined;
  private readonly autoFlush: boolean;
  private flushScheduled = false;

  constructor(options: RetainedExecutionRuntimeOptions = {}) {
    this.projectionFactory = options.createScopeProjection;
    this.autoFlush = options.autoFlush ?? true;
  }

  /**
   * Mounts a root scope and evaluates it synchronously (initial render is
   * eager like a mount; updates are scheduled through invalidate/flush).
   */
  mountRoot<P>(type: ViewComponentType<P>, props: P): RetainedExecutionScope<P> {
    const scope = new RetainedExecutionScope<P>(this, null, type, props, -1, undefined, NEXT_SCOPE_ID++);
    this.roots.push(scope);
    executionCounters.execution_scope_mounts += 1;
    try {
      this.runWork(scope);
      this.commitBatch([scope]);
    } catch (error) {
      this.abortScope(scope);
      this.disposeScopeTree(scope);
      this.roots.splice(this.roots.indexOf(scope), 1);
      throw error;
    }
    return scope;
  }

  /**
   * Marks a scope dirty and enqueues it exactly once. Invalidation during a
   * flush joins a later pass of the same flush (AMENDMENT-C §22.3).
   */
  invalidate(scope: RetainedExecutionScope): void {
    if (scope.disposed) return;
    if (scope.dirty) {
      executionCounters.execution_scope_duplicate_invalidations += 1;
      return;
    }
    scope.dirty = true;
    this.queue.push(scope);
    executionCounters.execution_scope_dirty_enqueues += 1;
    this.scheduleFlush();
  }

  /**
   * Coalesces a synchronous burst of invalidations into ONE flush at the end
   * of the current microtask turn (§12.1). Explicit `flush()` pre-empts it;
   * the scheduled callback then finds an empty queue and becomes a no-op.
   */
  private scheduleFlush(): void {
    if (!this.autoFlush || this.flushing || this.flushScheduled) return;
    this.flushScheduled = true;
    queueMicrotask(() => {
      this.flushScheduled = false;
      if (this.queue.length > 0) this.flush();
    });
  }

  /**
   * Runs the batch protocol over every enqueued scope until the queue drains.
   */
  flush(): void {
    if (this.flushing) return; // re-entrant flush joins the outer one
    this.flushing = true;
    try {
      while (this.queue.length > 0) {
        const batch = this.queue;
        this.queue = [];
        executionCounters.execution_flush_passes += 1;
        // Parent-before-child within a pass (AMENDMENT-C §12.2).
        batch.sort((a, b) => a.depth - b.depth || a.ordinal - b.ordinal || a.id - b.id);
        const processed: RetainedExecutionScope[] = [];
        try {
          for (const scope of batch) {
            if (scope.disposed || !scope.dirty) continue;
            // §12.2/§22.4: an ancestor that ALREADY evaluated this pass may
            // have structurally dropped this scope — discard its queued work
            // instead of executing a doomed body.
            if (this.isDroppedDuringPreparation(scope)) {
              scope.dirty = false;
              continue;
            }
            scope.dirty = false;
            processed.push(scope);
            this.runWork(scope);
          }
          this.commitBatch(processed);
        } catch (error) {
          this.abortBatch(processed);
          throw error;
        }
      }
    } finally {
      this.flushing = false;
    }
  }

  /**
   * Invalidation entry used by tracked sources on confirmed value changes
   * (handoff §9). Joins the standard dirty queue: enqueued once per pass,
   * later passes inside a running flush (AMENDMENT-C §22.3).
   */
  invalidateFromState(scope: RetainedExecutionScope): void {
    if (scope.disposed) return;
    executionCounters.execution_scope_state_invalidations += 1;
    this.invalidate(scope);
  }

  /** Invalidate + flush convenience for a single scope. */
  update(scope: RetainedExecutionScope): void {
    this.invalidate(scope);
    this.flush();
  }

  /** Shuts down the runtime, disposing every root scope subtree. */
  dispose(): void {
    for (const root of this.roots.splice(0)) {
      this.disposeScopeTree(root);
    }
    this.queue.length = 0;
  }

  // --- internals -------------------------------------------------------------

  /**
   * Whether any evaluating ancestor has structurally dropped this scope from
   * its reconciled pending child list during the current preparation phase.
   * Walks the full ancestor chain so transitive drops are caught too.
   */
  private isDroppedDuringPreparation(scope: RetainedExecutionScope): boolean {
    let node = scope;
    let current = scope.parent;
    while (current !== null) {
      if (
        current.state === "evaluating" &&
        !current.pendingChildren.some((record) => record.scope === node)
      ) {
        return true;
      }
      node = current;
      current = current.parent;
    }
    return false;
  }

  private evaluateIntoPendings(scope: RetainedExecutionScope): void {
    scope.state = "evaluating";
    scope.table.begin();
    scope.pendingChildren = [];
    // Fresh dependency collection: the committed set stays subscribed until
    // this evaluation commits (abort retains it — AMENDMENT-C §7.1).
    scope.pendingDependencies = new Set();
    pushActive(scope);
    try {
      const output = scope.type.render(scope.currentProps as never);
      if (isPromiseLike(output)) {
        throw new ExecutionError(EXECUTION_ASYNC_BODY, "component bodies must be synchronous");
      }
      scope.pendingOutput = output;
    } finally {
      popActive(scope);
    }
  }

  private runWork(scope: RetainedExecutionScope): void {
    if (scope.disposed) throw new ExecutionError("TUI_EXECUTION_DISPOSED", "cannot evaluate a disposed scope");
    executionCounters.execution_scope_body_calls += 1;
    this.evaluateIntoPendings(scope);
  }

  private commitBatch(batch: ReadonlyArray<RetainedExecutionScope>): void {
    executionCounters.execution_commit_batches += 1;
    for (const scope of batch) this.commitScope(scope);
  }

  /**
   * Promotes one scope's prepared work, depth-first through children that
   * were evaluated inline during this pass (fresh mounts and re-evaluated
   * children carry pendingOutput; skipped/reused ones do not).
   */
  private commitScope(scope: RetainedExecutionScope): void {
    for (const record of scope.pendingChildren) {
      if (record.scope.pendingOutput !== undefined) this.commitScope(record.scope);
    }
    if (scope.pendingOutput === undefined) {
      throw new ExecutionError("TUI_EXECUTION_STATE", `committing scope ${scope.id} without prepared output`);
    }
    const newOutput = scope.pendingOutput;
    if (scope.projection !== undefined && newOutput !== scope.projectedOutput) {
      // Swap the independently retained sub-DAG root BEFORE promoting: a
      // failed install keeps the old content authoritative on BOTH sides
      // (ViewSlot's boundary preserves the previous root on failure, §22.2),
      // and the surrounding batch abort then has nothing to unwind here.
      scope.projection.install(newOutput!);
      scope.projectedOutput = newOutput;
    }
    if (scope.pendingOutput === scope.currentOutput) {
      executionCounters.execution_scope_noop_outputs += 1;
    } else {
      executionCounters.execution_scope_changed_outputs += 1;
    }
    scope.currentOutput = scope.pendingOutput;
    scope.pendingOutput = undefined;

    // Child promotion: the reconciled pending list IS the new committed
    // list; committed children absent from it are unmounted and disposed
    // (including displaced type-mismatched predecessors, §9.1).
    const kept = new Set<RetainedExecutionScope>();
    for (const record of scope.pendingChildren) kept.add(record.scope);
    for (const oldRecord of scope.children) {
      if (!kept.has(oldRecord.scope)) oldRecord.scope.dispose();
    }
    scope.children.length = 0;
    for (let index = 0; index < scope.pendingChildren.length; index += 1) {
      const record = scope.pendingChildren[index]!;
      scope.children[index] = record;
      record.scope.mounted = true;
    }
    scope.pendingChildren = [];

    // Dependency promotion: unsubscribe sources no longer read; subscribe
    // newly-read sources; swap pending -> committed (§21 pointer-swap class).
    for (const dep of scope.dependencies) {
      if (!scope.pendingDependencies.has(dep)) dep.unsubscribe(scope);
    }
    for (const dep of scope.pendingDependencies) {
      if (!scope.dependencies.has(dep)) dep.subscribe(scope);
    }
    scope.dependencies.clear();
    for (const dep of scope.pendingDependencies) scope.dependencies.add(dep);
    scope.pendingDependencies.clear();

    scope.table.commit();
    scope.mounted = true;
    scope.state = "clean";
  }

  /**
   * Aborts the prepared work of the given scopes (recursing through children
   * evaluated inline during the aborted pass): committed outputs, child
   * lists, and slots stay authoritative (handoff §21); freshly created
   * never-committed subtrees are disposed so aborted bodies cannot leak
   * references (§43.4 discipline).
   */
  private abortBatch(batch: ReadonlyArray<RetainedExecutionScope>): void {
    executionCounters.execution_commit_aborts += 1;
    for (const scope of batch) this.abortScope(scope);
  }

  private abortScope(scope: RetainedExecutionScope): void {
    for (const record of scope.pendingChildren) {
      const child = record.scope;
      if (child.pendingOutput !== undefined || !child.mounted) this.abortScope(child);
    }
    scope.pendingOutput = undefined;
    scope.table.rollback();
    scope.state = "clean";
    scope.dirty = false;
    // Dispose fresh subtrees AFTER their rollback — they never committed.
    for (const record of scope.pendingChildren) {
      if (!record.scope.mounted && !record.scope.disposed) this.disposeScopeTree(record.scope);
    }
    scope.pendingChildren = [];
  }

  private disposeScopeTree(scope: RetainedExecutionScope): void {
    scope.dispose();
  }

  noteUnmount(_scope: RetainedExecutionScope): void {
    executionCounters.execution_scope_unmounts += 1;
  }

  /**
   * Reconciles ONE child invocation against the parent's committed children
   * by ordinal + type (positional path; keyed dynamics arrive in R8):
   *
   *   same ordinal + same type      -> reuse the existing scope instance
   *   same ordinal + different type -> replacement (previous disposed at commit)
   *   no committed child            -> fresh mount
   *
   * The reconciled record lands at `ordinal` in the parent's pending list.
   */
  private reconcileChild(
    parent: RetainedExecutionScope,
    type: ViewComponentType<never>,
    key: ViewKey | undefined,
    ordinal: number,
  ): { scope: RetainedExecutionScope; created: boolean } {
    const committed = parent.children[ordinal];
    if (committed !== undefined && !committed.scope.disposed && committed.type === type) {
      parent.pendingChildren[ordinal] = { type, key, scope: committed.scope };
      return { scope: committed.scope, created: false };
    }
    const scope = new RetainedExecutionScope(this, parent, type, undefined, ordinal, key, NEXT_SCOPE_ID++);
    if (this.projectionFactory !== undefined) {
      scope.projection = this.projectionFactory(scope as RetainedExecutionScope<never>) ?? undefined;
    }
    executionCounters.execution_scope_mounts += 1;
    parent.pendingChildren[ordinal] = { type, key, scope };
    return { scope, created: true };
  }

  /**
   * Invokes a component within the CURRENTLY EVALUATING scope (the active
   * scope is the reconciliation parent — this becomes defineView's wrapper
   * in R2). Returns the child's View artifact plus the child scope.
   *
   * Fresh/replaced children evaluate immediately (initial render belongs to
   * the parent's render). Surviving children with shallow-equal props SKIP
   * their body entirely (AMENDMENT-C §6.2) and re-present their committed
   * output; surviving children with changed props re-evaluate now, since the
   * parent's execution supplies the new inputs (§8.2).
   */
  invokeChild<P>(
    component: ViewComponentType<P>,
    props: P,
    key: ViewKey | undefined,
  ): { view: View; scope: RetainedExecutionScope<P> } {
    if (typeof component?.render !== "function") {
      throw new ExecutionError(
        "TUI_EXECUTION_NOT_A_COMPONENT",
        "component invocation requires a component value with a render entry",
      );
    }
    const parent = activeExecutionScope();
    if (parent === undefined) {
      throw new ExecutionError("TUI_EXECUTION_NO_ACTIVE_SCOPE", "component invocation outside any evaluating scope");
    }
    const ordinal = parent.pendingChildren.length;
    const { scope, created } = this.reconcileChild(parent, component as ViewComponentType<never>, key, ordinal);
    const typed = scope as RetainedExecutionScope<P>;
    const embeddable = (): View => scope.projection !== undefined ? scope.projection.view : (
      created ? typed.pendingOutput! : scope.currentOutput!
    );
    if (!created && propsShallowEqual(scope.currentProps, props)) {
      executionCounters.execution_scope_prop_skips += 1;
      return { view: embeddable(), scope: typed };
    }
    typed.currentProps = props;
    // Inline evaluation supplies newer inputs than any queued dirty work for
    // this scope: supersede it (AMENDMENT-C §12.2 - no double execution).
    typed.dirty = false;
    executionCounters.execution_scope_body_calls += 1;
    this.evaluateIntoPendings(typed);
    return { view: embeddable(), scope: typed };
  }
}

/**
 * Invokes a child component inside the currently evaluating scope. Must be
 * called from inside a component body (or the driver equivalent). See
 * {@link RetainedExecutionRuntime.invokeChild}.
 */
export function invokeComponent<P>(
  component: ViewComponentType<P>,
  props: P,
  key?: ViewKey,
): { view: View; scope: RetainedExecutionScope<P> } {
  const parent = activeExecutionScope();
  if (parent === undefined) {
    throw new ExecutionError("TUI_EXECUTION_NO_ACTIVE_SCOPE", "component invocation outside any evaluating scope");
  }
  return parent.runtime.invokeChild(component, props, key);
}
