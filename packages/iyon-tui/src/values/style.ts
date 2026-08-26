import type { ColorNode, StyleNode } from "../ir.ts";

export type Color = ColorNode;

export class StyleSpec {
  readonly kind = "style" as const;

  constructor(readonly value: StyleNode = { attributes: {} }) {}

  theme(key: string): StyleSpec {
    if (key.length === 0) throw new RangeError("style theme key cannot be empty");
    return new StyleSpec({ ...this.value, theme: key });
  }

  foreground(color: Color): StyleSpec {
    return new StyleSpec({ ...this.value, foreground: color });
  }

  background(color: Color): StyleSpec {
    return new StyleSpec({ ...this.value, background: color });
  }

  attribute(name: string, enabled = true): StyleSpec {
    return new StyleSpec({ ...this.value, attributes: { ...this.value.attributes, [name]: enabled } });
  }

  bold(): StyleSpec { return this.attribute("bold"); }
  dim(): StyleSpec { return this.attribute("dim"); }
  italic(): StyleSpec { return this.attribute("italic"); }
  underline(): StyleSpec { return this.attribute("underline"); }
  reversed(): StyleSpec { return this.attribute("reversed"); }
  strikethrough(): StyleSpec { return this.attribute("strikethrough"); }
  plain(): StyleSpec { return new StyleSpec({ ...this.value, attributes: Object.fromEntries(["bold", "dim", "italic", "underline", "reversed", "strikethrough"].map((key) => [key, false])) }); }
}

export const Style = {
  plain: (): StyleSpec => new StyleSpec().plain(),
  new: (): StyleSpec => new StyleSpec(),
};
