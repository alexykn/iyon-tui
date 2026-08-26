import type { RenderContext, Renderer, View } from "../types.ts";

export class RendererAdapter implements Renderer {
  constructor(private readonly implementation: Renderer) {}
  render(view: View, context?: RenderContext): View | Promise<View> { return queue(() => this.implementation.render(view, context)); }
}

function queue<T>(callback: () => T | Promise<T>): Promise<T> {
  return Promise.resolve().then(callback);
}
