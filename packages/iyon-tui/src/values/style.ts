import type {
  AnsiColor,
  ColorSpec,
  StyleSelectorValue,
  StyleSpecValue,
  TextAttribute,
} from "../types.ts";
import { ThemeKey } from "./theme-key.ts";

export class StyleSpec {
  readonly kind = "style" as const;

  constructor(readonly value: StyleSpecValue = { attributes: {} }) {}

  foreground(color: ColorSpec): StyleSpec {
    return new StyleSpec({ ...this.value, foreground: color });
  }

  background(color: ColorSpec): StyleSpec {
    return new StyleSpec({ ...this.value, background: color });
  }

  attribute(name: TextAttribute, enabled = true): StyleSpec {
    validateTextAttribute(name);
    return new StyleSpec({ ...this.value, attributes: { ...this.value.attributes, [name]: enabled } });
  }

  bold(): StyleSpec { return this.attribute("bold"); }
  dim(): StyleSpec { return this.attribute("dim"); }
  italic(): StyleSpec { return this.attribute("italic"); }
  underline(): StyleSpec { return this.attribute("underline"); }
  reversed(): StyleSpec { return this.attribute("reversed"); }
  strikethrough(): StyleSpec { return this.attribute("strikethrough"); }
  plain(): StyleSpec {
    return new StyleSpec({
      ...this.value,
      attributes: Object.fromEntries(
        ["bold", "dim", "italic", "underline", "reversed", "strikethrough"].map((key) => [key, false]),
      ),
    });
  }
}

/** Semantic named-style identity plus an optional sparse local override. */
export class StyleRef {
  readonly kind = "style-ref" as const;

  private constructor(
    readonly themeKey: ThemeKey | undefined,
    readonly local: StyleSpec,
  ) {}

  static direct(style: StyleSpec = new StyleSpec()): StyleRef {
    return new StyleRef(undefined, style);
  }

  static theme(key: string | ThemeKey): StyleRef {
    return new StyleRef(toThemeKey(key), new StyleSpec());
  }

  static themed(key: string | ThemeKey, overrides: StyleSpec = new StyleSpec()): StyleRef {
    return new StyleRef(toThemeKey(key), overrides);
  }

  static from(style: StyleSpec | StyleRef): StyleRef {
    return style.kind === "style-ref" ? style : StyleRef.direct(style);
  }

  overrides(patch: StyleSpec): StyleRef {
    return new StyleRef(this.themeKey, new StyleSpec(mergeStyleValues(this.local.value, patch.value)));
  }
}

/** Semantic state dimension name used by StyleSelector and View.styleState. */
export class StyleStateKey {
  readonly kind = "style-state-key" as const;

  constructor(readonly value: string) {
    if (typeof value !== "string" || value.length === 0) throw new RangeError("style state key cannot be empty");
  }

  static from(value: string): StyleStateKey { return new StyleStateKey(value); }
}

/** Semantic state dimension value used by StyleSelector and View.styleState. */
export class StyleStateValue {
  readonly kind = "style-state-value" as const;

  constructor(readonly value: string) {
    if (typeof value !== "string" || value.length === 0) throw new RangeError("style state value cannot be empty");
  }

  static from(value: string): StyleStateValue { return new StyleStateValue(value); }
}

/** Positive conjunction of focus predicates and application-owned state facts. */
export class StyleSelector {
  readonly kind = "style-selector" as const;

  private constructor(readonly value: StyleSelectorValue = {}) {}

  static any(): StyleSelector { return new StyleSelector(); }
  static focused(): StyleSelector { return StyleSelector.any().andFocused(); }
  static focusWithin(): StyleSelector { return StyleSelector.any().andFocusWithin(); }
  static state(key: string | StyleStateKey, value: string | StyleStateValue): StyleSelector {
    return StyleSelector.any().andState(key, value);
  }

  andFocused(): StyleSelector { return this.with({ focused: true }); }
  andFocusWithin(): StyleSelector { return this.with({ focusWithin: true }); }
  andState(key: string | StyleStateKey, value: string | StyleStateValue): StyleSelector {
    const stateKey = stateKeyValue(key);
    const stateValue = stateValueValue(value);
    return this.with({ states: { ...(this.value.states ?? {}), [stateKey]: stateValue } });
  }

  private with(update: Partial<StyleSelectorValue>): StyleSelector {
    return new StyleSelector({ ...this.value, ...update });
  }
}

/** Converts a selector facade to the plain host-bound selector value. */
export function styleSelectorValue(selector: StyleSelector): StyleSelectorValue {
  return {
    ...selector.value,
    ...(selector.value.states === undefined ? {} : { states: { ...selector.value.states } }),
  };
}

function toThemeKey(key: string | ThemeKey): ThemeKey {
  if (typeof key === "string") return new ThemeKey(key);
  if (typeof key.value !== "string" || key.value.length === 0) {
    throw new RangeError("theme key cannot be empty");
  }
  return key;
}

function stateKeyValue(key: string | StyleStateKey): string {
  const value = typeof key === "string" ? new StyleStateKey(key).value : key.value;
  if (typeof value !== "string" || value.length === 0) throw new RangeError("style state key cannot be empty");
  return value;
}

function stateValueValue(value: string | StyleStateValue): string {
  const result = typeof value === "string" ? new StyleStateValue(value).value : value.value;
  if (typeof result !== "string" || result.length === 0) throw new RangeError("style state value cannot be empty");
  return result;
}

function mergeStyleValues(base: StyleSpecValue, patch: StyleSpecValue): StyleSpecValue {
  return {
    ...(base.foreground === undefined && patch.foreground === undefined
      ? {}
      : { foreground: patch.foreground ?? base.foreground }),
    ...(base.background === undefined && patch.background === undefined
      ? {}
      : { background: patch.background ?? base.background }),
    attributes: { ...base.attributes, ...patch.attributes },
  };
}

/** @internal Validates the closed native text-attribute vocabulary. */
export function validateTextAttribute(name: string): TextAttribute {
  if (!TEXT_ATTRIBUTES.has(name as TextAttribute)) {
    throw new RangeError(`unknown text attribute ${JSON.stringify(name)}`);
  }
  return name as TextAttribute;
}

const TEXT_ATTRIBUTES = new Set<TextAttribute>([
  "bold",
  "dim",
  "italic",
  "underline",
  "reversed",
  "strikethrough",
]);

export type { AnsiColor } from "../types.ts";

export const Style = {
  plain: (): StyleSpec => new StyleSpec().plain(),
  new: (): StyleSpec => new StyleSpec(),
};
