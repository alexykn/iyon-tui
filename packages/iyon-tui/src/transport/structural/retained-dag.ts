/**
 * PERF-12 retained-DAG identity fast paths over the semantic View model.
 *
 * The semantic node DAG is the declaration; this module owns the physical
 * NativeRef correspondence and generated structural calls:
 *
 * - `SEMANTIC_NATIVE`: generation-scoped NativeRef hints in a WeakMap sidecar.
 *   Hints are weak acceleration, never per-node leases (§16).
 * - `ensureNative` (§19): identity-first resolution — hint, transaction-local
 *   ref, ceiling-gated NodeId→NativeRef promotion, then — only for genuinely
 *   new nodes — semantic payload inspection and direct ABI materialization.
 * - exact-root fast path (§20): one `hostRenderRef`, zero semantic field
 *   reads, zero buffer writes.
 * - `RetainedRootBoundary` (§18): the root-lease protocol every View-bearing
 *   boundary follows — previous root stays leased until the replacement is
 *   fully materialized and installed; temporary leases drain in one batch;
 *   the private NodeId high-water is captured as `nativeLookupCeiling`.
 *
 * The retained path consumes semantic nodes directly. Complete bridge objects
 * are produced only by the separate cold fallback.
 */

import { native, type NativeTuiHostContract, type NativeViewAbiHandle } from "../native/addon.ts";
import { NativeAbiStatusError, axisBuilderBegin, axisBuilderFinish, axisBuilderPush, axisBuilderAbort, hostRenderRef, viewStateAttach, styleAtomCreateCstring, styleCreateBits, viewAxisCreateBuffer, viewAxisSetChild, viewAxisSpliceBuffer, viewClampCreate, viewCommonPatchRoot, viewColumnCreate0, viewColumnCreate1, viewColumnCreate2, viewColumnCreate3, viewColumnCreate4, viewComponentCreate, viewContainerCreate, viewDecoratedCreateBuffer, viewDiffCreateBuffer, viewGridCreateBuffer, viewGridSetCell, viewHangingCreate, viewRefForNodeId, viewReleaseMany, viewRenderRef, viewRowCreate0, viewRowCreate1, viewRowCreate2, viewRowCreate3, viewRowCreate4, viewSpacerCreate, viewTextCreateCstring, viewTextCreateCstring2, viewTextCreateCstring3, viewTextCreateCstring4, viewTextCreateUtf8, viewTextCreateUtf82, viewTextCreateUtf83, viewTextCreateUtf84, viewTextLayoutPatchRoot } from "../abi/structural/generated/view_calls.ts";
import {
  axisKind,
  axisTrackWord,
  colorAtomValue,
  commonScalarEncoding,
  decorationWordEncoding,
  diffLineMetadata,
  gridCellAlignmentWord,
  gridCellSpanWord,
  gridTrackWord,
  horizontalAlignCode,
  layoutTrackWord,
  overflowKindCode,
  styleAttributeEncoding,
  u64Words,
  wrapModeCode,
} from "./encoding.ts";
import { isSemanticViewNode, semanticNodeOf, peekSemanticDerivation, peekSemanticGridSequenceOverride, peekSemanticSequenceOverride, SEMANTIC_VIEW_KIND, type SemanticColor, type SemanticDerivation, type SemanticLayoutChild, type SemanticStyle, type SemanticViewNode } from "../../api/view/semantic-node.ts";
import { componentIdForHandleId } from "./component-id.ts";
import { nativeResourceForHandleId } from "../native/resources.ts";
import type { NativeViewStateContract } from "../native/addon.ts";
import { lowerSemanticView } from "./cold-lowering.ts";
import { viewNodeIdHighWater, type View } from "../../api/view/view.ts";
import type { NativeViewAbiSession } from "./native-view-abi.ts";
import {
  MAX_DIRECT_AXIS_REFS,
  MAX_DIRECT_DIFF_BYTES,
  MAX_DIRECT_DIFF_WORDS,
  MAX_DIRECT_GRID_WORDS,
  MAX_DIRECT_TEXT_BYTES,
  MAX_RETAINED_DEPTH,
  MAX_RETAINED_NEW_NODES,
} from "./policy.ts";

/** Generation-scoped NativeRef hint; weak acceleration only (§15/§16). */
export interface SemanticNativeHint {
  readonly generation: number;
  readonly nativeRef: number;
}

/** NodeId → NativeRef hints. Values die with the semantic node (§15). */
const SEMANTIC_NATIVE = new WeakMap<SemanticViewNode, SemanticNativeHint>();

/**
 * PERF-12 T8 (§30): environment-level reusable axis-ref scratch (small tier).
 * One buffer is retained per active materialization depth. A parent may keep a
 * borrowed buffer live while a child is materialized, so a single global slot
 * would let the child overwrite the parent's ABI input before its call.
 * Native retains no pointer into these buffers after any call returns (§29).
 */
const AXIS_REF_SCRATCH: {
  runtime: NativeViewAbiHandle | undefined;
  arrays: Uint32Array[];
} = {
  runtime: undefined,
  arrays: [],
};

/** PERF-12 T10 (§30/§36): reusable medium-tier flat-grid word scratch. */
const GRID_WORD_SCRATCH: {
  runtime: NativeViewAbiHandle | undefined;
  arrays: Uint32Array[];
} = {
  runtime: undefined,
  arrays: [],
};

/** PERF-12 T11 (§30): reusable byte tier for text/diff UTF-8 payloads. */
const BYTE_SCRATCH: { runtime: NativeViewAbiHandle | undefined; array: Uint8Array } = {
  runtime: undefined,
  array: new Uint8Array(0),
};

/**
 * PERF-12 T11 (§40): generation-scoped StyleRef sidecar. Stable style objects
 * map to native style refs exactly once per runtime generation; the native
 * runtime's style table stays the only authoritative style cache (§40 bans a
 * second one — this sidecar is acceleration metadata, like SEMANTIC_NATIVE).
 * Refs die with the generation; the single-slot reset mirrors AXIS_REF_SCRATCH.
 */
const STYLE_REF_CACHE: {
  runtime: NativeViewAbiHandle | undefined;
  generation: number;
  refs: WeakMap<object, number>;
  atoms: Map<string, number>;
} = { runtime: undefined, generation: 0, refs: new WeakMap(), atoms: new Map() };

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

/** §90 phase visibility hook used by isolated benchmark harnesses only. */
export interface RetainedPhaseSample {
  readonly transport_prepare_ns: number;
  readonly native_materialize_ns: number;
  readonly host_commit_ns: number;
}

export interface RetainedPhaseInstrumentation {
  readonly now_ns: () => number;
  readonly record: (sample: RetainedPhaseSample) => void;
}

let phaseInstrumentation: RetainedPhaseInstrumentation | undefined;

export function setRetainedPhaseInstrumentation(
  instrumentation: RetainedPhaseInstrumentation | undefined,
): void {
  phaseInstrumentation = instrumentation;
}

function phaseNow(): number | undefined {
  return phaseInstrumentation?.now_ns();
}

function recordPhaseSample(sample: RetainedPhaseSample): void {
  phaseInstrumentation?.record(sample);
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
  readonly refs = new Map<SemanticViewNode, number>();
  readonly inProgress = new Set<SemanticViewNode>();
  /** Refs this tx must release unless ownership transfers to the boundary. */
  readonly temporaryLeases: number[] = [];
  /** Hint hits borrowed for this tx; no lease was taken (§16/§47). */
  readonly borrowedHints: { readonly node: SemanticViewNode; readonly nativeRef: number }[] = [];
  newNodeCount = 0;
  depth = 0;
  /** One targeted stale-ref recovery is allowed per root transaction (§47). */
  staleRefRetries = 0;

  constructor(
    readonly symbols: NativeViewAbiSession["symbols"],
    readonly runtime: NativeViewAbiHandle,
    readonly generation: number,
    readonly nativeLookupCeiling: number,
  ) {}

  noteBorrowedHint(node: SemanticViewNode, nativeRef: number): void {
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
    // Keep one reusable buffer per active semantic recursion level. The
    // inProgress set includes the node currently being materialized, so its
    // size is a stable nesting slot even when a derivation asks for child refs
    // before a generated constructor runs.
    const depth = this.inProgress.size;
    if (AXIS_REF_SCRATCH.runtime !== this.runtime) {
      AXIS_REF_SCRATCH.runtime = this.runtime;
      AXIS_REF_SCRATCH.arrays = [];
    }
    let array = AXIS_REF_SCRATCH.arrays[depth];
    if (array === undefined || array.length < words) {
      array = new Uint32Array(Math.max(words, 1));
      AXIS_REF_SCRATCH.arrays[depth] = array;
    }
    counters.transport_scratch_reuses += 1;
    return array.subarray(0, words);
  }

  /** Returns reusable u32 construction scratch; no per-node TypedArray. */
  private wordScratch(wordCount: number, cap: number, label: string): Uint32Array {
    if (wordCount > cap) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError(
        `${label} word payload ${wordCount} exceeds the retained cap ${cap}`,
      );
    }
    const depth = this.inProgress.size;
    if (GRID_WORD_SCRATCH.runtime !== this.runtime) {
      GRID_WORD_SCRATCH.runtime = this.runtime;
      GRID_WORD_SCRATCH.arrays = [];
    }
    let array = GRID_WORD_SCRATCH.arrays[depth];
    if (array === undefined || array.length < wordCount) {
      array = new Uint32Array(Math.max(wordCount, 1));
      GRID_WORD_SCRATCH.arrays[depth] = array;
    }
    counters.transport_scratch_reuses += 1;
    return array.subarray(0, wordCount);
  }

  /** PERF-12 T10 (§30/§36): reusable flat-grid construction scratch. */
  gridWordScratch(wordCount: number): Uint32Array {
    return this.wordScratch(wordCount, MAX_DIRECT_GRID_WORDS, "grid");
  }

  /** PERF-12 T11 (§30/§41): reusable diff framing scratch (same u32 tier). */
  diffWordScratch(wordCount: number): Uint32Array {
    return this.wordScratch(wordCount, MAX_DIRECT_DIFF_WORDS, "diff");
  }

  /**
   * PERF-12 T11 (§30): the medium byte tier — one environment-level
   * Uint8Array reused by every transaction for text/diff UTF-8 payloads.
   * `needed` above the cap refuses the retained path (§50).
   */
  byteScratch(needed: number, label: string): Uint8Array {
    if (needed > MAX_DIRECT_TEXT_BYTES) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError(
        `${label} payload ${needed} bytes exceeds the retained cap ${MAX_DIRECT_TEXT_BYTES}`,
      );
    }
    if (BYTE_SCRATCH.runtime !== this.runtime || BYTE_SCRATCH.array.length < needed) {
      BYTE_SCRATCH.runtime = this.runtime;
      BYTE_SCRATCH.array = new Uint8Array(MAX_DIRECT_TEXT_BYTES);
    }
    counters.transport_scratch_reuses += 1;
    return BYTE_SCRATCH.array.subarray(0, needed);
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

  /** Releases every temporary lease except one root lease (§18.4). */
  releaseAllExcept(keepRef: number): void {
    // A transaction normally has one lease per newly-created ref. Remove only
    // the transferred occurrence: releasing every equal number would under-
    // release if a future publication path legitimately acquires twice.
    const keepIndex = this.temporaryLeases.indexOf(keepRef);
    const remaining = this.temporaryLeases.slice();
    if (keepIndex >= 0) remaining.splice(keepIndex, 1);
    this.temporaryLeases.length = 0;
    if (remaining.length === 0) return;
    const batch = Uint32Array.from(remaining);
    viewReleaseMany(this.symbols, this.runtime, batch, batch.length);
  }
}

