import type { ExtensionAPI } from "iyon:plugins";
import { createIyonApp } from "./app.ts";
import type { IyonAppDependencies } from "./app.ts";

export const IYON_APP_ID = "iyon" as const;

export function activate(api: ExtensionAPI): void {
  api.apps.register({
    id: IYON_APP_ID,
    create(context) {
      return createIyonApp(context as IyonAppDependencies);
    },
  });
}

export { createIyonApp } from "./app.ts";
export type { IyonApp, IyonAppDependencies } from "./app.ts";
export { ComposerPasteStore, isLargePaste, normalizePaste, MAX_COMPOSER_ROWS } from "./composer.ts";
export { createInitialState, cycleReasoningEffort, draftIdFor, reduceIyonState, updateInfo } from "./state.ts";
export { createIyonTheme } from "./theme.ts";
export { createIyonChrome, syncChromeStates, IyonRootView, footerText, workingFrames } from "./view.ts";
export type { ChromeState, IyonRootProps } from "./view.ts";
export { CoreEventMapper, coalesceFrontendEvents, startCoreEventBridge } from "./backend.ts";
export { AssistantStreamBuffer, NativeAssistantStream } from "./streaming.ts";
export { ToolCardStore } from "./tool-cards.ts";
export { ApprovalStore, pendingApproval } from "./approvals.ts";
export { handleIyonAction } from "./actions.ts";
export type {
  FrontendEvent,
  InfoState,
  IyonAction,
  IyonAgent,
  IyonCoreCommands,
  IyonModelMetadata,
  IyonState,
  LiveTool,
  PendingApproval,
  ToolDraftKey,
  ToolUpdatePresentation,
} from "./contracts.ts";
