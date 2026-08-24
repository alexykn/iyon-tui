import {
  KernelSession as NativeKernelSession,
  native,
  type JsonValue,
  type NativeKernelSessionContract,
  type NativeModelTurnContract,
  type NativeToolExecutionContract,
} from "../native.ts";
import type {
  AgentSession as AgentSessionContract,
  CoreEvent,
  KernelSession as KernelSessionContract,
  MessageId,
  ModelTurnOptions,
  ModelTurnResult,
  SessionEntry,
  SessionSnapshot,
  ToolExecutionRequest,
  ApprovalState,
  ToolLifecycleEvent,
  ToolLifecycleState,
  ToolResult,
  ToolUpdateEvent,
} from "../../../iyon-sdk/src/core.ts";
import type { ModelError, ModelStreamEvent } from "../../../iyon-sdk/src/api.ts";
import { eventsFromNextEvent } from "./async-events.ts";
import { runWithAbortSignal } from "./abort.ts";

function jsonValue(value: unknown): JsonValue {
  return value as JsonValue;
}

export class ModelTurn {
  constructor(private readonly handle: NativeModelTurnContract) {}

  push(event: ModelStreamEvent, signal?: AbortSignal): Promise<void> {
    return runWithAbortSignal(signal, {
      run: () => this.handle.push(jsonValue(event)),
      cancel: () => this.handle.cancel().then(() => undefined),
    });
  }

  pushMany(events: ModelStreamEvent[], signal?: AbortSignal): Promise<void> {
    return runWithAbortSignal(signal, {
      run: () => this.handle.pushMany(events.map(jsonValue)),
      cancel: () => this.handle.cancel().then(() => undefined),
    });
  }

  finish(): Promise<ModelTurnResult> {
    return this.handle.finish() as unknown as Promise<ModelTurnResult>;
  }

  fail(error: ModelError | string): Promise<void> {
    return this.handle.fail(jsonValue(error));
  }

  cancel(): Promise<ModelTurnResult> {
    return this.handle.cancel() as unknown as Promise<ModelTurnResult>;
  }
}

export class ToolExecution {
  constructor(private readonly handle: NativeToolExecutionContract) {}

  state(): ToolLifecycleState {
    return this.handle.state() as ToolLifecycleState;
  }

  events(): ToolLifecycleEvent[] {
    return this.handle.events() as unknown as ToolLifecycleEvent[];
  }

  prepared(argumentsValue: unknown): void {
    this.handle.prepared(jsonValue(argumentsValue));
  }

  start(): void {
    this.handle.start();
  }

  sendUpdate(update: ToolUpdateEvent): void {
    this.handle.sendUpdate(jsonValue(update));
  }

  requestApproval(requirement?: unknown): ApprovalState | null {
    return this.handle.requestApproval(jsonValue(requirement)) as unknown as ApprovalState | null;
  }

  approve(approvalId: number): void {
    this.handle.approve(approvalId);
  }

  reject(approvalId: number, reason?: string): void {
    this.handle.reject(approvalId, reason);
  }

  finish(result: ToolResult): void {
    this.handle.finish(jsonValue(result));
  }

  fail(error: string): void {
    this.handle.fail(error);
  }

  cancel(reason?: string): void {
    this.handle.cancel(reason);
  }
}

export class KernelSession implements KernelSessionContract {
  protected readonly handle: NativeKernelSessionContract;

  constructor(options?: { id?: number; eventCapacity?: number }) {
    this.handle = options === undefined
      ? new NativeKernelSession()
      : new NativeKernelSession(jsonValue(options));
  }

  snapshot(): SessionSnapshot {
    return this.handle.snapshot() as unknown as SessionSnapshot;
  }

  appendMessage(message: unknown): MessageId {
    return this.handle.appendMessage(jsonValue(message)) as MessageId;
  }

  deliverUserMessage(text: string): MessageId {
    return this.handle.deliverUserMessage(text) as MessageId;
  }

  appendEntry(entry: SessionEntry): void {
    this.handle.appendEntry(jsonValue(entry));
  }

  nextEvent(): Promise<CoreEvent | null> {
    return this.handle.nextEvent() as Promise<CoreEvent | null>;
  }

  nextEvents(max = 64): Promise<CoreEvent[]> {
    return this.handle.nextEvents(max) as Promise<CoreEvent[]>;
  }

  events(): AsyncIterable<CoreEvent> {
    return eventsFromNextEvent(() => this.nextEvent());
  }

  beginModelTurn(options: ModelTurnOptions): ModelTurn {
    return new ModelTurn(this.handle.beginModelTurn(jsonValue(options)));
  }

  prepareToolExecution(request: ToolExecutionRequest): ToolExecution {
    return new ToolExecution(this.handle.prepareToolExecution(jsonValue(request)));
  }

  enqueue(kind: "prompt" | "steer" | "followUp", text: string): Promise<number> {
    return Promise.resolve(this.handle.enqueue(kind, text));
  }

  dequeue(kind: "prompt" | "steer" | "followUp"): string | null {
    return this.handle.dequeue(kind);
  }

  queueSnapshot(): Record<string, number | boolean> {
    return this.handle.queueSnapshot() as Record<string, number | boolean>;
  }

  abort(): void {
    this.handle.abort();
  }

  close(): void {
    this.handle.close();
  }
}

export class AgentSession extends KernelSession implements AgentSessionContract {
  private model = "";
  private reasoning = "";
  private activeTools: string[] = [];

  async prompt(text: string, options?: { signal?: AbortSignal }): Promise<ModelTurn> {
    this.appendMessage({
      kind: "message",
      role: "user",
      content: [{ type: "text", text }],
    });
    return this.beginModelTurn({
      request: {
        messages: [{ role: "user", content: [{ type: "text", text }] }],
        tools: this.activeTools.map((name) => ({ name, description: "", inputSchema: {} })),
        params: this.reasoning ? { reasoning: this.reasoning as never } : {},
        metadata: this.model ? { sessionId: this.model } : {},
      },
      signal: options?.signal,
    });
  }

  steer(text: string): Promise<number> {
    return this.enqueue("steer", text);
  }

  followUp(text: string): Promise<number> {
    return this.enqueue("followUp", text);
  }

  setModel(model: string): void {
    this.model = model;
  }

  setReasoning(reasoning: string): void {
    this.reasoning = reasoning;
  }

  setActiveTools(tools: string[]): void {
    this.activeTools = [...tools];
  }
}

export { IyonNativeError, asIyonError, isCancelledError } from "./errors.ts";