type NodeMaterializer = (node: SemanticViewNode, tx: MaterializeTx) => number;

const FAST_CACHE_MISS = 0x8000_0004;
const STATUS_DETAIL_CHILD_KIND = 1;
const STATUS_DETAIL_BASE_KIND = 2;
const STATUS_DETAIL_CHILD_INDEX = 0x4000_0000;

function isExpectedNativeStatus(error: unknown): boolean {
  return error instanceof Error && /^native ABI status 0x[0-9a-f]+$/u.test(error.message);
}

function nativeStatusCode(error: unknown): number | undefined {
  if (error instanceof NativeAbiStatusError) return error.status;
  if (!isExpectedNativeStatus(error)) return undefined;
  const match = /^native ABI status 0x([0-9a-f]+)$/u.exec((error as Error).message);
  return match === null ? undefined : Number.parseInt(match[1]!, 16);
}

function nativeStatusDetail(error: unknown): number {
  return error instanceof NativeAbiStatusError ? error.detail : 0;
}

function staleChildOrdinal(error: unknown): number | undefined {
  if (nativeStatusCode(error) !== FAST_CACHE_MISS) return undefined;
  const detail = nativeStatusDetail(error);
  if ((detail >>> 30) !== STATUS_DETAIL_CHILD_KIND) return undefined;
  return detail & 0x3fff_ffff;
}

function isStaleBase(error: unknown): boolean {
  return nativeStatusCode(error) === FAST_CACHE_MISS
    && (nativeStatusDetail(error) >>> 30) === STATUS_DETAIL_BASE_KIND;
}

/** Exceptional §73 recovery: cold-decode one semantic node and return a lease. */
function recoverNodeWithDirectDecode(node: SemanticViewNode, tx: MaterializeTx): number | undefined {
  const decodeRef = native.tuiViewAbiDecodeRef;
  if (decodeRef === undefined) return undefined;
  const reference = decodeRef(lowerSemanticView(node) as unknown as object);
  if (!isValidNativeRef(reference)) return undefined;
  installHint(node, tx.generation, reference);
  tx.refs.set(node, reference);
  tx.temporaryLeases.push(reference);
  return reference;
}

function childAtOrdinal(node: SemanticViewNode, ordinal: number): SemanticViewNode | undefined {
  if (!Number.isSafeInteger(ordinal) || ordinal < 0) return undefined;
  if (node.kind === SEMANTIC_VIEW_KIND.row || node.kind === SEMANTIC_VIEW_KIND.column) {
    const override = peekSemanticSequenceOverride(node);
    const child = override?.sequence.get(ordinal) ?? node.children[ordinal];
    return child?.child;
  }
  if (node.kind === SEMANTIC_VIEW_KIND.grid) {
    const override = peekSemanticGridSequenceOverride(node);
    if (override !== undefined) return override.sequence.get(ordinal)?.view;
    let offset = ordinal;
    for (const row of node.rows) {
      if (offset < row.cells.length) return row.cells[offset]?.view;
      offset -= row.cells.length;
    }
  }
  if (node.kind === SEMANTIC_VIEW_KIND.hanging) {
    return [node.prefix, node.continuation, node.body][ordinal];
  }
  if (node.kind === SEMANTIC_VIEW_KIND.container || node.kind === SEMANTIC_VIEW_KIND.clamp || node.kind === SEMANTIC_VIEW_KIND.contentMax) {
    return ordinal === 0 ? node.child : undefined;
  }
  if (node.kind === SEMANTIC_VIEW_KIND.decorated) return ordinal === 0 ? node.child : undefined;
  return undefined;
}

function derivationChildAt(derivation: SemanticDerivation, ordinal: number): SemanticViewNode | undefined {
  switch (derivation.kind) {
    case "axisSet":
    case "gridCell":
      return ordinal === 0 ? derivation.child : undefined;
    case "axisSplice":
      return derivation.inserted[ordinal]?.child;
    case "textLayout":
    case "commonScalar":
      return undefined;
  }
}

/** Invalidates one stale hint and performs bounded semantic recovery. */
function recoverStaleNode(node: SemanticViewNode, tx: MaterializeTx): number | undefined {
  if (tx.staleRefRetries >= 1) return undefined;
  tx.staleRefRetries += 1;
  counters.stale_ref_retries += 1;
  deleteSemanticNativeHint(node);
  tx.refs.delete(node);
  try {
    return ensureSemanticNative(node, tx);
  } catch (error) {
    if (!(error instanceof RetainedFastFallbackError) && !(error instanceof RetainedCycleError)) throw error;
  }
  return recoverNodeWithDirectDecode(node, tx);
}

function materializeWithRecovery(
  node: SemanticViewNode,
  tx: MaterializeTx,
  materializer: NodeMaterializer,
): number {
  const invoke = (): number => {
    counters.direct_materializer_calls += 1;
    tx.depth += 1;
    try {
      return materializer(node, tx);
    } finally {
      tx.depth -= 1;
    }
  };
  try {
    return invoke();
  } catch (error) {
    const ordinal = staleChildOrdinal(error);
    const child = ordinal === undefined ? undefined : childAtOrdinal(node, ordinal);
    if (child !== undefined && recoverStaleNode(child, tx) !== undefined) {
      try {
        return invoke();
      } catch (retryError) {
        if (isExpectedNativeStatus(retryError)) {
          throw new RetainedFastFallbackError("native constructor retry reported a failure status");
        }
        throw retryError;
      }
    }
    if (isExpectedNativeStatus(error)) {
      throw new RetainedFastFallbackError("native constructor reported a failure status");
    }
    throw error;
  }
}

/** Runs one direct encoding and leaves expected native failures to recovery. */
function runMaterializer(_kind: string, lower: () => number): number {
  return lower();
}

function materializeSpacerNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.spacer) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("spacer", () => viewSpacerCreate(tx.symbols, tx.runtime, low, high, node.rows));
}

type SemanticAxisNode = Extract<SemanticViewNode, { kind: typeof SEMANTIC_VIEW_KIND.row | typeof SEMANTIC_VIEW_KIND.column }>;

function axisChildAt(node: SemanticAxisNode, index: number): SemanticLayoutChild | undefined {
  const override = peekSemanticSequenceOverride(node);
  return override === undefined ? node.children[index] : override.sequence.get(index);
}

function axisChildCount(node: SemanticAxisNode): number {
  return peekSemanticSequenceOverride(node)?.sequence.length ?? node.children.length;
}

function materializeAxisNode(node: SemanticAxisNode, tx: MaterializeTx): number {
  const count = axisChildCount(node);
  const childAt = (index: number): SemanticLayoutChild => {
    const child = axisChildAt(node, index);
    if (child === undefined) throw new RetainedFastFallbackError("axis sequence contains a missing child");
    return child;
  };
  counters.bridge_children_visited += count;
  counters.bridge_semantic_nodes_inspected += 1;
  const [low, high] = splitNodeId(node.id);
  if (count <= 4) {
    const child0 = count > 0 ? childAt(0) : undefined;
    const child1 = count > 1 ? childAt(1) : undefined;
    const child2 = count > 2 ? childAt(2) : undefined;
    const child3 = count > 3 ? childAt(3) : undefined;
    const ref = (child: SemanticLayoutChild): number => ensureSemanticNative(child.child, tx);
    if (node.kind === SEMANTIC_VIEW_KIND.row) {
      switch (count) {
        case 0: return runMaterializer("row", () => viewRowCreate0(tx.symbols, tx.runtime, low, high, node.gap));
        case 1: return runMaterializer("row", () => viewRowCreate1(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!)));
        case 2: return runMaterializer("row", () => viewRowCreate2(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!), layoutTrackWord(child1!), ref(child1!)));
        case 3: return runMaterializer("row", () => viewRowCreate3(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!), layoutTrackWord(child1!), ref(child1!), layoutTrackWord(child2!), ref(child2!)));
        default: return runMaterializer("row", () => viewRowCreate4(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!), layoutTrackWord(child1!), ref(child1!), layoutTrackWord(child2!), ref(child2!), layoutTrackWord(child3!), ref(child3!)));
      }
    }
    switch (count) {
      case 0: return runMaterializer("column", () => viewColumnCreate0(tx.symbols, tx.runtime, low, high, node.gap));
      case 1: return runMaterializer("column", () => viewColumnCreate1(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!)));
      case 2: return runMaterializer("column", () => viewColumnCreate2(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!), layoutTrackWord(child1!), ref(child1!)));
      case 3: return runMaterializer("column", () => viewColumnCreate3(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!), layoutTrackWord(child1!), ref(child1!), layoutTrackWord(child2!), ref(child2!)));
      default: return runMaterializer("column", () => viewColumnCreate4(tx.symbols, tx.runtime, low, high, node.gap, layoutTrackWord(child0!), ref(child0!), layoutTrackWord(child1!), ref(child1!), layoutTrackWord(child2!), ref(child2!), layoutTrackWord(child3!), ref(child3!)));
    }
  }
  const words = tx.axisRefScratch(count);
  let offset = 0;
  for (let index = 0; index < count; index += 1) {
    const child = childAt(index);
    words[offset++] = layoutTrackWord(child);
    words[offset++] = ensureSemanticNative(child.child, tx);
  }
  tx.noteRefWords(offset);
  return runMaterializer("axis", () => viewAxisCreateBuffer(
    tx.symbols,
    tx.runtime,
    low,
    high,
    axisKind(node.kind),
    node.gap,
    words,
    count,
  ));
}

function materializeRowNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.row) throw new RetainedFastFallbackError("kind mismatch");
  return materializeAxisNode(node, tx);
}

function materializeColumnNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.column) throw new RetainedFastFallbackError("kind mismatch");
  return materializeAxisNode(node, tx);
}

/** PERF-12 T10 (§36): new Grid construction through one borrowed word lane. */
function materializeGridNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.grid) throw new RetainedFastFallbackError("kind mismatch");
  const override = peekSemanticGridSequenceOverride(node);
  const rowCount = override?.rowTracks.length ?? node.rows.length;
  const cellCount = override?.sequence.length ?? node.rows.reduce((total, row) => total + row.cells.length, 0);
  const wordCount = 2 + node.columns.length + rowCount * 2 + cellCount * 3;
  const words = tx.gridWordScratch(wordCount);
  let offset = 0;
  words[offset++] = node.columns.length;
  for (const track of node.columns) words[offset++] = gridTrackWord(track);
  words[offset++] = rowCount;
  for (let rowIndex = 0; rowIndex < rowCount; rowIndex += 1) {
    const row = override === undefined ? node.rows[rowIndex]! : undefined;
    const track = override?.rowTracks[rowIndex] ?? row?.track;
    if (track === undefined) throw new RetainedFastFallbackError("grid sequence contains a missing row track");
    const start = override?.rowOffsets[rowIndex] ?? 0;
    const end = override?.rowOffsets[rowIndex + 1] ?? row!.cells.length;
    const rowCells = override === undefined ? row!.cells : undefined;
    words[offset++] = gridTrackWord(track);
    words[offset++] = end - start;
    for (let index = start; index < end; index += 1) {
      const cell = override === undefined ? rowCells![index - start]! : override.sequence.get(index);
      if (cell === undefined) throw new RetainedFastFallbackError("grid sequence contains a missing cell");
      words[offset++] = ensureSemanticNative(cell.view, tx);
      words[offset++] = gridCellSpanWord(cell.columnSpan, cell.rowSpan);
      words[offset++] = gridCellAlignmentWord(cell.horizontalAlign, cell.verticalAlign);
    }
  }
  tx.noteRefWords(wordCount);
  counters.bridge_children_visited += cellCount;
  counters.bridge_semantic_nodes_inspected += 1;
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("grid", () => viewGridCreateBuffer(
    tx.symbols,
    tx.runtime,
    low,
    high,
    node.columnGap,
    node.rowGap,
    words,
    wordCount,
  ));
}

/**
 * PERF-12 T11 (§37/§40): retained text and style payload lanes.
 *
 * Styles resolve through a generation-scoped WeakMap sidecar into the native
 * runtime's authoritative style table (created once per distinct style via
 * the existing style_atom_create_cstring / style_create_bits ABI functions;
 * §25 reuse before adding, §40 no second native style cache). Text uses the
 * cstring family whenever every span is NUL-free — zero JS encoding, Bun
 * lowers the strings natively — and the exact-byte utf8 family otherwise,
 * encoding once into the reusable byte tier. Span counts outside the 1..=4
 * constructor families route to the complete cold path (§49/§76).
 */
const TEXT_ENCODER = new TextEncoder();

function ensureStyleCache(tx: MaterializeTx): void {
  if (STYLE_REF_CACHE.runtime !== tx.runtime || STYLE_REF_CACHE.generation !== tx.generation) {
    STYLE_REF_CACHE.runtime = tx.runtime;
    STYLE_REF_CACHE.generation = tx.generation;
    STYLE_REF_CACHE.refs = new WeakMap();
    STYLE_REF_CACHE.atoms = new Map();
  }
}

function styleColorAtom(color: SemanticColor): string {
  return colorAtomValue(color);
}

function styleAtomRef(value: string, tx: MaterializeTx): number {
  if (value.indexOf("\0") !== -1) {
    throw new RetainedFastFallbackError("style atom contains an embedded NUL");
  }
  ensureStyleCache(tx);
  const cached = STYLE_REF_CACHE.atoms.get(value);
  if (cached !== undefined) return cached;
  const reference = styleAtomCreateCstring(tx.symbols, tx.runtime, value);
  STYLE_REF_CACHE.atoms.set(value, reference);
  return reference;
}

/** Resolves one stable style object to its native StyleRef (0 = unstyled). */
function styleRefFor(style: SemanticStyle | undefined, tx: MaterializeTx): number {
  if (style === undefined) return 0;
  ensureStyleCache(tx);
  const cached = STYLE_REF_CACHE.refs.get(style);
  if (cached !== undefined) return cached;
  const attributes = styleAttributeEncoding(style);
  if (!attributes.valid) {
    throw new RetainedFastFallbackError(
      attributes.reason === "unknown"
        ? `unknown text attribute ${attributes.name}`
        : `text attribute ${attributes.name} must be boolean`,
    );
  }
  const foreground = style.foreground === undefined ? 0 : styleAtomRef(styleColorAtom(style.foreground), tx);
  const background = style.background === undefined ? 0 : styleAtomRef(styleColorAtom(style.background), tx);
  const theme = style.theme === undefined ? 0 : styleAtomRef(`theme:${style.theme}`, tx);
  const reference = styleCreateBits(tx.symbols, tx.runtime, 0, attributes.present, attributes.truth, foreground, background, theme);
  STYLE_REF_CACHE.refs.set(style, reference);
  return reference;
}

function materializeTextNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.text) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const spans = node.spans;
  if (spans.length < 1 || spans.length > 4) {
    counters.cold_fallbacks += 1;
    throw new RetainedFastFallbackError(`text span count ${spans.length} is outside the retained family`);
  }
  // Payload dependencies resolve before any transport (children-first analog).
  const styleRefs = spans.map((span) => {
    try {
      return styleRefFor(span.style, tx);
    } catch (error) {
      if (error instanceof RetainedFastFallbackError || isExpectedNativeStatus(error)) {
        counters.cold_fallbacks += 1;
        if (isExpectedNativeStatus(error)) {
          throw new RetainedFastFallbackError("style publication reported a failure status");
        }
        throw error;
      }
      throw error;
    }
  });
  const wrap = wrapModeCode(node.wrap);
  const align = horizontalAlignCode(node.align);
  const [low, high] = splitNodeId(node.id);
  let hasEmbeddedNul = false;
  for (const span of spans) {
    if (span.text.indexOf("\0") !== -1) {
      hasEmbeddedNul = true;
      break;
    }
  }
  if (!hasEmbeddedNul) {
    counters.byte_payload_bytes += 0; // cstring lane encodes nothing in JS.
    return runMaterializer("text", () => {
      switch (spans.length) {
        case 1: return viewTextCreateCstring(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, wrap, align);
        case 2: return viewTextCreateCstring2(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, wrap, align);
        case 3: return viewTextCreateCstring3(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, spans[2]!.text, styleRefs[2]!, wrap, align);
        default: return viewTextCreateCstring4(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, spans[2]!.text, styleRefs[2]!, spans[3]!.text, styleRefs[3]!, wrap, align);
      }
    });
  }
  // Exact-byte lane: encode once into the reusable byte tier; capacity
  // overrun refuses the retained path instead of truncating (§30/§50).
  const scratch = tx.byteScratch(MAX_DIRECT_TEXT_BYTES, "text");
  let offset = 0;
  const lengths: number[] = [];
  for (const span of spans) {
    const encoded = TEXT_ENCODER.encodeInto(span.text, scratch.subarray(offset));
    if (encoded.read !== span.text.length) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError("text payload exceeds the retained byte tier");
    }
    lengths.push(encoded.written);
    offset += encoded.written;
  }
  counters.byte_payload_bytes += offset;
  return runMaterializer("text", () => {
    switch (spans.length) {
      case 1: return viewTextCreateUtf8(tx.symbols, tx.runtime, low, high, scratch, offset, styleRefs[0]!, wrap, align);
      case 2: return viewTextCreateUtf82(tx.symbols, tx.runtime, low, high, scratch, offset, lengths[0]!, styleRefs[0]!, lengths[1]!, styleRefs[1]!, wrap, align);
      case 3: return viewTextCreateUtf83(tx.symbols, tx.runtime, low, high, scratch, offset, lengths[0]!, styleRefs[0]!, lengths[1]!, styleRefs[1]!, lengths[2]!, styleRefs[2]!, wrap, align);
      default: return viewTextCreateUtf84(tx.symbols, tx.runtime, low, high, scratch, offset, lengths[0]!, styleRefs[0]!, lengths[1]!, styleRefs[1]!, lengths[2]!, styleRefs[2]!, lengths[3]!, styleRefs[3]!, wrap, align);
    }
  });
}

/** Splits a safe-integer coordinate into the canonical lo/hi word pair. */
function requiredU64Words(value: number): readonly [number, number] {
  const words = u64Words(value);
  if (words === undefined) {
    throw new RetainedFastFallbackError(`diff coordinate ${value} is not a safe non-negative integer`);
  }
  return words;
}

