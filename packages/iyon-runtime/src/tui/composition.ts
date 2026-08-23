/**
 * PERF-12 T13.1 Step 2 — Retained View Composition runtime (§8/§10/§11/§14/
 * §16/§25/§35).
 *
 * Framework-owned continuity metadata between successive declarative renders:
 * a composition root remembers, per logical construction address
 * (module + lexical site + occurrence-or-key), the last COMMITTED immutable
 * View so composition helpers can return the exact previous object on exact
 * semantic repeats (§17) and hand PERF-12 a structurally shared DAG.
 *
 * Hard boundaries (§26/§51): this layer stores View values only. It never
 * stores NodeIds as identity, NativeRefs, payload copies, or transport state,
 * and it is not a second semantic cache — the slot View IS the semantic
 * authority's own immutable value.
 *
 * Lifetime rules (§25): one committed View per live slot plus one pending
 * View during a pass; positional tails shrink on commit (never on abort);
 * keyed groups absent from a committed pass are removed at commit (never on
 * abort, §11.7); a site that was absent at the last committed-pass boundary
 * is logically unmounted — its previous value is NOT offered as a reuse
 * candidate when it reappears (§25.5 discontinuity) — without destroying
 * committed state while the new pass may still abort.
 *
 * Epoch discipline: every touchable record carries TWO epoch fields.
 * `touchEpoch` is the marker of the last pass that touched it (any pass,
 * including aborted ones) and drives per-pass dedupe/cursor logic. Only
 * commit() advances `mountedEpoch`, the "last COMMITTED pass that staged
 * this record" marker that continuity decisions compare against. This
 * separation is what makes aborts non-destructive: an aborted pass can never
 * make committed metadata look mounted, unmounted, displaced, or stale.
 */

import { compositionModuleSiteCount } from "./composition_registry.ts";
import type { View } from "./values/view.ts";

/** Local, scope-limited composition key (§11). Not a NodeId, not global. */
export type ViewKey = string | number;

export const COMPOSITION_DUPLICATE_KEY = "TUI_COMPOSITION_DUPLICATE_KEY";

export class CompositionError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "CompositionError";
  }
}

// --- Counters (§35): plain field increments on already-executing paths. ----

export interface CompositionCounters {
  composition_passes: number;
  composition_commits: number;
  composition_aborts: number;
  composition_modules_touched: number;
  composition_sites_touched: number;
  composition_positional_slot_hits: number;
  composition_positional_slot_misses: number;
  composition_keyed_slot_hits: number;
  composition_keyed_slot_misses: number;
  composition_exact_view_reuses: number;
  composition_new_views: number;
  composition_predecessor_hints: number;
  composition_duplicate_key_errors: number;
  composition_removed_positional_slots: number;
  composition_removed_keyed_slots: number;
  composition_untransformed_fallbacks: number;
}

export const compositionCounters: CompositionCounters = {
  composition_passes: 0,
  composition_commits: 0,
  composition_aborts: 0,
  composition_modules_touched: 0,
  composition_sites_touched: 0,
  composition_positional_slot_hits: 0,
  composition_positional_slot_misses: 0,
  composition_keyed_slot_hits: 0,
  composition_keyed_slot_misses: 0,
  composition_exact_view_reuses: 0,
  composition_new_views: 0,
  composition_predecessor_hints: 0,
  composition_duplicate_key_errors: 0,
  composition_removed_positional_slots: 0,
  composition_removed_keyed_slots: 0,
  composition_untransformed_fallbacks: 0,
};

const COUNTER_KEYS = Object.keys(compositionCounters) as Array<keyof CompositionCounters>;

export function compositionCounterSnapshot(): CompositionCounters {
  return structuredClone(compositionCounters);
}

export function resetCompositionCounters(): void {
  for (const key of COUNTER_KEYS) compositionCounters[key] = 0;
}

// --- Slot / bucket / scope tables (§8). ------------------------------------

