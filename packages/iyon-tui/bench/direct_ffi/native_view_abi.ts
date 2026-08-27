import type { Pointer } from "bun:ffi";
import { native, type NativeViewAbiBootstrap } from "./native.ts";
import { ensureNative, MaterializeTx, RetainedCycleError, RetainedFastFallbackError } from "./retained_dag.ts";
import { linkViewAbi, type NativeAbiPointers } from "./view_abi.ts";
import {
  hostRenderRef,
  pathChild,
  pathRoot,
  viewRowCreate0,
  viewRowCreate1,
  viewRowCreate2,
  viewRowCreate3,
  viewRowCreate4,
  viewColumnCreate0,
  viewColumnCreate1,
  viewColumnCreate2,
  viewColumnCreate3,
  viewColumnCreate4,
  axisBuilderBegin,
  axisBuilderPush,
  axisBuilderFinish,
  axisBuilderAbort,
  viewAxisSetChild,
  viewAxisSpliceBuffer,
  viewGridSetCell,
  viewRefForNodeId,
  viewReleaseMany,
  editTxnBegin,
  editTxnAddTextLayout,
  editTxnCommitRender,
  editTxnAbort,
  type ViewAbiSymbols,
} from "./view_calls.ts";
import { nodeForBridge } from "../../src/view-internals.ts";
import {
  nodeIdPair,
  viewNodeId,
  type NativePathLineage,
  type View,
} from "../../src/values/view.ts";
import manifest from "../../src/generated/view_abi_manifest.json";
import {
  NATIVE_BUILDER_MAX_CHILDREN,
  NATIVE_SMALL_AXIS_ARITY_MAX,
} from "../../src/native_view_policy.ts";

export interface NativeViewAbiSession {
  readonly runtime: Pointer;
  readonly symbols: ViewAbiSymbols;
  readonly abi: NativeViewAbiBootstrap;
}

export interface NativeViewRenderHost {
  readonly tuiViewAbiHostPointer?: () => number;
  /** Native-ref installation for generic View-bearing controls. */
  readonly tuiViewAbiInstallRef?: (viewRef: number) => void;
}

export type NativeViewRoute =
  | "no_op"
  | "render_ref"
  | "retained"
  | "fallback";

export type NativeViewRouteSnapshot = Readonly<Record<NativeViewRoute, number>>;

const ROUTE_NAMES: readonly NativeViewRoute[] = [
  "no_op",
  "render_ref",
  "retained",
  "fallback",
];
const routeCounts: Record<NativeViewRoute, number> = Object.fromEntries(
  ROUTE_NAMES.map((name) => [name, 0]),
) as Record<NativeViewRoute, number>;

/** Benchmark-only route counters; disabled unless explicitly requested. */
export function resetNativeViewRouteCounters(): void {
  for (const name of ROUTE_NAMES) routeCounts[name] = 0;
}

export function nativeViewRouteSnapshot(): NativeViewRouteSnapshot {
  return Object.freeze({ ...routeCounts });
}

export function recordNativeViewRoute(route: NativeViewRoute): void {
  if (Bun.env.PERF_NATIVE_VIEW_STATS !== "1") return;
  routeCounts[route] += 1;
}

/**
 * One typed transaction edit. NodeIds are ordered from changed leaf toward the
 * new root, matching the fixed lanes in the generated ABI.
 */
export interface NativeTextLayoutTransactionEdit {
  readonly lineage: NativePathLineage;
  /** Construction-time scalar NodeIds, ordered leaf toward root. */
  readonly nodeIds: readonly number[];
  readonly wrap: number;
  readonly align: number;
}

export interface NativeAxisSpliceChild {
  readonly view: View;
  /** `(track kind in low byte, value in the high 16 bits)`; zero means content. */
  readonly trackWord?: number;
}

export interface NativeAxisBuilderChild {
  readonly view: View;
  /** `(track kind in low byte, value in the high 16 bits)`; zero means content. */
  readonly trackWord?: number;
}