/** PERF-12 T11 (§41): new-Diff construction through one words+bytes call. */
function materializeDiffNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.diff) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const hunks = node.hunks;
  let wordCount = 1;
  for (const hunk of hunks) wordCount += 9 + hunk.lines.length * 6;
  const words = tx.diffWordScratch(wordCount);
  const bytes = tx.byteScratch(MAX_DIRECT_TEXT_BYTES, "diff");
  let wordOffset = 0;
  let byteOffset = 0;
  const writeWords = (...values: number[]): void => {
    for (const value of values) words[wordOffset++] = value;
  };
  writeWords(hunks.length);
  for (const hunk of hunks) {
    const oldStart = requiredU64Words(hunk.oldRange.start);
    const oldCount = requiredU64Words(hunk.oldRange.count);
    const newStart = requiredU64Words(hunk.newRange.start);
    const newCount = requiredU64Words(hunk.newRange.count);
    writeWords(oldStart[0], oldStart[1], oldCount[0], oldCount[1]);
    writeWords(newStart[0], newStart[1], newCount[0], newCount[1]);
    writeWords(hunk.lines.length);
    for (const line of hunk.lines) {
      const meta = diffLineMetadata(line.kind, line.termination);
      const oldLine = line.oldLine === undefined ? [0, 0] as const : requiredU64Words(line.oldLine);
      const newLine = line.newLine === undefined ? [0, 0] as const : requiredU64Words(line.newLine);
      const encoded = TEXT_ENCODER.encodeInto(line.text, bytes.subarray(byteOffset));
      if (encoded.read !== line.text.length || encoded.written > 0xffff_ffff) {
        counters.cold_fallbacks += 1;
        throw new RetainedFastFallbackError("diff line payload exceeds the retained byte tier");
      }
      writeWords(meta, oldLine[0], oldLine[1], newLine[0], newLine[1], encoded.written);
      byteOffset += encoded.written;
    }
  }
  counters.ref_words_written += wordCount;
  counters.byte_payload_bytes += byteOffset;
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("diff", () =>
    viewDiffCreateBuffer(tx.symbols, tx.runtime, low, high, words, wordCount, bytes, byteOffset)
  );
}

/**
 * T13: styleRefFor with retained-refusal accounting — publication failures
 * count one cold fallback and route the complete cold path like any cap miss.
 */
function styleRefCounted(style: SemanticStyle | undefined, tx: MaterializeTx): number {
  try {
    return styleRefFor(style, tx);
  } catch (error) {
    if (error instanceof RetainedFastFallbackError) throw error;
    if (isExpectedNativeStatus(error)) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError("style publication reported a failure status");
    }
    throw error;
  }
}

function materializeHangingNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.hanging) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const prefixRef = ensureSemanticNative(node.prefix, tx);
  const continuationRef = ensureSemanticNative(node.continuation, tx);
  const bodyRef = ensureSemanticNative(node.body, tx);
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("hanging", () =>
    viewHangingCreate(tx.symbols, tx.runtime, low, high, prefixRef, continuationRef, bodyRef)
  );
}

function materializeContainerNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.container) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const childRef = ensureSemanticNative(node.child, tx);
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("container", () =>
    viewContainerCreate(tx.symbols, tx.runtime, low, high, childRef)
  );
}

/** Lowers clamp/contentMax nodes; contentMax is a clamp with no indicator. */
function materializeClampNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.clamp && node.kind !== SEMANTIC_VIEW_KIND.contentMax) {
    throw new RetainedFastFallbackError("kind mismatch");
  }
  counters.bridge_semantic_nodes_inspected += 1;
  const childRef = ensureSemanticNative(node.child, tx);
  let overflowStyleRef = 0;
  let prefix = "";
  let overflowKind = 0;
  if (node.kind === SEMANTIC_VIEW_KIND.clamp) {
    overflowKind = overflowKindCode(node.overflow);
    if (node.overflow.kind !== "none") {
      overflowStyleRef = styleRefCounted(node.overflow.style, tx);
      if (node.overflow.kind === "footer") prefix = node.overflow.prefix;
    }
  }
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("clamp", () =>
    viewClampCreate(tx.symbols, tx.runtime, low, high, childRef, node.maxRows, overflowKind, overflowStyleRef, prefix)
  );
}

function materializeComponentNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.component) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const handleWords = requiredU64Words(componentIdForHandleId(node.handleId));
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("component", () =>
    viewComponentCreate(tx.symbols, tx.runtime, low, high, handleWords[0], handleWords[1])
  );
}

function materializeDecoratedNode(node: SemanticViewNode, tx: MaterializeTx): number {
  if (node.kind !== SEMANTIC_VIEW_KIND.decorated) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const childRef = ensureSemanticNative(node.child, tx);
  const decoration = node.decoration;
  if (decoration.border?.glyphs !== undefined && Object.keys(decoration.border.glyphs).length > 0) {
    counters.cold_fallbacks += 1;
    throw new RetainedFastFallbackError("custom border glyphs are not expressible on the retained lane");
  }
  const encodedDecoration = decorationWordEncoding(decoration);
  const states = Object.entries(decoration.styleStates ?? {});
  // Fixed header: mask + 9 payload words + state count, then 4 words per state.
  const wordCount = 11 + states.length * 4;
  const words = tx.diffWordScratch(wordCount);
  const bytes = tx.byteScratch(MAX_DIRECT_TEXT_BYTES, "decorated");
  let byteOffset = 0;
  let colorAtoms: [number, number] = [0, 0];
  try {
    colorAtoms = [
      decoration.foreground === undefined ? 0 : styleAtomRef(styleColorAtom(decoration.foreground), tx),
      decoration.background === undefined ? 0 : styleAtomRef(styleColorAtom(decoration.background), tx),
    ];
  } catch (error) {
    if (error instanceof RetainedFastFallbackError || isExpectedNativeStatus(error)) {
      counters.cold_fallbacks += 1;
      throw error instanceof RetainedFastFallbackError ? error : new RetainedFastFallbackError("decoration color atom publication failed");
    }
    throw error;
  }
  const borderColorAtom = decoration.border?.color === undefined || decoration.border === undefined ? 0 : styleAtomRef(styleColorAtom(decoration.border.color), tx);
  let wordOffset = 0;
  const writeWord = (value: number): void => {
    words[wordOffset++] = value;
  };
  writeWord(encodedDecoration.mask);
  writeWord(encodedDecoration.paddingTopRight);
  writeWord(encodedDecoration.paddingBottomLeft);
  writeWord(encodedDecoration.sizeModes);
  writeWord(encodedDecoration.minWidthMaxWidth);
  writeWord(encodedDecoration.minHeightMaxHeight);
  writeWord(colorAtoms[0]);
  writeWord(colorAtoms[1]);
  writeWord(encodedDecoration.borderStyleEdges);
  writeWord(borderColorAtom);
  writeWord(states.length);
  for (const [key, value] of states) {
    const keyBytes = TEXT_ENCODER.encodeInto(key, bytes.subarray(byteOffset));
    if (keyBytes.read !== key.length) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError("style-state payload exceeds the retained byte tier");
    }
    const keyOffset = byteOffset;
    byteOffset += keyBytes.written;
    const valueBytes = TEXT_ENCODER.encodeInto(value, bytes.subarray(byteOffset));
    if (valueBytes.read !== value.length) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError("style-state payload exceeds the retained byte tier");
    }
    const valueOffset = byteOffset;
    byteOffset += valueBytes.written;
    writeWord(keyOffset);
    writeWord(keyBytes.written);
    writeWord(valueOffset);
    writeWord(valueBytes.written);
  }
  counters.ref_words_written += wordCount;
  counters.byte_payload_bytes += byteOffset;
  const styleRef = styleRefCounted(decoration.style, tx);
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("decorated", () =>
    viewDecoratedCreateBuffer(tx.symbols, tx.runtime, low, high, childRef, styleRef, words, wordCount, bytes, byteOffset)
  );
}

/**
 * Resolves a framework ViewState handle only at the native structural
 * boundary. Semantic nodes retain the branded HandleId; the native View
 * stores the host-local state identity needed by frame-time overlays.
 */
function attachStateIfPresent(
  node: SemanticViewNode,
  tx: MaterializeTx,
  baseRef: number,
): number {
  if (node.stateAttachment === undefined) return baseRef;
  try {
    const resource = nativeResourceForHandleId<NativeViewStateContract>(node.stateAttachment, "state");
    if (typeof (resource as { readonly stateId?: unknown }).stateId !== "function") {
      // API-H3 internal fixtures may register a validation-only state
      // attachment. It has no native presentation record to lower.
      return baseRef;
    }
    const stateId = resource.stateId();
    const stateWords = u64Words(stateId);
    if (stateWords === undefined || stateId === 0) {
      throw new RetainedFastFallbackError("ViewState native identity is not a safe positive integer");
    }
    const [nodeLow, nodeHigh] = splitNodeId(node.id);
    return viewStateAttach(
      tx.symbols,
      tx.runtime,
      baseRef,
      nodeLow,
      nodeHigh,
      stateWords[0],
      stateWords[1],
    );
  } catch (error) {
    // The ordinary constructor returned a lease before the attachment
    // replacement. It is not in tx.temporaryLeases yet, so release it here
    // before propagating the prepare failure.
    viewReleaseMany(tx.symbols, tx.runtime, Uint32Array.of(baseRef), 1);
    throw error;
  }
}

/**
 * Per-kind generated materializer dispatch (§22 children-first, §32 fixed
 * arities). T7 covers spacer plus row/column arities 0..=4; T10 adds Grid;
 * T11 adds the text cstring/utf8 payload lanes and the diff words+bytes lane;
 * T13 adds hanging, container, clamp/contentMax, component references, and
 * decorated nodes — every §76 kind is now direct-materialized or explicitly
 * fallback-routed (text spans >4, oversized payloads, custom border glyphs).
 */
