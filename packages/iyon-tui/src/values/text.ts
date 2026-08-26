import { StyleRef, StyleSpec } from "./style.ts";
import type { HorizontalAlign, TextSelectorValue, TextSpanValue, WrapMode } from "../types.ts";

export class TextSelector {
  private constructor(readonly value: TextSelectorValue = {}) {}

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

  private with(update: Partial<TextSelectorValue>): TextSelector {
    return new TextSelector({ ...this.value, ...update });
  }
}

export type { HorizontalAlign, TextSelectorValue, TextSpanValue, WrapMode } from "../types.ts";

export class TextSpan {
  readonly kind = "text-span" as const;

  constructor(readonly value: TextSpanValue) {}

  static plain(text: string): TextSpan {
    return new TextSpan({ text });
  }

  static styled(text: string, style: StyleRef | StyleSpec): TextSpan {
    return new TextSpan({ text, style: StyleRef.from(style) });
  }
}

export function textStyle(value: StyleRef | StyleSpec | undefined): StyleRef | undefined {
  return value === undefined ? undefined : StyleRef.from(value);
}
