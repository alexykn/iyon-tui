/**
 * PERF-12 T13.1 R8 — retained execution substrate (handoff §10/§32.2,
 * AMENDMENT-C §4/§9/§10).
 *
 * R8 additions over R1–R7:
 *   - ChildOwnerState / KeyGroup (child-owner.ts): strict positional unkeyed
 *     identity plus lazily-allocated keyed namespaces; WIP maps so evaluation
 *     never mutates committed keyed state;
 *   - ACTIVE_CHILD_OWNER context (execution-context.ts): View.key swaps only
 *     where component invocations reconcile — never the executing scope;
 *   - PublicationTarget split from ScopeProjection: builder roots publish
 *     somewhere without being projected into a parent;
 *   - OwnedBuilderRoot: producer is part of the transaction (§32.2.6).
 *
 * All R1–R7 invariants stand: immutable outputs, prepare-all/commit-once,
 * abort leaves committed state authoritative, pure synchronous bodies,
 * allocation-free exact reuse inside dirty scopes.
 */

import { nodeForBridge, type View } from "./values/view.ts";
import type { TrackedStateSource } from "./tracked-state.ts";
import {
  ChildOwnerState,
  KeyGroup,
  type ChildRecord,
  type OwnsChildren,
  type ViewKey,
} from "./child-owner.ts";
import {
  executionContext,
  activeExecutionScope,
  popActiveFrame,
  protocolState,
  pushActiveFrame,
  resolveKeyedGroup,
} from "./execution-context.ts";

export type { ViewKey };
export { executionContext, activeExecutionScope };

// --- Public-machinery types --------------------------------------------------

/**
 * Core component abstraction: a stable identity token carrying its pure,
 * synchronous render body. Identity is object REFERENCE equality —
 * `defineView` returns one object per definition; structurally identical
 * literals are DIFFERENT types (mismatch ⇒ remount, §9.1). Property-arrow
 * syntax enforces contravariant props checking.
 */
export interface ViewComponentType<P = unknown> {
  readonly render: (props: P) => View;
}

/** The PUBLIC callable component value returned by `defineView`. */
export interface ViewComponent<P = unknown> extends ViewComponentType<P> {
  (props: P): View;
}

/**
 * One fallible-free publication of a prepared native root (R7 contract):
 * commit is infallible after successful preparation; abort leaves the old
 * root installed and leased.
 */
export interface PreparedPublication {
  commit(): void;
  abort(): void;
}

/**
 * WHERE a scope's latest immutable output gets installed (R8 split from
 * projection). Builder roots have a target but no projection.
 */
export interface PublicationTarget {
  /**
   * Prepares everything fallible without publishing. `undefined` counts as
   * refused preparation (the enclosing batch aborts atomically).
   */
  preparePublication(output: View): PreparedPublication | undefined;
  /**
   * Optional metadata-only publication trigger. Root boundaries use this for
   * Scene sideband changes (for example History) when the semantic body View
   * is unchanged.
   */
  needsPublication?(output: View): boolean;
}

/**
 * HOW a child scope is represented in its parent: a stable component/ref
 * view created once at mount, behind which content swaps happen.
 */
export interface ScopeProjection {
  readonly view: View;
  /**
   * R7 transactional publication. OPTIONAL on projections: legacy/detached
   * projections without it fall back to per-scope `install` (non-atomic).
   * Production targets always provide it.
   */
  preparePublication?(output: View): PreparedPublication | undefined;
  /** Legacy per-scope fallback (non-transactional). */
  install(output: View): void;
  /** Releases the sub-root lease and native slot. Must be idempotent. */
  dispose(): void;
}

/** Optional factory consulted when a child scope mounts. */
export type ScopeProjectionFactory = (scope: RetainedExecutionScope<never>) => ScopeProjection | undefined;

interface SemanticSlot {
  current: View | undefined;
  pending: View | undefined;
}

class ScopeSemanticTable {
  slots: SemanticSlot[] = [];
  cursor = 0;
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

export type ExecutionScopeState = "clean" | "evaluating" | "aborted";

/**
 * One logical component instance. Continuity boundary between successive
 * evaluations: ExecutionScope identity ≠ NodeId ≠ NativeRef (handoff §6).
 */
export class RetainedExecutionScope<P = unknown> implements OwnsChildren {
  readonly id: number;
  readonly runtime: RetainedExecutionRuntime;
  readonly parent: RetainedExecutionScope | null;
  readonly depth: number;
  readonly key: ViewKey | undefined;
  readonly type: ViewComponentType<never>;

  ordinal: number;
  /** Last committed inputs. Pending prop updates stay separate until commit. */
  currentProps: P | undefined;
  pendingProps: P | undefined;
  pendingPropsActive = false;
  /** Last committed immutable output — the scope's observable artifact. */
  currentOutput: View | undefined;
  pendingOutput: View | undefined;

  state: ExecutionScopeState = "clean";
  /** Set on first successful commit; aborted fresh mounts never set it. */
  mounted = false;
  dirty = false;
  disposed = false;

  /** Unkeyed positional children + keyed namespace (R8). */
  readonly owner = new ChildOwnerState();

  readonly table = new ScopeSemanticTable();

  readonly dependencies = new Set<TrackedStateSource>();
  pendingDependencies = new Set<TrackedStateSource>();

