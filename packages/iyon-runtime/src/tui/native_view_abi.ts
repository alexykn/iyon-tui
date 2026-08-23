import type { Pointer } from "bun:ffi";
import { native, type NativeViewAbiBootstrap } from "../native.ts";
import { linkViewAbi, type NativeAbiPointers } from "./generated/view_abi.ts";
import {
  hostRenderRef,
  viewRenderRef,
  pathChild,
  pathRoot,
  viewCommonPatchRoot,
  viewSpacerCreate,
  viewAxisCreateBuffer,
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
  viewAxisSetChildPath,
  viewGridSetCellPath,
  viewRefForNodeId,
  viewReleaseMany,
  viewTextLayoutPatchPathD1,
  viewTextLayoutPatchPathD2,
  viewTextLayoutPatchPathD3,
  viewTextLayoutPatchPathD4,
  viewTextLayoutPatchRoot,
  editTxnBegin,
  editTxnAddTextLayout,
  editTxnCommitRender,
  editTxnAbort,
  styleAtomCreateCstring,
  styleCreateBits,
  viewTextCreateCstring,
  viewTextCreateUtf8,
  viewTextCreateUtf82,
  viewTextCreateUtf83,
  viewTextCreateUtf84,
  viewTextCreateCstring2,
  viewTextCreateCstring3,
  viewTextCreateCstring4,
  type ViewAbiSymbols,
} from "./generated/view_calls.ts";
import {
  BRIDGE_VIEW_KIND,
  type BridgeViewNode,
  type StyleNode,
  type ColorNode,
} from "./ir.ts";
import {
  nativePathLineage,
  nativeScalarPatch,
  nativeAxisRecipe,
  nativeSpacerRecipe,
  nativeTextRecipe,
  nativeStructuralEdit,
  nativeTextLayoutTransaction,
  nodeForBridge,
  nodeIdPair,
  viewNodeId,
  type NativePathLineage,
  type View,
} from "./values/view.ts";
import manifest from "./generated/view_abi_manifest.json";
import {
  NATIVE_BUILDER_MAX_CHILDREN,
  NATIVE_COLD_MAX_DEPTH,
  NATIVE_COLD_MAX_NODES,
  NATIVE_SMALL_AXIS_ARITY_MAX,
  NATIVE_TEXT_MAX_BYTES,
} from "./native_view_policy.ts";

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
  | "scalar"
  | "shallow_depth"
  | "path_ref"
  | "structural"
  | "edit_transaction"
  | "native_builder"
  | "fallback"
  | "recovery";

export type NativeViewRouteSnapshot = Readonly<Record<NativeViewRoute, number>>;

