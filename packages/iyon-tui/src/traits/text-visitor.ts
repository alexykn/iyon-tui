import type { TextContent, TextVisitor } from "../types.ts";

export class TextVisitorAdapter implements TextVisitor {
  constructor(private readonly implementation: TextVisitor) {}
  visit(content: TextContent): void | Promise<void> { return Promise.resolve().then(() => this.implementation.visit(content)); }
}
