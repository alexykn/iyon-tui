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
import { native } from "../native.ts";
import {
  materializeColumn,
  materializeRow,
  materializeSpacer,
} from "./generated/view_materialize.ts";
import { NativeAbiStatusError, hostRenderRef, styleAtomCreateCstring, styleCreateBits, viewAxisSetChild, viewAxisSpliceBuffer, viewClampCreate, viewCommonPatchRoot, viewComponentCreate, viewContainerCreate, viewDecoratedCreateBuffer, viewDiffCreateBuffer, viewGridCreateBuffer, viewGridSetCell, viewHangingCreate, viewRefForNodeId, viewReleaseMany, viewTextCreateCstring, viewTextCreateCstring2, viewTextCreateCstring3, viewTextCreateCstring4, viewTextCreateUtf8, viewTextCreateUtf82, viewTextCreateUtf83, viewTextCreateUtf84, viewTextLayoutPatchRoot } from "./generated/view_calls.ts";
import { BRIDGE_DIFF_LINE_KIND, BRIDGE_DIFF_LINE_TERMINATION, BRIDGE_GRID_TRACK_KIND, BRIDGE_OVERFLOW_KIND, BRIDGE_VIEW_KIND, peekBridgeDerivation, peekBridgeGridSequenceOverride, peekBridgeSequenceOverride, type BridgeGridTrackNode, type BridgeViewNode, type ColorNode, type StyleNode } from "./ir.ts";
import { nodeForBridge, viewNodeIdHighWater, type View } from "./values/view.ts";
import type { NativeViewAbiSession } from "./native_view_abi.ts";
import {
  MAX_DIRECT_AXIS_REFS,
  MAX_DIRECT_DIFF_BYTES,
  MAX_DIRECT_DIFF_WORDS,
  MAX_DIRECT_GRID_WORDS,
  MAX_DIRECT_TEXT_BYTES,
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
 * Single-slot: one live NativeViewRuntime per environment, and a stale
 * pointer's storage is simply replaced at the next allocation, so nothing
 * accumulates across environment resets. Native retains no pointer into it
 * after any call returns (§29).
 */
const AXIS_REF_SCRATCH: { runtime: Pointer | undefined; array: Uint32Array } = {
  runtime: undefined,
  array: new Uint32Array(0),
};

/** PERF-12 T10 (§30/§36): reusable medium-tier flat-grid word scratch. */
const GRID_WORD_SCRATCH: { runtime: Pointer | undefined; array: Uint32Array } = {
  runtime: undefined,
  array: new Uint32Array(0),
};

/** PERF-12 T11 (§30): reusable byte tier for text/diff UTF-8 payloads. */
const BYTE_SCRATCH: { runtime: Pointer | undefined; array: Uint8Array } = {
  runtime: undefined,
  array: new Uint8Array(0),
};

/**
 * PERF-12 T11 (§40): generation-scoped StyleRef sidecar. Stable style objects
 * map to native style refs exactly once per runtime generation; the native
 * runtime's style table stays the only authoritative style cache (§40 bans a
 * second one — this sidecar is acceleration metadata, like BRIDGE_NATIVE).
 * Refs die with the generation; the single-slot reset mirrors AXIS_REF_SCRATCH.
 */
const STYLE_REF_CACHE: {
  runtime: Pointer | undefined;
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
  /** One targeted stale-ref recovery is allowed per root transaction (§47). */
  staleRefRetries = 0;

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
    // Small tier sized for exactly the retained cap; allocated once per
    // runtime generation and reused by every transaction.
    if (
      AXIS_REF_SCRATCH.runtime !== this.runtime ||
      AXIS_REF_SCRATCH.array.length < words
    ) {
      AXIS_REF_SCRATCH.runtime = this.runtime;
      AXIS_REF_SCRATCH.array = new Uint32Array(MAX_DIRECT_AXIS_REFS * 2);
    }
    counters.transport_scratch_reuses += 1;
    return AXIS_REF_SCRATCH.array.subarray(0, words);
  }

  /** Returns reusable u32 construction scratch; no per-node TypedArray. */
  private wordScratch(wordCount: number, cap: number, label: string): Uint32Array {
    if (wordCount > cap) {
      counters.cold_fallbacks += 1;
      throw new RetainedFastFallbackError(
        `${label} word payload ${wordCount} exceeds the retained cap ${cap}`,
      );
    }
    const size = Math.max(MAX_DIRECT_DIFF_WORDS, MAX_DIRECT_GRID_WORDS);
    if (GRID_WORD_SCRATCH.runtime !== this.runtime || GRID_WORD_SCRATCH.array.length < size) {
      GRID_WORD_SCRATCH.runtime = this.runtime;
      GRID_WORD_SCRATCH.array = new Uint32Array(size);
    }
    counters.transport_scratch_reuses += 1;
    return GRID_WORD_SCRATCH.array.subarray(0, wordCount);
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

type NodeMaterializer = (node: BridgeViewNode, tx: MaterializeTx) => number;

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

/** Exceptional §73 recovery: Direct decodes one node and returns one lease. */
function recoverNodeWithDirectDecode(node: BridgeViewNode, tx: MaterializeTx): number | undefined {
  const decodeRef = native.tuiViewAbiDecodeRef;
  if (decodeRef === undefined) return undefined;
  const reference = decodeRef(node as unknown as object);
  if (!isValidNativeRef(reference)) return undefined;
  installHint(node, tx.generation, reference);
  tx.refs.set(node, reference);
  tx.temporaryLeases.push(reference);
  return reference;
}

function childAtOrdinal(node: BridgeViewNode, ordinal: number): BridgeViewNode | undefined {
  if (!Number.isSafeInteger(ordinal) || ordinal < 0) return undefined;
  if (node.kind === BRIDGE_VIEW_KIND.row || node.kind === BRIDGE_VIEW_KIND.column) {
    const override = peekBridgeSequenceOverride(node);
    const child = override?.sequence.get(ordinal) ?? node.children[ordinal];
    return child?.child;
  }
  if (node.kind === BRIDGE_VIEW_KIND.grid) {
    const override = peekBridgeGridSequenceOverride(node);
    if (override !== undefined) return override.sequence.get(ordinal)?.view;
    let offset = ordinal;
    for (const row of node.rows) {
      if (offset < row.cells.length) return row.cells[offset]?.view;
      offset -= row.cells.length;
    }
  }
  if (node.kind === BRIDGE_VIEW_KIND.hanging) {
    return [node.prefix, node.continuation, node.body][ordinal];
  }
  if (node.kind === BRIDGE_VIEW_KIND.container || node.kind === BRIDGE_VIEW_KIND.clamp || node.kind === BRIDGE_VIEW_KIND.contentMax) {
    return ordinal === 0 ? node.child : undefined;
  }
  if (node.kind === BRIDGE_VIEW_KIND.decorated) return ordinal === 0 ? node.child : undefined;
  return undefined;
}

function derivationChildAt(derivation: NonNullable<ReturnType<typeof peekBridgeDerivation>>, ordinal: number): BridgeViewNode | undefined {
  switch (derivation.kind) {
    case "axisSet":
    case "gridCell":
      return ordinal === 0 ? derivation.child : undefined;
    case "axisSplice":
      return derivation.inserted[ordinal]?.node;
    case "textLayout":
    case "commonScalar":
      return undefined;
  }
}

/**
 * Invalidates one stale hint and performs the bounded recovery used by both
 * constructors and retained edit primitives. The Direct decoder is the
 * authoritative exceptional fallback when this node has no retained
 * materializer of its own (§47/§73).
 */
function recoverStaleNode(node: BridgeViewNode, tx: MaterializeTx): number | undefined {
  if (tx.staleRefRetries >= 1) return undefined;
  tx.staleRefRetries += 1;
  counters.stale_ref_retries += 1;
  deleteBridgeNativeHintForTests(node);
  tx.refs.delete(node);
  try {
    return ensureNative(node, tx);
  } catch (error) {
    if (!(error instanceof RetainedFastFallbackError) && !(error instanceof RetainedCycleError)) throw error;
  }
  return recoverNodeWithDirectDecode(node, tx);
}

function materializeWithRecovery(
  node: BridgeViewNode,
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

/**
 * Runs one generated lowering and converts expected native failure statuses
 * into a fast fallback so the caller routes the complete cold path (§49).
 * Unexpected errors propagate.
 */
function runMaterializer(_kind: string, lower: () => number): number {
  // Native status errors stay typed until ensureNative can perform the one
  // targeted stale-child retry. Non-cache failures become the complete
  // fallback there; unexpected exceptions remain visible to the caller.
  return lower();
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

function gridTrackAmount(value: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff) {
    throw new RetainedFastFallbackError(`grid ${label} is outside the u16 range`);
  }
  return value;
}

function gridTrackWord(track: BridgeGridTrackNode): number {
  switch (track.kind) {
    case BRIDGE_GRID_TRACK_KIND.content: return 1;
    case BRIDGE_GRID_TRACK_KIND.contentMax: return 2 | (gridTrackAmount(track.max, "contentMax") << 8);
    case BRIDGE_GRID_TRACK_KIND.fixed: return 3 | (gridTrackAmount(track.size, "fixed") << 8);
    case BRIDGE_GRID_TRACK_KIND.flex: return 4;
    case BRIDGE_GRID_TRACK_KIND.flexMax: return 5 | (gridTrackAmount(track.max, "flexMax") << 8);
  }
}

/** PERF-12 T10 (§36): new Grid construction through one borrowed word lane. */
function materializeGridNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.grid) throw new RetainedFastFallbackError("kind mismatch");
  const wordCount = 2 + node.columns.length
    + node.rows.reduce((total, row) => total + 2 + row.cells.length * 3, 0);
  const words = tx.gridWordScratch(wordCount);
  let offset = 0;
  words[offset++] = node.columns.length;
  for (const track of node.columns) words[offset++] = gridTrackWord(track);
  words[offset++] = node.rows.length;
  for (const row of node.rows) {
    words[offset++] = gridTrackWord(row.track);
    words[offset++] = row.cells.length;
    for (const cell of row.cells) {
      words[offset++] = ensureNative(cell.view, tx);
      words[offset++] = (cell.columnSpan & 0xffff) | ((cell.rowSpan & 0xffff) << 16);
      words[offset++] = (cell.horizontalAlign & 0xffff) | ((cell.verticalAlign & 0xffff) << 16);
    }
  }
  tx.noteRefWords(wordCount);
  counters.bridge_children_visited += node.rows.reduce((total, row) => total + row.cells.length, 0);
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

const STYLE_ATTRIBUTE_BITS = {
  bold: 1,
  dim: 2,
  italic: 4,
  underline: 8,
  reversed: 16,
  strikethrough: 32,
} as const;

function ensureStyleCache(tx: MaterializeTx): void {
  if (STYLE_REF_CACHE.runtime !== tx.runtime || STYLE_REF_CACHE.generation !== tx.generation) {
    STYLE_REF_CACHE.runtime = tx.runtime;
    STYLE_REF_CACHE.generation = tx.generation;
    STYLE_REF_CACHE.refs = new WeakMap();
    STYLE_REF_CACHE.atoms = new Map();
  }
}

function styleColorAtom(color: ColorNode): string {
  return typeof color === "string" ? color : `ansi:${color.value}`;
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
function styleRefFor(style: StyleNode | undefined, tx: MaterializeTx): number {
  if (style === undefined) return 0;
  ensureStyleCache(tx);
  const cached = STYLE_REF_CACHE.refs.get(style);
  if (cached !== undefined) return cached;
  let present = 0;
  let truth = 0;
  for (const name of Object.keys(style.attributes)) {
    const bit = STYLE_ATTRIBUTE_BITS[name as keyof typeof STYLE_ATTRIBUTE_BITS];
    if (bit === undefined) {
      throw new RetainedFastFallbackError(`unknown text attribute ${name}`);
    }
    present |= bit;
    if (style.attributes[name]) truth |= bit;
  }
  const foreground = style.foreground === undefined ? 0 : styleAtomRef(styleColorAtom(style.foreground), tx);
  const background = style.background === undefined ? 0 : styleAtomRef(styleColorAtom(style.background), tx);
  const theme = style.theme === undefined ? 0 : styleAtomRef(`theme:${style.theme}`, tx);
  const reference = styleCreateBits(tx.symbols, tx.runtime, 0, present, truth, foreground, background, theme);
  STYLE_REF_CACHE.refs.set(style, reference);
  return reference;
}

function materializeTextNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.text) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const spans = node.spans;
  if (spans.length < 1 || spans.length > 4) {
    counters.cold_fallbacks += 1;
    throw new RetainedFastFallbackError(`text span count ${spans.length} is outside the retained family`);
  }
  // Payload dependencies resolve before any transport (children-first analog).
  // Style publication failures are retained-path refusals: they count the
  // fallback and route the complete cold path like any other cap/shape miss.
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
        case 1: return viewTextCreateCstring(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, node.wrap, node.align);
        case 2: return viewTextCreateCstring2(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, node.wrap, node.align);
        case 3: return viewTextCreateCstring3(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, spans[2]!.text, styleRefs[2]!, node.wrap, node.align);
        default: return viewTextCreateCstring4(tx.symbols, tx.runtime, low, high, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, spans[2]!.text, styleRefs[2]!, spans[3]!.text, styleRefs[3]!, node.wrap, node.align);
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
      case 1: return viewTextCreateUtf8(tx.symbols, tx.runtime, low, high, scratch, offset, styleRefs[0]!, node.wrap, node.align);
      case 2: return viewTextCreateUtf82(tx.symbols, tx.runtime, low, high, scratch, offset, lengths[0]!, styleRefs[0]!, lengths[1]!, styleRefs[1]!, node.wrap, node.align);
      case 3: return viewTextCreateUtf83(tx.symbols, tx.runtime, low, high, scratch, offset, lengths[0]!, styleRefs[0]!, lengths[1]!, styleRefs[1]!, lengths[2]!, styleRefs[2]!, node.wrap, node.align);
      default: return viewTextCreateUtf84(tx.symbols, tx.runtime, low, high, scratch, offset, lengths[0]!, styleRefs[0]!, lengths[1]!, styleRefs[1]!, lengths[2]!, styleRefs[2]!, lengths[3]!, styleRefs[3]!, node.wrap, node.align);
    }
  });
}

