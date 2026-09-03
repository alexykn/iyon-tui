import type { Output } from "../../controls/output.ts";
import type { View } from "../../view/view.ts";

declare const componentIdBrand: unique symbol;
/** Native host identity for a mounted component, distinct from a JS handle id. */
export type ComponentId = number & { readonly [componentIdBrand]: "ComponentId" };

export interface ComponentCapabilities {
  readonly focusable?: boolean;
  readonly modal?: boolean;
  readonly keys?: readonly string[];
  readonly paste?: boolean;
  readonly ticks?: boolean;
}

export interface KeyEvent {
  readonly type: "key";
  readonly key: string;
  readonly modifiers?: readonly string[];
}

export interface PasteEvent {
  readonly type: "paste";
  readonly text: string;
}

/** Result of a generic component interaction; emitted values use ComponentContext.emit(). */
export type InteractionResult =
  | { readonly type: "handled" }
  | { readonly type: "ignored" };

/** Borrow-scoped component event context with typed output channels. */
export interface ComponentContext {
  readonly componentId: ComponentId;
  emit<T>(output: Output<T>, payload: T): void;
}

export interface ComponentAdapter {
  view(context: ComponentContext): View | Promise<View>;
  capabilities?(context: ComponentContext): ComponentCapabilities | Promise<ComponentCapabilities>;
  onKey?(event: KeyEvent, context: ComponentContext): InteractionResult | Promise<InteractionResult>;
  onPaste?(event: PasteEvent, context: ComponentContext): InteractionResult | Promise<InteractionResult>;
  onTick?(context: ComponentContext): InteractionResult | Promise<InteractionResult>;
}

export class AsyncComponentAdapter {
  constructor(private readonly implementation: ComponentAdapter) {}
  view(context: ComponentContext): Promise<View> { return Promise.resolve().then(() => this.implementation.view(context)); }
  capabilities(context: ComponentContext): Promise<ComponentCapabilities> { return Promise.resolve().then(() => this.implementation.capabilities?.(context) ?? {}); }
  key(event: KeyEvent, context: ComponentContext): Promise<InteractionResult> { return Promise.resolve().then(() => this.implementation.onKey?.(event, context) ?? { type: "ignored" }); }
  paste(event: PasteEvent, context: ComponentContext): Promise<InteractionResult> { return Promise.resolve().then(() => this.implementation.onPaste?.(event, context) ?? { type: "ignored" }); }
  tick(context: ComponentContext): Promise<InteractionResult> { return Promise.resolve().then(() => this.implementation.onTick?.(context) ?? { type: "ignored" }); }
}