export interface CompositionSlot {
  /** Last committed View for this logical address (strong, by design §25.1). */
  current: View | undefined;
  /** Staged View for the active pass; dropped on abort (§43.4). */
  pending: View | undefined;
  /** Marker of the last pass that touched this slot (any pass). */
  touchEpoch: number;
  /** Last committed pass that staged this slot (advanced only by commit()). */
  mountedEpoch: number;
  /**
   * Whether `current` belongs to a continuously mounted logical instance
   * (§25.5). Recomputed on every touch; false right after a committed
   * absence, so helpers treat the slot as a fresh mount.
   */
  continuous: boolean;
}

function createSlot(): CompositionSlot {
  return { current: undefined, pending: undefined, touchEpoch: 0, mountedEpoch: 0, continuous: false };
}

interface SiteBucket {
  /** Marker of the last pass that touched this bucket (any pass). */
  touchEpoch: number;
  /** Last committed pass that visited this bucket (advanced only by commit()). */
  mountedEpoch: number;
  /** Captured at first touch of a pass: whether the bucket was absent at the
   *  last committed-pass boundary (§25.5 stale detection). */
  staleAtTouch: boolean;
  /** Visits so far in the active pass (occurrence cursor, §10). */
  cursor: number;
  positional: CompositionSlot[];
  keyed: Map<ViewKey, KeyedGroup> | undefined;
}

function createBucket(): SiteBucket {
  return { touchEpoch: 0, mountedEpoch: 0, staleAtTouch: false, cursor: 0, positional: [], keyed: undefined };
}

function assertBuilding(pass: ViewCompositionPass): void {
  if (pass.state !== "building") {
    throw new CompositionError("TUI_COMPOSITION_STATE", `composition access from state ${pass.state}`);
  }
}

function markBucketTouched(
  bucket: SiteBucket,
  pass: ViewCompositionPass,
  scope: CompositionScope,
  moduleId: number,
): void {
  if (bucket.touchEpoch === pass.epoch) return;
  // Stale = the bucket did not take part in the last COMMITTED pass
  // (intervening aborted passes are not continuity).
  bucket.staleAtTouch = bucket.mountedEpoch !== pass.baseEpoch;
  bucket.touchEpoch = pass.epoch;
  pass.initialPositionalLengths.set(bucket, bucket.positional.length);
  bucket.cursor = 0;
  pass.touchedBuckets.push(bucket);
  if (!pass.touchedModules.has(moduleId)) {
    pass.touchedModules.add(moduleId);
    compositionCounters.composition_modules_touched += 1;
  }
  compositionCounters.composition_sites_touched += 1;
  if (scope.lastActiveEpoch !== pass.epoch) scope.lastActiveEpoch = pass.epoch;
}

interface KeyedGroup {
  readonly key: ViewKey;
  /** Child composition scope owned by this keyed instance (§11.6). */
  readonly scope: CompositionScope;
  /** Marker of the last pass that resolved this group (any pass). */
  touchEpoch: number;
  /** Last committed pass that visited this group (advanced only by commit()). */
  mountedEpoch: number;
}

/**
 * A scope owns the site tables for one composition region: a root's base
 * region or one keyed instance's nested region (§11.6).
 */
export class CompositionScope {
  readonly modules: Array<SiteBucket[] | undefined> = [];
  /** Epoch of the last pass that touched anything inside this scope. */
  lastActiveEpoch = 0;
}

function validateAddress(moduleId: number, siteId: number): void {
  if (!Number.isSafeInteger(moduleId) || moduleId < 0 || !Number.isSafeInteger(siteId) || siteId < 0) {
    throw new CompositionError("TUI_COMPOSITION_ADDRESS", `composition address must use non-negative safe integers: ${moduleId}/${siteId}`);
  }
}

function validateKey(key: ViewKey): void {
  if (typeof key === "string") return;
  if (typeof key === "number" && Number.isFinite(key)) return;
  throw new CompositionError("TUI_COMPOSITION_KEY", `composition key must be a finite number or string: ${String(key)}`);
}

function siteBucket(scope: CompositionScope, moduleId: number, siteId: number): SiteBucket {
  validateAddress(moduleId, siteId);
  const siteCount = compositionModuleSiteCount(moduleId);
  if (siteCount === undefined) throw new CompositionError("TUI_COMPOSITION_MODULE", `unregistered composition module ${moduleId}`);
  if (siteId >= siteCount) throw new CompositionError("TUI_COMPOSITION_SITE", `site ${siteId} is outside module ${moduleId} site count ${siteCount}`);
  let table = scope.modules[moduleId];
  if (table === undefined) {
    table = [];
    scope.modules[moduleId] = table;
  }
  let bucket = table[siteId];
  if (bucket === undefined) {
    bucket = createBucket();
    table[siteId] = bucket;
  }
  return bucket;
}