let cachedSession: NativeViewAbiSession | undefined;
const PATH_REFS = new WeakMap<NativeViewAbiSession, WeakMap<object, number>>();
const PATH_SHAPE_REFS = new WeakMap<NativeViewAbiSession, Map<string, number>>();
const SINGLE_REF_RELEASE = new Uint32Array(1);
const ABI_FUNCTION_NAMES = [
  "runtimeNoop",
  "viewStatusDetail",
  "viewRenderRef",
  "hostRenderRef",
  "viewSpacerCreate",
  "viewTextLayoutPatchRoot",
  "viewCommonPatchRoot",
  "viewAxisCreateBuffer",
  "viewRowCreate0",
  "viewRowCreate1",
  "viewRowCreate2",
  "viewRowCreate3",
  "viewRowCreate4",
  "viewColumnCreate0",
  "viewColumnCreate1",
  "viewColumnCreate2",
  "viewColumnCreate3",
  "viewColumnCreate4",
  "axisBuilderBegin",
  "axisBuilderPush",
  "axisBuilderFinish",
  "axisBuilderAbort",
  "viewAxisSetChild",
  "viewAxisSpliceBuffer",
  "viewGridSetCell",
  "viewAxisSetChildPath",
  "viewGridSetCellPath",
  "viewGridCreateBuffer",
  "viewDiffCreateBuffer",
  "viewHangingCreate",
  "viewContainerCreate",
  "viewClampCreate",
  "viewComponentCreate",
  "viewDecoratedCreateBuffer",
  "viewReleaseMany",
  "viewRefForNodeId",
  "pathRoot",
  "pathChild",
  "viewTextLayoutPatchPath",
  "viewTextLayoutPatchPathD1",
  "viewTextLayoutPatchPathD2",
  "viewTextLayoutPatchPathD3",
  "viewTextLayoutPatchPathD4",
  "editTxnBegin",
  "editTxnAddTextLayout",
  "editTxnCommitRender",
  "editTxnAbort",
  "styleAtomCreateCstring",
  "styleCreateBits",
  "viewTextCreateCstring",
  "viewTextCreateUtf8",
  "viewTextCreateUtf82",
  "viewTextCreateUtf83",
  "viewTextCreateUtf84",
  "viewTextCreateCstring2",
  "viewTextCreateCstring3",
  "viewTextCreateCstring4",
] as const;

