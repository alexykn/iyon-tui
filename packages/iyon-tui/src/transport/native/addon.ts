/**
 * Private native contract for the generic TUI package.
 *
 * Application/session bindings deliberately do not belong here. The addon is
 * the S6 `iyon-tui-native` artifact and this contract exposes only framework
 * handles, View ABI calls, and generic terminal operations.
 */

import { tuiError } from "../../api/errors.ts";
import type { NativeViewAbiHandle } from "../abi/structural/generated/view_abi.ts";
export type { NativeViewAbiHandle };

export interface NativeTuiOutputContract {
  readonly output?: unknown;
}

export interface NativeHistoryContract {
  dispose(): void;
  layout(): object;
  setLayout(layout: object): void;
  isDetached(): boolean;
  push(view: object): number;
  pushRef(viewRef: number): number;
  freeze(unit: number, view: object): void;
  freezeRef(unit: number, viewRef: number): void;
  discardLive(unit: number): void;
  pushStream(stream: object): void;
  sealStream(stream: object): void;
}

export interface NativeStateWake {
  readonly schedule_environment_drain: boolean;
}

export interface NativeStructuralAttachmentContract {
  attachmentId(): number;
}

export interface NativeViewStateContract extends NativeStructuralAttachmentContract {
  dispose(): void;
  stateId(): number;
  validateNodeKind(targetNodeKind: number): void;
  setGeometry(patch: object): NativeStateWake;
  clearGeometry(properties?: readonly string[]): NativeStateWake;
  setPresentation(patch: object): NativeStateWake;
  clearPresentation(properties?: readonly string[]): NativeStateWake;
  setStyleState(key: string, value: string): NativeStateWake;
  clearStyleState(key: string): NativeStateWake;
}

export interface NativeTextInputContract {
  dispose(): void;
  text(): string;
  cursorBytes(): number;
  setText(value: string): void;
  clear(): void;
  submitted(): NativeTuiOutputContract;
  setMultiline(enabled: boolean): void;
  isMultiline(): boolean;
  componentId(): number | null;
}

export interface NativeTextStreamContract {
  dispose(): void;
  update(text: string): void;
  append(text: string, annotations?: readonly object[]): void;
  seal(): void;
  snapshot(): object;
}

export interface NativeTextSourceContract {
  dispose(): void;
  sourceId(): number;
  sourceGeneration(): number;
  family(): string;
}

export interface NativeContentConnectorContract {
  activate(): NativeStateWake;
  deactivate(): NativeStateWake;
  dispose(): NativeStateWake;
  status(): object;
}

export interface NativeContentPortContract extends NativeStructuralAttachmentContract {
  dispose(): void;
  portId(): number;
  portGeneration(): number;
  family(): string;
  deactivate(): NativeStateWake;
  connect(source: NativeTextSourceContract, funnel: object): NativeContentConnectorContract;
  mounted(): boolean;
}

export interface NativeProjectorContract {
  dispose(): void;
  project(text: string, sealed?: boolean): object;
}