const MATERIALIZERS = new Map<number, NodeMaterializer>([
  [SEMANTIC_VIEW_KIND.spacer, materializeSpacerNode],
  [SEMANTIC_VIEW_KIND.row, materializeRowNode],
  [SEMANTIC_VIEW_KIND.column, materializeColumnNode],
  [SEMANTIC_VIEW_KIND.grid, materializeGridNode],
  [SEMANTIC_VIEW_KIND.text, materializeTextNode],
  [SEMANTIC_VIEW_KIND.diff, materializeDiffNode],
  [SEMANTIC_VIEW_KIND.hanging, materializeHangingNode],
  [SEMANTIC_VIEW_KIND.container, materializeContainerNode],
  [SEMANTIC_VIEW_KIND.clamp, materializeClampNode],
  [SEMANTIC_VIEW_KIND.contentMax, materializeClampNode],
  [SEMANTIC_VIEW_KIND.component, materializeComponentNode],
  [SEMANTIC_VIEW_KIND.decorated, materializeDecoratedNode],
]);

/**
 * Installs or refreshes the NativeRef hint for a semantic node under the tx's
 * generation. Hints are acceleration only; they never take a lease (§16).
 */
function installHint(node: SemanticViewNode, generation: number, nativeRef: number): void {
  SEMANTIC_NATIVE.set(node, { generation, nativeRef });
}

/** @internal Refreshes a hint after a lease-bearing NodeId promotion. */
export function refreshNativeHint(view: View, generation: number, nativeRef: number): void {
  installHint(semanticNodeOf(view), generation, nativeRef);
}

/** @internal Drops a confirmed-stale hint before complete fallback recovery. */
export function clearNativeHint(view: View): void {
  deleteSemanticNativeHint(semanticNodeOf(view));
}

function deleteSemanticNativeHint(node: SemanticViewNode): void {
  SEMANTIC_NATIVE.delete(node);
}

function splitNodeId(id: number): [number, number] {
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

/**
 * Core identity-first resolution (§19). Hard ordering:
 * semantic NativeRef hint lookup → transaction-local ref → ceiling-gated
 * NodeId→NativeRef promotion → semantic payload inspection → child traversal.
 */
export function ensureNative(node: SemanticViewNode, tx: MaterializeTx): number;
export function ensureNative(node: object, tx: MaterializeTx): number;
export function ensureNative(node: SemanticViewNode | object, tx: MaterializeTx): number {
  if (!isSemanticViewNode(node)) {
    throw new RetainedFastFallbackError("structural materialization requires a semantic node");
  }
  return ensureSemanticNative(node, tx);
}

/** @internal Semantic-only retained entrypoint; bridge callers use ensureNative. */
export function ensureSemanticNative(node: SemanticViewNode, tx: MaterializeTx): number {
  const hint = SEMANTIC_NATIVE.get(node);
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
    // §19 ordering: derivations are tried after identity resolution but
    // before payload-inspecting materialization.
    let reference = tryDerivation(node, tx);
    if (reference === undefined) {
      const materializer = MATERIALIZERS.get(node.kind);
      if (materializer === undefined) {
        counters.bridge_semantic_nodes_inspected += 1;
        counters.cold_fallbacks += 1;
        throw new RetainedFastFallbackError(`no generated materializer for kind ${node.kind}`);
      }
      reference = materializeWithRecovery(node, tx, materializer);
    }
    reference = attachStateIfPresent(node, tx, reference);
    installHint(node, tx.generation, reference);
    tx.refs.set(node, reference);
    tx.temporaryLeases.push(reference);
    return reference;
  } finally {
    tx.inProgress.delete(node);
  }
}

/**
 * Resolves the derivation base's same-generation NativeRef (§27).
 *
 * A base hint is used directly. Otherwise — and only when the base existed
 * before the boundary's last commit — one ceiling-gated NodeId→NativeRef
 * promotion recovers the ref exactly as `ensureNative` would; the acquired
 * lease joins the transaction's temporary leases so it drains with the tx on
 * every path. An unavailable base leaves the hint unused (§38 fallback rule).
 */
function derivationBaseRef(base: SemanticViewNode, tx: MaterializeTx): number | undefined {
  const hint = SEMANTIC_NATIVE.get(base);
  if (hint !== undefined && hint.generation === tx.generation) return hint.nativeRef;
  if (base.id > tx.nativeLookupCeiling) return undefined;
  counters.node_id_ref_promotion_attempts += 1;
  const [low, high] = splitNodeId(base.id);
  try {
    const recovered = viewRefForNodeId(tx.symbols, tx.runtime, low, high);
    counters.node_id_ref_promotion_hits += 1;
    installHint(base, tx.generation, recovered);
    tx.refs.set(base, recovered);
    tx.temporaryLeases.push(recovered);
    return recovered;
  } catch (error) {
    if (!isExpectedNativeStatus(error)) throw error;
    counters.node_id_ref_promotion_misses += 1;
    return undefined;
  }
}

/**
 * PERF-12 T9 (§27/§28/§38): tries the node's derivation hint against an
 * exact native retained primitive before any payload inspection.
 *
 * Preconditions follow §27: the node has no NativeRef yet (ensureNative's
 * ordering guarantees this) and the hint exists; the base must carry a
 * same-generation NativeRef, recoverable by promotion for pre-commit nodes.
 * Any expected native failure status ignores the hint cleanly — the caller
 * materializes the node from its semantic fields (§38: 'If the base NativeRef
 * is unavailable, fall back to direct semantic materialization'). The hint
 * itself stays attached: it may succeed after the base is re-adopted.
 */
function tryDerivation(node: SemanticViewNode, tx: MaterializeTx): number | undefined {
  const derivation = peekSemanticDerivation(node);
  if (derivation === undefined) return undefined;
  const baseRef = derivationBaseRef(derivation.base, tx);
  if (baseRef === undefined) return undefined;
  const [low, high] = splitNodeId(node.id);
  try {
    let reference: number;
    if (derivation.kind === "textLayout") {
      reference = viewTextLayoutPatchRoot(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        wrapModeCode(derivation.wrap),
        horizontalAlignCode(derivation.align),
      );
    } else if (derivation.kind === "commonScalar") {
      const encoded = commonScalarEncoding(derivation.changes);
      reference = viewCommonPatchRoot(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        encoded.mask,
        encoded.paddingTopRight,
        encoded.paddingBottomLeft,
        encoded.widthRule,
        encoded.heightRule,
        encoded.minWidth,
        encoded.maxWidth,
        encoded.minHeight,
        encoded.maxHeight,
        0,
      );
    } else if (derivation.kind === "axisSet") {
      // §35 children-first: resolve only the replacement child, never the
      // old wide sequence.
      const childRef = ensureSemanticNative(derivation.child, tx);
      reference = viewAxisSetChild(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        derivation.index,
        axisTrackWord(derivation.track),
        childRef,
      );
    } else if (derivation.kind === "axisSplice") {
      // Only inserted refs cross FFI; the old sequence remains native-retained.
      const scratch = tx.axisRefScratch(derivation.inserted.length);
      let offset = 0;
      for (const entry of derivation.inserted) {
        scratch[offset++] = axisTrackWord(entry.track);
        scratch[offset++] = ensureSemanticNative(entry.child, tx);
      }
      tx.noteRefWords(offset);
      reference = viewAxisSpliceBuffer(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        derivation.index,
        derivation.removeCount,
        scratch,
        derivation.inserted.length,
      );
    } else {
      const childRef = ensureSemanticNative(derivation.child, tx);
      reference = viewGridSetCell(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        derivation.row,
        derivation.column,
        childRef,
      );
    }
    counters.derivation_fast_path_calls += 1;
    return reference;
  } catch (error) {
    if (!isExpectedNativeStatus(error)) throw error;
    const staleNode = isStaleBase(error)
      ? derivation.base
      : (() => {
        const ordinal = staleChildOrdinal(error);
        return ordinal === undefined ? undefined : derivationChildAt(derivation, ordinal);
      })();
    if (staleNode !== undefined && recoverStaleNode(staleNode, tx) !== undefined) {
      // `recoverStaleNode` consumes the single transaction retry budget; the
      // recursive call therefore performs at most one retry and then falls
      // back cleanly if the repaired ref is still rejected.
      return tryDerivation(node, tx);
    }
    return undefined;
  }
}

function isValidNativeRef(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0 && value < 0x8000_0000;
}

