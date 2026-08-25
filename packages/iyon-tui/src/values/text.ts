import type { StyleSpec } from "./style.ts";
import type { StyleNode, TextSpanNode } from "../ir.ts";

export interface TextSelectorNode {
  readonly focused?: boolean;
  readonly focusWithin?: boolean;
  readonly states?: Readonly<Record<string, string>>;
  readonly roles?: readonly string[];
  readonly parts?: readonly string[];
  readonly annotations?: readonly { readonly namespace: string; readonly name: string }[];
  readonly language?: string;
  readonly origin?: string;
  readonly format?: string;
}

export class TextSelector {
  private constructor(readonly value: TextSelectorNode = {}) {}

  static any(): TextSelector { return new TextSelector(); }
  static role(role: string): TextSelector { return TextSelector.any().role(role); }
  static part(part: string): TextSelector { return TextSelector.any().part(part); }
  static annotation(namespace: string, name: string): TextSelector { return TextSelector.any().annotation(namespace, name); }
  static heading(): TextSelector { return TextSelector.role("heading"); }
  static inlineCode(): TextSelector { return TextSelector.role("inlineCode"); }
  static codeBlock(): TextSelector { return TextSelector.role("codeBlock"); }

  role(role: string): TextSelector { return this.with({ roles: [...(this.value.roles ?? []), role] }); }
  part(part: string): TextSelector { return this.with({ parts: [...(this.value.parts ?? []), part] }); }
  annotation(namespace: string, name: string): TextSelector {
    return this.with({ annotations: [...(this.value.annotations ?? []), { namespace, name }] });
  }
  language(language: string): TextSelector { return this.with({ language }); }
  origin(origin: string): TextSelector { return this.with({ origin }); }
  format(format: string): TextSelector { return this.with({ format }); }
  andFocused(): TextSelector { return this.with({ focused: true }); }
  andFocusWithin(): TextSelector { return this.with({ focusWithin: true }); }
  andState(key: string, value: string): TextSelector {
    return this.with({ states: { ...(this.value.states ?? {}), [key]: value } });
  }

  private with(update: Partial<TextSelectorNode>): TextSelector {
    return new TextSelector({ ...this.value, ...update });
  }
}

export type WrapMode = "wordThenGrapheme" | "grapheme" | "noWrap";
export type HorizontalAlign = "start" | "center" | "end";

export class TextSpan {
  readonly kind = "text-span" as const;

  constructor(readonly value: TextSpanNode) {}

  static plain(text: string): TextSpan {
    return new TextSpan({ text });
  }

  static styled(text: string, style: StyleSpec): TextSpan {
    return new TextSpan({ text, style: style.value });
  }
}

export function textStyle(value: StyleSpec | undefined): StyleNode | undefined {
  return value?.value;
}
