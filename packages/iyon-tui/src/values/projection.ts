import { TextContent } from "./text-content.ts";

export interface ProjectionSpan {
  readonly sourceStart: number;
  readonly sourceEnd: number;
  readonly text: string;
}

export class Projection {
  readonly kind = "projection" as const;
  constructor(readonly source: TextContent, readonly spans: readonly ProjectionSpan[]) { validateSpans(spans); }
  text(): string { return this.spans.map((span) => span.text).join(""); }
  sourceRange(): { readonly start: number; readonly end: number } { return { start: this.spans[0]?.sourceStart ?? 0, end: this.spans.at(-1)?.sourceEnd ?? 0 }; }
}

export class ProjectionBuilder {
  private readonly spans: ProjectionSpan[] = [];
  constructor(private readonly source: TextContent) {}
  span(sourceStart: number, sourceEnd: number, text: string): this { this.spans.push({ sourceStart, sourceEnd, text }); return this; }
  finish(): Projection { return new Projection(this.source, this.spans); }
}

export class Smooth {
  readonly kind = "smooth" as const;
  constructor(readonly through = 0) { if (!Number.isInteger(through) || through < 0) throw new RangeError("smooth offset must be non-negative"); }
}

function validateSpans(spans: readonly ProjectionSpan[]): void {
  let expected = 0;
  for (const span of spans) {
    if (!Number.isInteger(span.sourceStart) || !Number.isInteger(span.sourceEnd) || span.sourceStart < expected || span.sourceEnd < span.sourceStart) throw new RangeError("projection spans must be contiguous and ordered");
    expected = span.sourceEnd;
  }
}
