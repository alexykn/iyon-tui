import type { TextContent } from "../../content/text-content.ts";

export interface TextRewriter {
  rewrite(content: TextContent): TextContent | Promise<TextContent>;
}

export class TextRewriterAdapter implements TextRewriter {
  constructor(private readonly implementation: TextRewriter) {}
  rewrite(content: TextContent): TextContent | Promise<TextContent> { return Promise.resolve().then(() => this.implementation.rewrite(content)); }
}