export type CompositionPassState = "building" | "prepared" | "committed" | "aborted";

interface CreatedGroupRecord {
  readonly map: Map<ViewKey, KeyedGroup>;
  readonly key: ViewKey;
  /** Committed group displaced by this pass's creation, if any (abort restore). */
  readonly previous: KeyedGroup | undefined;
}

export class ViewCompositionPass {
  readonly touchedSlots: CompositionSlot[] = [];
  readonly touchedBuckets: SiteBucket[] = [];
  readonly initialPositionalLengths = new Map<SiteBucket, number>();
  readonly touchedModules = new Set<number>();
  readonly createdGroups: CreatedGroupRecord[] = [];
  state: CompositionPassState = "building";

  constructor(
    readonly root: ViewCompositionRoot,
    /** Epoch of the last successful commit at begin() time. */
    readonly baseEpoch: number,
    /** This pass's unique epoch (touch-mark value for all its metadata). */
    readonly epoch: number,
    /** Active scope stack; bottom is the root's base scope (§14). */
    readonly scopeStack: CompositionScope[],
  ) {}

  prepare(): void {
    if (this.state !== "building") throw new CompositionError("TUI_COMPOSITION_STATE", `prepare() from state ${this.state}`);
    this.state = "prepared";
  }

  get topScope(): CompositionScope {
    return this.scopeStack[this.scopeStack.length - 1]!;
  }
}

export class ViewCompositionRoot {
  private committedEpoch = 0;
  private epochCounter = 0;
  private disposed = false;
  private activePass: ViewCompositionPass | undefined;
  /** Active buckets from the last committed pass (§25.5). */
  private activeBuckets: SiteBucket[] = [];
  private readonly baseScope = new CompositionScope();

  begin(): ViewCompositionPass {
    if (this.disposed) throw new CompositionError("TUI_COMPOSITION_DISPOSED", "composition root is disposed");
    if (this.activePass !== undefined) throw new CompositionError("TUI_COMPOSITION_ACTIVE", "composition root already has an active pass");
    this.epochCounter += 1;
    compositionCounters.composition_passes += 1;
    const pass = new ViewCompositionPass(this, this.committedEpoch, this.epochCounter, [this.baseScope]);
    this.activePass = pass;
    return pass;
  }

  /**
   * Commits the pass transactionally (§16): pending Views promote to current,
   * positional tails shrink to this pass's cardinality (§25.3), keyed groups
   * absent from this pass are removed (§25.4), and the committed-lifetime
   * markers (`mountedEpoch`) advance for exactly this pass's records. Commit
   * performs only pointer swaps, array truncations, and map deletions (§16.4).
   */
  commit(pass: ViewCompositionPass): void {
    if (pass.root !== this) throw new CompositionError("TUI_COMPOSITION_ROOT", "pass belongs to another root");
    if (pass.state === "building") pass.prepare();
    if (pass.state !== "prepared") throw new CompositionError("TUI_COMPOSITION_STATE", `commit() from state ${pass.state}`);
    for (const slot of pass.touchedSlots) {
      slot.current = slot.pending;
      slot.pending = undefined;
      slot.mountedEpoch = pass.epoch;
    }
    const touched = new Set(pass.touchedBuckets);
    for (const bucket of this.activeBuckets) {
      if (!touched.has(bucket)) clearBucket(bucket);
    }
    for (const bucket of pass.touchedBuckets) {
      if (bucket.positional.length > bucket.cursor) {
        compositionCounters.composition_removed_positional_slots += bucket.positional.length - bucket.cursor;
        bucket.positional.length = bucket.cursor;
      }
      if (bucket.keyed !== undefined) {
        for (const [key, group] of bucket.keyed) {
          if (group.touchEpoch !== pass.epoch) {
            bucket.keyed.delete(key);
            compositionCounters.composition_removed_keyed_slots += 1;
          } else {
            group.mountedEpoch = pass.epoch;
          }
        }
      }
      bucket.mountedEpoch = pass.epoch;
    }
    this.activeBuckets = pass.touchedBuckets.slice();
    this.committedEpoch = pass.epoch;
    pass.state = "committed";
    this.activePass = undefined;
    compositionCounters.composition_commits += 1;
  }

