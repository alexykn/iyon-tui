declare module "iyon:api" {
  export type * from "@iyon/sdk";
  export const apiSmoke: "iyon:api/t1";
  export const nativeVersion: () => string;
  export const echoJson: (value: import("./native.ts").JsonValue) => import("./native.ts").JsonValue;
  export const echoString: (value: string) => string;
  export const echoBuffer: (value: Buffer) => Buffer;
}

declare module "iyon:core" {
  export const AgentSession: any;
  export const IyonNativeError: any;
  export const KernelSession: new (...args: any[]) => {
    snapshot(): SessionSnapshot;
    appendMessage(...args: any[]): number;
    [key: string]: any;
  };
  export const ModelTurn: any;
  export const ToolExecution: any;
  export const asIyonError: any;
  export const isCancelledError: any;
  export type SessionEntry = { readonly role?: string };
  export type SessionSnapshot = {
    readonly sessionId?: number;
    readonly entries: readonly SessionEntry[];
  };
  export type ToolResult = any;
  export const coreSmoke: "iyon:core/t1";
  export function runWithAbortSignal<T>(
    signal: AbortSignal,
    operation: { run(): Promise<T>; cancel(): void },
  ): Promise<T>;
  export function cancellationOperation(ms: number): {
    run(): Promise<string>;
    cancel(): void;
  };
  export const asyncSleep: (ms: number) => Promise<string>;
  export const CancellationProbe: new () => {
    run(ms: number): Promise<string>;
    cancel(): void;
  };
  export const NativeCounter: new () => import("./native.ts").NativeCounterContract;
  export const EventQueueProbe: new () => import("./native.ts").EventQueueProbeContract;
  export const nativeCounterStats: () => import("./native.ts").NativeCounterStats;
  export const resetNativeCounterStats: () => void;
}

declare module "iyon:tui" {
  export const tuiSmoke: "iyon:tui/t1";  export const View: typeof import("./tui/values/view.ts").View;
  export const Insets: typeof import("./tui/values/geometry.ts").Insets;
  export const Style: typeof import("./tui/values/style.ts").Style;
  export const StyleSpec: typeof import("./tui/values/style.ts").StyleSpec;
  export const TextSpan: typeof import("./tui/values/text.ts").TextSpan;
  export const TextSelector: typeof import("./tui/values/text.ts").TextSelector;
  export const Theme: typeof import("./tui/values/theme.ts").Theme;
  export const History: typeof import("./tui/history.ts").History;
  export const TextInput: typeof import("./tui/text-input.ts").TextInput;
  export const TextStream: typeof import("./tui/stream.ts").TextStream;
  export const NativeScrollPane: typeof import("./tui/scroll-pane.ts").NativeScrollPane;
  export const Component: typeof import("./tui/component.ts").Component;
  export const defineView: typeof import("./tui/define-view.ts").defineView;
  export const state: typeof import("./tui/tracked-state.ts").state;
  export type State<T> = import("./tui/tracked-state.ts").State<T>;
  export const Scene: typeof import("./tui/scene.ts").Scene;
  export const Tui: typeof import("./tui/runtime.ts").Tui;
  export const AppHarness: typeof import("./tui/testing.ts").AppHarness;
  export class TuiError extends Error {
    readonly category: import("./tui/errors.ts").TuiErrorCategory;
    readonly nativeCode?: string;
    readonly context?: Readonly<Record<string, unknown>>;
  }
  export function asTuiError(error: unknown): TuiError;
  export function isTuiError(error: unknown): error is TuiError;
  export function isTuiCancelledError(error: unknown): boolean;
}

declare module "iyon:plugins" {
  export * from "@iyon/plugins";
}

declare module "*.node" {
  const nativeAddon: import("./native.ts").NativeAddon;
  export default nativeAddon;
}