/** Splits a safe-integer coordinate into the canonical lo/hi word pair. */
function u64Words(value: number): [number, number] {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RetainedFastFallbackError(`diff coordinate ${value} is not a safe non-negative integer`);
  }
  return [value % 0x1_0000_0000, Math.floor(value / 0x1_0000_0000)];
}

/** PERF-12 T11 (§41): new-Diff construction through one words+bytes call. */
function materializeDiffNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.diff) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const hunks = node.hunks;
  let lineTotal = 0;
  let wordCount = 1;
  for (const hunk of hunks) {
    lineTotal += hunk.lines.length;
    wordCount += 9 + hunk.lines.length * 6;
  }
  const words = tx.diffWordScratch(wordCount);
  const bytes = tx.byteScratch(MAX_DIRECT_TEXT_BYTES, "diff");
  let wordOffset = 0;
  let byteOffset = 0;
  const writeWords = (...values: number[]): void => {
    for (const value of values) words[wordOffset++] = value;
  };
  writeWords(hunks.length);
  for (const hunk of hunks) {
    const oldStart = u64Words(hunk.oldRange.start);
    const oldCount = u64Words(hunk.oldRange.count);
    const newStart = u64Words(hunk.newRange.start);
    const newCount = u64Words(hunk.newRange.count);
    writeWords(oldStart[0], oldStart[1], oldCount[0], oldCount[1]);
    writeWords(newStart[0], newStart[1], newCount[0], newCount[1]);
    writeWords(hunk.lines.length);
    for (const line of hunk.lines) {
      const meta = line.kind | (line.termination << 16);
      const oldLine = line.oldLine === undefined ? [0, 0] : u64Words(line.oldLine);
      const newLine = line.newLine === undefined ? [0, 0] : u64Words(line.newLine);
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

/** PERF-12 T13: overflow-indicator codes shared with the native parser. */
const OVERFLOW_NONE = 0;
const OVERFLOW_ELLIPSIS = 1;
const OVERFLOW_FOOTER = 2;

/**
 * T13: styleRefFor with retained-refusal accounting — publication failures
 * count one cold fallback and route the complete cold path like any cap miss.
 */
function styleRefCounted(style: StyleNode | undefined, tx: MaterializeTx): number {
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

function materializeHangingNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.hanging) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const prefixRef = ensureNative(node.prefix, tx);
  const continuationRef = ensureNative(node.continuation, tx);
  const bodyRef = ensureNative(node.body, tx);
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("hanging", () =>
    viewHangingCreate(tx.symbols, tx.runtime, low, high, prefixRef, continuationRef, bodyRef)
  );
}

function materializeContainerNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.container) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const childRef = ensureNative(node.child, tx);
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("container", () =>
    viewContainerCreate(tx.symbols, tx.runtime, low, high, childRef)
  );
}