  /**
   * Aborts the pass (§11.7/§16.2/§43.4): committed state is untouched —
   * pending Views are released and groups created during this pass are
   * unlinked (restoring any displaced committed group) — so the old
   * composition remains authoritative. `touchEpoch` markers may hold the
   * aborted pass's epoch; continuity logic never compares against them.
   */
  abort(pass: ViewCompositionPass): void {
    if (pass.root !== this) throw new CompositionError("TUI_COMPOSITION_ROOT", "pass belongs to another root");
    if (pass.state !== "building" && pass.state !== "prepared") {
      throw new CompositionError("TUI_COMPOSITION_STATE", `abort() from state ${pass.state}`);
    }
    for (const slot of pass.touchedSlots) slot.pending = undefined;
    for (const bucket of pass.touchedBuckets) {
      bucket.positional.length = pass.initialPositionalLengths.get(bucket)!;
    }
    for (const record of pass.createdGroups) {
      if (record.previous === undefined) record.map.delete(record.key);
      else record.map.set(record.key, record.previous);
    }
    pass.state = "aborted";
    this.activePass = undefined;
    compositionCounters.composition_aborts += 1;
  }

  /** Releases every strong View reference held by this root (§25.6). */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.activePass = undefined;
    for (const bucket of this.activeBuckets) clearBucket(bucket);
    this.activeBuckets = [];
    disposeScope(this.baseScope);
  }

  // --- Address resolution used by the composition-context API below. -------

  /** Resolves the slot for one positional site visit (§10 occurrence rule). */
  currentPositionalSlot(pass: ViewCompositionPass, moduleId: number, siteId: number): CompositionSlot {
    assertBuilding(pass);
    const scope = pass.topScope;
    const bucket = siteBucket(scope, moduleId, siteId);
    markBucketTouched(bucket, pass, scope, moduleId);
    const occurrence = bucket.cursor;
    bucket.cursor += 1;
    let slot = bucket.positional[occurrence];
    if (slot === undefined) {
      slot = createSlot();
      bucket.positional[occurrence] = slot;
    }
    // §25.5 continuity: continuously mounted only when the slot was staged in
    // exactly the last committed pass inside a continuously visited bucket.
    // Recomputed unconditionally so aborted passes leave no residue.
    slot.continuous = !bucket.staleAtTouch && slot.mountedEpoch === pass.baseEpoch;
    slot.touchEpoch = pass.epoch;
    pass.touchedSlots.push(slot);
    if (slot.continuous && slot.current !== undefined) compositionCounters.composition_positional_slot_hits += 1;
    else compositionCounters.composition_positional_slot_misses += 1;
    return slot;
  }

  /**
   * Resolves (or creates) the keyed group for one keyed site visit (§11).
   * The caller pushes the returned group's child scope via
   * `withCompositionScope`. Duplicate keys at the same site within one pass
   * are a deterministic error (§11.4).
   */
  keyedGroup(pass: ViewCompositionPass, moduleId: number, siteId: number, key: ViewKey): KeyedGroup {
    assertBuilding(pass);
    validateKey(key);
    const scope = pass.topScope;
    const bucket = siteBucket(scope, moduleId, siteId);
    markBucketTouched(bucket, pass, scope, moduleId);
    let map = bucket.keyed;
    if (map === undefined) {
      map = new Map();
      bucket.keyed = map;
    }
    const existing = map.get(key);
    if (existing !== undefined && existing.touchEpoch === pass.epoch) {
      compositionCounters.composition_duplicate_key_errors += 1;
      throw new CompositionError(
        COMPOSITION_DUPLICATE_KEY,
        `duplicate composition key ${String(key)} at module ${moduleId} site ${siteId}`,
      );
    }
    if (existing !== undefined && existing.mountedEpoch === pass.baseEpoch) {
      // Continuously mounted keyed instance: resume its child scope (§38).
      existing.touchEpoch = pass.epoch;
      if (existing.scope.lastActiveEpoch !== pass.epoch) existing.scope.lastActiveEpoch = pass.epoch;
      compositionCounters.composition_keyed_slot_hits += 1;
      return existing;
    }
    // Fresh instance, or an existing group absent at the last committed
    // boundary (§25.5: not continuously mounted — start a fresh instance).
    const group: KeyedGroup = { key, scope: new CompositionScope(), touchEpoch: pass.epoch, mountedEpoch: 0 };
    pass.createdGroups.push({ map, key, previous: existing });
    map.set(key, group);
    compositionCounters.composition_keyed_slot_misses += 1;
    return group;
  }
}

