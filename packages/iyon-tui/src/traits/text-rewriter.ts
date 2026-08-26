import type { TextContent, TextRewriter } from "../types.ts";

export class TextRewriterAdapter implements TextRewriter {
  constructor(private readonly implementation: TextRewriter) {}
  rewrite(content: TextContent): TextContent | Promise<TextContent> { return Promise.resolve().then(() => this.implementation.rewrite(content)); }
}
