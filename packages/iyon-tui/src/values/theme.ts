import type {
  StyleSelectorValue,
  StyleSpecValue,
  TextSelectorValue,
  ThemeColor,
  ThemeDefinition,
  ThemeStyleEntry,
} from "../types.ts";
import {
  StyleSelector,
  StyleSpec,
  styleSelectorValue,
} from "./style.ts";
import type { TextSelector } from "./text.ts";
import { ThemeKey } from "./theme-key.ts";

interface ThemeEntry<T> {
  readonly base?: T;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: T }[];
}

interface TextThemeEntry {
  readonly selector: TextSelectorValue;
  readonly value: StyleSpecValue;
}

export { ThemeKey } from "./theme-key.ts";

export class Theme {
  readonly kind = "theme" as const;

  private constructor(
    private readonly styles: ReadonlyMap<string, ThemeEntry<StyleSpecValue>>,
    private readonly colors: ReadonlyMap<string, ThemeEntry<ThemeColor>>,
    private readonly textStyles: readonly TextThemeEntry[],
  ) {}

  static new(): Theme { return new Theme(new Map(), new Map(), []); }

  withStyle(key: string | ThemeKey, style: StyleSpec): Theme {
    const values = new Map(this.styles);
    values.set(themeKey(key), { base: style.value, variants: [] });
    return new Theme(values, this.colors, this.textStyles);
  }

  withStyleVariant(key: string | ThemeKey, selector: StyleSelector, style: StyleSpec): Theme {
    const name = themeKey(key);
    const current = this.styles.get(name);
    const values = new Map(this.styles);
    values.set(name, {
      base: current?.base,
      variants: [
        ...(current?.variants ?? []),
        { selector: styleSelectorValue(selector), value: style.value },
      ],
    });
    return new Theme(values, this.colors, this.textStyles);
  }

  withColor(key: string | ThemeKey, color: ThemeColor): Theme {
    const values = new Map(this.colors);
    values.set(themeKey(key), { base: color, variants: [] });
    return new Theme(this.styles, values, this.textStyles);
  }

  withColorVariant(key: string | ThemeKey, selector: StyleSelector, color: ThemeColor): Theme {
    const name = themeKey(key);
    const current = this.colors.get(name);
    const values = new Map(this.colors);
    values.set(name, {
      base: current?.base,
      variants: [
        ...(current?.variants ?? []),
        { selector: styleSelectorValue(selector), value: color },
      ],
    });
    return new Theme(this.styles, values, this.textStyles);
  }

  withTextStyle(selector: TextSelector, style: StyleSpec): Theme {
    return new Theme(
      this.styles,
      this.colors,
      [...this.textStyles, { selector: selector.value, value: style.value }],
    );
  }

  /** Returns the declared base style; named-style selection uses StyleRef. */
  style(key: string | ThemeKey): StyleSpec | undefined {
    const value = this.styles.get(themeKey(key))?.base;
    return value === undefined ? undefined : new StyleSpec(value);
  }

  /** Returns the color selected by an unconditional variant, if one exists. */
  color(key: string | ThemeKey): ThemeColor | undefined {
    const entry = this.colors.get(themeKey(key));
    if (entry === undefined) return undefined;
    let color = entry.base;
    for (const variant of entry.variants) {
      if (isUnconditional(variant.selector)) color = variant.value;
    }
    return color;
  }

  materialize(): ThemeDefinition {
    return {
      styles: Object.fromEntries(
        [...this.styles].map(([name, entry]): [string, ThemeStyleEntry] => [name, {
          ...(entry.base === undefined ? {} : { base: entry.base }),
          variants: entry.variants.map((variant) => ({
            selector: variant.selector,
            value: variant.value,
          })),
        }]),
      ),
      colors: Object.fromEntries([...this.colors].map(([name, entry]) => [name, {
        ...(entry.base === undefined ? {} : { base: entry.base }),
        variants: entry.variants.map((variant) => ({
          selector: variant.selector,
          value: variant.value,
        })),
      }])),
      textStyles: this.textStyles.map((entry) => ({
        selector: entry.selector,
        value: entry.value,
      })),
    };
  }
}

function themeKey(key: string | ThemeKey): string {
  return typeof key === "string" ? validateThemeKey(key) : key.value;
}

function validateThemeKey(key: string): string {
  if (key.length === 0) throw new RangeError("theme key cannot be empty");
  return key;
}

function isUnconditional(selector: StyleSelectorValue): boolean {
  return selector.focused !== true
    && selector.focusWithin !== true
    && (selector.states === undefined || Object.keys(selector.states).length === 0);
}

export type { ColorSpec, StyleSelectorValue, ThemeColor } from "../types.ts";
export { StyleSelector } from "./style.ts";