function isValidPointer(value: unknown): value is Pointer {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

/**
 * Links the generated first-slice ABI once for this Bun environment.
 * The native runtime pointer is environment-owned and remains stable until
 * N-API cleanup; callers must not retain the session after addon teardown.
 */
export function nativeViewAbiSession(): NativeViewAbiSession | undefined {
  if (cachedSession !== undefined) return cachedSession;
  const bootstrap = native.tuiViewAbiBootstrap?.();
  if (bootstrap === undefined) return undefined;
  const functionsValue = bootstrap.functions as unknown;
  if (functionsValue === null || typeof functionsValue !== "object") {
    throw new Error("native View ABI bootstrap metadata is incompatible");
  }
  const functions = functionsValue as Record<string, unknown>;
  const functionNames = Object.keys(functions);
  if (
    bootstrap.abi_name !== "iyon_tui_view"
    || bootstrap.abi_version !== 1
    || bootstrap.semantic_version !== 1
    || bootstrap.schema_blake3 !== manifest.schema_blake3
    || bootstrap.generator_blake3 !== manifest.generator_blake3
    || !Number.isSafeInteger(bootstrap.generation)
    || bootstrap.generation < 1
    || bootstrap.function_count !== manifest.functions.length
    || functionNames.length !== ABI_FUNCTION_NAMES.length
    || ABI_FUNCTION_NAMES.some((name) => !functionNames.includes(name) || !isValidPointer(functions[name]))
    || !isValidPointer(bootstrap.runtime_ptr)
  ) {
    throw new Error("native View ABI bootstrap metadata is incompatible");
  }
  const pointers: NativeAbiPointers = {
    runtimeNoop: bootstrap.functions.runtimeNoop as Pointer,
    viewStatusDetail: bootstrap.functions.viewStatusDetail as Pointer,
    viewRenderRef: bootstrap.functions.viewRenderRef as Pointer,
    hostRenderRef: bootstrap.functions.hostRenderRef as Pointer,
    viewSpacerCreate: bootstrap.functions.viewSpacerCreate as Pointer,
    viewTextLayoutPatchRoot: bootstrap.functions.viewTextLayoutPatchRoot as Pointer,
    viewCommonPatchRoot: bootstrap.functions.viewCommonPatchRoot as Pointer,
    viewAxisCreateBuffer: bootstrap.functions.viewAxisCreateBuffer as Pointer,
    viewGridCreateBuffer: bootstrap.functions.viewGridCreateBuffer as Pointer,
    viewDiffCreateBuffer: bootstrap.functions.viewDiffCreateBuffer as Pointer,
    viewHangingCreate: bootstrap.functions.viewHangingCreate as Pointer,
    viewContainerCreate: bootstrap.functions.viewContainerCreate as Pointer,
    viewClampCreate: bootstrap.functions.viewClampCreate as Pointer,
    viewComponentCreate: bootstrap.functions.viewComponentCreate as Pointer,
    viewDecoratedCreateBuffer: bootstrap.functions.viewDecoratedCreateBuffer as Pointer,
    viewRowCreate0: bootstrap.functions.viewRowCreate0 as Pointer,
    viewRowCreate1: bootstrap.functions.viewRowCreate1 as Pointer,
    viewRowCreate2: bootstrap.functions.viewRowCreate2 as Pointer,
    viewRowCreate3: bootstrap.functions.viewRowCreate3 as Pointer,
    viewRowCreate4: bootstrap.functions.viewRowCreate4 as Pointer,
    viewColumnCreate0: bootstrap.functions.viewColumnCreate0 as Pointer,
    viewColumnCreate1: bootstrap.functions.viewColumnCreate1 as Pointer,
    viewColumnCreate2: bootstrap.functions.viewColumnCreate2 as Pointer,
    viewColumnCreate3: bootstrap.functions.viewColumnCreate3 as Pointer,
    viewColumnCreate4: bootstrap.functions.viewColumnCreate4 as Pointer,
    axisBuilderBegin: bootstrap.functions.axisBuilderBegin as Pointer,
    axisBuilderPush: bootstrap.functions.axisBuilderPush as Pointer,
    axisBuilderFinish: bootstrap.functions.axisBuilderFinish as Pointer,
    axisBuilderAbort: bootstrap.functions.axisBuilderAbort as Pointer,
    viewAxisSetChild: bootstrap.functions.viewAxisSetChild as Pointer,
    viewAxisSpliceBuffer: bootstrap.functions.viewAxisSpliceBuffer as Pointer,
    viewGridSetCell: bootstrap.functions.viewGridSetCell as Pointer,
    viewAxisSetChildPath: bootstrap.functions.viewAxisSetChildPath as Pointer,
    viewGridSetCellPath: bootstrap.functions.viewGridSetCellPath as Pointer,
    viewReleaseMany: bootstrap.functions.viewReleaseMany as Pointer,
    viewRefForNodeId: bootstrap.functions.viewRefForNodeId as Pointer,
    pathRoot: bootstrap.functions.pathRoot as Pointer,
    pathChild: bootstrap.functions.pathChild as Pointer,
    viewTextLayoutPatchPath: bootstrap.functions.viewTextLayoutPatchPath as Pointer,
    viewTextLayoutPatchPathD1: bootstrap.functions.viewTextLayoutPatchPathD1 as Pointer,
    viewTextLayoutPatchPathD2: bootstrap.functions.viewTextLayoutPatchPathD2 as Pointer,
    viewTextLayoutPatchPathD3: bootstrap.functions.viewTextLayoutPatchPathD3 as Pointer,
    viewTextLayoutPatchPathD4: bootstrap.functions.viewTextLayoutPatchPathD4 as Pointer,
    editTxnBegin: bootstrap.functions.editTxnBegin as Pointer,
    editTxnAddTextLayout: bootstrap.functions.editTxnAddTextLayout as Pointer,
    editTxnCommitRender: bootstrap.functions.editTxnCommitRender as Pointer,
    editTxnAbort: bootstrap.functions.editTxnAbort as Pointer,
    styleAtomCreateCstring: bootstrap.functions.styleAtomCreateCstring as Pointer,
    styleCreateBits: bootstrap.functions.styleCreateBits as Pointer,
    viewTextCreateCstring: bootstrap.functions.viewTextCreateCstring as Pointer,
    viewTextCreateUtf8: bootstrap.functions.viewTextCreateUtf8 as Pointer,
    viewTextCreateUtf82: bootstrap.functions.viewTextCreateUtf82 as Pointer,
    viewTextCreateUtf83: bootstrap.functions.viewTextCreateUtf83 as Pointer,
    viewTextCreateUtf84: bootstrap.functions.viewTextCreateUtf84 as Pointer,
    viewTextCreateCstring2: bootstrap.functions.viewTextCreateCstring2 as Pointer,
    viewTextCreateCstring3: bootstrap.functions.viewTextCreateCstring3 as Pointer,
    viewTextCreateCstring4: bootstrap.functions.viewTextCreateCstring4 as Pointer,
  };
  const linked = linkViewAbi(pointers);
  const runtime = bootstrap.runtime_ptr as Pointer;
  if (linked.symbols.runtimeNoop(runtime) !== 1) {
    throw new Error("native View ABI bootstrap probe failed");
  }
  cachedSession = {
    runtime,
    symbols: linked.symbols,
    abi: bootstrap,
  };
  return cachedSession;
}

/**
 * Obtains the environment-local NativeRef after an authoritative direct
 * decode has installed the corresponding View. The caller owns the resulting
 * root lease until it replaces or closes that root.
 */
export function nativeViewRefForNodeId(view: View): number | undefined {
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const [nodeIdLow, nodeIdHigh] = nodeIdPair(view);
  return viewRefForNodeId(session.symbols, session.runtime, nodeIdLow, nodeIdHigh);
}

/**
 * PERF-12 T13 (§78/§80): retained materialization for boundaries that keep a
 * View only transiently (History unit import, animation frames) or that run
 * their own §18 boundary. Identity-first: BridgeNativeHint hits reuse already-
 * materialized nodes with zero payload reads; ceiling is 0 so genuinely new
 * nodes skip the NodeId probe exactly like the §19 ordering requires — cold
 * unit imports pay no extra per-node round trips, while anything previously
 * materialized through any boundary rides its hint.
 *
 * Returns the root ref carrying exactly ONE live lease (every other temporary
 * lease drains in one batch), or undefined when the retained path refused
 * (cap/shape/fallback); the caller then routes its complete fallback.
 */
export function tryRetainedMaterializeRef(next: View): number | undefined {
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const tx = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, 0);
  try {
    const reference = ensureNative(nodeForBridge(next), tx);
    tx.releaseAllExcept(reference);
    return reference;
  } catch (error) {
    tx.releaseAll();
    if (error instanceof RetainedFastFallbackError || error instanceof RetainedCycleError) return undefined;
    throw error;
  }
}