export interface NativeViewSlotContract {
  dispose(): void;
  revision(): number;
  componentId(): number | null;
  setView(view: object): void;
  setViewRef(viewRef: number): void;
  setAnimation(frames: object[], intervalMs: number): void;
  setAnimationAtCycleBoundary(frames: object[], intervalMs: number): void;
  setAnimationRef1?(ref0: number, intervalMs: number): void;
  setAnimationRef2?(ref0: number, ref1: number, intervalMs: number): void;
  setAnimationRef3?(ref0: number, ref1: number, ref2: number, intervalMs: number): void;
  setAnimationRef4?(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void;
  setAnimationRef1AtCycleBoundary?(ref0: number, intervalMs: number): void;
  setAnimationRef2AtCycleBoundary?(ref0: number, ref1: number, intervalMs: number): void;
  setAnimationRef3AtCycleBoundary?(ref0: number, ref1: number, ref2: number, intervalMs: number): void;
  setAnimationRef4AtCycleBoundary?(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void;
  setAnimationRefs(refs: Uint32Array, usedCount: number, intervalMs: number): void;
  setAnimationRefsAtCycleBoundary(refs: Uint32Array, usedCount: number, intervalMs: number): void;
  stopAnimation(view: object): void;
  stopAnimationRef(viewRef: number): void;
}

export interface NativeScrollPaneContract {
  dispose(): void;
  componentId(): number | null;
  setContent(view: object): void;
  setContentRef(viewRef: number): void;
  followEnd(): void;
}

export interface NativeHostEpochs {
  readonly host_id: string | number;
  readonly desired_structural_revision: string | number;
  readonly visible_structural_revision?: string | number;
  readonly visible_frame_revision: string | number;
  readonly pending_epoch: string | number;
  readonly committed_epoch: string | number;
}

export interface NativeTuiHostContract {
  dispose(): void;
  exit(): void;
  history(): object;
  viewState(): NativeViewStateContract;
  contentPort(family?: string): NativeContentPortContract;
  disposeContentResources?(): void;
  textInput(multiline?: boolean, border?: object): NativeTextInputContract;
  setTheme(theme: object): void;
  setHistory(history: object): void;
  exited(): boolean;
  bindKey(key: string, modifiers: readonly string[] | undefined, routeId: string): void;
  route(output: NativeTuiOutputContract, routeId: string): void;
  interceptPaste(input: object, routeId: string): void;
  render(view: object): void;
  dispatchKey(key: string, modifiers?: readonly string[]): void;
  dispatchPaste(text: string): void;
  forwardPaste(text: string): void;
  pollTerminal(): void;
  nextWakeMs(): number;
  nextOutput(): { route_id: string; payload?: string | null } | null;
  waitForOutput(): Promise<{ route_id: string; payload?: string | null } | null>;
  screenRows(): string[];
  nativeHistoryRows(): string[];
  epochs?(): NativeHostEpochs;
  setDesiredViewRef?(viewRef: number): {
    readonly host_id: string | number;
    readonly schedule_environment_drain: boolean;
  };
  clearViewStateBindings?(): void;
  flushPendingHosts?(budget?: number, forceRetry?: boolean): {
    readonly rearm: boolean;
    readonly attempted: number;
    readonly committed_hosts: readonly (string | number)[];
    readonly commits?: readonly {
      readonly host_id: string | number;
      readonly committed_epoch: string | number;
      readonly visible_structural_revision: string | number;
    }[];
    readonly errors: readonly {
      readonly host_id: string | number;
      readonly attempted_epoch: string | number;
      readonly desired_revision: string | number;
      readonly phase: string;
      readonly code: string;
      readonly retryable: boolean;
      readonly diagnostic: string;
    }[];
    readonly wake_epoch: string | number;
  };
  resize(width: number, height: number): void;
  advanceTime(milliseconds: number): void;
  createViewSlot(initial: object): object;
  createViewSlotRef(viewRef: number): object;
  scrollPane(initial: object): object;
  scrollPaneRef(viewRef: number): object;
  styleAt(row: number, column: number): object | null;
  cellXOfText(row: number, text: string): number | null;
}

export interface NativeTuiAddon {
  nativeVersion(): string;
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
  tuiViewAbiSession?: () => NativeViewAbiHandle;
  tuiViewAbiDecodeRef?: (view: object) => number;
  tuiViewBridgeEnvironmentCount(): number;
  NativeHistory?: new () => NativeHistoryContract;
  NativeTextInput?: new (multiline?: boolean) => NativeTextInputContract;
  NativeTuiHost?: new (width?: number, height?: number, headless?: boolean) => NativeTuiHostContract;
  NativeTuiOutput?: new () => NativeTuiOutputContract;
  NativeTextStream?: new (options?: "markdown" | { readonly projector?: "markdown"; readonly presentation?: object; readonly pacing?: object }) => NativeTextStreamContract;
  NativeTextSource?: new (kind?: "block" | "stream", options?: object) => NativeTextSourceContract;
  NativeMarkdownProjector?: new () => NativeProjectorContract;
  NativePlainProjector?: new () => NativeProjectorContract;
  NativeViewSlot?: new (initial: object) => NativeViewSlotContract;
  NativeScrollPane?: new (initial: object) => NativeScrollPaneContract;
}

// The package owns this loader and its staged `iyon-tui-native.node` artifact.
export const native = require("../../../native/iyon-tui-native.node") as NativeTuiAddon;

/** Requires a native constructor without exposing addon details to callers. */
export function requireNativeClass<T>(factory: T | undefined, name: string): T {
  if (factory === undefined) throw tuiError("runtime", `${name} is unavailable in the native addon`);
  return factory;
}