  linkDependency(source: TrackedStateSource): void {
    if (!this.pendingDependencies.has(source)) this.pendingDependencies.add(source);
  }

  /** WHERE output installs (builder roots / projected children). */
  publicationTarget: PublicationTarget | undefined = undefined;
  /** HOW represented in the parent (projected children only). */
  projection: ScopeProjection | undefined = undefined;
  projectedOutput: View | undefined = undefined;
  stagedPublication: PreparedPublication | undefined = undefined;

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

  nextSemanticSlot(): SemanticSlot {
    return this.table.next();
  }

  /** Committed UNKEYED children (compat view over the child-owner state). */
  get children(): ChildRecord[] {
    return this.owner.committedChildren;
  }

  get pendingChildren(): ChildRecord[] {
    return this.owner.pendingChildren;
  }

  get committedSlotCount(): number {
    return this.table.slots.length;
  }

  /** Active publication target: explicit target, else a transactional projection. */
  get effectivePublicationTarget(): PublicationTarget | undefined {
    if (this.publicationTarget !== undefined) return this.publicationTarget;
    const prepare = this.projection?.preparePublication;
    return prepare === undefined
      ? undefined
      : { preparePublication: (output: View) => prepare.call(this.projection!, output) };
  }

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
    this.publicationTarget = undefined;
    for (const dep of this.dependencies) dep.unsubscribe(this);
    this.dependencies.clear();
    this.pendingDependencies.clear();
    this.currentOutput = undefined;
    this.pendingOutput = undefined;
    this.currentProps = undefined;
    this.pendingProps = undefined;
    this.pendingPropsActive = false;
    this.state = "clean";
    this.table.release();
    // Dispose the entire owned subtree: unkeyed children and arbitrarily
    // nested keyed namespaces. A bounded-depth walk would retain deeper
    // keyed scopes and their subscriptions after the parent is disposed.
    disposeOwnedChildren(this.owner);
    this.runtime.noteUnmount(this);
  }
}

// --- Active context (execution-context.ts owns the stack). --------------------

function pushActive(scope: RetainedExecutionScope): void {
  pushActiveFrame(scope);
}

