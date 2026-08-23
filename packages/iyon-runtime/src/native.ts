export type JsonPrimitive = null | boolean | number | string;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export interface NativeModelTurnContract {
  push(event: JsonValue): Promise<void>;
  pushMany(events: JsonValue[]): Promise<void>;
  finish(): Promise<JsonValue>;
  fail(error: JsonValue): Promise<void>;
  cancel(): Promise<JsonValue>;
}

export interface NativeToolExecutionContract {
  state(): string;
  events(): JsonValue[];
  prepared(argumentsValue: JsonValue): void;
  start(): void;
  requestApproval(requirement?: JsonValue): JsonValue | null;
  approve(approvalId: number): void;
  reject(approvalId: number, reason?: string): void;
  finish(result: JsonValue): void;
  fail(error: string): void;
  cancel(reason?: string): void;
}

export interface NativeKernelSessionContract {
  snapshot(): JsonValue;
  appendMessage(message: JsonValue): number;
  deliverUserMessage(text: string): number;
  appendEntry(entry: JsonValue): void;
  nextEvent(): Promise<JsonValue | null>;
  nextEvents(max?: number): Promise<JsonValue[]>;
  beginModelTurn(options: JsonValue): NativeModelTurnContract;
  prepareToolExecution(request: JsonValue): NativeToolExecutionContract;
  enqueue(kind: string, text: string): number;
  dequeue(kind: string): string | null;
  queueSnapshot(): JsonValue;
  abort(): void;
  close(): void;
}

export interface NativeCounterStats {
  live: number;
  finalized: number;
}

export interface CancellationProbeContract {
  run(ms: number): Promise<string>;
  cancel(): void;
}

export interface NativeCounterContract {
  increment(): number;
  value(): number;
}

export interface EventQueueProbeContract {
  send(event: JsonValue): Promise<void>;
  nextEvent(): Promise<JsonValue | null>;
  close(): void;
}

export interface NativeViewAbiBootstrap {
  runtime_ptr: number;
  abi_name: string;
  abi_version: number;
  semantic_version: number;
  schema_blake3: string;
  generator_blake3: string;
  generation: number;
  fast_view_abi: boolean;
  function_count: number;
  diagnostics?: {
    semantic_cache_entries: number;
    native_ref_slots: number;
    leased_slots: number;
    path_nodes: number;
    builders: number;
    edit_transactions: number;
    style_atoms: number;
    styles: number;
    fast_slot_tables: number;
    fast_slots: number;
    stale_removals: number;
    release_batches: number;
    released_refs: number;
    live_weak_upgrades: number;
    generation: number;
    alive: boolean;
  };
  functions: {
    runtimeNoop: number;
    viewStatusDetail: number;
    viewRenderRef: number;
    hostRenderRef: number;
    viewSpacerCreate: number;
    viewTextLayoutPatchRoot: number;
    viewCommonPatchRoot: number;
    viewAxisCreateBuffer: number;
    viewRowCreate0: number;
    viewRowCreate1: number;
    viewRowCreate2: number;
    viewRowCreate3: number;
    viewRowCreate4: number;
    viewColumnCreate0: number;
    viewColumnCreate1: number;
    viewColumnCreate2: number;
    viewColumnCreate3: number;
    viewColumnCreate4: number;
    axisBuilderBegin: number;
    axisBuilderPush: number;
    axisBuilderFinish: number;
    axisBuilderAbort: number;
    viewAxisSetChild: number;
    viewAxisSpliceBuffer: number;
    viewGridSetCell: number;
    viewGridCreateBuffer: number;
    viewDiffCreateBuffer: number;
    viewAxisSetChildPath: number;
    viewGridSetCellPath: number;
    viewReleaseMany: number;
    viewRefForNodeId: number;
    pathRoot: number;
    pathChild: number;
    viewTextLayoutPatchPath: number;
    viewTextLayoutPatchPathD1: number;
    viewTextLayoutPatchPathD2: number;
    viewTextLayoutPatchPathD3: number;
    viewTextLayoutPatchPathD4: number;
    editTxnBegin: number;
    editTxnAddTextLayout: number;
    editTxnCommitRender: number;
    editTxnAbort: number;
    styleAtomCreateCstring: number;
    styleCreateBits: number;
    viewTextCreateCstring: number;
    viewTextCreateUtf8: number;
    viewTextCreateUtf82: number;
    viewTextCreateUtf83: number;
    viewTextCreateUtf84: number;
    viewTextCreateCstring2: number;
    viewTextCreateCstring3: number;
    viewTextCreateCstring4: number;
  };
}