/**
 * Obtains one leased native root without painting. Existing NodeId entries are
 * promoted first; a missing entry uses the native direct decoder, matching
 * the safe N-API retained fallback semantics.
 */
export function tryNativeMaterialize(next: View): number | undefined {
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const node = nodeForBridge(next);
  try {
    const [low, high] = nodeIdPair(next);
    return viewRefForNodeId(session.symbols, session.runtime, low, high);
  } catch (error) {
    if (!isExpectedNativeStatus(error)) throw error;
  }
  const decodeRef = native.tuiViewAbiDecodeRef;
  if (decodeRef === undefined) return undefined;
  const reference = decodeRef(node as unknown as object);
  return Number.isSafeInteger(reference) && reference > 0 && reference < 0x8000_0000
    ? reference
    : undefined;
}

/**
 * Materializes a small/new axis without a JS child packet. The children must
 * already have NativeRefs.
 */
export function tryNativeAxisCreate(
  next: View,
  horizontal: boolean,
  gap: number,
  children: readonly NativeAxisBuilderChild[],
): number | undefined {
  if (
    !Number.isInteger(gap)
    || gap < 0
    || gap > 65_535
    || children.length > NATIVE_BUILDER_MAX_CHILDREN
  ) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const refs: number[] = [];
  try {
    for (const child of children) {
      const reference = nativeViewRefForNodeId(child.view);
      if (reference === undefined) return undefined;
      refs.push(reference);
    }
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
    const axisKind = horizontal ? 1 : 2;
    return children.length <= NATIVE_SMALL_AXIS_ARITY_MAX
      ? createSmallAxis(session, horizontal, nodeIdLow, nodeIdHigh, gap, children, refs)
      : createAxisWithBuilder(session, axisKind, nodeIdLow, nodeIdHigh, gap, children, refs);
  } catch (error) {
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  } finally {
    for (const reference of refs) releaseNativeViewRef(session, reference);
  }
}

