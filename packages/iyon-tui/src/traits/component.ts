import type { ComponentAdapter, ComponentCapabilities, ComponentContext, InteractionResult, KeyEvent, PasteEvent, View } from "../types.ts";

export class ComponentAdapterBridge {
  constructor(private readonly implementation: ComponentAdapter) {}
  view(context: ComponentContext): Promise<View> { return Promise.resolve().then(() => this.implementation.view(context)); }
  capabilities(context: ComponentContext): Promise<ComponentCapabilities> { return Promise.resolve().then(() => this.implementation.capabilities?.(context) ?? {}); }
  key(event: KeyEvent, context: ComponentContext): Promise<InteractionResult> { return Promise.resolve().then(() => this.implementation.onKey?.(event, context) ?? { type: "ignored" }); }
  paste(event: PasteEvent, context: ComponentContext): Promise<InteractionResult> { return Promise.resolve().then(() => this.implementation.onPaste?.(event, context) ?? { type: "ignored" }); }
  tick(context: ComponentContext): Promise<InteractionResult> { return Promise.resolve().then(() => this.implementation.onTick?.(context) ?? { type: "ignored" }); }
}
