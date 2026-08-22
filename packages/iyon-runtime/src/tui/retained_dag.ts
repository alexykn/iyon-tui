/**
 * PERF-12 T6: retained-DAG identity fast paths (handoff §18–§21, §48, §15/§16).
 *
 * The immutable BridgeViewNode DAG is the semantic declaration; the Rust View
 * DAG is the retained native representation; a NativeRef is the correspondence
 * between them. This module owns the JS side of that correspondence:
 *
 * - `BRIDGE_NATIVE`: generation-scoped NativeRef hints in a WeakMap sidecar.
 *   Hints are weak acceleration, never per-node leases (§16).
 * - `ensureNative` (§19): identity-first resolution — hint, transaction-local
 *   ref, ceiling-gated NodeId→NativeRef promotion, then — only for genuinely
 *   new nodes — payload inspection and generated direct materialization.
 * - exact-root fast path (§20): one `hostRenderRef`, zero semantic field
 *   reads, zero buffer writes.
 * - `RetainedRootBoundary` (§18): the root-lease protocol every View-bearing
 *   boundary follows — previous root stays leased until the replacement is
 *   fully materialized and installed; temporary leases drain in one batch;
 *   the private NodeId high-water is captured as `nativeLookupCeiling`.
 *
 * Production routing is NOT switched over here (T13 routes boundaries); this
 * module is exercised through its own conformance and scaling suites until
 * then. Only materializers emitted by the canonical generator are available;
 * unknown kinds raise `RetainedFastFallbackError` so callers route to the
 * complete cold path (§49).
 */

import type { Pointer } from "bun:ffi";
import {
  materializeColumn,
  materializeRow,
  materializeSpacer,
} from "./generated/view_materialize.ts";
import { hostRenderRef, viewRefForNodeId, viewReleaseMany } from "./generated/view_calls.ts";
import { BRIDGE_VIEW_KIND, type BridgeViewNode } from "./ir.ts";
import { nodeForBridge, viewNodeIdHighWater, type View } from "./values/view.ts";
import type { NativeViewAbiSession } from "./native_view_abi.ts";
import {
  MAX_DIRECT_AXIS_REFS,
  MAX_RETAINED_DEPTH,
  MAX_RETAINED_NEW_NODES,
} from "./native_view_policy.ts";

/** Generation-scoped NativeRef hint; weak acceleration only (§15/§16). */
export interface BridgeNativeHint {
  readonly generation: number;
  readonly nativeRef: number;
}

/** NodeId → NativeRef hints. Values die with the semantic node (§15). */
const BRIDGE_NATIVE = new WeakMap<BridgeViewNode, BridgeNativeHint>();

/**
 * PERF-12 T8 (§30): environment-level reusable axis-ref scratch (small tier).
 * Keyed by runtime pointer so a re-bootstrapped session allocates fresh
 * storage; native retains no pointer into it after any call returns (§29).
 * A plain Map: runtime pointers are numbers, and there is at most one live
 * entry per environment.
 */
const AXIS_REF_SCRATCH = new Map<Pointer, Uint32Array>();

/**
 * Structural counters (§91 subset relevant to T6). Plain field increments on
 * already-executing paths — the "compile-time-cheap" arm of §56/§101/§68; no
 * scans, no atomics, no allocation. They prove asymptotic behavior
 * independently of timing noise (exact root = 0 field reads / 0 ref words).
 */
export interface RetainedIdentityCounters {
  bridge_hint_hits: number;
  bridge_hint_misses: number;
  node_id_ref_promotion_attempts: number;
  node_id_ref_promotion_hits: number;
  node_id_ref_promotion_misses: number;
  bridge_semantic_nodes_inspected: number;
  bridge_children_visited: number;
  direct_materializer_calls: number;
  derivation_fast_path_calls: number;
  ref_words_written: number;
  byte_payload_bytes: number;
  transport_scratch_reuses: number;
  stale_ref_retries: number;
  cold_fallbacks: number;
  host_mutations: number;
}