/** Host render statuses returned by the generated `hostRenderRef`. */
const HOST_STATUS_OK = 0;
const HOST_STATUS_CACHE_MISS = 1;

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
  host: NativeTuiHostContract,
  view: View,
): ExactRootRender {
  const node = semanticNodeOf(view);
  const generation = session.abi.generation;
  const hint = SEMANTIC_NATIVE.get(node);

  if (hint !== undefined && hint.generation === generation) {
    counters.bridge_hint_hits += 1;
    const hostStart = phaseNow();
    const status = hostRenderRef(session.symbols, session.runtime, host, hint.nativeRef);
    const hostEnd = phaseNow();
    if (status === HOST_STATUS_OK) {
      counters.host_mutations += 1;
      if (hostStart !== undefined && hostEnd !== undefined) {
        recordPhaseSample({ transport_prepare_ns: 0, native_materialize_ns: 0, host_commit_ns: hostEnd - hostStart });
      }
      return { status: "ok", rootRef: hint.nativeRef, recovered: false };
    }
    if (status === HOST_STATUS_CACHE_MISS) {
      // One targeted retry (§47 hard rule): the hinted ref went stale.
      counters.stale_ref_retries += 1;
      counters.node_id_ref_promotion_attempts += 1;
      SEMANTIC_NATIVE.delete(node);
      const [low, high] = splitNodeId(node.id);
      let recoveredRef: number | undefined;
      try {
        recoveredRef = viewRefForNodeId(session.symbols, session.runtime, low, high);
        counters.node_id_ref_promotion_hits += 1;
      } catch (error) {
        if (!isExpectedNativeStatus(error)) throw error;
        counters.node_id_ref_promotion_misses += 1;
        const decodeRef = native.tuiViewAbiDecodeRef;
        if (decodeRef !== undefined) {
          recoveredRef = decodeRef(lowerSemanticView(node) as unknown as object);
        }
      }
      if (recoveredRef === undefined || !isValidNativeRef(recoveredRef)) {
        SEMANTIC_NATIVE.delete(node);
        return { status: "no_root_ref" };
      }
      // A NodeId promotion or direct recovery returns one lease. Keep it on a
      // successful retry so the owning boundary can transfer it to its root
      // lease; only failed retries release it here.
      let releaseRecoveredLease = true;
      installHint(node, generation, recoveredRef);
      try {
        const retryStart = phaseNow();
        const retryStatus = hostRenderRef(session.symbols, session.runtime, host, recoveredRef);
        const retryEnd = phaseNow();
        if (retryStatus === HOST_STATUS_OK) {
          counters.host_mutations += 1;
          if (retryStart !== undefined && retryEnd !== undefined) {
            recordPhaseSample({ transport_prepare_ns: 0, native_materialize_ns: 0, host_commit_ns: retryEnd - retryStart });
          }
          releaseRecoveredLease = false;
          return { status: "ok", rootRef: recoveredRef, recovered: true };
        }
        if (retryStatus === HOST_STATUS_CACHE_MISS) {
          SEMANTIC_NATIVE.delete(node);
          return { status: "no_root_ref" };
        }
        throw new Error(`hostRenderRef retry failed with status ${retryStatus}`);
      } finally {
        if (releaseRecoveredLease) {
          const batch = Uint32Array.of(recoveredRef);
          viewReleaseMany(session.symbols, session.runtime, batch, 1);
        }
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
  const node = semanticNodeOf(view);
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
 * update (legacy/direct boundary):
 *   1. keep previousRef leased
 *   2. materialize next root (ensureNative)
 *   3. hostRenderRef(nextRef)
 *   4. success → release previousRef, transfer nextRef temp lease to
 *      boundary.previousRef, release every other temporary ref
 *      failure → keep previousRef, release every temporary ref
 * update (H3 host boundary): install desiredRef first; release the old
 *   visibleRef only from commitVisible() after the host frame succeeds
 * close:
 *   release every distinct desired/visible root exactly once
 * ```
 *
 * After each successful commit the private NodeId allocator high-water is
 * captured as `nativeLookupCeiling`, letting later sidecar misses distinguish
 * definitely-new NodeIds from older possibly-cached ones (§19).
 */
/**
 * PERF-12 T13.1 R8 — injectable COLD materializer used by
 * {@link RetainedRootBoundary.prepareColdInstall}: decodes a whole View tree
 * into a leased native root reference WITHOUT painting. Bootstrap wires this
 * to the Direct N-API decode (`tryNativeMaterialize`) — kept behind a hook
 * because retained-dag must not import native-view-abi (cycle).
 */
let COLD_ROOT_MATERIALIZER: ((view: View) => number | undefined) | undefined;

/** Bootstrap wiring for {@link COLD_ROOT_MATERIALIZER} (idempotent). */
export function setRootColdMaterializer(
  materializer: (view: View) => number | undefined,
): void {
  COLD_ROOT_MATERIALIZER = materializer;
}

/**
 * PERF-12 T13.1 R7 — one prepared-but-unpublished root replacement
 * (handoff §32.1 R7, AMENDMENT-C §13).
 *
 * Lifecycle: `prepareInstall` runs every fallible step (retained walk,
 * materialization, lease acquisition/recovery) WITHOUT touching the
 * installed root; the returned publication's `commit()` then publishes and
 * performs bookkeeping only (pointer swap, ceiling capture, lease ownership
 * swap) — no materialization, no allocation-heavy work, no user code.
 * `abort()` releases everything the prepare acquired; the previous root
 * stays installed and leased throughout.
 */
export interface RootPublication {
  /** The prepared native root reference (held under lease until commit/abort). */
  readonly rootRef: number;
  /** Diagnostic route selected during preparation. */
  readonly route?: "retained" | "fallback";
  /** Publishes the prepared root. Infallible after successful preparation. */
  commit(): void;
  /** Discards the prepared root; no visible mutation ever occurred. */
  abort(): void;
}

interface PreparedRootInstall {
  readonly node: SemanticViewNode;
  readonly rootRef: number;
  readonly tx: MaterializeTx;
  readonly ownsTempLease: boolean;
  readonly phase?: {
    readonly transport_prepare_ns: number;
    readonly native_materialize_ns: number;
  };
  /** True when prepare acquired a fresh boundary lease by NodeId promotion. */
  acquiredBoundaryLease: boolean;
}

export interface RetainedRootBoundaryOptions {
  /** H3 commit records desired structure; a later frame barrier presents it. */
  readonly deferHostCommit?: boolean;
}

interface DesiredRootRevision {
  readonly revision: string;
  readonly ref: number;
  readonly node: SemanticViewNode;
}

function rootRevisionKey(value: string | number | undefined): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) throw new TypeError("native revision must be a non-negative safe integer");
    return BigInt(value).toString();
  }
  if (!/^\d+$/u.test(value)) throw new TypeError("native revision must be a decimal integer string");
  return BigInt(value).toString();
}

function compareRootRevisions(left: string, right: string): number {
  const a = BigInt(left);
  const b = BigInt(right);
  return a < b ? -1 : a > b ? 1 : 0;
}

export class RetainedRootBoundary {
  private previousRef: number | undefined;
  private desiredRef: number | undefined;
  private visibleRef: number | undefined;
  private desiredNode: SemanticViewNode | undefined;
  private visibleNode: SemanticViewNode | undefined;
  /** Desired roots retained until the corresponding frame is visible. */
  private supersededDesired: DesiredRootRevision[] = [];
  private desiredRevision: string | undefined;
  private visibleRevision: string | undefined;
  private closed = false;
  private readonly deferHostCommit: boolean;
  /** NodeId allocator high-water at the last successful commit (§18). */
  nativeLookupCeiling = 0;

  constructor(
    private readonly session: NativeViewAbiSession,
    private readonly host: () => NativeTuiHostContract | undefined,
    /**
     * PERF-12 T13 (§80): optional alternative commit step for boundaries whose
     * mutation is a direct ref install (`ViewSlot.setViewRef`,
     * `ScrollPane.setContentRef`) rather than a host scene render. Returning
     * false means the boundary change failed; the old root stays installed.
     */
    private readonly installRef?: (rootRef: number) => boolean,
    options: RetainedRootBoundaryOptions = {},
  ) {
    this.deferHostCommit = options.deferHostCommit ?? false;
  }

  /**
   * Adopts a root that already exists natively as the boundary's first root:
   * unconditional NodeId promotion, root-lease transfer, ceiling capture.
   * This is how a boundary takes over state the Direct decoder published.
   */
  adopt(view: View): boolean {
    if (this.closed) throw new Error("boundary is closed");
    const reference = acquireKnownRoot(this.session, view);
    if (reference === undefined) return false;
    if (!this.deferHostCommit) {
      const previous = this.previousRef;
      this.transferRoot(reference);
      // acquireKnownRoot always returns a new lease. If the adopted root is
      // already this boundary's root, keep the existing boundary lease and
      // release only the promotion lease rather than accumulating one per
      // adoption/recovery.
      if (previous === reference) this.releaseReference(reference);
      return true;
    }

    const previousDesired = this.desiredRef;
    const previousVisible = this.visibleRef;
    this.releaseSupersededDesired();
    this.desiredRef = reference;
    this.visibleRef = reference;
    this.desiredNode = semanticNodeOf(view);
    this.visibleNode = this.desiredNode;
    this.desiredRevision = undefined;
    this.visibleRevision = undefined;
    this.nativeLookupCeiling = viewNodeIdHighWater();
    if (previousDesired !== undefined && previousDesired !== reference && previousDesired !== previousVisible) {
      this.releaseReference(previousDesired);
    }
    if (previousVisible !== undefined && previousVisible !== reference) this.releaseReference(previousVisible);
    // The promotion above is the one lease needed by the new desired/visible
    // root. If an older role already held the same reference, discard only the
    // extra promotion lease.
    if (previousDesired === reference || previousVisible === reference) this.releaseReference(reference);
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
    const prepared = this.prepareFrom(view);
    if (prepared === undefined) return undefined;
    if (!this.publishPrepared(prepared)) return undefined;
    return prepared.rootRef;
  }

  /**
   * PERF-12 T13.1 R7: transactional form of {@link install}. Runs every
   * fallible step (retained walk, materialization, NodeId promotion /
   * stale recovery, lease acquisition) WITHOUT publishing anything — the
   * installed root, its lease, and every visible byte stay untouched.
   *
   * Returns `undefined` when the retained path refused (fallback/budget/
   * unrecoverable stale node); the caller keeps the old root, identically to
   * a refused `install`.
   *
   * The returned publication's `commit()` performs only: the single publish
   * call (validated inputs — lease held, generation current), temporary-
   * lease drain, and root-transfer bookkeeping. After successful preparation
   * a commit refusal indicates runtime teardown, not a recoverable condition.
   */
  prepareInstall(view: View): RootPublication | undefined {
    if (this.closed) throw new Error("boundary is closed");
    const prepared = this.prepareFrom(view);
    if (prepared === undefined) return undefined;
    let finished = false;
    return {
      rootRef: prepared.rootRef,
      route: "retained",
      commit: (): void => {
        if (finished) throw new Error("root publication already finished");
        finished = true;
        if (!this.publishPrepared(prepared)) {
          // Pathological: preparation validated the lease and generation, so
          // a publish refusal means the runtime is tearing down underneath
          // us. Surface it loudly instead of silently going stale.
          throw new Error("TUI_ROOT_PUBLISH_REFUSED_AFTER_PREPARE");
        }
      },
      abort: (): void => {
        if (finished) return;
        finished = true;
        this.unwindPrepared(prepared);
      },
    };
  }

  /**
   * H3-only structural publication for the PERF-13 frame seam. Preparation
   * materializes and validates the candidate; commit installs the desired
   * native root but deliberately does not paint it. `commitVisible()` is
   * called only after the later host frame transaction succeeds.
   */
  prepareDesiredInstall(view: View): RootPublication | undefined {
    if (this.closed) throw new Error("boundary is closed");
    if (!this.deferHostCommit) return this.prepareInstall(view);
    const prepared = this.prepareFrom(view);
    if (prepared === undefined) return undefined;
    let finished = false;
    return {
      rootRef: prepared.rootRef,
      route: "retained",
      commit: (): void => {
        if (finished) throw new Error("root publication already finished");
        finished = true;
        try {
          this.publishDesiredPrepared(prepared);
        } catch (error) {
          this.unwindPrepared(prepared);
          throw error;
        }
      },
      abort: (): void => {
        if (finished) return;
        finished = true;
        this.unwindPrepared(prepared);
      },
    };
  }

  /**
   * Promotes the latest desired root into the visible role after a successful
   * host frame. No native call occurs here; the host already rendered the
   * desired ref during the frame transaction.
   */
  commitVisible(revision?: string | number): void {
    if (!this.deferHostCommit || this.desiredRef === undefined) return;
    const key = rootRevisionKey(revision);
    if (key === undefined) {
      this.releaseSupersededDesired();
      const previous = this.visibleRef;
      this.visibleRef = this.desiredRef;
      this.visibleNode = this.desiredNode;
      this.visibleRevision = undefined;
      if (previous !== undefined && previous !== this.visibleRef) this.releaseReference(previous);
      return;
    }
    if (this.visibleRevision !== undefined && compareRootRevisions(key, this.visibleRevision) <= 0) return;

    const target = this.desiredRevision === key
      ? { revision: key, ref: this.desiredRef, node: this.desiredNode }
      : this.supersededDesired.find((entry) => entry.revision === key);
    if (target === undefined || target.node === undefined) return;

    const previous = this.visibleRef;
    const released = new Set<number>();
    this.visibleRef = target.ref;
    this.visibleNode = target.node;
    this.visibleRevision = key;
    if (previous !== undefined && previous !== this.visibleRef) {
      this.releaseReference(previous);
      released.add(previous);
    }

    const remaining: DesiredRootRevision[] = [];
    for (const entry of this.supersededDesired) {
      if (entry.ref === target.ref && entry.revision === target.revision) continue;
      if (compareRootRevisions(entry.revision, key) <= 0) {
        if (
          entry.ref !== this.visibleRef
          && entry.ref !== this.desiredRef
          && !released.has(entry.ref)
        ) {
          this.releaseReference(entry.ref);
          released.add(entry.ref);
        }
      } else {
        remaining.push(entry);
      }
    }
    this.supersededDesired = remaining;
  }

  /** Returns whether the boundary has a desired root awaiting visibility. */
  hasDesiredRoot(): boolean {
    return this.deferHostCommit ? this.desiredRef !== undefined : this.previousRef !== undefined;
  }

  /**
   * Everything fallible in `install`, stopping BEFORE the publish call:
   * retained walk, materialization, lease acquisition/recovery. All failure
   * paths drain temporary leases before returning.
   */
  private prepareFrom(
    view: View,
  ): {
    node: SemanticViewNode;
    rootRef: number;
    tx: MaterializeTx;
    ownsTempLease: boolean;
    phase?: {
      readonly transport_prepare_ns: number;
      readonly native_materialize_ns: number;
    };
    acquiredBoundaryLease: boolean;
  } | undefined {
    const node = semanticNodeOf(view);
    if (this.installRef === undefined && this.host() === undefined) return undefined;
    const prepareStart = phaseNow();
    const tx = new MaterializeTx(
      this.session.symbols,
      this.session.runtime,
      this.session.abi.generation,
      this.nativeLookupCeiling,
    );
    const prepareEnd = phaseNow();
    const materializeStart = phaseNow();
    const hintedRoot = SEMANTIC_NATIVE.get(node);
    let resolvedRef: number;
    try {
      resolvedRef = ensureSemanticNative(node, tx);
    } catch (error) {
      // Fallback, cycle guard, and unexpected errors all drain every
      // temporary lease before the caller sees the failure.
      tx.releaseAll();
      if (error instanceof RetainedFastFallbackError || error instanceof RetainedCycleError) return undefined;
      throw error;
    }
    // A generation-valid hint is only an acceleration hint. The current-root
    // case is the one that cannot be repaired by the NodeId promotion below:
    // it deliberately avoids taking a second lease for the already-installed
    // ref. Validate that hint before returning a prepared publication;
    // otherwise a scavenged slot would fail during commit, where the
    // transaction can no longer take the documented cold-fallback path.
    if (hintedRoot?.generation === this.session.abi.generation && resolvedRef === this.currentRootRef()) {
      try {
        viewRenderRef(this.session.symbols, this.session.runtime, resolvedRef);
      } catch (error) {
        if (!isExpectedNativeStatus(error)) {
          tx.releaseAll();
          throw error;
        }
        let recovered: number | undefined;
        try {
          recovered = recoverStaleNode(node, tx);
        } catch (recoveryError) {
          tx.releaseAll();
          throw recoveryError;
        }
        if (recovered === undefined) {
          tx.releaseAll();
          return undefined;
        }
        resolvedRef = recovered;
      }
    }
    // Does this tx own a lease on the resolved ref (promotion/materialization),
    // or was it borrowed from a hint whose lease belongs to someone else?
    let ownsTempLease = tx.temporaryLeases.includes(resolvedRef);
    // A NodeId promotion can reacquire the boundary's current root after a
    // stale/missing JS hint. That lease is only a temporary probe; retaining
    // it in `releaseAllExcept` would leak one native lease on every such
    // recovery because transferRoot correctly sees the same root already
    // installed.
    if (ownsTempLease && resolvedRef === this.currentRootRef()) ownsTempLease = false;
    let rootRef = resolvedRef;
    let acquiredBoundaryLease = false;
    if (!ownsTempLease && resolvedRef !== this.currentRootRef()) {
      counters.node_id_ref_promotion_attempts += 1;
      const [low, high] = splitNodeId(node.id);
      try {
        rootRef = viewRefForNodeId(this.session.symbols, this.session.runtime, low, high);
        counters.node_id_ref_promotion_hits += 1;
        // A live NodeId promotion may return a new NativeRef after the old
        // hint was scavenged. Keep the sidecar aligned so the next boundary
        // update gets the fast path instead of repeating the promotion.
        installHint(node, this.session.abi.generation, rootRef);
        acquiredBoundaryLease = true;
      } catch (error) {
        if (!isExpectedNativeStatus(error)) {
          tx.releaseAll();
          throw error;
        }
        counters.node_id_ref_promotion_misses += 1;
        let recovered: number | undefined;
        try {
          recovered = recoverStaleNode(node, tx);
        } catch (recoveryError) {
          tx.releaseAll();
          throw recoveryError;
        }
        if (recovered === undefined) {
          tx.releaseAll();
          return undefined;
        }
        rootRef = recovered;
        ownsTempLease = true;
      }
    }
    const materializeEnd = phaseNow();
    const phase = prepareStart !== undefined && prepareEnd !== undefined && materializeStart !== undefined && materializeEnd !== undefined
      ? {
        transport_prepare_ns: prepareEnd - prepareStart,
        native_materialize_ns: materializeEnd - materializeStart,
      }
      : undefined;
    return { node, rootRef, tx, ownsTempLease, phase, acquiredBoundaryLease };
  }

  /**
   * PERF-12 T13.1 R8: COLD transactional publication. Decodes the whole tree
   * via the injected cold materializer (Direct decode, NO painting) and
   * returns a publication whose commit either paints the prepared ref once
   * for a legacy/direct boundary or installs it as desired structure for the
   * H3 host boundary. Used when the retained path refuses — guarantees "cold
   * fallback never paints during PREPARE" (handoff §32.2.3 hard rule).
   */
  prepareColdInstall(view: View): RootPublication | undefined {
    if (this.deferHostCommit) return this.prepareDesiredColdInstall(view);
    if (this.closed) throw new Error("boundary is closed");
    const node = semanticNodeOf(view);
    if (this.installRef === undefined && this.host() === undefined) return undefined;
    const materialize = COLD_ROOT_MATERIALIZER;
    if (materialize === undefined) return undefined;
    const materializeStart = phaseNow();
    const rootRef = materialize(view);
    const materializeEnd = phaseNow();
    if (rootRef === undefined) return undefined;
    const coldPhase = materializeStart !== undefined && materializeEnd !== undefined
      ? materializeEnd - materializeStart
      : undefined;
    let finished = false;
    return {
      rootRef,
      route: "fallback",
      commit: (): void => {
        if (finished) throw new Error("root publication already finished");
        finished = true;
        let transferred = false;
        const hostStart = phaseNow();
        try {
          if (this.installRef !== undefined) {
            // Component/scroll boundaries publish through their retained slot
            // target rather than the scene host. Cold preparation is still
            // paint-free; the ref install is the only commit-side mutation.
            if (!this.installRef(rootRef)) {
              throw new Error("TUI_ROOT_COLD_PUBLISH_REFUSED: component target refused");
            }
          } else {
            const host = this.host();
            if (host === undefined) {
              throw new Error("TUI_ROOT_COLD_PUBLISH_REFUSED: no host");
            }
            const status = hostRenderRef(this.session.symbols, this.session.runtime, host, rootRef);
            if (status !== HOST_STATUS_OK) {
              throw new Error(`TUI_ROOT_COLD_PUBLISH_REFUSED: status ${status}`);
            }
          }
          const hostEnd = phaseNow();
          if (coldPhase !== undefined && hostStart !== undefined && hostEnd !== undefined) {
            recordPhaseSample({
              transport_prepare_ns: 0,
              native_materialize_ns: coldPhase,
              host_commit_ns: hostEnd - hostStart,
            });
          }
          counters.host_mutations += 1;
          // The boundary now owns the recovered lease, so its JS-side hint is
          // safe to use for the next exact-root render.
          installHint(node, this.session.abi.generation, rootRef);
          // Transfer our lease on the new root into the boundary; the previous
          // root's lease is released here exactly like transferRoot does. A
          // cold decoder may rediscover the already-installed root, in which
          // case its extra lease must not become a permanent duplicate.
          const duplicateRoot = this.previousRef === rootRef;
          this.transferRoot(rootRef);
          if (duplicateRoot) this.releaseColdLease(rootRef);
          transferred = true;
        } catch (error) {
          // A thrown install/host callback must not strand the materializer's
          // temporary lease. The publication is already finished, so abort()
          // will not run as a second cleanup path.
          if (!transferred) this.releaseColdLease(rootRef);
          throw error;
        }
      },
      abort: (): void => {
        if (finished) return;
        finished = true;
        this.releaseColdLease(rootRef);
      },
    };
  }

  private prepareDesiredColdInstall(view: View): RootPublication | undefined {
    if (this.closed) throw new Error("boundary is closed");
    const node = semanticNodeOf(view);
    const materialize = COLD_ROOT_MATERIALIZER;
    if (materialize === undefined) return undefined;
    const rootRef = materialize(view);
    if (rootRef === undefined) return undefined;
    let finished = false;
    return {
      rootRef,
      route: "fallback",
      commit: (): void => {
        if (finished) throw new Error("root publication already finished");
        finished = true;
        const host = this.host();
        const setDesired = host?.setDesiredViewRef;
        if (host === undefined || setDesired === undefined) {
          throw new Error("TUI_ROOT_DESIRED_PUBLISH_UNAVAILABLE");
        }
        try {
          setDesired.call(host, rootRef);
          const duplicate = this.desiredRef === rootRef || this.visibleRef === rootRef;
          this.transferDesiredRoot(rootRef, node, this.desiredRevisionForHost(host));
          installHint(node, this.session.abi.generation, rootRef);
          if (duplicate) this.releaseColdLease(rootRef);
        } catch (error) {
          this.releaseColdLease(rootRef);
          throw error;
        }
      },
      abort: (): void => {
        if (finished) return;
        finished = true;
        this.releaseColdLease(rootRef);
      },
    };
  }

  private releaseColdLease(rootRef: number): void {
    this.releaseReference(rootRef);
  }

  /**
   * Commits a prepared root as desired structure. The native host receives the
   * root but does not paint until the environment frame drain runs.
   */
  private publishDesiredPrepared(prepared: PreparedRootInstall): void {
    const host = this.host();
    const setDesired = host?.setDesiredViewRef;
    if (host === undefined || setDesired === undefined) {
      throw new Error("TUI_ROOT_DESIRED_PUBLISH_UNAVAILABLE");
    }
    const sameVisible = this.visibleRef === prepared.rootRef;
    setDesired.call(host, prepared.rootRef);
    const desiredRevision = this.desiredRevisionForHost(host);
    if (prepared.ownsTempLease) {
      if (sameVisible) prepared.tx.releaseAll();
      else prepared.tx.releaseAllExcept(prepared.rootRef);
    } else {
      prepared.tx.releaseAll();
    }
    this.transferDesiredRoot(prepared.rootRef, prepared.node, desiredRevision);
    // A NodeId promotion is not tracked in the transaction lease list. When
    // it selected the already-visible ref, release that extra promotion lease
    // after the desired role has been switched to the existing visible lease.
    if (prepared.acquiredBoundaryLease && sameVisible) this.releaseReference(prepared.rootRef);
  }

  /**
   * The publish + bookkeeping tail of `install`. Returns false only on a
   * publish refusal from the host callback (which leaves the old root
   * installed); unwinds all acquired leases in that case.
   */
  private publishPrepared(prepared: {
    node: SemanticViewNode;
    rootRef: number;
    tx: MaterializeTx;
    ownsTempLease: boolean;
    phase?: {
      readonly transport_prepare_ns: number;
      readonly native_materialize_ns: number;
    };
    acquiredBoundaryLease: boolean;
  }): boolean {
    const rootRef = prepared.rootRef;
    const hostStart = phaseNow();
    try {
      if (this.installRef !== undefined) {
        if (!this.installRef(rootRef)) {
          this.unwindPrepared(prepared);
          return false;
        }
      } else {
        const host = this.host();
        if (host === undefined) {
          // A root publication without a live host is a refusal, not a
          // successful no-op. Keep the old root authoritative and release all
          // state acquired during prepare.
          this.unwindPrepared(prepared);
          return false;
        }
        const status = hostRenderRef(this.session.symbols, this.session.runtime, host, rootRef);
        if (status !== HOST_STATUS_OK) {
          // Release only a freshly acquired boundary lease whose ref is not
          // already the boundary's leased previous root (§18 failure keeps
          // the old root installed).
          this.unwindPrepared(prepared);
          return false;
        }
        counters.host_mutations += 1;
      }
      const hostEnd = phaseNow();
      if (prepared.phase !== undefined && hostStart !== undefined && hostEnd !== undefined) {
        recordPhaseSample({
          ...prepared.phase,
          host_commit_ns: hostEnd - hostStart,
        });
      }
      if (prepared.ownsTempLease) prepared.tx.releaseAllExcept(rootRef);
      else prepared.tx.releaseAll();
      this.transferRoot(rootRef);
      return true;
    } catch (error) {
      // The host callback/FFI boundary is part of the transaction too: an
      // unexpected throw must not strand temporary or newly-acquired root
      // leases after semantic materialization has already succeeded.
      this.unwindPrepared(prepared);
      throw error;
    }
  }

  /** Drains temporary leases and releases any freshly acquired boundary lease. */
  private unwindPrepared(prepared: {
    tx: MaterializeTx;
    rootRef: number;
    acquiredBoundaryLease: boolean;
  }): void {
    prepared.tx.releaseAll();
    if (prepared.acquiredBoundaryLease && prepared.rootRef !== this.currentRootRef()) {
      this.releaseReference(prepared.rootRef);
      prepared.acquiredBoundaryLease = false;
    }
  }

  /** §20 exact-root fast path against the currently installed root hint. */
  renderExact(view: View): ExactRootRender {
    if (this.closed) throw new Error("boundary is closed");
    if (this.deferHostCommit) {
      if (this.desiredRef === undefined || this.desiredNode !== semanticNodeOf(view)) {
        return { status: "no_root_ref" };
      }
      // The frame broker owns the actual host mutation in deferred mode. An
      // identity hit therefore only confirms that the desired root is already
      // known; it must not create a second desired revision.
      return { status: "ok", rootRef: this.desiredRef, recovered: false };
    }
    const host = this.host();
    if (host === undefined) return { status: "no_root_ref" };
    const result = renderExactRoot(this.session, host, view);
    if (result.status === "ok" && result.recovered) {
      const previous = this.previousRef;
      this.transferRoot(result.rootRef);
      // A defensive native implementation could return the same ref after a
      // stale-status retry. In that case transferRoot must keep the existing
      // root lease and release only the extra promotion lease.
      if (previous === result.rootRef) this.releaseReference(result.rootRef);
    }
    return result;
  }

  /** Releases the boundary's root lease exactly once (§18 close protocol). */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    if (this.deferHostCommit) {
      const desired = this.desiredRef;
      const visible = this.visibleRef;
      this.releaseSupersededDesired();
      this.desiredRef = undefined;
      this.visibleRef = undefined;
      this.desiredNode = undefined;
      this.visibleNode = undefined;
      this.desiredRevision = undefined;
      this.visibleRevision = undefined;
      if (desired !== undefined) this.releaseReference(desired);
      if (visible !== undefined && visible !== desired) this.releaseReference(visible);
      return;
    }
    const ref = this.previousRef;
    this.previousRef = undefined;
    if (ref !== undefined) this.releaseReference(ref);
  }

  private currentRootRef(): number | undefined {
    return this.deferHostCommit ? this.desiredRef : this.previousRef;
  }

  private transferDesiredRoot(
    reference: number,
    node: SemanticViewNode,
    revision?: string | number,
  ): void {
    const key = rootRevisionKey(revision);
    const previous = this.desiredRef;
    const visible = this.visibleRef;
    if (key !== undefined && this.desiredRevision !== undefined) {
      this.supersededDesired.push({
        revision: this.desiredRevision,
        ref: previous!,
        node: this.desiredNode!,
      });
    } else {
      this.releaseSupersededDesired();
      if (previous !== undefined && previous !== reference && previous !== visible) {
        this.releaseReference(previous);
      }
    }
    this.desiredRef = reference;
    this.desiredNode = node;
    this.desiredRevision = key;
    this.nativeLookupCeiling = viewNodeIdHighWater();
  }

  private desiredRevisionForHost(host: NativeTuiHostContract): string | number | undefined {
    return host.epochs?.().desired_structural_revision;
  }

  private releaseSupersededDesired(): void {
    for (const entry of this.supersededDesired) {
      if (entry.ref !== this.visibleRef && entry.ref !== this.desiredRef) {
        this.releaseReference(entry.ref);
      }
    }
    this.supersededDesired = [];
  }

  private transferRoot(reference: number): void {
    const previous = this.previousRef;
    this.previousRef = reference;
    this.nativeLookupCeiling = viewNodeIdHighWater();
    if (previous !== undefined && previous !== reference) this.releaseReference(previous);
  }

  private releaseReference(reference: number): void {
    if (!isValidNativeRef(reference)) return;
    const batch = Uint32Array.of(reference);
    viewReleaseMany(this.session.symbols, this.session.runtime, batch, 1);
  }
}

/**
 * PERF-12 T13: the theme epoch rule. Themed StyleRefs resolve theme KEYS
 * against the host theme at paint time, but the JS sidecar caches resolved
 * refs per (runtime, generation). Replacing the host theme therefore requires
 * dropping cached refs so later materializations re-resolve under the new
 * theme. Called by `Tui.setTheme` before installing the new theme.
 */
export function resetStyleRefCacheForThemeChange(): void {
  STYLE_REF_CACHE.runtime = undefined;
  STYLE_REF_CACHE.generation = -1;
  STYLE_REF_CACHE.refs = new WeakMap();
  STYLE_REF_CACHE.atoms = new Map();
}
