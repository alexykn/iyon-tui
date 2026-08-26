import { View } from "./view.ts";

export type TextFormat = "plain" | "markdown";

export interface TextOrigin {
  readonly format: TextFormat;
  readonly source?: string;
}

export class TextContent {
  readonly kind = "text-content" as const;

  private constructor(readonly value: string, readonly origin: TextOrigin) {}

  static plain(value: string): TextContent { return new TextContent(value, { format: "plain" }); }
  static markdown(value: string): TextContent { return new TextContent(value, { format: "markdown" }); }
  static raw(value: string, origin: TextOrigin = { format: "plain" }): TextContent { return new TextContent(value, origin); }

  withOrigin(origin: TextOrigin): TextContent { return new TextContent(this.value, origin); }
  text(): string { return this.value; }
  render(): View { return View.text(this.value); }
  walk(visitor: (text: string) => void): void { visitor(this.value); }
  rewrite(rewriter: (text: string) => string): TextContent { return new TextContent(rewriter(this.value), this.origin); }
}

export class RawText {
  readonly kind = "raw-text" as const;
  constructor(readonly value: string, readonly origin: TextOrigin = { format: "plain" }) {}
  content(): TextContent { return TextContent.raw(this.value, this.origin); }
}
