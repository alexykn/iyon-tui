import { TextContent } from "./values/text-content.ts";
import { Projection } from "./values/projection.ts";
import { nativeTui } from "./handles.ts";

export class PlainTextProjector {
  private readonly native = nativeTui.plainProjector();

  project(content: TextContent): Projection {
    const result = this.native.project(content.text()) as {
      readonly spans: readonly { readonly sourceStart: number; readonly sourceEnd: number; readonly text: string }[];
    };
    return new Projection(content, result.spans);
  }

  dispose(): void {
    this.native.dispose();
  }
}

export class MarkdownProjector {
  private readonly native = nativeTui.markdownProjector();

  project(content: TextContent, sealed = true): Projection {
    const result = this.native.project(content.text(), sealed) as {
      readonly spans: readonly { readonly sourceStart: number; readonly sourceEnd: number; readonly text: string }[];
    };
    return new Projection(content, result.spans);
  }

  dispose(): void {
    this.native.dispose();
  }
}
