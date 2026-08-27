import { StyleRef, StyleSpec, StyleStateKey, StyleStateValue } from "./style.ts";
import type { HorizontalAlign, TextPart, TextRole, TextSelectorValue, TextSpanValue, WrapMode } from "../types.ts";

export class TextSelector {
  private constructor(readonly value: TextSelectorValue = {}) {}

  static any(): TextSelector { return new TextSelector(); }
  static role(role: TextRole): TextSelector { return TextSelector.any().role(role); }
  static part(part: TextPart): TextSelector { return TextSelector.any().part(part); }
  static annotation(namespace: string, name: string): TextSelector { return TextSelector.any().annotation(namespace, name); }
  static heading(): TextSelector { return TextSelector.role("heading"); }
  static inlineCode(): TextSelector { return TextSelector.role("inlineCode"); }
  static codeBlock(): TextSelector { return TextSelector.role("codeBlock"); }

  role(role: TextRole): TextSelector {
    validateTextRole(role);
    return this.with({ roles: [...(this.value.roles ?? []), role] });
  }
  part(part: TextPart): TextSelector {
    validateTextPart(part);
    return this.with({ parts: [...(this.value.parts ?? []), part] });
  }
  annotation(namespace: string, name: string): TextSelector {
    validateTextName(namespace, "annotation namespace");
    validateTextName(name, "annotation name");
    return this.with({ annotations: [...(this.value.annotations ?? []), { namespace, name }] });
  }
  language(language: string): TextSelector {
    validateTextName(language, "language");
    return this.with({ language });
  }
  origin(origin: string): TextSelector {
    validateTextName(origin, "text origin");
    return this.with({ origin });
  }
  format(format: string): TextSelector {
    validateTextName(format, "text format");
    return this.with({ format });
  }
  andFocused(): TextSelector { return this.with({ focused: true }); }
  andFocusWithin(): TextSelector { return this.with({ focusWithin: true }); }
  andState(key: string | StyleStateKey, value: string | StyleStateValue): TextSelector {
    const stateKey = typeof key === "string" ? key : key.value;
    const stateValue = typeof value === "string" ? value : value.value;
    return this.with({ states: { ...(this.value.states ?? {}), [stateKey]: stateValue } });
  }

  private with(update: Partial<TextSelectorValue>): TextSelector {
    return new TextSelector({ ...this.value, ...update });
  }
}

function validateTextRole(role: string): TextRole {
  if (!TEXT_ROLES.has(role as TextRole)) throw new RangeError(`unknown text role ${JSON.stringify(role)}`);
  return role as TextRole;
}

function validateTextPart(part: string): TextPart {
  if (!TEXT_PARTS.has(part as TextPart)) throw new RangeError(`unknown text part ${JSON.stringify(part)}`);
  return part as TextPart;
}

function validateTextName(value: string, label: string): void {
  if (value.length === 0 || /\s/u.test(value)) {
    throw new RangeError(`${label} must be non-empty and contain no whitespace`);
  }
}

const TEXT_ROLES = new Set<TextRole>([
  "paragraph",
  "heading",
  "blockQuote",
  "list",
  "listItem",
  "codeBlock",
  "table",
  "tableRow",
  "tableCell",
  "thematicBreak",
  "rawBlock",
  "container",
  "strong",
  "emphasis",
  "strikethrough",
  "underline",
  "superscript",
  "subscript",
  "smallCaps",
  "inlineCode",
  "link",
  "image",
  "rawInline",
]);

const TEXT_PARTS = new Set<TextPart>([
  "listMarker",
  "taskMarker",
  "quoteMarker",
  "codeLabel",
  "tableRule",
  "thematicRule",
  "imageFallback",
]);

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