/** Lowers clamp/contentMax nodes; contentMax is a clamp with no indicator. */
function materializeClampNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.clamp && node.kind !== BRIDGE_VIEW_KIND.contentMax) {
    throw new RetainedFastFallbackError("kind mismatch");
  }
  counters.bridge_semantic_nodes_inspected += 1;
  const childRef = ensureNative(node.child, tx);
  let overflowKind = OVERFLOW_NONE;
  let overflowStyleRef = 0;
  let prefix = "";
  if (node.kind === BRIDGE_VIEW_KIND.clamp && node.overflow !== undefined && node.overflow.kind !== BRIDGE_OVERFLOW_KIND.none) {
    const overflow = node.overflow;
    overflowKind = overflow.kind === BRIDGE_OVERFLOW_KIND.ellipsis ? OVERFLOW_ELLIPSIS : OVERFLOW_FOOTER;
    overflowStyleRef = styleRefCounted(overflow.style, tx);
    if (overflow.kind === BRIDGE_OVERFLOW_KIND.footer) prefix = overflow.prefix;
  }
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("clamp", () =>
    viewClampCreate(tx.symbols, tx.runtime, low, high, childRef, node.maxRows ?? 0, overflowKind, overflowStyleRef, prefix)
  );
}

function materializeComponentNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.component) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const handleWords = u64Words(node.handle);
  const [low, high] = splitNodeId(node.id);
  return runMaterializer("component", () =>
    viewComponentCreate(tx.symbols, tx.runtime, low, high, handleWords[0], handleWords[1])
  );
}