export interface NativeAddon {
  nativeVersion(): string;
  echoJson(value: JsonValue): JsonValue;
  echoString(value: string): string;
  echoBuffer(value: Buffer): Buffer;
  tuiSmoke(): string;
  tuiViewAbiMaintain?: (full?: boolean) => {
    full: boolean;
    semantic_cache_entries: number;
    native_ref_slots: number;
    scavenge_queue_len: number;
    scavenge_processed: number;
    semantic_cache_full_sweeps: number;
  };
  tuiViewRuntimeMemorySnapshot?: (countLive?: boolean) => {
    semantic_cache_entries: number;
    semantic_cache_live: number;
    native_ref_slots: number;
    native_ref_pages: number;
    native_ref_pages_freed: number;
    leased_slots: number;
    unleased_live_slots: number;
    node_ref_entries: number;
    path_nodes: number;
    path_keys: number;
    builders: number;
    edit_txns: number;
    style_refs: number;
    string_bytes: number | null;
    scavenge_queue: number;
    scavenge_processed: number;
    semantic_cache_expired_seen: number;
    semantic_cache_full_sweeps: number;
    semantic_cache_entries_removed: number;
    native_ref_expired_slots_removed: number;
    nodes_inserted_since_full_sweep: number;
    generation: number;
    alive: boolean;
  };
  tuiViewAbiBootstrap?: (pruneExpired?: boolean) => NativeViewAbiBootstrap;
  /** Exceptional T12 recovery: decode one semantic bridge node and return a leased root ref. */
  tuiViewAbiDecodeRef?: (view: object) => number;
  tuiPerfAbiProbe?(): {
    noop_ptr: number;
    u32_8_ptr: number;
    i32_4_ptr: number;
    buffer_ptr: number;
    cstring_ptr: number;
  };
  tuiPerfAbiConformanceProbe?(): {
    u8_8: number;
    u16_8: number;
    u32_8: number;
    u32_16: number;
    i32_4: number;
    f32_4: number;
    f64_4: number;
    pointer: number;
    buffer: number;
    cstring: number;
  };
  tuiViewBridgeEnvironmentCount(): number;
  asyncSleep(ms: number): Promise<string>;
  CancellationProbe: new () => CancellationProbeContract;
  NativeCounter: new () => NativeCounterContract;
  EventQueueProbe: new () => EventQueueProbeContract;
  KernelSession: new (options?: JsonValue) => NativeKernelSessionContract;
  nativeCounterStats(): NativeCounterStats;
  resetNativeCounterStats(): void;
  credentialGet(service: string, account: string): string | undefined;
  credentialSet(service: string, account: string, secret: string): void;
  credentialDelete(service: string, account: string): void;
  credentialHas(service: string, account: string): boolean;
  NativeHistory?: new () => { dispose(): void; layout(): object; setLayout(layout: object): void; isDetached(): boolean; push(view: object): number; pushRef(viewRef: number): number; freeze(unit: number, view: object): void; freezeRef(unit: number, viewRef: number): void; discardLive(unit: number): void; pushStream(stream: object): void; sealStream(stream: object): void };
  NativeTextInput?: new (multiline?: boolean) => { dispose(): void; text(): string; cursorBytes(): number; setText(value: string): void; clear(): void; submitted(): NativeTuiOutputContract; setMultiline(enabled: boolean): void; isMultiline(): boolean; componentId(): number | null };
  NativeTuiHost?: new (width?: number, height?: number, headless?: boolean) => NativeTuiHostContract;
  NativeTuiOutput?: new () => NativeTuiOutputContract;
  NativeTextStream?: new (options?: "markdown" | { readonly projector?: "markdown"; readonly presentation?: object; readonly pacing?: object }) => { dispose(): void; update(text: string): void; append(text: string, annotations?: readonly object[]): void; seal(): void; snapshot(): object };
  NativeMarkdownProjector?: new () => { dispose(): void; project(text: string, sealed?: boolean): object };
  NativePlainProjector?: new () => { dispose(): void; project(text: string): object };
  NativeViewSlot?: new (initial: object) => { dispose(): void; revision(): number; componentId(): number | null; setView(view: object): void; setViewRef(viewRef: number): void; setAnimation(frames: object[], intervalMs: number): void; setAnimationAtCycleBoundary(frames: object[], intervalMs: number): void; setAnimationRef1?(ref0: number, intervalMs: number): void; setAnimationRef2?(ref0: number, ref1: number, intervalMs: number): void; setAnimationRef3?(ref0: number, ref1: number, ref2: number, intervalMs: number): void; setAnimationRef4?(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void; setAnimationRef1AtCycleBoundary?(ref0: number, intervalMs: number): void; setAnimationRef2AtCycleBoundary?(ref0: number, ref1: number, intervalMs: number): void; setAnimationRef3AtCycleBoundary?(ref0: number, ref1: number, ref2: number, intervalMs: number): void; setAnimationRef4AtCycleBoundary?(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void; setAnimationRefs(refs: Uint32Array, usedCount: number, intervalMs: number): void; setAnimationRefsAtCycleBoundary(refs: Uint32Array, usedCount: number, intervalMs: number): void; stopAnimation(view: object): void; stopAnimationRef(viewRef: number): void };
  NativeScrollPane?: new (initial: object) => { dispose(): void; componentId(): number | null; setContent(view: object): void; setContentRef(viewRef: number): void; followEnd(): void };
  tuiPerfV3ResetViewBridgeCache?: () => void;
  tuiPerfV3ViewBridgeCacheSize?: () => number;
  tuiPerfV3PackedSlotPages?: () => number;
  tuiPerfV3ViewBridgeGeneration?: () => number;
  tuiPerfV4ResetViewBridgeCache?: () => void;
  tuiPerfV4ViewBridgeCacheSize?: () => number;
  tuiPerfV4ViewBridgeGeneration?: () => number;
}

export interface NativeTuiOutputContract { readonly output?: unknown; }

export interface NativeTuiHostContract {
  dispose(): void;
  exit(): void;
  history(): object;
  textInput(multiline?: boolean, border?: object): object;
  setTheme(theme: object): void;
  setHistory(history: object): void;
  exited(): boolean;
  bindKey(key: string, modifiers: readonly string[] | undefined, routeId: string): void;
  route(output: NativeTuiOutputContract, routeId: string): void;
  interceptPaste(input: object, routeId: string): void;
  render(view: object): void;
  tuiViewAbiHostPointer?(): number;
  dispatchKey(key: string, modifiers?: readonly string[]): void;
  dispatchPaste(text: string): void;
  forwardPaste(text: string): void;
  pollTerminal(): void;
  nextWakeMs(): number;
  nextOutput(): { route_id: string; payload?: string | null } | null;
  waitForOutput(): Promise<{ route_id: string; payload?: string | null } | null>;
  nextAction(): { action_id: string; payload?: string | null } | null;
  waitForAction(): Promise<{ action_id: string; payload?: string | null } | null>;
  screenRows(): string[];
  nativeHistoryRows(): string[];
  resize(width: number, height: number): void;
  advanceTime(milliseconds: number): void;
  createViewSlot(initial: object): object;
  createViewSlotRef(viewRef: number): object;
  scrollPane(initial: object): object;
  scrollPaneRef(viewRef: number): object;
  styleAt(row: number, column: number): object | null;
  cellXOfText(row: number, text: string): number | null;
  tuiPerfV3PackedRender?(words: Uint32Array, bytes: Uint8Array): void;
  tuiPerfV3PackedRenderStrings?(words: Uint32Array, strings: readonly string[]): void;
  tuiPerfV3PackedRenderRef?(generation: number, packedRef: number): void;
  tuiPerfV4PackedRender?(words: Uint32Array, bytes: Uint8Array): void;
  tuiPerfV4PackedRenderRef?(generation: number, packedRef: number): void;
}

// This is the one static addon seam. The stage script materializes this exact
// path before Bun typechecking, tests, or standalone compilation. A static
// require keeps the .node reachable to Bun's compiler for embedding.
export const native = require("../native/iyon-native.node") as NativeAddon;

export const {
  nativeVersion,
  echoJson,
  echoString,
  echoBuffer,
  tuiSmoke: nativeTuiSmoke,
  asyncSleep,
  CancellationProbe,
  NativeCounter,
  EventQueueProbe,
  KernelSession,
  nativeCounterStats,
  resetNativeCounterStats,
  credentialGet,
  credentialSet,
  credentialDelete,
  credentialHas,
} = native;