const counters: RetainedIdentityCounters = {
  bridge_hint_hits: 0,
  bridge_hint_misses: 0,
  node_id_ref_promotion_attempts: 0,
  node_id_ref_promotion_hits: 0,
  node_id_ref_promotion_misses: 0,
  bridge_semantic_nodes_inspected: 0,
  bridge_children_visited: 0,
  direct_materializer_calls: 0,
  derivation_fast_path_calls: 0,
  ref_words_written: 0,
  byte_payload_bytes: 0,
  transport_scratch_reuses: 0,
  stale_ref_retries: 0,
  cold_fallbacks: 0,
  host_mutations: 0,
};

export function retainedIdentityCounterSnapshot(): RetainedIdentityCounters {
  return { ...counters };
}

export function resetRetainedIdentityCounters(): void {
  for (const key of Object.keys(counters) as (keyof RetainedIdentityCounters)[]) {
    counters[key] = 0;
  }
}

/** Raised when a node cannot be materialized on the retained path (§49/§50). */
export class RetainedFastFallbackError extends Error {
  constructor(reason: string) {
    super(`retained fast fallback: ${reason}`);
    this.name = "RetainedFastFallbackError";
  }
}

/** Raised on corrupt/private cyclic input (§75); public DAGs cannot do this. */
export class RetainedCycleError extends Error {
  constructor() {
    super("retained materialization cycle detected");
    this.name = "RetainedCycleError";
  }
}

/**
 * Transaction-local state for one retained root materialization (§44).
 * Nothing here escapes the call; newly-created refs stay leased until the
 * root is installed, then all non-root leases drain through `viewReleaseMany`.
 */
export class MaterializeTx {
  readonly refs = new Map<BridgeViewNode, number>();
  readonly inProgress = new Set<BridgeViewNode>();
  /** Refs this tx must release unless ownership transfers to the boundary. */
  readonly temporaryLeases: number[] = [];
  /** Hint hits borrowed for this tx; no lease was taken (§16/§47). */
  readonly borrowedHints: { readonly node: BridgeViewNode; readonly nativeRef: number }[] = [];
  newNodeCount = 0;
  depth = 0;

  constructor(
    readonly symbols: NativeViewAbiSession["symbols"],
    readonly runtime: Pointer,
    readonly generation: number,
    readonly nativeLookupCeiling: number,
  ) {}

  noteBorrowedHint(node: BridgeViewNode, nativeRef: number): void {
    this.borrowedHints.push({ node, nativeRef });
  }

  /**
   * PERF-12 T8 (§29/§30): the reusable borrowed scratch for one variable-axis
   * transport. The small tier is a single environment-level Uint32Array
   * allocated once and reused by every transaction (single owner thread, no
   * pointer outlives the synchronous call). Counts above MAX_DIRECT_AXIS_REFS
   * refuse the retained path entirely (§30 cap rule / §50).
   */
  axisRefScratch(childCount: number): Uint32Array {
    if (childCount > MAX_DIRECT_AXIS_REFS) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError(
        `axis arity ${childCount} exceeds the retained buffer cap ${MAX_DIRECT_AXIS_REFS}`,
      );
    }
    const words = childCount * 2;
    let scratch = AXIS_REF_SCRATCH.get(this.runtime);
    if (scratch === undefined || scratch.length < words) {
      // Small tier sized for exactly the retained cap; allocated once.
      scratch ??= new Uint32Array(MAX_DIRECT_AXIS_REFS * 2);
      AXIS_REF_SCRATCH.set(this.runtime, scratch);
    }
    counters.transport_scratch_reuses += 1;
    return scratch.subarray(0, words);
  }

  /** §90: borrowed-buffer preparation is counted, never hidden. */
  noteRefWords(words: number): void {
    counters.ref_words_written += words;
  }

  /** Releases every temporary lease (failure path / non-root drains). */
  releaseAll(): void {
    if (this.temporaryLeases.length === 0) return;
    const batch = Uint32Array.from(this.temporaryLeases);
    viewReleaseMany(this.symbols, this.runtime, batch, batch.length);
    this.temporaryLeases.length = 0;
  }

  /** Releases every temporary lease except `keepRef` (success path, §18.4). */
  releaseAllExcept(keepRef: number): void {
    const remaining = this.temporaryLeases.filter((ref) => ref !== keepRef);
    this.temporaryLeases.length = 0;
    if (remaining.length === 0) return;
    const batch = Uint32Array.from(remaining);
    viewReleaseMany(this.symbols, this.runtime, batch, batch.length);
  }
}

