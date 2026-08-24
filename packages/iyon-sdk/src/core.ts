import type {
  ContentBlock,
  JsonValue,
  ModelError,
  ModelRequest,
  ModelStreamEvent,
  StopReason,
  Usage,
} from "./api.ts";

export type Brand<T, Name extends string> = T & { readonly __iyonBrand: Name };
export type SessionId = Brand<number, "SessionId">;
export type TurnId = Brand<number, "TurnId">;
export type MessageId = Brand<number, "MessageId">;
export type ApprovalId = Brand<number, "ApprovalId">;
export type ToolCallId = Brand<string, "ToolCallId">;

export type MessageRole = "user" | "assistant" | "toolResult" | "status";
export type MessageContent = ContentBlock[];

export type TranscriptMessage =
  | {
      kind: "message";
      role: "user";
      id: MessageId;
      content: MessageContent;
      timestamp: string;
    }
  | {
      kind: "message";
      role: "assistant";
      id: MessageId;
      content: MessageContent;
      usage?: Usage;
      stopReason?: StopReason;
      timestamp: string;
    }
  | {
      kind: "message";
      role: "toolResult";
      id: MessageId;
      toolCallId: ToolCallId;
      toolName: string;
      content: MessageContent;
      details: JsonValue;
      isError: boolean;
      timestamp: string;
    }
  | {
      kind: "message";
      role: "status";
      id: MessageId;
      text: string;
      timestamp: string;
    };

export type SessionEntry =
  | TranscriptMessage
  | { kind: "custom"; namespace: string; data: JsonValue };

export interface SessionSnapshot {
  sessionId: SessionId;
  entries: SessionEntry[];
}

export type CoreEvent =
  | { type: "agentStarted" }
  | { type: "agentFinished" }
  | { type: "turnStarted"; turnId: TurnId }
  | { type: "steerQueued"; queueId: number; text: string }
  | { type: "messageStarted"; turnId: TurnId; messageId: MessageId; role: MessageRole }
  | { type: "messageDelta"; turnId: TurnId; messageId: MessageId; delta: MessageDelta }
  | { type: "messageFinished"; turnId: TurnId; messageId: MessageId }
  | {
      type: "toolCallStarted";
      turnId: TurnId;
      messageId: MessageId;
      toolCallId: string;
      toolName: string;
      arguments: JsonValue;
    }
  | {
      type: "toolCallFinished";
      turnId: TurnId;
      messageId: MessageId;
      toolCallId: string;
      toolName: string;
      isError: boolean;
    }
  | {
      type: "toolCallUpdated";
      turnId: TurnId;
      messageId: MessageId;
      toolCallId: string;
      toolName: string;
      update: ToolUpdateEvent;
    }
  | {
      type: "toolResultStarted";
      turnId: TurnId;
      messageId: MessageId;
      toolCallId: string;
      toolName: string;
      isError: boolean;
    }
  | {
      type: "toolResultFinished";
      turnId: TurnId;
      messageId: MessageId;
      toolCallId: string;
      toolName: string;
      text: string;
      details: JsonValue;
      isError: boolean;
    }
  | {
      type: "toolApprovalRequested";
      turnId: TurnId;
      approvalId: ApprovalId;
      messageId: MessageId;
      toolCallId: string;
      toolName: string;
      arguments: JsonValue;
    }
  | {
      type: "toolApprovalResolved";
      turnId: TurnId;
      approvalId: ApprovalId;
      toolCallId: string;
      approved: boolean;
      reason?: string;
    }
  | { type: "turnFinished"; turnId: TurnId }
  | { type: "turnFailed"; turnId: TurnId; message: string }
  | { type: "turnCancelled"; turnId: TurnId }
  | { type: "configChanged"; provider: string; modelId: string; reasoningEffort: string };

export type MessageDelta =
  | { type: "text"; text: string }
  | { type: "thinking"; text: string }
  | { type: "toolCall"; delta: ToolCallDelta };

export type ToolCallDelta =
  | { type: "start"; contentIndex: number; toolCallId?: string; toolName?: string }
  | {
      type: "arguments";
      contentIndex: number;
      toolCallId?: string;
      toolName?: string;
      delta: string;
    }
  | {
      type: "end";
      contentIndex: number;
      toolCallId: string;
      toolName: string;
      arguments: JsonValue;
    };