/** Decoration mask bits shared with the native parser (T13 §76 framing). */
const DECORATION_PADDING = 1;
const DECORATION_BACKGROUND = 2;
const DECORATION_FOREGROUND = 4;
const DECORATION_BORDER = 8;
const DECORATION_WIDTH = 16;
const DECORATION_HEIGHT = 32;
const DECORATION_MIN_WIDTH = 64;
const DECORATION_MAX_WIDTH = 128;
const DECORATION_MIN_HEIGHT = 256;
const DECORATION_MAX_HEIGHT = 512;

const BORDER_STYLE_CODES = { plain: 1, rounded: 2, double: 3 } as const;
const BORDER_EDGE_CODES = { all: 1, topBottom: 2 } as const;
const WIDTH_RULE_CODES = { fit: 1, fill: 2 } as const;

function materializeDecoratedNode(node: BridgeViewNode, tx: MaterializeTx): number {
  if (node.kind !== BRIDGE_VIEW_KIND.decorated) throw new RetainedFastFallbackError("kind mismatch");
  counters.bridge_semantic_nodes_inspected += 1;
  const childRef = ensureNative(node.child, tx);
  const decoration = node.decoration;
  if (decoration.border?.glyphs !== undefined && Object.keys(decoration.border.glyphs).length > 0) {
    counters.cold_fallbacks += 1;
    throw new RetainedFastFallbackError("custom border glyphs are not expressible on the retained lane");
  }
  let mask = 0;
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
  const borderStyle = decoration.border?.style === undefined ? 0 : BORDER_STYLE_CODES[decoration.border.style];
  const borderEdges = decoration.border?.edges === undefined ? 0 : BORDER_EDGE_CODES[decoration.border.edges];
  const borderColorAtom = decoration.border?.color === undefined || decoration.border === undefined ? 0 : styleAtomRef(styleColorAtom(decoration.border.color), tx);
  const padding = decoration.padding;
  if (padding !== undefined) mask |= DECORATION_PADDING;
  if (padding !== undefined) mask |= DECORATION_PADDING;
  if (decoration.background !== undefined) mask |= DECORATION_BACKGROUND;
  if (decoration.foreground !== undefined) mask |= DECORATION_FOREGROUND;
  if (decoration.border !== undefined) mask |= DECORATION_BORDER;
  if (decoration.width !== undefined) mask |= DECORATION_WIDTH;
  if (decoration.height !== undefined) mask |= DECORATION_HEIGHT;
  if (decoration.minWidth !== undefined) mask |= DECORATION_MIN_WIDTH;
  if (decoration.maxWidth !== undefined) mask |= DECORATION_MAX_WIDTH;
  if (decoration.minHeight !== undefined) mask |= DECORATION_MIN_HEIGHT;
  if (decoration.maxHeight !== undefined) mask |= DECORATION_MAX_HEIGHT;
  let wordOffset = 0;
  const writeWord = (value: number): void => {
    words[wordOffset++] = value;
  };
  writeWord(mask);
  writeWord(padding === undefined ? 0 : (padding.top | (padding.right << 16)) >>> 0);
  writeWord(padding === undefined ? 0 : (padding.bottom | (padding.left << 16)) >>> 0);
  writeWord((decoration.width === undefined ? 0 : WIDTH_RULE_CODES[decoration.width])
    | ((decoration.height === undefined ? 0 : WIDTH_RULE_CODES[decoration.height]) << 16));
  writeWord((decoration.minWidth ?? 0) | ((decoration.maxWidth ?? 0) << 16));
  writeWord((decoration.minHeight ?? 0) | ((decoration.maxHeight ?? 0) << 16));
  writeWord(colorAtoms[0]);
  writeWord(colorAtoms[1]);
  writeWord(borderStyle | (borderEdges << 8));
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
 * Per-kind generated materializer dispatch (§22 children-first, §32 fixed
 * arities). T7 covers spacer plus row/column arities 0..=4; T10 adds Grid;
 * T11 adds the text cstring/utf8 payload lanes and the diff words+bytes lane;
 * T13 adds hanging, container, clamp/contentMax, component references, and
 * decorated nodes — every §76 kind is now direct-materialized or explicitly
 * fallback-routed (text spans >4, oversized payloads, custom border glyphs).
 */
const MATERIALIZERS = new Map<number, NodeMaterializer>([
  [BRIDGE_VIEW_KIND.spacer, materializeSpacerNode],
  [BRIDGE_VIEW_KIND.row, materializeRowNode],
  [BRIDGE_VIEW_KIND.column, materializeColumnNode],
  [BRIDGE_VIEW_KIND.grid, materializeGridNode],
  [BRIDGE_VIEW_KIND.text, materializeTextNode],
  [BRIDGE_VIEW_KIND.diff, materializeDiffNode],
  [BRIDGE_VIEW_KIND.hanging, materializeHangingNode],
  [BRIDGE_VIEW_KIND.container, materializeContainerNode],
  [BRIDGE_VIEW_KIND.clamp, materializeClampNode],
  [BRIDGE_VIEW_KIND.contentMax, materializeClampNode],
  [BRIDGE_VIEW_KIND.component, materializeComponentNode],
  [BRIDGE_VIEW_KIND.decorated, materializeDecoratedNode],
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
function derivationBaseRef(base: BridgeViewNode, tx: MaterializeTx): number | undefined {
  const hint = BRIDGE_NATIVE.get(base);
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
function tryDerivation(node: BridgeViewNode, tx: MaterializeTx): number | undefined {
  const derivation = peekBridgeDerivation(node);
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
        derivation.wrap,
        derivation.align,
      );
    } else if (derivation.kind === "commonScalar") {
      reference = viewCommonPatchRoot(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        derivation.mask,
        derivation.paddingTopRight,
        derivation.paddingBottomLeft,
        derivation.widthRule,
        derivation.heightRule,
        derivation.minWidth,
        derivation.maxWidth,
        derivation.minHeight,
        derivation.maxHeight,
        0,
      );
    } else if (derivation.kind === "axisSet") {
      // §35 children-first: resolve only the replacement child, never the
      // old wide sequence.
      const childRef = ensureNative(derivation.child, tx);
      reference = viewAxisSetChild(
        tx.symbols,
        tx.runtime,
        baseRef,
        low,
        high,
        derivation.index,
        derivation.trackWord,
        childRef,
      );
    } else if (derivation.kind === "axisSplice") {
      // Only inserted refs cross FFI; the old sequence remains native-retained.
      const scratch = tx.axisRefScratch(derivation.inserted.length);
      let offset = 0;
      for (const entry of derivation.inserted) {
        scratch[offset++] = entry.trackWord;
        scratch[offset++] = ensureNative(entry.node, tx);
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
      const childRef = ensureNative(derivation.child, tx);
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
      let recoveredRef: number | undefined;
      try {
        recoveredRef = viewRefForNodeId(session.symbols, session.runtime, low, high);
        counters.node_id_ref_promotion_hits += 1;
      } catch (error) {
        if (!isExpectedNativeStatus(error)) throw error;
        counters.node_id_ref_promotion_misses += 1;
        const decodeRef = native.tuiViewAbiDecodeRef;
        if (decodeRef !== undefined) {
          recoveredRef = decodeRef(node as unknown as object);
        }
      }
      if (recoveredRef === undefined || !isValidNativeRef(recoveredRef)) {
        BRIDGE_NATIVE.delete(node);
        return { status: "no_root_ref" };
      }
      installHint(node, generation, recoveredRef);
      try {
        const retryStatus = hostRenderRef(session.symbols, session.runtime, hostPointer, recoveredRef);
        if (retryStatus === HOST_STATUS_OK) {
          counters.host_mutations += 1;
          return { status: "ok", rootRef: recoveredRef, recovered: true };
        }
        BRIDGE_NATIVE.delete(node);
        return { status: "no_root_ref" };
      } finally {
        // viewRefForNodeId/§73 recovery returns one temporary lease. The
        // boundary's existing root lease remains the durable owner.
        const batch = Uint32Array.of(recoveredRef);
        viewReleaseMany(session.symbols, session.runtime, batch, 1);
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
/**
 * PERF-12 T13.1 R8 — injectable COLD materializer used by
 * {@link RetainedRootBoundary.prepareColdInstall}: decodes a whole View tree
 * into a leased native root reference WITHOUT painting. Bootstrap wires this
 * to the Direct N-API decode (`tryNativeMaterialize`) — kept behind a hook
 * because retained_dag must not import native_view_abi (cycle).
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
  /** Publishes the prepared root. Infallible after successful preparation. */
  commit(): void;
  /** Discards the prepared root; no visible mutation ever occurred. */
  abort(): void;
}

interface PreparedRootInstall {
  readonly node: BridgeViewNode;
  readonly rootRef: number;
  readonly tx: MaterializeTx;
  readonly ownsTempLease: boolean;
  /** True when prepare acquired a fresh boundary lease by NodeId promotion. */
  acquiredBoundaryLease: boolean;
}

export class RetainedRootBoundary {
  private previousRef: number | undefined;
  private closed = false;
  /** NodeId allocator high-water at the last successful commit (§18). */
  nativeLookupCeiling = 0;

  constructor(
    private readonly session: NativeViewAbiSession,
    private readonly hostPointer: () => Pointer | undefined,
    /**
     * PERF-12 T13 (§80): optional alternative commit step for boundaries whose
     * mutation is a direct ref install (`ViewSlot.setViewRef`,
     * `ScrollPane.setContentRef`) rather than a host scene render. Returning
     * false means the boundary change failed; the old root stays installed.
     */
    private readonly installRef?: (rootRef: number) => boolean,
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
   * Everything fallible in `install`, stopping BEFORE the publish call:
   * retained walk, materialization, lease acquisition/recovery. All failure
   * paths drain temporary leases before returning.
   */
  private prepareFrom(
    view: View,
  ): {
    node: BridgeViewNode;
    rootRef: number;
    tx: MaterializeTx;
    ownsTempLease: boolean;
    acquiredBoundaryLease: boolean;
  } | undefined {
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
    let ownsTempLease = tx.temporaryLeases.includes(resolvedRef);
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
        if (!isExpectedNativeStatus(error)) {
          tx.releaseAll();
          throw error;
        }
        counters.node_id_ref_promotion_misses += 1;
        const recovered = recoverStaleNode(nodeForBridge(view), tx);
        if (recovered === undefined) {
          tx.releaseAll();
          return undefined;
        }
        rootRef = recovered;
        ownsTempLease = true;
      }
    }
    return { node: nodeForBridge(view), rootRef, tx, ownsTempLease, acquiredBoundaryLease };
  }

  /**
   * PERF-12 T13.1 R8: COLD transactional publication. Decodes the whole tree
   * via the injected cold materializer (Direct decode, NO painting) and
   * returns a publication whose commit paints the prepared ref once
   * (hostRenderRef) and transfers its lease into the boundary. Used when the
   * retained path refuses — guarantees "cold fallback never paints during
   * PREPARE" (handoff §32.2.3 hard rule).
   */
  prepareColdInstall(view: View): RootPublication | undefined {
    if (this.closed) throw new Error("boundary is closed");
    const materialize = COLD_ROOT_MATERIALIZER;
    if (materialize === undefined) return undefined;
    const rootRef = materialize(view);
    if (rootRef === undefined) return undefined;
    let finished = false;
    return {
      rootRef,
      commit: (): void => {
        if (finished) throw new Error("root publication already finished");
        finished = true;
        const hostPointer = this.hostPointer();
        if (hostPointer === undefined) {
          this.releaseColdLease(rootRef);
          throw new Error("TUI_ROOT_COLD_PUBLISH_REFUSED: no host pointer");
        }
        const status = hostRenderRef(this.session.symbols, this.session.runtime, hostPointer, rootRef);
        if (status !== HOST_STATUS_OK) {
          this.releaseColdLease(rootRef);
          throw new Error(`TUI_ROOT_COLD_PUBLISH_REFUSED: status ${status}`);
        }
        counters.host_mutations += 1;
        // Transfer our lease on the new root into the boundary; the previous
        // root's lease is released here exactly like transferRoot does.
        this.transferRoot(rootRef);
      },
      abort: (): void => {
        if (finished) return;
        finished = true;
        this.releaseColdLease(rootRef);
      },
    };
  }

  private releaseColdLease(rootRef: number): void {
    viewReleaseMany(this.session.symbols, this.session.runtime, Uint32Array.of(rootRef), 1);
  }

  /**
   * The publish + bookkeeping tail of `install`. Returns false only on a
   * publish refusal from the host callback (which leaves the old root
   * installed); unwinds all acquired leases in that case.
   */
  private publishPrepared(prepared: {
    node: BridgeViewNode;
    rootRef: number;
    tx: MaterializeTx;
    ownsTempLease: boolean;
    acquiredBoundaryLease: boolean;
  }): boolean {
    const rootRef = prepared.rootRef;
    try {
      if (this.installRef !== undefined) {
        if (!this.installRef(rootRef)) {
          this.unwindPrepared(prepared);
          return false;
        }
      } else {
        const hostPointer = this.hostPointer();
        if (hostPointer !== undefined) {
          const status = hostRenderRef(this.session.symbols, this.session.runtime, hostPointer, rootRef);
          if (status !== HOST_STATUS_OK) {
            // Release only a freshly acquired boundary lease whose ref is not
            // already the boundary's leased previous root (§18 failure keeps
            // the old root installed).
            this.unwindPrepared(prepared);
            return false;
          }
        }
        counters.host_mutations += 1;
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
    if (prepared.acquiredBoundaryLease && prepared.rootRef !== this.previousRef) {
      const batch = Uint32Array.of(prepared.rootRef);
      viewReleaseMany(this.session.symbols, this.session.runtime, batch, 1);
      prepared.acquiredBoundaryLease = false;
    }
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