type NodeMaterializer = (node: BridgeViewNode, tx: MaterializeTx) => number;

function isExpectedNativeStatus(error: unknown): boolean {
  return error instanceof Error && /^native ABI status 0x[0-9a-f]+$/u.test(error.message);
}

/**
 * Runs one generated lowering and converts expected native failure statuses
 * into a fast fallback so the caller routes the complete cold path (§49).
 * Unexpected errors propagate.
 */
function runMaterializer(kind: string, lower: () => number): number {
  try {
    return lower();
  } catch (error) {
    if (isExpectedNativeStatus(error)) {
      throw new RetainedFastFallbackError(`${kind} constructor reported a native failure status`);
    }
    throw error;
  }
}

function materializeSpacerNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.spacer) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  return runMaterializer("spacer", () =>
    materializeSpacer(node as unknown as Parameters<typeof materializeSpacer>[0], tx),
  );
}

function materializeRowNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.row) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_children_visited += node.children.length;
  counters.bridge_semantic_nodes_inspected += 1;
  return runMaterializer("row", () => materializeRow(node as unknown as Parameters<typeof materializeRow>[0], tx));
}

function materializeColumnNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.column) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_children_visited += node.children.length;
  counters.bridge_semantic_nodes_inspected += 1;
  return runMaterializer(
    "column",
    () => materializeColumn(node as unknown as Parameters<typeof materializeColumn>[0], tx),
  );
}

/**
 * Per-kind generated materializer dispatch (§22 children-first, §32 fixed
 * arities). T7 covers spacer plus row/column arities 0..=4; container,
 * clamp, hanging, grid, text, diff, decorated, and component route to the
 * complete fallback until their owning tranches land. Unknown kinds fall
 * back instead of guessing (§49).
 */
const MATERIALIZERS = new Map<number, NodeMaterializer>([
  [BRIDGE_VIEW_KIND.spacer, materializeSpacerNode],
  [BRIDGE_VIEW_KIND.row, materializeRowNode],
  [BRIDGE_VIEW_KIND.column, materializeColumnNode],
]);

/**
 * Installs or refreshes the NativeRef hint for a node under the tx's
 * generation. Hints are acceleration only; they never take a lease (§16).
 */
function installHint(node: BridgeViewNode, generation: number, nativeRef: number): void {
  BRIDGE_NATIVE.set(node, { generation, nativeRef });
}

/** Test/diagnostic peek at a node's current hint. */
export function peekBridgeNativeHint(node: BridgeViewNode): BridgeNativeHint | undefined {
  return BRIDGE_NATIVE.get(node);
}

/** Test-only hint injection (stale-generation and stale-ref scenarios). */
export function forceBridgeNativeHintForTests(node: BridgeViewNode, hint: BridgeNativeHint): void {
  BRIDGE_NATIVE.set(node, hint);
}

/** Test-only hint removal (stale-child recovery scenarios, §47). */
export function deleteBridgeNativeHintForTests(node: BridgeViewNode): void {
  BRIDGE_NATIVE.delete(node);
}