function popActive(scope: RetainedExecutionScope): void {
  try {
    popActiveFrame(scope);
  } catch {
    throw new ExecutionError("TUI_EXECUTION_CONTEXT", "execution context stack corrupted");
  }
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

// --- Counters (handoff §28) ---------------------------------------------------

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

/** Pathological commit-phase failures surfaced per flush (diagnostics only). */
export const pathologicalCommitFailures: unknown[] = [];

export function executionCounterSnapshot(): ExecutionCounters {
  return structuredClone(executionCounters);
}

export function resetExecutionCounters(): void {
  for (const key of COUNTER_KEYS) executionCounters[key] = 0;
}

// --- Shallow props comparison (Review Addendum §33.6) ------------------------

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

let NEXT_SCOPE_ID = 1;

export interface RetainedExecutionRuntimeOptions {
  createScopeProjection?: ScopeProjectionFactory;
  autoFlush?: boolean;
}

export class RetainedExecutionRuntime {
  private queue: RetainedExecutionScope[] = [];
  private flushing = false;
  private readonly roots: RetainedExecutionScope[] = [];
  private readonly projectionFactory: ScopeProjectionFactory | undefined;
  private readonly autoFlush: boolean;
  /**
   * Scheduled-flush token (post-R9 review): a generation counter, not a bare
   * boolean, so an EXPLICIT flush can consume an already-pending microtask.
   * Without this, `invalidate(); update()` that fails mid-flush would be
   * silently retried by the stale microtask — violating "abort preserves the
   * retry obligation but never retries automatically".
   */
  private scheduledGeneration = 0;
  private flushScheduled = false;
  /** Set while any scope body is evaluating or a batch is committing (§32.2.7 guard). */
  private mutating = false;

  constructor(options: RetainedExecutionRuntimeOptions = {}) {
    this.projectionFactory = options.createScopeProjection;
    this.autoFlush = options.autoFlush ?? true;
  }

  mountRoot<P>(component: ViewComponentType<P>, props: P): RetainedExecutionScope<P> {
    const scope = new RetainedExecutionScope<P>(this, null, component, props, -1, undefined, NEXT_SCOPE_ID++);
    this.roots.push(scope);
    executionCounters.execution_scope_mounts += 1;
    try {
      this.runWork(scope);
    } catch (error) {
      // Roll back WIP (fresh evaluated children collected + disposed) before
      // detaching the failed root.
      const fresh: RetainedExecutionScope[] = [];
      this.abortLevel(scope.owner, fresh);
      scope.pendingOutput = undefined;
      scope.pendingProps = undefined;
      scope.pendingPropsActive = false;
      scope.table.rollback();
      scope.pendingDependencies = new Set();
      scope.state = "clean";
      for (const s of fresh) this.disposeScopeTree(s);
      this.disposeScopeTree(scope);
      this.roots.splice(this.roots.indexOf(scope), 1);
      throw error;
    }
    const staged: RetainedExecutionScope[] = [];
    try {
      this.stagePublicationsRecursive(scope, staged);
      this.commitBatch([scope]);
    } catch (error) {
      let unwindError: unknown;
      try {
        unwindStaged(staged);
      } catch (cleanupError) {
        unwindError = cleanupError;
      }
      const fresh: RetainedExecutionScope[] = [];
      this.abortLevel(scope.owner, fresh);
      scope.pendingOutput = undefined;
      scope.pendingProps = undefined;
      scope.pendingPropsActive = false;
      scope.table.rollback();
      scope.pendingDependencies = new Set();
      scope.state = "clean";
      for (const s of fresh) this.disposeScopeTree(s);
      this.disposeScopeTree(scope);
      this.roots.splice(this.roots.indexOf(scope), 1);
      if (unwindError !== undefined) {
        throw new AggregateError([error, unwindError], "retained execution mount cleanup failed");
      }
      throw error;
    }
    return scope;
  }

  invalidate(scope: RetainedExecutionScope): void {
    if (scope.disposed) return;
    if (scope.dirty) {
      executionCounters.execution_scope_duplicate_invalidations += 1;
      // Already queued (possibly as a restored retry obligation, §32.3):
      // never enqueue twice, but DO make sure a future flush is armed.
      this.scheduleFlush();
      return;
    }
    scope.dirty = true;
    this.queue.push(scope);
    executionCounters.execution_scope_dirty_enqueues += 1;
    this.scheduleFlush();
  }

  private scheduleFlush(): void {
    if (!this.autoFlush || this.flushing || this.flushScheduled) return;
    this.flushScheduled = true;
    const generation = ++this.scheduledGeneration;
    queueMicrotask(() => {
      // A stale token: an explicit drain already consumed this schedule.
      if (!this.flushScheduled || generation !== this.scheduledGeneration) return;
      this.flushScheduled = false;
      if (this.queue.length > 0) this.flush();
    });
  }

  flush(): void {
    if (this.flushing) return; // re-entrant flush joins the outer one
    // An explicit/synchronous drain OWNS pending scheduled work now: consume
    // the token so a failure below cannot be auto-retried by the stale
    // microtask (post-R9 invariant §32.3).
    if (this.flushScheduled) {
      this.flushScheduled = false;
      ++this.scheduledGeneration;
    }
    this.flushing = true;
    protocolState.mutating = true;
    try {
      while (this.queue.length > 0) {
        const batch = this.queue;
        this.queue = [];
        // Level-triggered dirty (§32.3): every still-live scope in this batch
        // carries an invalidation OBLIGATION — its committed output may not
        // reflect its current authoritative inputs (State values mutate before
        // invalidation and survive aborts). Snapshot it at acquisition; on an
        // evaluation/PREPARE abort the whole set is restored, so no obligation
        // is consumed by a transaction that did not commit.
        const retryObligations = batch.filter((scope) => !scope.disposed && scope.dirty);
        executionCounters.execution_flush_passes += 1;
        // Parent-before-child within a pass (AMENDMENT-C §12.2).
        batch.sort((a, b) => a.depth - b.depth || a.ordinal - b.ordinal || a.id - b.id);
        const processed: RetainedExecutionScope[] = [];

        // PHASE 1: evaluate every queued scope (children of evaluating parents
        // are reached inline; §12.2/§22.4 drops skip doomed entries).
        try {
          this.mutating = true;
          for (const scope of batch) {
            if (scope.disposed || !scope.dirty) continue;
            if (this.isDroppedDuringPreparation(scope)) {
              scope.dirty = false;
              continue;
            }
            scope.dirty = false;
            processed.push(scope);
            this.runWork(scope);
          }
        } catch (error) {
          // Evaluation failed: roll back all WIP, then RESTORE the original
          // batch's dirty obligations (processed, unprocessed, superseded,
          // dropped-in-WIP alike — no commit happened, so the previous frame
          // stays authoritative while inputs stay current). No automatic
          // retry is armed; a later re-drive drains them (§32.3).
          this.abortBatch(processed);
          this.restoreRetryObligations(retryObligations);
          throw error;
        } finally {
          this.mutating = false;
        }

        // PHASE 2: prepare all publications. Fallible — any failure unwinds
        // every staged publication plus the JS batch, leaving the previous
        // frame fully authoritative.
        const stagedPublications: RetainedExecutionScope[] = [];
        const finalized: RetainedExecutionScope[] = [];
        this.batchRemoved = finalized;
        let hasCommitError = false;
        try {
          this.mutating = true;
          for (const scope of processed) this.stagePublicationsRecursive(scope, stagedPublications);
        } catch (prepareError) {
          this.mutating = false;
          let unwindError: unknown;
          try {
            unwindStaged(stagedPublications);
          } catch (cleanupError) {
            unwindError = cleanupError;
          }
          // Same contract as evaluation failure: PREPARE aborted, nothing
          // committed, so the original batch's dirty obligations survive.
          this.abortBatch(processed);
          this.restoreRetryObligations(retryObligations);
          if (unwindError !== undefined) {
            throw new AggregateError([prepareError, unwindError], "retained execution batch cleanup failed");
          }
          throw prepareError;
        }

        // PHASE 3: publish + promote. Infallible after prepare by contract; a
        // throw here is pathological teardown and propagates deliberately.
        this.mutating = true;
        try {
          this.commitBatch(processed);
          for (const scope of stagedPublications) scope.stagedPublication = undefined;
        } catch (commitError) {
          hasCommitError = true;
          pathologicalCommitFailures.push(commitError);
          throw commitError;
        } finally {
          this.mutating = false;
          const sink = this.batchRemoved;
          this.batchRemoved = [];
          // FINALIZE deferred disposals only when promotion completed
          // normally. A commit-phase throw means unspecified state (§32.1
          // R7): surface it without half-disposing.
          if (!hasCommitError) {
            for (const scope of sink) this.disposeScopeTree(scope);
          }
        }
      }
    } finally {
      this.flushing = false;
      protocolState.mutating = false;
    }
  }

  invalidateFromState(scope: RetainedExecutionScope): void {
    if (scope.disposed) return;
    executionCounters.execution_scope_state_invalidations += 1;
    this.invalidate(scope);
  }

  /**
   * Restores failed-batch invalidation obligations WITHOUT arming the
   * scheduler (§32.3): recovery requires an application re-drive — an
   * explicit {@link flush}, a later State write, or any other normal
   * scheduling trigger. Automatic retry would turn a persistent throwing
   * component into an infinite microtask loop.
   *
   * @internal also used by OwnedBuilderRoot producer rollback via cancelRetry.
   */
  private restoreRetryObligations(obligations: ReadonlyArray<RetainedExecutionScope>): void {
    for (const scope of obligations) {
      if (scope.disposed) continue;
      scope.dirty = true;
      if (!this.queue.includes(scope)) this.queue.push(scope);
    }
  }

  /**
   * Cancels one scope's retry obligation: used ONLY by producer rollback
   * (OwnedBuilderRoot.replaceProducer) when the attempted replacement input
   * itself was rolled back to the previously authoritative producer, so the
   * obligation introduced solely by that attempt must not linger. A scope
   * with a pre-existing obligation keeps it (the restored producer plus the
   * still-newer State values is exactly what a retry should render).
   *
   * @internal narrow framework operation — not public dirty manipulation.
   */
  cancelRetry(scope: RetainedExecutionScope): void {
    scope.dirty = false;
    const index = this.queue.indexOf(scope);
    if (index !== -1) this.queue.splice(index, 1);
  }

  update(scope: RetainedExecutionScope): void {
    this.invalidate(scope);
    this.flush();
  }

  dispose(): void {
    for (const root of this.roots.splice(0)) {
      this.disposeScopeTree(root);
    }
    this.queue.length = 0;
  }

  /** Reentrancy guard (§32.2.7): no builder-boundary mutations mid-protocol. */
  assertNotMutating(operation: string): void {
    if (this.mutating) {
      throw new ExecutionError(
        "TUI_EXECUTION_REENTRANT_MUTATION",
        `${operation} while the retained execution protocol is running is forbidden`,
      );
    }
  }

  private isDroppedDuringPreparation(scope: RetainedExecutionScope): boolean {
    let node = scope;
    let current = scope.parent;
    while (current !== null) {
      if (current.state === "evaluating" && !ownsScope(current.owner, node)) return true;
      node = current;
      current = current.parent;
    }
    return false;
  }

  private evaluateIntoPendings(scope: RetainedExecutionScope): void {
    scope.state = "evaluating";
    scope.table.begin();
    // Opens fresh WIP for the unkeyed stream AND marks keyed participation:
    // an evaluating owner with zero keyed children unmounts its old groups.
    scope.owner.beginChildPass();
    scope.stagedPublication = undefined;
    scope.pendingDependencies = new Set();
    pushActive(scope);
    try {
      const props = scope.pendingPropsActive ? scope.pendingProps : scope.currentProps;
      const output = scope.type.render(props as never);
      if (isPromiseLike(output)) {
        throw new ExecutionError(EXECUTION_ASYNC_BODY, "component bodies must be synchronous");
      }
      // The public type says bodies return View, but runtime consumers are JS
      // and can violate that contract. Validate before the output can become
      // pending state; otherwise an ignored invalid child could be promoted as
      // mounted without an authoritative output.
      nodeForBridge(output);
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

  private stagePublicationsRecursive(
    scope: RetainedExecutionScope,
    staged: RetainedExecutionScope[],
  ): void {
    this.stageOwnerPublications(scope.owner, staged);
    const target = scope.effectivePublicationTarget;
    const prepare = target?.preparePublication;
    const pendingOutput = scope.pendingOutput;
    const needsPublication = prepare !== undefined
      && pendingOutput !== undefined
      && (
        pendingOutput !== scope.projectedOutput
        || target?.needsPublication?.(pendingOutput) === true
      );
    if (needsPublication) {
      const publication = prepare.call(target, pendingOutput);
      if (publication === undefined) {
        // A projection may lack a native boundary (for example a detached
        // compatibility host). Its legacy install path remains the complete
        // fallback. Explicit publication targets, such as the scene root,
        // have no legacy target and must still abort atomically on refusal.
        if (scope.projection !== undefined && scope.publicationTarget === undefined) return;
        throw new ExecutionError(
          "TUI_EXECUTION_PREPARE_REFUSED",
          `publication target for scope ${scope.id} refused preparation; batch aborts atomically`,
        );
      }
      scope.stagedPublication = publication;
      staged.push(scope);
    }
  }

  /** Stages every pending component below an execution owner. */
  private stageOwnerPublications(owner: ChildOwnerState, staged: RetainedExecutionScope[]): void {
    for (const record of owner.pendingChildren) {
      this.stagePublicationsRecursive(record.scope, staged);
    }
    if (owner.pendingKeyed !== undefined) {
      for (const group of owner.pendingKeyed.values()) {
        this.stageKeyGroupPublications(group, staged);
      }
    }
  }

  /** KeyGroups are identity namespaces, so only their descendant scopes publish. */
  private stageKeyGroupPublications(group: KeyGroup, staged: RetainedExecutionScope[]): void {
    for (const record of group.owner.pendingChildren) {
      this.stagePublicationsRecursive(record.scope, staged);
    }
    if (group.owner.pendingKeyed !== undefined) {
      for (const nested of group.owner.pendingKeyed.values()) {
        this.stageKeyGroupPublications(nested, staged);
      }
    }
  }

  private commitBatch(batch: ReadonlyArray<RetainedExecutionScope>): void {
    executionCounters.execution_commit_batches += 1;
    // A scope can be reached TWICE in one pass: recursively as a descendant
    // of an evaluating parent AND as its own queue entry (independently dirty
    // child whose parent re-invoked it with UNCHANGED props — the skip gate
    // leaves the queued obligation intact). The first commit wins; the second
    // must be a no-op, not a "without prepared output" protocol failure.
    const committed = new Set<RetainedExecutionScope>();
    for (const scope of batch) this.commitScope(scope, this.batchRemoved, committed);
  }

  /** Removal sink for the batch in flight (R8 deferred finalization). */
  private batchRemoved: RetainedExecutionScope[] = [];

  private commitScope(
    scope: RetainedExecutionScope,
    removed: RetainedExecutionScope[],
    committed: Set<RetainedExecutionScope>,
  ): void {
    if (committed.has(scope)) return; // already promoted earlier this pass
    committed.add(scope);
    // Commit descendants before their parent so every embedded projection is
    // authoritative when the parent output is promoted. This is recursive
    // rather than bounded to the two keyed levels used by the first R8 pass.
    this.commitOwnerChildren(scope.owner, removed, committed);

    if (scope.pendingOutput === undefined) {
      throw new ExecutionError("TUI_EXECUTION_STATE", `committing scope ${scope.id} without prepared output`);
    }
    const newOutput = scope.pendingOutput;
    const staged = scope.stagedPublication;
    if (staged !== undefined) {
      // R7 transactional publication prepared during PHASE 2. Publication
      // callbacks are framework-owned; allow their boundary calls to bypass
      // the user-facing reentrancy guard without exposing that escape hatch to
      // component bodies.
      protocolState.internalPublication = true;
      try {
        staged.commit();
      } finally {
        protocolState.internalPublication = false;
      }
      scope.stagedPublication = undefined;
      scope.projectedOutput = newOutput;
    } else if (scope.projection !== undefined && newOutput !== scope.projectedOutput) {
      // Legacy per-scope fallback for projections without transactions.
      protocolState.internalPublication = true;
      try {
        scope.projection.install(newOutput);
      } finally {
        protocolState.internalPublication = false;
      }
      scope.projectedOutput = newOutput;
    }
    if (newOutput === scope.currentOutput) {
      executionCounters.execution_scope_noop_outputs += 1;
    } else {
      executionCounters.execution_scope_changed_outputs += 1;
    }
    scope.currentOutput = newOutput;
    scope.pendingOutput = undefined;
    if (scope.pendingPropsActive) {
      scope.currentProps = scope.pendingProps;
      scope.pendingProps = undefined;
      scope.pendingPropsActive = false;
    }

    // Dependency promotion.
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

  private abortBatch(batch: ReadonlyArray<RetainedExecutionScope>): void {
    executionCounters.execution_commit_aborts += 1;
    const fresh: RetainedExecutionScope[] = [];
    for (const scope of batch) {
      this.abortLevel(scope.owner, fresh);
      scope.pendingOutput = undefined;
      scope.pendingProps = undefined;
      scope.pendingPropsActive = false;
      scope.table.rollback();
      scope.pendingDependencies = new Set();
      scope.state = "clean";
      scope.dirty = false;
      scope.stagedPublication = undefined;
    }
    // Fresh never-committed scopes die AFTER rollback — they never became
    // part of the authoritative frame (§43.4).
    for (const scope of fresh) this.disposeScopeTree(scope);
  }

  /**
   * Rolls back WIP at this owner level and returns every FRESH
   * never-committed scope encountered (recursively, including keyed-group
   * namespaces) so the caller can dispose them after unwinding.
   */
  private abortLevel(
    owner: ChildOwnerState,
    fresh: RetainedExecutionScope[],
  ): void {
    for (const record of owner.pendingChildren) {
      const child = record.scope;
      if (child.pendingOutput !== undefined || child.state === "evaluating") {
        // Evaluated inline this pass: roll back its subtree first.
        this.abortLevel(child.owner, fresh);
        child.pendingOutput = undefined;
        child.pendingProps = undefined;
        child.pendingPropsActive = false;
        child.table.rollback();
        child.pendingDependencies = new Set();
        child.state = "clean";
        child.dirty = false;
        child.stagedPublication = undefined;
        if (!child.mounted) fresh.push(child);
      } else if (!child.mounted) {
        // A fresh child can have thrown before producing an output.
        fresh.push(child);
      }
    }
    if (owner.pendingKeyed !== undefined) {
      for (const group of owner.pendingKeyed.values()) {
        this.abortLevel(group.owner, fresh);
      }
    }
    // Drop only WIP. Committed keyed groups/children remain authoritative on
    // abort, including at arbitrary nesting depth.
    owner.dropPending();
  }

  /**
   * Mounts an externally constructed root scope (OwnedBuilderRoot): runs its
   * body once, stages/commits publications through its assigned publication
   * target. On failure rolls back WIP, disposes the subtree and rethrows
   * (the root never becomes authoritative).
   */
  mountExistingRoot(scope: RetainedExecutionScope): void {
    this.roots.push(scope);
    executionCounters.execution_scope_mounts += 1;
    const staged: RetainedExecutionScope[] = [];
    try {
      this.runWork(scope);
      this.stagePublicationsRecursive(scope, staged);
      this.commitBatch([scope]);
    } catch (error) {
      let unwindError: unknown;
      try {
        unwindStaged(staged);
      } catch (cleanupError) {
        unwindError = cleanupError;
      }
      const fresh: RetainedExecutionScope[] = [];
      this.abortLevel(scope.owner, fresh);
      scope.pendingOutput = undefined;
      scope.pendingProps = undefined;
      scope.pendingPropsActive = false;
      scope.table.rollback();
      scope.pendingDependencies = new Set();
      scope.state = "clean";
      for (const s of fresh) this.disposeScopeTree(s);
      this.disposeScopeTree(scope);
      this.roots.splice(this.roots.indexOf(scope), 1);
      if (unwindError !== undefined) {
        throw new AggregateError([error, unwindError], "retained execution mount cleanup failed");
      }
      throw error;
    }
  }

  /** Detaches a mounted root scope from the runtime (OwnedBuilderRoot dispose). */
  detachRoot(scope: RetainedExecutionScope): void {
    this.disposeScopeTree(scope);
    const index = this.roots.indexOf(scope);
    if (index !== -1) this.roots.splice(index, 1);
    // A detached builder can have a queued invalidation when autoFlush is
    // disabled (or before its scheduled microtask runs). Do not retain the
    // disposed scope tree through the dirty queue after ownership ends.
    this.queue = this.queue.filter((queued) => !queued.disposed);
  }

  private disposeScopeTree(scope: RetainedExecutionScope): void {
    scope.dispose();
  }

  /** Commits every pending descendant below an execution owner. */
  private commitOwnerChildren(
    owner: ChildOwnerState,
    removed: RetainedExecutionScope[],
    committed: Set<RetainedExecutionScope>,
  ): void {
    for (const record of owner.pendingChildren) {
      if (record.scope.pendingOutput !== undefined) this.commitScope(record.scope, removed, committed);
    }
    if (owner.pendingKeyed !== undefined) {
      for (const group of owner.pendingKeyed.values()) {
        this.commitKeyGroup(group, removed, committed);
      }
    }
    promoteOwnedChildren(owner, removed);
  }

  /** Recursively commits a keyed namespace and all nested namespaces. */
  private commitKeyGroup(
    group: KeyGroup,
    removed: RetainedExecutionScope[],
    committed: Set<RetainedExecutionScope>,
  ): void {
    for (const record of group.owner.pendingChildren) {
      if (record.scope.pendingOutput !== undefined) this.commitScope(record.scope, removed, committed);
    }
    if (group.owner.pendingKeyed !== undefined) {
      for (const nested of group.owner.pendingKeyed.values()) {
        this.commitKeyGroup(nested, removed, committed);
      }
    }
    promoteOwnedChildren(group.owner, removed);
  }

  noteUnmount(_scope: RetainedExecutionScope): void {
    executionCounters.execution_scope_unmounts += 1;
  }

  private reconcileChild(
    owner: ChildOwnerState,
    parent: RetainedExecutionScope,
    type: ViewComponentType<never>,
    key: ViewKey | undefined,
  ): { scope: RetainedExecutionScope; created: boolean } {
    const ordinal = owner.cursor;
    owner.cursor += 1;
    const committed = owner.committedChildren[ordinal];
    if (
      committed !== undefined &&
      !committed.scope.disposed &&
      committed.type === type &&
      committed.scope.key === undefined
    ) {
      owner.pendingChildren[ordinal] = { type, key, scope: committed.scope };
      return { scope: committed.scope, created: false };
    }
    const scope = new RetainedExecutionScope(this, parent, type, undefined, ordinal, key, NEXT_SCOPE_ID++);
    if (this.projectionFactory !== undefined) {
      scope.projection = this.projectionFactory(scope as RetainedExecutionScope<never>) ?? undefined;
    }
    executionCounters.execution_scope_mounts += 1;
    owner.pendingChildren[ordinal] = { type, key, scope };
    return { scope, created: true };
  }

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
    const execScope = activeExecutionScope();
    if (execScope === undefined) {
      throw new ExecutionError("TUI_EXECUTION_NO_ACTIVE_SCOPE", "component invocation outside any evaluating scope");
    }
    // ONE keyed identity mechanism (review SS10): a keyed invocation routes
    // reconciliation into its group's child-owner; unkeyed invocations use
    // the active owner's positional stream.
    if (key !== undefined) {
      const owner = executionContext.childOwner ?? execScope.owner;
      const group = resolveKeyedGroup(owner, key);
      const typed = this.invokeInto(group.owner, execScope, component, props, undefined);
      const embeddable = (): View =>
        typed.projection !== undefined ? typed.projection.view : (
          typed.pendingOutput ?? typed.currentOutput!
        );
      return { view: embeddable(), scope: typed };
    }
    const childOwner = executionContext.childOwner ?? execScope.owner;
    const typed = this.invokeInto(childOwner, execScope, component, props, undefined);
    const embeddable = (): View =>
      typed.projection !== undefined ? typed.projection.view : (
        typed.pendingOutput ?? typed.currentOutput!
      );
    return { view: embeddable(), scope: typed };
  }

  /** Core reconciliation+evaluation used by invokeChild and keyed paths. */
  private invokeInto<P>(
    owner: ChildOwnerState,
    execScope: RetainedExecutionScope,
    component: ViewComponentType<P>,
    props: P,
    key: ViewKey | undefined,
  ): RetainedExecutionScope<P> {
    const { scope, created } = this.reconcileChild(
      owner,
      execScope,
      component as ViewComponentType<never>,
      key,
    );
    const typed = scope as RetainedExecutionScope<P>;
    if (!created && propsShallowEqual(typed.currentProps, props)) {
      executionCounters.execution_scope_prop_skips += 1;
      return typed;
    }
    // Props are WIP as well: a failed child evaluation must not make the new
    // inputs look committed and cause a later parent pass to skip stale UI.
    typed.pendingProps = props;
    typed.pendingPropsActive = true;
    // Inline evaluation supersedes queued dirty work (AMENDMENT-C §12.2).
    typed.dirty = false;
    executionCounters.execution_scope_body_calls += 1;
    this.evaluateIntoPendings(typed);
    return typed;
  }
}

function unwindStaged(staged: ReadonlyArray<RetainedExecutionScope>): void {
  const errors: unknown[] = [];
  for (const scope of staged) {
    try {
      scope.stagedPublication?.abort();
    } catch (error) {
      errors.push(error);
    } finally {
      scope.stagedPublication = undefined;
    }
  }
  if (errors.length > 0) {
    throw new AggregateError(errors, "retained execution publication cleanup failed");
  }
}

/**
 * Promotes one owner's unkeyed stream and keyed namespace after evaluation.
 * Removed subtrees are COLLECTED, not disposed — finalization is deferred
 * until the entire R7 batch has committed (review directive: never dispose
 * while other publications in the batch can still fail).
 *
 * Keyed handling is gated on `wipActive`: an owner that evaluated with zero
 * keyed children unmounts all committed groups; an untouched owner preserves
 * them (AMENDMENT-C §32.2.5 / handoff §32.2.8).
 */
function promoteOwnedChildren(owner: ChildOwnerState, removed: RetainedExecutionScope[]): void {
  const kept = new Set<RetainedExecutionScope>();
  for (const record of owner.pendingChildren) kept.add(record.scope);
  for (const oldRecord of owner.committedChildren) {
    if (!kept.has(oldRecord.scope)) removed.push(oldRecord.scope);
  }
  owner.committedChildren.length = 0;
  for (let index = 0; index < owner.pendingChildren.length; index += 1) {
    const record = owner.pendingChildren[index]!;
    owner.committedChildren[index] = record;
    record.scope.mounted = true;
  }
  owner.pendingChildren = [];
  owner.cursor = 0;

  if (!owner.wipActive) return;

  const pending = owner.pendingKeyed ?? new Map<ViewKey, KeyGroup>();
  const committed = (owner.committedKeyed ??= new Map<ViewKey, KeyGroup>());
  for (const [key, group] of pending) committed.set(key, group);
  for (const [key, group] of committed) {
    if (pending.has(key)) continue;
    collectGroupScopes(group, removed);
    group.owner.release();
    committed.delete(key);
  }
  owner.pendingKeyed = undefined;
}

/** Collects every live scope of a doomed keyed group subtree for disposal. */
function collectGroupScopes(group: KeyGroup, removed: RetainedExecutionScope[]): void {
  for (const record of group.owner.committedChildren) removed.push(record.scope);
  if (group.owner.committedKeyed !== undefined) {
    for (const nested of group.owner.committedKeyed.values()) collectGroupScopes(nested, removed);
  }
}

/** Disposes all owned descendants, including pending WIP and nested keys. */
function disposeOwnedChildren(owner: ChildOwnerState): void {
  const records = [...owner.committedChildren, ...owner.pendingChildren];
  for (const record of records) record.scope.dispose();
  const groups = new Set<KeyGroup>();
  for (const group of owner.committedKeyed?.values() ?? []) groups.add(group);
  for (const group of owner.pendingKeyed?.values() ?? []) groups.add(group);
  for (const group of groups) disposeKeyGroup(group);
  owner.release();
}

function disposeKeyGroup(group: KeyGroup): void {
  const records = [...group.owner.committedChildren, ...group.owner.pendingChildren];
  for (const record of records) record.scope.dispose();
  const nestedGroups = new Set<KeyGroup>();
  for (const nested of group.owner.committedKeyed?.values() ?? []) nestedGroups.add(nested);
  for (const nested of group.owner.pendingKeyed?.values() ?? []) nestedGroups.add(nested);
  for (const nested of nestedGroups) disposeKeyGroup(nested);
  group.owner.release();
}

function ownsScope(owner: ChildOwnerState, scope: RetainedExecutionScope): boolean {
  if (owner.pendingChildren.some((record) => record.scope === scope)) return true;
  if (owner.pendingKeyed !== undefined) {
    for (const group of owner.pendingKeyed.values()) {
      if (ownsScope(group.owner, scope)) return true;
    }
  }
  return false;
}

/**
 * R8 diagnostics: the keyed group that owns `scope`, if any. Key identity
 * lives on the group, not on child records (handoff §32.2.5).
 */
export function keyGroupOf(scope: RetainedExecutionScope): ViewKey | undefined {
  const parent = scope.parent;
  if (parent === null) return undefined;
  // Check WIP first (group may have been touched but not yet promoted), then
  // the committed namespace.
  return findKeyGroup(parent.owner, scope);
}

function findKeyGroup(owner: ChildOwnerState, scope: RetainedExecutionScope): ViewKey | undefined {
  const maps = [owner.pendingKeyed, owner.committedKeyed];
  for (const map of maps) {
    if (map === undefined) continue;
    for (const [key, group] of map) {
      const pools = [group.owner.pendingChildren, group.owner.committedChildren];
      for (const pool of pools) {
        if (pool.some((record) => record.scope === scope)) return key;
      }
      const nested = findKeyGroup(group.owner, scope);
      if (nested !== undefined) return nested;
    }
  }
  return undefined;
}

/**
 * Invokes a child component inside the currently evaluating scope. Must be
 * called from inside a component body. Reconciles under the ACTIVE CHILD
 * OWNER (the enclosing scope normally; a keyed group inside View.key).
 */
export function invokeComponent<P>(
  component: ViewComponentType<P>,
  props: P,
  key?: ViewKey,
): { view: View; scope: RetainedExecutionScope<P> } {
  const execScope = activeExecutionScope();
  if (execScope === undefined) {
    throw new ExecutionError("TUI_EXECUTION_NO_ACTIVE_SCOPE", "component invocation outside any evaluating scope");
  }
  return execScope.runtime.invokeChild(component, props, key);
}


/**
 * R8 — OwnedBuilderRoot (handoff §32.2.6).
 *
 * A retained execution root whose View producer is owned by a boundary
 * (Tui body / ViewSlot content / ScrollPane content). The producer is part
 * of the transaction INVARIANT (§32.3): a failed replacement must restore
 * the previously authoritative producer and cancel any retry obligation
 * introduced solely by that attempted replacement. The implementation
 * assigns `currentProducer` optimistically and restores it in the failure
 * path — representation may vary; the invariant above may not.
 *
 * Ownership transitions (direct↔builder↔animation) are transactional too:
 * the new owner takes over only after its initial publication succeeds.
 */
export class OwnedBuilderRoot {
  readonly scope: RetainedExecutionScope;
  private currentProducer: () => View;
  private onFailure?: (previousProducer: () => View) => void;

  private constructor(
    runtime: RetainedExecutionRuntime,
    producer: () => View,
    target: PublicationTarget,
    onFailure?: (previousProducer: () => View) => void,
  ) {
    this.currentProducer = producer;
    this.onFailure = onFailure;
    // The scope renders whatever the CURRENT producer yields; the producer
    // itself is transactional state owned by this root (§32.2.6).
    const self = this;
    const componentType: ViewComponentType<void> = { render: () => self.currentProducer() };
    this.scope = new RetainedExecutionScope<void>(runtime, null, componentType, undefined, -1, undefined, NEXT_SCOPE_ID++);
    this.scope.publicationTarget = target;
  }

  /** Starts an owned builder root and evaluates its initial content. */
  static start(
    runtime: RetainedExecutionRuntime,
    producer: () => View,
    target: PublicationTarget,
    onFailure?: (previousProducer: () => View) => void,
  ): OwnedBuilderRoot {
    const root = new OwnedBuilderRoot(runtime, producer, target, onFailure);
    runtime.mountExistingRoot(root.scope);
    return root;
  }

  /**
   * Stages a new producer and re-drives the root synchronously. On success
   * the new producer becomes current; on failure the previous producer is
   * restored so the boundary keeps rendering its last authoritative content.
   */
  replaceProducer(producer: () => View): void {
    if (this.currentProducer === producer) return;
    const previous = this.currentProducer;
    // A scope already carrying an invalidation obligation keeps it across a
    // FAILED replacement: the restored producer plus the still-newer inputs
    // is exactly what the retry should render (post-R9 invariant §32.3).
    const hadPendingObligation = this.scope.dirty;
    this.currentProducer = producer;
    try {
      this.scope.runtime.update(this.scope);
    } catch (error) {
      // Restore: the failed producer must not be retried implicitly.
      this.currentProducer = previous;
      // Cancel ONLY the retry obligation introduced solely by this failed
      // attempt — the producer input itself was rolled back, so re-running is
      // pointless. Pre-existing obligations survive untouched.
      if (!hadPendingObligation) this.scope.runtime.cancelRetry(this.scope);
      throw error;
    }
  }

  dispose(): void {
    this.scope.runtime.detachRoot(this.scope);
  }
}
