import type { ContentBlock, JsonValue, ModelToolSpec } from "./api.ts";
import type {
  ApprovalRequirement,
  ApprovalState,
  MessageId,
  SessionId,
  ToolCallId,
  ToolLifecycleState,
  ToolResult as CoreToolResult,
  TurnId,
  ToolUpdateEvent,
} from "./core.ts";
import type { View } from "@iyon/tui";

export type ToolExecutionMode = "parallel" | "sequential";
export type ToolApprovalPolicy = "neverAsk" | "alwaysAsk" | ApprovalRequirement;

export interface ToolMetadata {
  readonly executionMode?: ToolExecutionMode;
  readonly approval?: ToolApprovalPolicy;
  readonly promptSnippet?: string;
  readonly promptGuidelines?: readonly string[];
  readonly [key: string]: unknown;
}

export interface ToolCall<TArgs = JsonValue> {
  readonly id: ToolCallId;
  readonly name: string;
  readonly arguments?: TArgs;
  readonly turnId?: TurnId;
  readonly messageId?: MessageId;
  readonly state: ToolLifecycleState;
  readonly argumentPreview?: string;
  readonly showArgPreview?: boolean;
  readonly pulse?: boolean;
}

export interface ToolResult<TValue = unknown> extends CoreToolResult {
  readonly value?: TValue;
  readonly toolCallId?: ToolCallId;
  readonly toolName?: string;
  readonly text?: string;
  readonly state?: ToolLifecycleState;
  readonly terminate?: boolean;
}

export interface ToolUpdateSink {
  send(update: ToolUpdateEvent): Promise<void>;
}

export interface WorkspaceHandle {
  readonly root?: string;
  resolveReadPath?(path: string): string | Promise<string>;
  resolveWritePath?(path: string): string | Promise<string>;
  resolveSearchPath?(path?: string): string | Promise<string>;
  readText?(path: string): string | Promise<string>;
  writeText?(path: string, content: string): void | Promise<void>;
  ensureReadAllowed?(path: string): void | Promise<void>;
  ensureWriteAllowed?(path: string): void | Promise<void>;
}

export interface ToolContext {
  readonly sessionId: SessionId;
  readonly turnId: TurnId;
  readonly messageId: MessageId;
  readonly toolCallId: ToolCallId;
  readonly cwd: string;
  readonly workspace: WorkspaceHandle;
  readonly signal: AbortSignal;
  readonly updates: ToolUpdateSink;
  update(update: ToolUpdateEvent): Promise<void>;
  approval?: (state: ApprovalState) => Promise<boolean>;
}

export interface Tool<TArgs = JsonValue, TValue = unknown> {
  readonly [key: string]: unknown;
  readonly id?: string;
  readonly name: string;
  readonly description: string;
  readonly inputSchema: JsonValue;
  readonly execution?: ToolMetadata;
  readonly metadata?: ToolMetadata;
  readonly executionMode?: ToolExecutionMode;
  readonly approval?: ToolApprovalPolicy;
  readonly execute: (context: ToolContext, args: TArgs) => Promise<ToolResult<TValue>>;
  readonly renderCall: (call: ToolCall<TArgs>) => View;
  readonly renderResult: (result: ToolResult<TValue>) => View;
  readonly modelSpec?: ModelToolSpec;
  readonly policy?: unknown;
}

export type ToolDefinition<TArgs = JsonValue, TValue = unknown> = Tool<TArgs, TValue>;

export function defineTool<TArgs = JsonValue, TValue = unknown>(tool: Tool<TArgs, TValue>): Tool<TArgs, TValue> & { readonly id: string } {
  if (!tool || typeof tool !== "object") throw new TypeError("tool definition must be an object");
  if (!tool.name || typeof tool.name !== "string") throw new TypeError("tool name must be a non-empty string");
  if (typeof tool.execute !== "function") throw new TypeError(`tool ${tool.name} must define execute`);
  if (typeof tool.renderCall !== "function" || typeof tool.renderResult !== "function") {
    throw new TypeError(`tool ${tool.name} must define renderCall and renderResult`);
  }
  return Object.freeze({ ...tool, id: tool.id ?? tool.name, modelSpec: tool.modelSpec ?? { name: tool.name, description: tool.description, inputSchema: tool.inputSchema } });
}

export type NativeToolResult = CoreToolResult;
export type ToolContent = ContentBlock;