/** Installs a newly-created native axis with one host mutation. */
export function tryNativeAxisCreateRender(
  host: NativeViewRenderHost,
  next: View,
  horizontal: boolean,
  gap: number,
  children: readonly NativeAxisBuilderChild[],
): number | undefined {
  if (!canInstallNativeRef(host)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const nextRef = tryNativeAxisCreate(next, horizontal, gap, children);
  if (nextRef === undefined) return undefined;
  try {
    const status = installNativeRef(host, session, nextRef);
    if (status !== 0) {
      releaseNativeViewRef(session, nextRef);
      return undefined;
    }
    return nextRef;
  } catch (error) {
    releaseNativeViewRef(session, nextRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

function createSmallAxis(
  session: NativeViewAbiSession,
  horizontal: boolean,
  nodeIdLow: number,
  nodeIdHigh: number,
  gap: number,
  children: readonly NativeAxisBuilderChild[],
  refs: readonly number[],
): number {
  const track = (index: number): number => children[index]?.trackWord ?? 0;
  const ref = (index: number): number => refs[index]!;
  if (horizontal) {
    switch (children.length) {
      case 0: return viewRowCreate0(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap);
      case 1: return viewRowCreate1(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0));
      case 2: return viewRowCreate2(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0), track(1), ref(1));
      case 3: return viewRowCreate3(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0), track(1), ref(1), track(2), ref(2));
      case 4: return viewRowCreate4(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0), track(1), ref(1), track(2), ref(2), track(3), ref(3));
      default: throw new RangeError("small native axis arity is unsupported");
    }
  }
  switch (children.length) {
    case 0: return viewColumnCreate0(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap);
    case 1: return viewColumnCreate1(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0));
    case 2: return viewColumnCreate2(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0), track(1), ref(1));
    case 3: return viewColumnCreate3(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0), track(1), ref(1), track(2), ref(2));
    case 4: return viewColumnCreate4(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, gap, track(0), ref(0), track(1), ref(1), track(2), ref(2), track(3), ref(3));
    default: throw new RangeError("small native axis arity is unsupported");
  }
}