function clearBucket(bucket: SiteBucket): void {
  compositionCounters.composition_removed_positional_slots += bucket.positional.length;
  compositionCounters.composition_removed_keyed_slots += bucket.keyed?.size ?? 0;
  for (const slot of bucket.positional) {
    slot.current = undefined;
    slot.pending = undefined;
  }
  bucket.positional.length = 0;
  bucket.keyed?.clear();
  bucket.mountedEpoch = 0;
  bucket.cursor = 0;
}

function disposeScope(scope: CompositionScope): void {
  for (const table of scope.modules) {
    if (table === undefined) continue;
    for (const bucket of table) {
      if (bucket === undefined) continue;
      for (const slot of bucket.positional) {
        slot.current = undefined;
        slot.pending = undefined;
      }
      bucket.positional.length = 0;
      if (bucket.keyed !== undefined) {
        for (const group of bucket.keyed.values()) disposeScope(group.scope);
        bucket.keyed.clear();
      }
    }
  }
  scope.modules.length = 0;
}

// --- Composition context (§14): synchronous, nesting-safe. -----------------

const CONTEXT_STACK: ViewCompositionPass[] = [];

export function activeCompositionPass(): ViewCompositionPass | undefined {
  return CONTEXT_STACK[CONTEXT_STACK.length - 1];
}

export function pushCompositionPass(pass: ViewCompositionPass): void {
  CONTEXT_STACK.push(pass);
}

export function popCompositionPass(pass: ViewCompositionPass): void {
  const popped = CONTEXT_STACK.pop();
  if (popped !== pass) throw new CompositionError("TUI_COMPOSITION_CONTEXT", "composition context stack corrupted");
}

/**
 * Runs `build` synchronously inside `scope`. Composition builders are
 * synchronous by contract (§52.9); a returned Promise is rejected.
 */
export function withCompositionScope<T>(pass: ViewCompositionPass, scope: CompositionScope, build: () => T): T {
  assertBuilding(pass);
  pass.scopeStack.push(scope);
  try {
    const result = build();
    if (isPromiseLike(result)) {
      throw new CompositionError("TUI_COMPOSITION_ASYNC", "composition builders must be synchronous");
    }
    return result;
  } finally {
    pass.scopeStack.pop();
  }
}

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  if ((typeof value !== "object" || value === null) && typeof value !== "function") return false;
  return typeof (value as { then?: unknown }).then === "function";
}

// --- Slot staging / reuse primitives used by composition helpers. ----------

/**
 * The reuse candidate for a touched slot: the previous committed View only
 * when the logical instance is continuously mounted (§25.5).
 */
export function slotReuseCandidate(slot: CompositionSlot): View | undefined {
  if (slot.continuous && slot.current !== undefined) return slot.current;
  return undefined;
}

/** Stages the pass's value for a slot (pointer write only, §16.4). */
export function stageSlotValue(slot: CompositionSlot, view: View | undefined): void {
  slot.pending = view;
}

/** Counters used by generated/handwritten monomorphic compose helpers. */
export function noteExactViewReuse(): void {
  compositionCounters.composition_exact_view_reuses += 1;
}

export function noteNewView(): void {
  compositionCounters.composition_new_views += 1;
}

export function notePredecessorHint(): void {
  compositionCounters.composition_predecessor_hints += 1;
}

export function noteUntransformedFallback(): void {
  compositionCounters.composition_untransformed_fallbacks += 1;
}