export type ToolUpdateEvent =
  | { type: "text"; text: string }
  | { type: "progress"; label: string; current?: number; total?: number }
  | { type: "details"; details: JsonValue };

export type ModelTurnState = "active" | "finished" | "cancelled" | "failed";
export interface ModelTurnOptions {
  request: ModelRequest;
  signal?: AbortSignal;
}

export interface ModelTurnResult {
  turnId: TurnId;
  assistantMessage: TranscriptMessage;
  toolCalls: AssembledToolCall[];
  stopReason: StopReason;
  cancelled: boolean;
}

export interface AssembledToolCall {
  id: ToolCallId;
  name: string;
  arguments: JsonValue;
}

export type ApprovalRequirement =
  | { type: "notRequired" }
  | { type: "required"; reason?: string };
export type ApprovalStatus =
  | { type: "pending" }
  | { type: "approved" }
  | { type: "rejected"; reason?: string }
  | { type: "cancelled" };
export interface ApprovalState {
  id: ApprovalId;
  requirement: ApprovalRequirement;
  status: ApprovalStatus;
}

export interface ToolExecutionRequest {
  sessionId?: SessionId;
  turnId: TurnId;
  messageId: MessageId;
  toolCallId: ToolCallId;
  toolName: string;
  arguments: JsonValue;
  approval?: ApprovalRequirement;
}
export interface ToolResult {
  content: ContentBlock[];
  details: JsonValue;
  isError: boolean;
  terminate?: boolean;
}
export type ToolLifecycleState =
  | "preparing"
  | "prepared"
  | "running"
  | "pendingApproval"
  | "finished"
  | "failed"
  | "cancelled";
export interface ToolLifecycleEvent {
  sequence: number;
  toolCallId: ToolCallId;
  state: ToolLifecycleState;
  approvalId?: ApprovalId;
}

export interface NativeKernelSession {
  snapshot(): SessionSnapshot;
  appendMessage(message: unknown): MessageId;
  deliverUserMessage(text: string): MessageId;
  appendEntry(entry: SessionEntry): void;
  nextEvent(): Promise<CoreEvent | null>;
  nextEvents(max?: number): Promise<CoreEvent[]>;
  beginModelTurn(options: ModelTurnOptions): ModelTurn;
  prepareToolExecution(request: ToolExecutionRequest): ToolExecution;
  enqueue(kind: "prompt" | "steer" | "followUp", text: string): Promise<number>;
  dequeue(kind: "prompt" | "steer" | "followUp"): string | null;
  abort(): void;
  close(): void;
}

export interface NativeModelTurn {
  push(event: ModelStreamEvent): Promise<void>;
  pushMany(events: ModelStreamEvent[]): Promise<void>;
  finish(): Promise<ModelTurnResult>;
  fail(error: ModelError | string): Promise<void>;
  cancel(): Promise<ModelTurnResult>;
}

export interface NativeToolExecution {
  state(): ToolLifecycleState;
  events(): ToolLifecycleEvent[];
  prepared(argumentsValue: JsonValue): void;
  start(): void;
  requestApproval(requirement: ApprovalRequirement): ApprovalState | null;
  approve(approvalId: ApprovalId): void;
  reject(approvalId: ApprovalId, reason?: string): void;
  sendUpdate(update: ToolUpdateEvent): void;
  finish(result: ToolResult): void;
  fail(error: string): void;
  cancel(reason?: string): void;
}

export interface ModelTurn extends NativeModelTurn {}
export interface ToolExecution extends NativeToolExecution {}

export interface KernelSession extends NativeKernelSession {
  events(): AsyncIterable<CoreEvent>;
}

export interface AgentSession extends KernelSession {
  prompt(text: string, options?: { signal?: AbortSignal }): Promise<ModelTurn>;
  steer(text: string): Promise<number>;
  followUp(text: string): Promise<number>;
  setModel(model: string): void;
  setReasoning(reasoning: string): void;
  setActiveTools(tools: string[]): void;
}