function splitNodeId(id: number): [number, number] {
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

/**
 * Core identity-first resolution (§19). Hard ordering:
 * BRIDGE_NATIVE lookup → transaction-local ref → ceiling-gated
 * NodeId→NativeRef promotion → kind/payload inspection → child traversal.
 */
export function ensureNative(node: BridgeViewNode, tx: MaterializeTx): number {
  const hint = BRIDGE_NATIVE.get(node);
  if (hint !== undefined && hint.generation === tx.generation) {
    counters.bridge_hint_hits += 1;
    tx.noteBorrowedHint(node, hint.nativeRef);
    return hint.nativeRef;
  }

  const local = tx.refs.get(node);
  if (local !== undefined) return local;

  // A previous cold/direct/native path may have materialized this semantic
  // NodeId without ever installing a JS-side hint. Only nodes that existed
  // before the boundary's last successful commit are eligible; genuinely new
  // NodeIds skip the extra FFI probe entirely (§19).
  if (node.id <= tx.nativeLookupCeiling) {
    counters.node_id_ref_promotion_attempts += 1;
    const [low, high] = splitNodeId(node.id);
    try {
      const recovered = viewRefForNodeId(tx.symbols, tx.runtime, low, high);
      counters.node_id_ref_promotion_hits += 1;
      installHint(node, tx.generation, recovered);
      tx.refs.set(node, recovered);
      tx.temporaryLeases.push(recovered);
      return recovered;
    } catch (error) {
      if (!isExpectedNativeStatus(error)) throw error;
      counters.node_id_ref_promotion_misses += 1;
    }
  }

  if (tx.inProgress.has(node)) throw new RetainedCycleError();
  if (++tx.newNodeCount > MAX_RETAINED_NEW_NODES) {
    counters.cold_fallbacks += 1;
    throw new RetainedFastFallbackError(`new-node budget ${MAX_RETAINED_NEW_NODES} exceeded`);
  }
  if (tx.depth >= MAX_RETAINED_DEPTH) {
    counters.cold_fallbacks += 1;
    throw new RetainedFastFallbackError(`depth budget ${MAX_RETAINED_DEPTH} exceeded`);
  }

  tx.inProgress.add(node);
  try {
    const materializer = MATERIALIZERS.get(node.kind);
    if (materializer === undefined) {
      counters.bridge_semantic_nodes_inspected += 1;
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError(`no generated materializer for kind ${node.kind}`);
    }
    counters.direct_materializer_calls += 1;
    tx.depth += 1;
    let reference: number;
    try {
      reference = materializer(node, tx);
    } finally {
      tx.depth -= 1;
    }
    installHint(node, tx.generation, reference);
    tx.refs.set(node, reference);
    tx.temporaryLeases.push(reference);
    return reference;
  } finally {
    tx.inProgress.delete(node);
  }
}

/** Host render statuses returned by the generated `hostRenderRef`. */
const HOST_STATUS_OK = 0;
const HOST_STATUS_CACHE_MISS = 1;

function isValidNativeRef(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0 && value < 0x8000_0000;
}

export type ExactRootRender =
  | { readonly status: "ok"; readonly rootRef: number; readonly recovered: boolean }
  | { readonly status: "no_root_ref"; readonly rootRef?: undefined };

/**
 * Renders a host with a View whose root hint is already warm (§20).
 *
 * Required structural shape on the hit path: exactly one `hostRenderRef`
 * call, zero semantic payload reads, zero children visited, zero buffer
 * words written, zero node constructors. Independent of descendant count.
 *
 * On `FAST_CACHE_MISS` the single targeted recovery of §47 applies: drop the
 * stale hint, re-acquire by NodeId promotion, retry once. Anything else is
 * propagated to the caller's fallback handling.
 */
export function renderExactRoot(
  session: NativeViewAbiSession,
  hostPointer: Pointer,
  view: View,
): ExactRootRender {
  const node = nodeForBridge(view);
  const generation = session.abi.generation;
  const hint = BRIDGE_NATIVE.get(node);

  if (hint !== undefined && hint.generation === generation) {
    counters.bridge_hint_hits += 1;
    const status = hostRenderRef(session.symbols, session.runtime, hostPointer, hint.nativeRef);
    if (status === HOST_STATUS_OK) {
      counters.host_mutations += 1;
      return { status: "ok", rootRef: hint.nativeRef, recovered: false };
    }
    if (status === HOST_STATUS_CACHE_MISS) {
      // One targeted retry (§47 hard rule): the hinted ref went stale.
      counters.stale_ref_retries += 1;
      counters.node_id_ref_promotion_attempts += 1;
      BRIDGE_NATIVE.delete(node);
      const [low, high] = splitNodeId(node.id);
      try {
        const recoveredRef = viewRefForNodeId(session.symbols, session.runtime, low, high);
        counters.node_id_ref_promotion_hits += 1;
        installHint(node, generation, recoveredRef);
        const retryStatus = hostRenderRef(session.symbols, session.runtime, hostPointer, recoveredRef);
        if (retryStatus === HOST_STATUS_OK) {
          counters.host_mutations += 1;
          return { status: "ok", rootRef: recoveredRef, recovered: true };
        }
        BRIDGE_NATIVE.delete(node);
        return { status: "no_root_ref" };
      } catch (error) {
        if (!isExpectedNativeStatus(error)) throw error;
        return { status: "no_root_ref" };
      }
    }
    throw new Error(`hostRenderRef failed with status ${status}`);
  }

  return { status: "no_root_ref" };
}

/**
 * Acquires the NativeRef for a root that already exists natively (decoded by
 * the Direct path or built by an earlier transport) without walking the tree:
 * one ceiling-free NodeId→NativeRef promotion, one hint installation. Used by
 * `RetainedRootBoundary.adopt`; the returned ref carries one lease the
 * boundary owns as its root lease.
 */
export function acquireKnownRoot(session: NativeViewAbiSession, view: View): number | undefined {
  const node = nodeForBridge(view);
  const [low, high] = splitNodeId(node.id);
  counters.node_id_ref_promotion_attempts += 1;
  try {
    const reference = viewRefForNodeId(session.symbols, session.runtime, low, high);
    counters.node_id_ref_promotion_hits += 1;
    installHint(node, session.abi.generation, reference);
    return reference;
  } catch (error) {
    if (!isExpectedNativeStatus(error)) throw error;
    counters.node_id_ref_promotion_misses += 1;
    return undefined;
  }
}

/**
 * Root-lease protocol for one View-bearing boundary (§18).
 *
 * ```text
 * update:
 *   1. keep previousRef leased
 *   2. materialize next root (ensureNative)
 *   3. hostRenderRef(nextRef)
 *   4. success → release previousRef, transfer nextRef temp lease to
 *      boundary.previousRef, release every other temporary ref
 *      failure → keep previousRef, release every temporary ref
 * close:
 *   release previousRef exactly once
 * ```
 *
 * After each successful commit the private NodeId allocator high-water is
 * captured as `nativeLookupCeiling`, letting later sidecar misses distinguish
 * definitely-new NodeIds from older possibly-cached ones (§19).
 */
export class RetainedRootBoundary {
  private previousRef: number | undefined;
  private closed = false;
  /** NodeId allocator high-water at the last successful commit (§18). */
  nativeLookupCeiling = 0;

  constructor(
    private readonly session: NativeViewAbiSession,
    private readonly hostPointer: () => Pointer | undefined,
  ) {}

  /**
   * Adopts a root that already exists natively as the boundary's first root:
   * unconditional NodeId promotion, root-lease transfer, ceiling capture.
   * This is how a boundary takes over state the Direct decoder published.
   */
  adopt(view: View): boolean {
    if (this.closed) throw new Error("boundary is closed");
    const reference = acquireKnownRoot(this.session, view);
    if (reference === undefined) return false;
    this.transferRoot(reference);
    return true;
  }

  /**
   * Full §18 replace sequence via `ensureNative`. Returns the new root ref,
   * or undefined when the retained path fell back (caller routes the complete
   * cold path; the old root remains installed and leased).
   *
   * Lease ownership rule: the boundary's previousRef always carries exactly
   * one lease the boundary owns. A root resolved through a borrowed hint
   * (another owner holds the lease, e.g. a second host rendering the same
   * semantic root, §115) therefore acquires the boundary's own lease by
   * NodeId before installation.
   */
  install(view: View): number | undefined {
    if (this.closed) throw new Error("boundary is closed");
    const tx = new MaterializeTx(
      this.session.symbols,
      this.session.runtime,
      this.session.abi.generation,
      this.nativeLookupCeiling,
    );
    let resolvedRef: number;
    try {
      resolvedRef = ensureNative(nodeForBridge(view), tx);
    } catch (error) {
      // Fallback, cycle guard, and unexpected errors all drain every
      // temporary lease before the caller sees the failure.
      tx.releaseAll();
      if (error instanceof RetainedFastFallbackError || error instanceof RetainedCycleError) return undefined;
      throw error;
    }
    // Does this tx own a lease on the resolved ref (promotion/materialization),
    // or was it borrowed from a hint whose lease belongs to someone else?
    const ownsTempLease = tx.temporaryLeases.includes(resolvedRef);
    let rootRef = resolvedRef;
    let acquiredBoundaryLease = false;
    if (!ownsTempLease && resolvedRef !== this.previousRef) {
      counters.node_id_ref_promotion_attempts += 1;
      const [low, high] = splitNodeId(nodeForBridge(view).id);
      try {
        rootRef = viewRefForNodeId(this.session.symbols, this.session.runtime, low, high);
        counters.node_id_ref_promotion_hits += 1;
        acquiredBoundaryLease = true;
      } catch (error) {
        tx.releaseAll();
        if (!isExpectedNativeStatus(error)) throw error;
        counters.node_id_ref_promotion_misses += 1;
        return undefined;
      }
    }
    const hostPointer = this.hostPointer();
    if (hostPointer !== undefined) {
      const status = hostRenderRef(this.session.symbols, this.session.runtime, hostPointer, rootRef);
      if (status !== HOST_STATUS_OK) {
        tx.releaseAll();
        // Release only a freshly acquired boundary lease whose ref is not
        // already the boundary's leased previous root (§18 failure keeps the
        // old root installed).
        if (acquiredBoundaryLease && rootRef !== this.previousRef) {
          const batch = Uint32Array.of(rootRef);
          viewReleaseMany(this.session.symbols, this.session.runtime, batch, 1);
        }
        return undefined;
      }
      counters.host_mutations += 1;
    }
    if (ownsTempLease) tx.releaseAllExcept(rootRef);
    else tx.releaseAll();
    this.transferRoot(rootRef);
    return rootRef;
  }

  /** §20 exact-root fast path against the currently installed root hint. */
  renderExact(view: View): ExactRootRender {
    if (this.closed) throw new Error("boundary is closed");
    const hostPointer = this.hostPointer();
    if (hostPointer === undefined) return { status: "no_root_ref" };
    return renderExactRoot(this.session, hostPointer, view);
  }

  /** Releases the boundary's root lease exactly once (§18 close protocol). */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    const ref = this.previousRef;
    this.previousRef = undefined;
    if (ref !== undefined && isValidNativeRef(ref)) {
      const batch = Uint32Array.of(ref);
      viewReleaseMany(this.session.symbols, this.session.runtime, batch, 1);
    }
  }

  private transferRoot(reference: number): void {
    const previous = this.previousRef;
    this.previousRef = reference;
    this.nativeLookupCeiling = viewNodeIdHighWater();
    if (previous !== undefined && previous !== reference && isValidNativeRef(previous)) {
      const batch = Uint32Array.of(previous);
      viewReleaseMany(this.session.symbols, this.session.runtime, batch, 1);
    }
  }
}
