import type { StreamSnapshot, StreamingSource } from "../types.ts";

export class StreamingSourceAdapter implements StreamingSource {
  constructor(private readonly implementation: StreamingSource) {}
  snapshot(): StreamSnapshot | Promise<StreamSnapshot> { return Promise.resolve().then(() => this.implementation.snapshot()); }
  advance(): boolean | Promise<boolean> { return Promise.resolve().then(() => this.implementation.advance()); }
  seal(): void | Promise<void> { return Promise.resolve().then(() => this.implementation.seal()); }
  compact(): void | Promise<void> { return Promise.resolve().then(() => this.implementation.compact?.()); }
}