function createAxisWithBuilder(
  session: NativeViewAbiSession,
  axisKind: number,
  nodeIdLow: number,
  nodeIdHigh: number,
  gap: number,
  children: readonly NativeAxisBuilderChild[],
  refs: readonly number[],
): number | undefined {
  let builderRef: number | undefined;
  try {
    builderRef = axisBuilderBegin(session.symbols, session.runtime, axisKind, children.length);
    for (const [index, child] of children.entries()) {
      if (axisBuilderPush(session.symbols, session.runtime, builderRef, child.trackWord ?? 0, refs[index]!) !== 0) {
        axisBuilderAbort(session.symbols, session.runtime, builderRef);
        builderRef = undefined;
        return undefined;
      }
    }
    const result = axisBuilderFinish(session.symbols, session.runtime, builderRef, nodeIdLow, nodeIdHigh, gap);
    builderRef = undefined;
    return result;
  } catch (error) {
    if (builderRef !== undefined) axisBuilderAbort(session.symbols, session.runtime, builderRef);
    throw error;
  }
}

/**
 * Replaces one wide axis child through native PersistentSeq::set. The JS
 * operation supplies only the child NativeRef and the new root NodeId; it
 * does not encode the surrounding axis or descendants.
 */
export function tryNativeAxisSetChildRender(
  host: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  next: View,
  child: View,
  childIndex: number,
  trackWord = 0,
): number | undefined {
  if (!Number.isInteger(childIndex) || childIndex < 0 || !isValidNativeRef(previousRef)) return undefined;
  const session = nativeViewAbiSession();
  if (!canInstallNativeRef(host) || session === undefined) return undefined;
  let childRef: number | undefined;
  let nextRef: number | undefined;
  try {
    childRef = tryNativeMaterialize(child);
    if (childRef === undefined) return undefined;
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
    nextRef = viewAxisSetChild(session.symbols, session.runtime, previousRef, nodeIdLow, nodeIdHigh, childIndex, trackWord, childRef);
    const status = installNativeRef(host, session, nextRef);
    if (status !== 0) {
      releaseNativeViewRef(session, nextRef);
      releaseNativeViewRef(session, childRef);
      return undefined;
    }
    releaseNativeViewRef(session, childRef);
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (childRef !== undefined) releaseNativeViewRef(session, childRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/**
 * Inserts/removes a bounded set of axis children through a POD buffer. The
 * buffer contains only `(track_word, child_ref)` pairs; no View payload or
 * generic operation record is serialized.
 */
export function tryNativeAxisSpliceRender(
  host: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  next: View,
  index: number,
  removeCount: number,
  children: readonly NativeAxisSpliceChild[],
): number | undefined {
  if (!Number.isInteger(index) || index < 0 || !Number.isInteger(removeCount) || removeCount < 0 || !isValidNativeRef(previousRef)) return undefined;
  const session = nativeViewAbiSession();
  if (!canInstallNativeRef(host) || session === undefined) return undefined;
  // Bun's checked buffer lowering still requires a non-null aligned pointer
  // when the used count is zero; the spare pair is ignored by native code.
  const refs = new Uint32Array(Math.max(children.length * 2, 2));
  // Keep one release entry per acquired lease. The same View may appear more
  // than once in a splice, so a Set would under-release duplicate refs.
  const temporaryRefs: number[] = [];
  let nextRef: number | undefined;
  try {
    for (const [childIndex, entry] of children.entries()) {
      const childRef = tryNativeMaterialize(entry.view);
      if (childRef === undefined) return undefined;
      refs[childIndex * 2] = entry.trackWord ?? 0;
      refs[childIndex * 2 + 1] = childRef;
      temporaryRefs.push(childRef);
    }
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
    nextRef = viewAxisSpliceBuffer(session.symbols, session.runtime, previousRef, nodeIdLow, nodeIdHigh, index, removeCount, refs, children.length);
    const status = installNativeRef(host, session, nextRef);
    if (status !== 0) {
      releaseNativeViewRef(session, nextRef);
      nextRef = undefined;
      return undefined;
    }
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  } finally {
    for (const childRef of temporaryRefs) releaseNativeViewRef(session, childRef);
  }
}

/** Replaces one grid cell using the persistent cell sequence index. */
export function tryNativeGridSetCellRender(
  host: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  next: View,
  row: number,
  column: number,
  child: View,
): number | undefined {
  if (!Number.isInteger(row) || row < 0 || !Number.isInteger(column) || column < 0 || !isValidNativeRef(previousRef)) return undefined;
  const session = nativeViewAbiSession();
  if (!canInstallNativeRef(host) || session === undefined) return undefined;
  let childRef: number | undefined;
  let nextRef: number | undefined;
  try {
    childRef = tryNativeMaterialize(child);
    if (childRef === undefined) return undefined;
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
    nextRef = viewGridSetCell(session.symbols, session.runtime, previousRef, nodeIdLow, nodeIdHigh, row, column, childRef);
    const status = installNativeRef(host, session, nextRef);
    if (status !== 0) {
      releaseNativeViewRef(session, nextRef);
      releaseNativeViewRef(session, childRef);
      return undefined;
    }
    releaseNativeViewRef(session, childRef);
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (childRef !== undefined) releaseNativeViewRef(session, childRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/**
 * Stages multiple typed text-layout edits and atomically installs their shared
 * changed-path-trie result. Construction-time transaction metadata supplies
 * scalar NodeIds.
 */
export function tryNativeEditTransactionRender(
  host: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  edits: readonly NativeTextLayoutTransactionEdit[],
): number | undefined {
  if (edits.length === 0 || edits.length > 256 || !isValidNativeRef(previousRef)) return undefined;
  const hostPointer = host.tuiViewAbiHostPointer?.();
  if (!isValidPointer(hostPointer)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  let txnRef: number | undefined;
  try {
    txnRef = editTxnBegin(session.symbols, session.runtime, previousRef, edits.length);
    for (const edit of edits) {
      const depth = edit.lineage.depth;
      const pairs = transactionNodeIdPairs(edit, depth);
      if (
        depth < 0
        || depth > 4
        || pairs === undefined
        || edit.lineage.baseNodeId !== viewNodeId(previous)
      ) {
        editTxnAbort(session.symbols, session.runtime, txnRef);
        return undefined;
      }
      const pathRef = nativePathRefForLineage(session, edit.lineage);
      const target = pairs[0]!;
      const [targetLow, targetHigh] = target;
      // Generated checked wrappers validate every fixed NodeId lane. Unused
      // lanes carry the valid root identity and are ignored by native staging.
      const fallbackId = pairs[depth]!;
      const [ancestor0Low, ancestor0High] = pairs[1] ?? fallbackId;
      const [ancestor1Low, ancestor1High] = pairs[2] ?? fallbackId;
      const [ancestor2Low, ancestor2High] = pairs[3] ?? fallbackId;
      const [ancestor3Low, ancestor3High] = pairs[4] ?? fallbackId;
      const status = editTxnAddTextLayout(
        session.symbols,
        session.runtime,
        txnRef,
        pathRef,
        depth,
        targetLow,
        targetHigh,
        ancestor0Low,
        ancestor0High,
        ancestor1Low,
        ancestor1High,
        ancestor2Low,
        ancestor2High,
        ancestor3Low,
        ancestor3High,
        edit.wrap,
        edit.align,
      );
      if (status !== 0) {
        editTxnAbort(session.symbols, session.runtime, txnRef);
        return undefined;
      }
    }
    const result = editTxnCommitRender(session.symbols, session.runtime, hostPointer as Pointer, txnRef);
    txnRef = undefined;
    return result;
  } catch (error) {
    if (txnRef !== undefined) editTxnAbort(session.symbols, session.runtime, txnRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/** Interns one immutable JS path lineage into the environment PathStore. */
export function nativePathRefForLineage(
  session: NativeViewAbiSession,
  lineage: NativePathLineage,
): number {
  let refs = PATH_REFS.get(session);
  if (refs === undefined) {
    refs = new WeakMap<object, number>();
    PATH_REFS.set(session, refs);
  }
  const cached = refs.get(lineage);
  if (cached !== undefined) return cached;
  let shapes = PATH_SHAPE_REFS.get(session);
  if (shapes === undefined) {
    shapes = new Map<string, number>();
    PATH_SHAPE_REFS.set(session, shapes);
  }
  const shape = pathShape(lineage);
  const shaped = shapes.get(shape);
  if (shaped !== undefined) {
    refs.set(lineage, shaped);
    return shaped;
  }
  const parent = lineage.parent === undefined
    ? pathRoot(session.symbols, session.runtime)
    : nativePathRefForLineage(session, lineage.parent);
  const reference = lineage.step === undefined
    ? parent
    : pathChild(
      session.symbols,
      session.runtime,
      parent,
      lineage.step.kind,
      lineage.step.expectedViewKind,
      lineage.step.selector,
    );
  shapes.set(shape, reference);
  refs.set(lineage, reference);
  return reference;
}

function pathShape(lineage: NativePathLineage): string {
  const steps = pathLineageSteps(lineage);
  return steps.length === 0 ? "root" : steps.map((step) => `${step?.kind}:${step?.expectedViewKind}:${step?.selector}`).join("/");
}

function pathLineageSteps(lineage: NativePathLineage): NativePathLineage["step"][] {
  const steps: NativePathLineage["step"][] = [];
  let current: NativePathLineage | undefined = lineage;
  while (current?.step !== undefined) {
    steps.push(current.step);
    current = current.parent;
  }
  steps.reverse();
  return steps;
}

function splitNodeIdSafely(id: number): readonly [number, number] | undefined {
  return Number.isSafeInteger(id) && id > 0 && id <= Number.MAX_SAFE_INTEGER
    ? [id >>> 0, Math.floor(id / 0x1_0000_0000)]
    : undefined;
}

function transactionNodeIdPairs(
  edit: NativeTextLayoutTransactionEdit,
  depth: number,
): readonly (readonly [number, number])[] | undefined {
  if (depth < 0 || depth > 4) return undefined;
  const ids = edit.nodeIds;
  if (ids.length !== depth + 1) return undefined;
  const pairs = ids.map(splitNodeIdSafely);
  return pairs.every((pair): pair is readonly [number, number] => pair !== undefined)
    ? pairs
    : undefined;
}

function canInstallNativeRef(target: NativeViewRenderHost): boolean {
  const hostPointer = target.tuiViewAbiHostPointer?.();
  return isValidPointer(hostPointer) || target.tuiViewAbiInstallRef !== undefined;
}

function installNativeRef(
  target: NativeViewRenderHost,
  session: NativeViewAbiSession,
  viewRef: number,
): number {
  const hostPointer = target.tuiViewAbiHostPointer?.();
  if (isValidPointer(hostPointer)) {
    return hostRenderRef(session.symbols, session.runtime, hostPointer as Pointer, viewRef);
  }
  const install = target.tuiViewAbiInstallRef;
  if (install === undefined) return -1;
  install.call(target, viewRef);
  return 0;
}

export function releaseNativeViewRef(session: NativeViewAbiSession | undefined, ref: number): void {
  if (session === undefined || !isValidNativeRef(ref)) return;
  SINGLE_REF_RELEASE[0] = ref;
  viewReleaseMany(session.symbols, session.runtime, SINGLE_REF_RELEASE, 1);
}

function isValidNativeRef(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0 && value < 0x8000_0000;
}

function isExpectedNativeStatus(error: unknown): boolean {
  return error instanceof Error && /^native ABI status 0x[0-9a-f]+$/u.test(error.message);
}
