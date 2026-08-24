import type {
  ApprovalRequirement,
  JsonValue,
  NativeKernelSession,
  NativeToolExecution,
  ToolExecutionRequest,
  ToolLifecycleEvent,
  ToolLifecycleState,
  ToolUpdateEvent,
} from "@iyon/sdk";
import { asIyonError, isCancelledError } from "../modules/core.ts";
import { abortError } from "../modules/abort.ts";
import type { AnyTool, ToolExecutionHooks, ToolLifecycleOptions, ToolResult } from "./contract.ts";
import { renderGenericCall, renderGenericResult } from "./generic.ts";
import type { ToolCall, ToolContext } from "@iyon/sdk";

export interface ToolExecutionRequestWithContext extends ToolExecutionRequest {
  readonly cwd?: string;
  readonly workspace?: ToolContext["workspace"];
}

export interface ToolLifecycleResult {
  readonly result: ToolResult;
  readonly execution: NativeToolExecution;
  readonly events: readonly ToolLifecycleEvent[];
  readonly updates: readonly ToolUpdateEvent[];
}

export class ApprovalUnavailableError extends Error {
  readonly code = "ION_APPROVAL_UNAVAILABLE" as const;

  constructor(toolName: string) {
    super(`tool ${toolName} requires approval, but no approval resolver was provided`);
    this.name = "ApprovalUnavailableError";
  }
}

export async function executeTool(
  session: NativeKernelSession,
  tool: AnyTool | undefined,
  request: ToolExecutionRequestWithContext,
  options: ToolLifecycleOptions = {},
): Promise<ToolLifecycleResult> {
  const execution = session.prepareToolExecution(request);
  const signal = options.signal ?? new AbortController().signal;
  // Default tool update sink: push updates through the native kernel event
  // bus via the ToolExecution handle's sendUpdate.  This makes every
  // context.update() from a tool plugin appear as a CoreEvent::ToolCallUpdated
  // on the kernel event stream, which the frontend already translates into
  // toolCallUpdated frontend events and routes to live tool cards / panes.
  const updates = options.updates ?? {
    send: async (update: ToolUpdateEvent): Promise<void> => {
      (execution).sendUpdate(update);
    },
  };
  const context = createContext(request, signal, options, updates);
  let result: ToolResult | undefined;
  let terminalized = false;

  const fail = (error: unknown): never => {
    const normalized = asIyonError(error);
    if (!terminalized) {
      if (isCancelledError(normalized) || signal.aborted) execution.cancel(normalized.message);
      else execution.fail(normalized.message);
      terminalized = true;
    }
    throw normalized;
  };

  try {
    if (signal.aborted) return cancelAndThrow(execution, signal.reason);
    if (!tool) {
      execution.prepared(request.arguments);
      execution.start();
      const unknown = unknownResult(request.toolName, request.arguments);
      execution.finish(unknown);
      terminalized = true;
      return { result: unknown, execution, events: execution.events(), updates: [] };
    }

    const before = await options.hooks?.before?.(context, request.arguments);
    const argumentsValue = before && typeof before === "object" ? before as JsonValue : request.arguments;
    execution.prepared(argumentsValue);
    const requirement = approvalRequirement(tool, request.toolName, argumentsValue, options.policy);
    const approval = execution.requestApproval(requirement);
    if (approval) {
      if (!context.approval) {
        const error = new ApprovalUnavailableError(tool.name);
        execution.fail(error.message);
        terminalized = true;
        throw error;
      }
      const approve = await context.approval(approval);
      if (approve) {
        execution.approve(approval.id);
      } else {
        execution.reject(approval.id, "tool approval rejected");
      }
      if (!approve) {
        const rejected = errorResult(tool.name, "tool approval rejected");
        terminalized = true;
        return { result: rejected, execution, events: execution.events(), updates: [] };
      }
    }
    if (signal.aborted) return cancelAndThrow(execution, signal.reason);
    execution.start();
    const executedResult = await tool.execute(context, argumentsValue);
    result = { ...executedResult, toolName: tool.name, toolCallId: request.toolCallId };
    const after = await options.hooks?.after?.(context, result);
    if (after && typeof after === "object" && "content" in after) result = after;
    execution.finish(toNativeResult(result));
    terminalized = true;
    return { result, execution, events: execution.events(), updates: [] };
  } catch (error) {
    return fail(error);
  }
}

export async function dispatchToolCall(
  session: NativeKernelSession,
  tools: ReadonlyMap<string, AnyTool> | Readonly<Record<string, AnyTool>>,
  request: ToolExecutionRequestWithContext,
  options?: ToolLifecycleOptions,
): Promise<ToolLifecycleResult> {
  const tool = tools instanceof Map ? tools.get(request.toolName) : (tools as Readonly<Record<string, AnyTool>>)[request.toolName];
  return executeTool(session, tool, request, options);
}

export function toolCallForRender<TArgs extends JsonValue>(request: ToolExecutionRequestWithContext, state: ToolLifecycleState, args?: TArgs): ToolCall<TArgs> {
  return { id: request.toolCallId, name: request.toolName, arguments: (args ?? request.arguments) as TArgs, turnId: request.turnId, messageId: request.messageId, state };
}

export function renderUnknownToolCall(call: ToolCall): ReturnType<typeof renderGenericCall> { return renderGenericCall(call); }
export function renderUnknownToolResult(result: ToolResult): ReturnType<typeof renderGenericResult> { return renderGenericResult(result); }

function createContext(request: ToolExecutionRequestWithContext, signal: AbortSignal, options: ToolLifecycleOptions, updates: NonNullable<ToolLifecycleOptions["updates"]>): ToolContext {
  return {
    sessionId: request.sessionId ?? request.turnId as never,
    turnId: request.turnId,
    messageId: request.messageId,
    toolCallId: request.toolCallId,
    cwd: options.cwd ?? request.cwd ?? process.cwd(),
    workspace: options.workspace ?? request.workspace ?? {},
    signal,
    updates,
    update: (update) => updates.send(update),
    approval: options.approval,
  };
}

function approvalRequirement(tool: AnyTool, toolName: string, args: JsonValue, policy: ToolLifecycleOptions["policy"]): ApprovalRequirement {
  const approval = tool.approval ?? tool.execution?.approval ?? tool.metadata?.approval;
  const base = approval === "alwaysAsk" ? { type: "required" as const } : approval && typeof approval === "object" ? approval : { type: "notRequired" as const };
  const contribution = policy ?? (typeof tool.policy === "object" && tool.policy !== null && "approval" in tool.policy ? tool.policy as ToolLifecycleOptions["policy"] : undefined);
  return contribution?.approval(toolName, args, base) ?? base;
}

function toNativeResult(result: ToolResult): ToolResult {
  return { content: result.content, details: result.details, isError: result.isError };
}

function errorResult(toolName: string, message: string): ToolResult {
  return { content: [{ type: "text", text: message }], details: {}, isError: true, toolName };
}

function unknownResult(toolName: string, args: JsonValue): ToolResult {
  return { content: [{ type: "text", text: `Unknown tool: ${toolName}` }], details: { toolName, arguments: args }, isError: true, toolName };
}

function cancelAndThrow(execution: NativeToolExecution, reason: unknown): never {
  execution.cancel(reason instanceof Error ? reason.message : "tool cancelled");
  throw abortError();
}