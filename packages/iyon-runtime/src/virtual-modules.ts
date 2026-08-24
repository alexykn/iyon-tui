import {
  CancellationProbe,
  EventQueueProbe,
  NativeCounter,
  asyncSleep,
  echoBuffer,
  echoJson,
  echoString,
  nativeCounterStats,
  nativeVersion,
  resetNativeCounterStats,
} from "./native.ts";
import {
  apiSmoke,
  cancellationOperation,
  coreSmoke,
  runWithAbortSignal,
  tuiSmoke,
} from "./smoke.ts";
import {
  AgentSession,
  IyonNativeError,
  KernelSession,
  ModelTurn,
  ToolExecution,
  asIyonError,
  isCancelledError,
} from "./modules/core.ts";
import { AppHarness, Component, DiffHunk, DiffLine, DiffRange, DiffRenderer, History, Insets, NativeScrollPane, Scene, Style, StyleSpec, Tui, TextInput, TextSelector, TextSpan, TextStream, Theme, View } from "./tui/index.ts";

const virtualModules = {
  "iyon:api": `
    export type * from "@iyon/runtime/modules/api";
    export { apiSmoke } from "@iyon/runtime/smoke";
    export { nativeVersion, echoJson, echoString, echoBuffer } from "@iyon/runtime/native";
  `,
  "iyon:core": `
    export * from "@iyon/runtime/modules/core";
    export { coreSmoke, runWithAbortSignal, cancellationOperation } from "@iyon/runtime/smoke";
    export {
      asyncSleep,
      CancellationProbe,
      NativeCounter,
      EventQueueProbe,
      nativeCounterStats,
      resetNativeCounterStats,
    } from "@iyon/runtime/native";
  `,
  "iyon:tui": `
    export * from "@iyon/runtime/tui";
  `,
  "iyon:plugins": `
    export * from "@iyon/plugins";
  `,
} as const;

type IyonVirtualModule = keyof typeof virtualModules;

export const iyonVirtualModulePlugin: Bun.BunPlugin = {
  name: "iyon-virtual-modules",
  setup(build) {
    build.onResolve(
      { filter: /^(iyon:)?(api|core|tui|plugins)$/ },
      ({ path }) => ({
        path: path.startsWith("iyon:") ? path : `iyon:${path}`,
        namespace: "iyon-t1-virtual",
      }),
    );
    build.onLoad(
      { filter: /^(iyon:)?(api|core|tui|plugins)$/, namespace: "iyon-t1-virtual" },
      ({ path }) => {
        const moduleName = path.startsWith("iyon:") ? path : `iyon:${path}`;
        const source = virtualModules[moduleName as IyonVirtualModule];
        if (source === undefined) {
          throw new Error(`unknown Iyon virtual module: ${path}`);
        }
        return { contents: source, loader: "ts" };
      },
    );

  },
};

function registerRuntimeModules(build: Bun.PluginBuilder): void {
  // Bun 1.3 applies `module` to runtime dynamic imports while
  // onResolve/onLoad are the bundler path for the same canonical names.
  build.module("iyon:api", () => ({
    exports: { apiSmoke, nativeVersion, echoJson, echoString, echoBuffer },
    loader: "object",
  }));
  build.module("iyon:core", () => ({
    exports: {
      coreSmoke,
      runWithAbortSignal,
      cancellationOperation,
      asyncSleep,
      CancellationProbe,
      NativeCounter,
      EventQueueProbe,
      nativeCounterStats,
      resetNativeCounterStats,
      KernelSession,
      ModelTurn,
      ToolExecution,
      AgentSession,
      IyonNativeError,
      asIyonError,
      isCancelledError,
    },
    loader: "object",
  }));
  build.module("iyon:tui", () => ({
    exports: {
      tuiSmoke,
      AppHarness,
      Component,
      DiffHunk,
      DiffLine,
      DiffRange,
      DiffRenderer,
      History,
      Insets,
      Scene,
      Style,
      StyleSpec,
      Tui,
      TextInput,
      TextSelector,
      TextSpan,
      TextStream,
      NativeScrollPane,
      Theme,
      View,
      defineView: require("./tui/define-view.ts").defineView,
      state: require("./tui/tracked-state.ts").state,
      TuiError: require("./tui/errors.ts").TuiError,
      asTuiError: require("./tui/errors.ts").asTuiError,
      isTuiCancelledError: require("./tui/errors.ts").isTuiCancelledError,
      isTuiError: require("./tui/errors.ts").isTuiError,
    },
    loader: "object",
  }));
  build.module("iyon:plugins", () => ({
    exports: require("@iyon/plugins"),
    loader: "object",
  }));
}

let installed = false;

/** Install the three T1 canonical modules once, before application imports. */
export function installIyonVirtualModules(): void {
  if (installed) {
    return;
  }
  Bun.plugin({
    name: iyonVirtualModulePlugin.name,
    setup(build) {
      iyonVirtualModulePlugin.setup(build);
      registerRuntimeModules(build);
    },
  });
  installed = true;
}

export function isIyonVirtualModulesInstalled(): boolean {
  return installed;
}