const ROUTE_NAMES: readonly NativeViewRoute[] = [
  "no_op",
  "render_ref",
  "scalar",
  "shallow_depth",
  "path_ref",
  "structural",
  "edit_transaction",
  "native_builder",
  "fallback",
  "recovery",
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
 * One typed transaction edit. `views` is ordered from changed leaf toward the
 * new root, matching the fixed NodeId lanes in the generated ABI. It contains
 * semantic identities only; no bridge nodes or transport arrays are retained.
 */
export interface NativeTextLayoutTransactionEdit {
  readonly lineage: NativePathLineage;
  /** Construction-time scalar NodeIds, ordered leaf toward root. */
  readonly nodeIds?: readonly number[];
  /** Legacy/test helper input; production metadata uses `nodeIds`. */
  readonly views?: readonly View[];
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
const TEXT_ENCODER = new TextEncoder();
const STYLE_REFS = new WeakMap<NativeViewAbiSession, WeakMap<object, number>>();
const STYLE_ATOM_REFS = new WeakMap<NativeViewAbiSession, Map<string, number>>();
const TEXT_SCRATCH = new WeakMap<NativeViewAbiSession, Uint8Array>();
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

/** Installs an already-retained root without rebuilding or reacquiring it. */
export function tryNativeRenderRef(
  target: NativeViewRenderHost,
  viewRef: number,
): number | undefined {
  if (!isValidNativeRef(viewRef) || !canInstallNativeRef(target)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  try {
    const retainedRef = viewRenderRef(session.symbols, session.runtime, viewRef);
    if (installNativeRef(target, session, retainedRef) !== 0) return undefined;
    return retainedRef;
  } catch (error) {
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/** Materializes one pending text span through the direct cstring/buffer ABI. */
export function tryNativeTextCreateRender(
  host: NativeViewRenderHost,
  next: View,
): number | undefined {
  const session = nativeViewAbiSession();
  if (session === undefined || !canInstallNativeRef(host)) return undefined;
  let nextRef: number | undefined;
  try {
    nextRef = nativeTextCreateRef(session, next);
    if (nextRef === undefined) return undefined;
    if (installNativeRef(host, session, nextRef) !== 0) {
      releaseNativeViewRef(session, nextRef);
      return undefined;
    }
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/** Creates a supported pending View and returns its caller-owned NativeRef lease. */
export function tryNativeMaterialize(next: View): number | undefined {
  if (!nativeColdGraphWithinLimit(next)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  try {
    return nativeColdRefForView(session, next);
  } catch (error) {
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/** Creates and installs a new supported View at any native boundary. */
export function tryNativeViewBoundaryCreate(
  target: NativeViewRenderHost,
  next: View,
): number | undefined {
  const session = nativeViewAbiSession();
  if (session === undefined || !canInstallNativeRef(target)) return undefined;
  let nextRef: number | undefined;
  try {
    nextRef = tryNativeMaterialize(next);
    if (nextRef === undefined) return undefined;
    if (installNativeRef(target, session, nextRef) !== 0) {
      releaseNativeViewRef(session, nextRef);
      return undefined;
    }
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

/** Routes one retained View mutation to any native View-bearing target. */
export function tryNativeViewBoundaryRender(
  target: NativeViewRenderHost,
  previous: View,
  next: View,
  previousRef?: number,
): number | undefined {
  const session = nativeViewAbiSession();
  if (session === undefined || !canInstallNativeRef(target)) return undefined;
  const baseRef = previousRef;
  try {
    if (baseRef === undefined) return undefined;
    const scalar = tryNativeScalarRender(target, previous, baseRef, next);
    if (scalar !== undefined) return scalar;
    const path = tryNativePathScalarRender(target, previous, baseRef, next);
    if (path !== undefined) return path;
    const structural = tryNativeStructuralRender(target, previous, baseRef, next);
    if (structural !== undefined) return structural;
    return tryNativeTextCreateRender(target, next) ?? tryNativeColdRender(target, next);
  } catch (error) {
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
}

export function tryNativeStructuralRender(
  target: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  next: View,
): number | undefined {
  const edit = nativeStructuralEdit(next);
  if (edit === undefined || edit.base !== previous) return undefined;
  switch (edit.kind) {
    case "axisSet":
      return tryNativeAxisSetChildRender(target, previous, previousRef, next, edit.child, edit.index, edit.trackWord);
    case "axisSplice":
      return tryNativeAxisSpliceRender(target, previous, previousRef, next, edit.index, edit.removeCount, edit.children);
    case "gridCell":
      return tryNativeGridSetCellRender(target, previous, previousRef, next, edit.row, edit.column, edit.child);
  }
}

function nativeTextCreateRef(session: NativeViewAbiSession, next: View): number | undefined {
  const recipe = nativeTextRecipe(next);
  if (recipe === undefined || recipe.spans.length === 0 || recipe.spans.length > 4) return undefined;
  const styleRefs: number[] = [];
  for (const span of recipe.spans) {
    const styleRef = nativeStyleRef(session, span.style);
    if (styleRef === undefined) return undefined;
    styleRefs.push(styleRef);
  }
  const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
  const hasEmbeddedNul = recipe.spans.some((span) => span.text.indexOf("\0") !== -1);
  if (!hasEmbeddedNul) {
    const spans = recipe.spans;
    switch (spans.length) {
      case 1: return viewTextCreateCstring(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, spans[0]!.text, styleRefs[0]!, recipe.wrap, recipe.align);
      case 2: return viewTextCreateCstring2(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, recipe.wrap, recipe.align);
      case 3: return viewTextCreateCstring3(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, spans[2]!.text, styleRefs[2]!, recipe.wrap, recipe.align);
      case 4: return viewTextCreateCstring4(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, spans[0]!.text, styleRefs[0]!, spans[1]!.text, styleRefs[1]!, spans[2]!.text, styleRefs[2]!, spans[3]!.text, styleRefs[3]!, recipe.wrap, recipe.align);
      default: return undefined;
    }
  }
  const encoded = recipe.spans.map((span) => TEXT_ENCODER.encode(span.text));
  const totalBytes = encoded.reduce((total, bytes) => total + bytes.length, 0);
  if (totalBytes > NATIVE_TEXT_MAX_BYTES) return undefined;
  let scratch = TEXT_SCRATCH.get(session);
  if (scratch === undefined || scratch.length < totalBytes) {
    scratch = new Uint8Array(Math.max(totalBytes, 64));
    TEXT_SCRATCH.set(session, scratch);
  }
  const spanBytes: number[] = [];
  let offset = 0;
  for (const bytes of encoded) {
    scratch.set(bytes, offset);
    spanBytes.push(bytes.length);
    offset += bytes.length;
  }
  switch (recipe.spans.length) {
    case 1: return viewTextCreateUtf8(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, scratch, totalBytes, styleRefs[0]!, recipe.wrap, recipe.align);
    case 2: return viewTextCreateUtf82(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, scratch, totalBytes, spanBytes[0]!, styleRefs[0]!, spanBytes[1]!, styleRefs[1]!, recipe.wrap, recipe.align);
    case 3: return viewTextCreateUtf83(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, scratch, totalBytes, spanBytes[0]!, styleRefs[0]!, spanBytes[1]!, styleRefs[1]!, spanBytes[2]!, styleRefs[2]!, recipe.wrap, recipe.align);
    case 4: return viewTextCreateUtf84(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, scratch, totalBytes, spanBytes[0]!, styleRefs[0]!, spanBytes[1]!, styleRefs[1]!, spanBytes[2]!, styleRefs[2]!, spanBytes[3]!, styleRefs[3]!, recipe.wrap, recipe.align);
    default: return undefined;
  }
}

function nativeStyleRef(session: NativeViewAbiSession, style: StyleNode | undefined): number | undefined {
  if (style === undefined) return 0;
  let refs = STYLE_REFS.get(session);
  if (refs === undefined) {
    refs = new WeakMap<object, number>();
    STYLE_REFS.set(session, refs);
  }
  const cached = refs.get(style);
  if (cached !== undefined) return cached;
  const present = { value: 0 };
  const truth = { value: 0 };
  const attributes: Record<string, number> = {
    bold: 1,
    dim: 2,
    italic: 4,
    underline: 8,
    reversed: 16,
    strikethrough: 32,
  };
  for (const [name, enabled] of Object.entries(style.attributes)) {
    const bit = attributes[name];
    if (bit === undefined) return undefined;
    present.value |= bit;
    if (enabled) truth.value |= bit;
  }
  const foreground = style.foreground === undefined ? 0 : nativeStyleAtom(session, styleColorAtom(style.foreground));
  const background = style.background === undefined ? 0 : nativeStyleAtom(session, styleColorAtom(style.background));
  const theme = style.theme === undefined ? 0 : nativeStyleAtom(session, `theme:${style.theme}`);
  if (foreground === undefined || background === undefined || theme === undefined) return undefined;
  const reference = styleCreateBits(
    session.symbols,
    session.runtime,
    0,
    present.value,
    truth.value,
    foreground,
    background,
    theme,
  );
  refs.set(style, reference);
  return reference;
}

function nativeStyleAtom(session: NativeViewAbiSession, value: string): number | undefined {
  if (value.indexOf("\0") !== -1) return undefined;
  let refs = STYLE_ATOM_REFS.get(session);
  if (refs === undefined) {
    refs = new Map<string, number>();
    STYLE_ATOM_REFS.set(session, refs);
  }
  const cached = refs.get(value);
  if (cached !== undefined) return cached;
  const reference = styleAtomCreateCstring(session.symbols, session.runtime, value);
  refs.set(value, reference);
  return reference;
}

function styleColorAtom(color: ColorNode): string {
  return typeof color === "string" ? color : `ansi:${color.value}`;
}

/**
 * Materializes a small/new axis without a JS child packet. The children must
 * already have NativeRefs (a cold graph that cannot satisfy that condition is
 * deliberately left to the V4 fallback until its leaf constructors exist).
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
    || !nativeColdGraphWithinLimit(next)
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

/**
 * Cold-route a compact axis recipe. Supported leaves are materialized through
 * generated constructors; text and other unsupported leaves return undefined
 * so the caller can use the authoritative V4/direct fallback unchanged.
 */
export function tryNativeColdRender(host: NativeViewRenderHost, next: View): number | undefined {
  if (!canInstallNativeRef(host)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const recipe = nativeAxisRecipe(next);
  if (
    recipe === undefined
    || recipe.children.length > NATIVE_BUILDER_MAX_CHILDREN
    || !nativeColdGraphWithinLimit(next)
  ) return undefined;
  const refs: number[] = [];
  let nextRef: number | undefined;
  try {
    for (const child of recipe.children) {
      const reference = nativeColdRefForView(session, child.view);
      if (reference === undefined) return undefined;
      refs.push(reference);
    }
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
    nextRef = recipe.children.length <= NATIVE_SMALL_AXIS_ARITY_MAX
      ? createSmallAxis(session, recipe.horizontal, nodeIdLow, nodeIdHigh, recipe.gap, recipe.children, refs)
      : createAxisWithBuilder(session, recipe.horizontal ? 1 : 2, nodeIdLow, nodeIdHigh, recipe.gap, recipe.children, refs);
    if (nextRef === undefined) return undefined;
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
    for (const reference of refs) releaseNativeViewRef(session, reference);
  }
}

function nativeColdGraphWithinLimit(view: View): boolean {
  return nativeColdGraphNodeCount(view, NATIVE_COLD_MAX_NODES, 0) !== undefined;
}

function nativeColdGraphNodeCount(view: View, remaining: number, depth: number): number | undefined {
  if (depth > NATIVE_COLD_MAX_DEPTH || remaining <= 0) return undefined;
  const recipe = nativeAxisRecipe(view);
  if (recipe === undefined) return 1;
  let count = 1;
  if (count > remaining) return undefined;
  for (const child of recipe.children) {
    const childCount = nativeColdGraphNodeCount(child.view, remaining - count, depth + 1);
    if (childCount === undefined) return undefined;
    count += childCount;
    if (count > remaining) return undefined;
  }
  return count;
}

function nativeColdRefForView(session: NativeViewAbiSession, view: View): number | undefined {
  try {
    const existing = nativeViewRefForNodeId(view);
    if (existing !== undefined) return existing;
  } catch (error) {
    if (!isExpectedNativeStatus(error)) throw error;
  }
  const rows = nativeSpacerRecipe(view);
  if (rows !== undefined) {
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(view);
    return viewSpacerCreate(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, rows);
  }
  const text = nativeTextCreateRef(session, view);
  if (text !== undefined) return text;
  const recipe = nativeAxisRecipe(view);
  if (recipe === undefined || recipe.children.length > NATIVE_BUILDER_MAX_CHILDREN) return undefined;
  const refs: number[] = [];
  try {
    for (const child of recipe.children) {
      const reference = nativeColdRefForView(session, child.view);
      if (reference === undefined) return undefined;
      refs.push(reference);
    }
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(view);
    return recipe.children.length <= NATIVE_SMALL_AXIS_ARITY_MAX
      ? createSmallAxis(session, recipe.horizontal, nodeIdLow, nodeIdHigh, recipe.gap, recipe.children, refs)
      : createAxisWithBuilder(session, recipe.horizontal ? 1 : 2, nodeIdLow, nodeIdHigh, recipe.gap, recipe.children, refs);
  } finally {
    for (const reference of refs) releaseNativeViewRef(session, reference);
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
 * Attempts the Tranche 7 compact scalar route. Pending text/common patches
 * expose only fixed scalar fields here; this function deliberately does not
 * materialize either View's BridgeViewNode.
 */
export function tryNativeScalarRender(
  host: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  next: View,
): number | undefined {
  const patch = nativeScalarPatch(next);
  if (patch === undefined || patch.base !== previous) return undefined;
  if (!canInstallNativeRef(host) || !isValidNativeRef(previousRef)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;

  let nextRef: number | undefined;
  try {
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(next);
    nextRef = patch.kind === "textLayout"
      ? viewTextLayoutPatchRoot(session.symbols, session.runtime, previousRef, nodeIdLow, nodeIdHigh, patch.wrap, patch.align)
      : viewCommonPatchRoot(
        session.symbols,
        session.runtime,
        previousRef,
        nodeIdLow,
        nodeIdHigh,
        patch.mask,
        patch.paddingTopRight,
        patch.paddingBottomLeft,
        patch.widthRule,
        patch.heightRule,
        patch.minWidth,
        patch.maxWidth,
        patch.minHeight,
        patch.maxHeight,
        previousRef,
      );
    const status = installNativeRef(host, session, nextRef);
    if (status !== 0) {
      releaseNativeViewRef(session, nextRef);
      return undefined;
    }
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (isExpectedNativeStatus(error)) return undefined;
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
 * scalar NodeIds; the legacy View-array form remains for differential tests.
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

/** Routes a shallow retained text edit through a cached native PathRef. */
export function tryNativePathScalarRender(
  host: NativeViewRenderHost,
  previous: View,
  previousRef: number,
  next: View,
): number | undefined {
  const lineage = nativePathLineage(next);
  if (lineage === undefined || lineage.baseNodeId !== viewNodeId(previous) || lineage.depth < 1 || lineage.depth > 4) return undefined;
  if (!canInstallNativeRef(host) || !isValidNativeRef(previousRef)) return undefined;
  const session = nativeViewAbiSession();
  if (session === undefined) return undefined;
  const steps = pathLineageSteps(lineage);
  const nodes = pathNodes(nodeForBridge(next), steps);
  if (nodes === undefined || nodes.length !== lineage.depth + 1) return undefined;
  const target = splitNodeId(nodes[nodes.length - 1]!.id);
  const ancestors = nodes.slice(0, -1).reverse().map((node) => splitNodeId(node.id));
  const targetNode = nodes[nodes.length - 1]!;
  if (targetNode.kind !== BRIDGE_VIEW_KIND.text) return undefined;
  const wrap = targetNode.wrap;
  const align = targetNode.align;
  let nextRef: number | undefined;
  try {
    const pathRef = nativePathRefForLineage(session, lineage);
    nextRef = lineage.depth === 1
      ? viewTextLayoutPatchPathD1(session.symbols, session.runtime, previousRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], wrap, align)
      : lineage.depth === 2
        ? viewTextLayoutPatchPathD2(session.symbols, session.runtime, previousRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], ancestors[1]![0], ancestors[1]![1], wrap, align)
        : lineage.depth === 3
          ? viewTextLayoutPatchPathD3(session.symbols, session.runtime, previousRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], ancestors[1]![0], ancestors[1]![1], ancestors[2]![0], ancestors[2]![1], wrap, align)
          : viewTextLayoutPatchPathD4(session.symbols, session.runtime, previousRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], ancestors[1]![0], ancestors[1]![1], ancestors[2]![0], ancestors[2]![1], ancestors[3]![0], ancestors[3]![1], wrap, align);
    const status = installNativeRef(host, session, nextRef);
    if (status !== 0) {
      releaseNativeViewRef(session, nextRef);
      return undefined;
    }
    return nextRef;
  } catch (error) {
    if (nextRef !== undefined) releaseNativeViewRef(session, nextRef);
    if (isExpectedNativeStatus(error)) return undefined;
    throw error;
  }
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

function pathNodes(root: BridgeViewNode, steps: readonly NativePathLineage["step"][]): BridgeViewNode[] | undefined {
  const nodes = [root];
  let current = root;
  for (const step of steps) {
    if (step === undefined) return undefined;
    switch (step.kind) {
      case 1:
      case 2: {
        if (step.selector !== 0 || (current.kind !== BRIDGE_VIEW_KIND.container && current.kind !== BRIDGE_VIEW_KIND.clamp && current.kind !== BRIDGE_VIEW_KIND.contentMax)) return undefined;
        current = current.child;
        break;
      }
      case 4: {
        if (current.kind !== BRIDGE_VIEW_KIND.column) return undefined;
        const child = current.children[step.selector];
        if (child === undefined) return undefined;
        current = child.child;
        break;
      }
      case 5: {
        if (current.kind !== BRIDGE_VIEW_KIND.row) return undefined;
        const child = current.children[step.selector];
        if (child === undefined) return undefined;
        current = child.child;
        break;
      }
      case 6: {
        if (current.kind !== BRIDGE_VIEW_KIND.grid || step.selector < 0) return undefined;
        let remaining = step.selector;
        let found: BridgeViewNode | undefined;
        for (const row of current.rows) {
          for (const cell of row.cells) {
            if (remaining === 0) found = cell.view;
            remaining -= 1;
          }
        }
        if (found === undefined || remaining >= 0) return undefined;
        current = found;
        break;
      }
      case 7:
      case 8:
      case 9: {
        if (current.kind !== BRIDGE_VIEW_KIND.hanging || step.selector !== 0) return undefined;
        current = step.kind === 7 ? current.prefix : step.kind === 8 ? current.continuation : current.body;
        break;
      }
      default: return undefined;
    }
    nodes.push(current);
  }
  return nodes;
}

function splitNodeId(id: number): readonly [number, number] {
  if (!Number.isSafeInteger(id) || id <= 0 || id > Number.MAX_SAFE_INTEGER) throw new RangeError("native path NodeId is invalid");
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
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
  const ids = edit.nodeIds ?? edit.views?.map(viewNodeId);
  if (ids === undefined || ids.length !== depth + 1) return undefined;
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

/** Test-only reset; production sessions are environment-owned and stable. */
export function resetNativeViewAbiSessionForTests(): void {
  cachedSession = undefined;
}
