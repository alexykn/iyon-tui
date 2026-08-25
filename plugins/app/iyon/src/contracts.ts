import type {
  ApprovalId,
  JsonValue,
  MessageId,
  ReasoningLevel,
  ToolCallId,
} from "@iyon/sdk";
import type { ToolCall, ToolResult } from "@iyon/sdk";
import type { View } from "@iyon/tui";

export interface ToolDraftKey {
  readonly messageId: MessageId | number;
  readonly contentIndex: number;
}

export type ToolUpdatePresentation =
  | { readonly type: "text"; readonly text: string }
  | { readonly type: "progress"; readonly label: string; readonly current?: number; readonly total?: number }
  | { readonly type: "details"; readonly details: JsonValue };

export interface InfoState {
  readonly status: string;
  readonly provider: string;
  readonly modelId: string;
  readonly reasoningEffort: ReasoningLevel;
}

export type LiveToolStatus = "preparing" | "prepared" | "running" | "pendingApproval" | "finished" | "failed" | "cancelled";

export interface LiveTool {
  readonly draftKey?: ToolDraftKey;
  readonly toolCallId?: ToolCallId | string;
  readonly toolName?: string;
  readonly arguments?: JsonValue;
  readonly argumentPreview: string;
  readonly update?: ToolUpdatePresentation;
  readonly text?: string;
  readonly result?: ToolResult;
  readonly status: LiveToolStatus;
  readonly progress?: { readonly label: string; readonly current?: number; readonly total?: number };
  readonly details?: JsonValue;
  readonly isError: boolean;
  readonly frozen: boolean;
}

export interface PendingApproval {
  readonly approvalId: ApprovalId | number;
  readonly toolCallId: ToolCallId | string;
  readonly toolName: string;
  readonly arguments: JsonValue;
}

export interface ToolRendererContribution {
  readonly renderCall?: (call: ToolCall & { readonly arguments?: JsonValue; readonly argumentPreview?: string }) => View;
  readonly renderResult?: (result: ToolResult) => View;
}

export interface ToolResolver {
  readonly get: (toolName: string) => ToolRendererContribution | undefined;
}

export type FrontendEvent =
  | { readonly type: "turnStarted" }
  | { readonly type: "steerQueued"; readonly text: string; readonly queueId?: string | number }
  | { readonly type: "userMessage"; readonly text: string; readonly queueId?: string | number }
  | { readonly type: "assistantDelta"; readonly text: string }
  | { readonly type: "thinkingDelta"; readonly text: string }
  | { readonly type: "toolCallPreparing"; readonly key: ToolDraftKey; readonly toolCallId?: string; readonly toolName?: string }
  | { readonly type: "toolCallArguments"; readonly key: ToolDraftKey; readonly toolCallId?: string; readonly toolName?: string; readonly delta: string }
  | { readonly type: "toolCallPrepared"; readonly key: ToolDraftKey; readonly toolCallId: string; readonly toolName: string; readonly arguments: JsonValue }
  | { readonly type: "turnFinished" }
  | { readonly type: "turnFailed"; readonly message: string }
  | { readonly type: "turnCancelled" }
  | { readonly type: "toolCallStarted"; readonly toolCallId: string; readonly toolName: string; readonly arguments: JsonValue }
  | { readonly type: "toolCallUpdated"; readonly toolCallId: string; readonly update: ToolUpdatePresentation }
  | { readonly type: "toolCallFinished"; readonly toolCallId: string; readonly isError: boolean }
  | { readonly type: "toolApprovalRequested"; readonly approvalId: ApprovalId | number; readonly toolCallId: string; readonly toolName: string; readonly arguments: JsonValue }
  | { readonly type: "toolApprovalResolved"; readonly approvalId: ApprovalId | number; readonly toolCallId: string; readonly approved: boolean; readonly reason?: string }
  | { readonly type: "toolResult"; readonly toolCallId: string; readonly toolName: string; readonly text: string; readonly details: JsonValue; readonly isError: boolean }
  | { readonly type: "configChanged"; readonly provider: string; readonly modelId: string; readonly reasoningEffort: ReasoningLevel };

export type IyonAction =
  | { readonly type: "backend"; readonly event: FrontendEvent }
  | { readonly type: "submit"; readonly text: string; readonly queueId?: number }
  | { readonly type: "composerPaste"; readonly text: string }
  | { readonly type: "ctrlC" }
  | { readonly type: "escape" }
  | { readonly type: "cycleReasoningEffort" }
  | { readonly type: "approve"; readonly approvalId: ApprovalId | number }
  | { readonly type: "reject"; readonly approvalId: ApprovalId | number; readonly reason?: string }
  | { readonly type: "requestExit" };

export interface IyonState {
  readonly info: InfoState;
  readonly composerText: string;
  readonly userBatches: readonly string[];
  readonly working: boolean;
  readonly activityVisible: boolean;
  readonly steering: readonly string[];
  readonly steeringQueueIds: readonly (string | undefined)[];
  readonly assistantText: string;
  readonly thinkingText: string;
  readonly assistantOpen: boolean;
  readonly liveTools: ReadonlyMap<string, LiveTool>;
  readonly draftTools: ReadonlyMap<string, string>;
  readonly pendingApproval?: PendingApproval;
  readonly activeTurn: boolean;
  readonly goodbye: boolean;
}

export interface IyonAgent {
  readonly run?: (signal?: AbortSignal) => Promise<unknown>;
  readonly cancel?: () => Promise<void> | void;
}

export interface IyonCoreCommands {
  readonly submitPrompt?: (text: string) => Promise<number> | number;
  readonly steer?: (text: string) => Promise<number> | number;
  readonly followUp?: (text: string) => Promise<number> | number;
  readonly setReasoningEffort?: (level: ReasoningLevel) => Promise<void> | void;
  readonly submitTurn?: (text: string) => Promise<void> | void;
  readonly cancelActiveTurn?: () => Promise<void> | void;
  readonly cycleReasoningEffort?: () => Promise<void> | void;
  readonly approve?: (approvalId: ApprovalId | number) => Promise<void> | void;
  readonly reject?: (approvalId: ApprovalId | number, reason?: string) => Promise<void> | void;
}

export interface IyonModelMetadata {
  readonly provider: string;
  readonly modelId: string;
  readonly reasoningEffort?: ReasoningLevel;
}
