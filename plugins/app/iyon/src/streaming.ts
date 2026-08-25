import { TextStream } from "@iyon/tui";
import type { StreamSnapshot } from "@iyon/tui";

export type SegmentKind = "text" | "thinking";
export interface StreamSegment { readonly kind: SegmentKind; readonly text: string; }

export class AssistantStreamBuffer {
  private segments: StreamSegment[] = [];
  private sealed = false;

  append(kind: SegmentKind, text: string): void {
    if (this.sealed) throw new Error("assistant stream is sealed");
    if (text.length === 0) return;
    const previous = this.segments.at(-1);
    if (previous?.kind === kind) this.segments[this.segments.length - 1] = { kind, text: previous.text + text };
    else this.segments.push({ kind, text });
  }
  snapshot(): readonly StreamSegment[] { return this.segments.map((segment) => ({ ...segment })); }
  text(): string { return this.segments.map((segment) => segment.text).join(""); }
  seal(): void { this.sealed = true; }
  isSealed(): boolean { return this.sealed; }
}

export class NativeAssistantStream {
  readonly native: TextStream;
  readonly buffer = new AssistantStreamBuffer();

  constructor() {
    this.native = new TextStream({
      projector: "markdown",
      presentation: { insets: { top: 0, right: 2, bottom: 0, left: 2 } },
      pacing: { minUnitsPerSecond: 40, maxUnitsPerSecond: 800 },
    });
  }
  async append(kind: SegmentKind, text: string): Promise<void> {
    if (text.length === 0) return;
    const previous = this.buffer.snapshot().at(-1);
    const normalized = kind === "text"
      && previous?.kind === "thinking"
      && !previous.text.endsWith("\n")
      && !text.startsWith("\n")
      ? `\n\n${text}`
      : text;
    this.buffer.append(kind, normalized);
    await this.native.append(
      normalized,
      kind === "thinking" ? [{ namespace: "app", name: "thinking" }] : [],
    );
  }
  async snapshot(): Promise<StreamSnapshot> { return this.native.snapshot(); }
  async seal(): Promise<void> { this.buffer.seal(); await this.native.seal(); }
  async dispose(): Promise<void> { await this.native.dispose(); }
}
