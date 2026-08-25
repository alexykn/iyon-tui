import type { Projector, TextContent } from "../types.ts";

export class ProjectorAdapter implements Projector {
  constructor(private readonly implementation: Projector) {}
  project(content: TextContent): TextContent | Promise<TextContent> { return Promise.resolve().then(() => this.implementation.project(content)); }
}
