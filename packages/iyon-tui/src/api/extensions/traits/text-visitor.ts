import type { TextContent } from "../../content/text-content.ts";

export interface TextVisitor {
  visit(content: TextContent): void | Promise<void>;
}

export class TextVisitorAdapter implements TextVisitor {
  constructor(private readonly implementation: TextVisitor) {}
  visit(content: TextContent): void | Promise<void> { return Promise.resolve().then(() => this.implementation.visit(content)); }
}
