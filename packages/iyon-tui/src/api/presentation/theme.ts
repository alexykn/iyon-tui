import type { StyleSelectorValue, StyleSpecValue } from "./style.ts";
import {
  type StyleSelector,
  StyleSpec,
  styleSelectorValue,
} from "./style.ts";
import type { TextSelectorValue } from "../content/text.ts";
import type { TextSelector } from "../content/text.ts";
import type { ThemeKey } from "./theme-key.ts";

export type AnsiColor =
  | "black"
  | "red"
  | "green"
  | "yellow"
  | "blue"
  | "magenta"
  | "cyan"
  | "gray"
  | "darkGray"
  | "lightRed"
  | "lightGreen"
  | "lightYellow"
  | "lightBlue"
  | "lightMagenta"
  | "lightCyan"
  | "white";

/** A resolved color value stored in a Theme definition. */
export interface ThemeColorDefault {
  readonly type: "default";
}

export interface ThemeColorNamed {
  readonly type: "named";
  readonly value: AnsiColor;
}

export interface ThemeColorIndexed {
  readonly type: "indexed";
  readonly value: number;
}

export interface RgbColor {
  readonly type: "rgb";
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

export type ThemeColor = ThemeColorDefault | ThemeColorNamed | ThemeColorIndexed | RgbColor;

/** A semantic reference resolved by the active Theme. */
export interface ThemeColorReference {
  readonly type: "theme";
  readonly key: string | ThemeKey;
}

/** Explicit theme reference, named ANSI, indexed ANSI, or RGB color. */
export type ColorSpec = ThemeColorReference | ThemeColorNamed | ThemeColorIndexed | RgbColor;

interface ThemeEntry<T> {
  readonly base?: T;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: T }[];
}

interface TextThemeEntry {
  readonly selector: TextSelectorValue;
  readonly value: StyleSpecValue;
}

interface ThemeStyleEntry {
  readonly base?: StyleSpecValue;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: StyleSpecValue }[];
}

interface ThemeColorEntry {
  readonly base?: ThemeColor;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: ThemeColor }[];
}

interface ThemeDefinition {
  readonly styles: Readonly<Record<string, ThemeStyleEntry>>;
  readonly colors: Readonly<Record<string, ThemeColorEntry>>;
  readonly textStyles: readonly TextThemeEntry[];
}

export { ThemeKey } from "./theme-key.ts";

/** Creates an explicit semantic reference to a named theme color. */
export function themeColor(key: string | ThemeKey): ThemeColorReference {
  return { type: "theme", key: themeKey(key) };
}

interface ThemeData {
  readonly styles: ReadonlyMap<string, ThemeEntry<StyleSpecValue>>;
  readonly colors: ReadonlyMap<string, ThemeEntry<ThemeColor>>;
  readonly textStyles: readonly TextThemeEntry[];
}

const themeData = new WeakMap<Theme, ThemeData>();

export class Theme {
  readonly kind = "theme" as const;

  private constructor(
    private readonly styles: ReadonlyMap<string, ThemeEntry<StyleSpecValue>>,
    private readonly colors: ReadonlyMap<string, ThemeEntry<ThemeColor>>,
    private readonly textStyles: readonly TextThemeEntry[],
  ) {
    themeData.set(this, { styles, colors, textStyles });
  }

  static new(): Theme { return new Theme(new Map(), new Map(), []); }

  withStyle(key: string | ThemeKey, style: StyleSpec): Theme {
    const name = themeKey(key);
    const current = this.styles.get(name);
    const values = new Map(this.styles);
    values.set(name, { base: style.value, variants: current?.variants ?? [] });
    return new Theme(values, this.colors, this.textStyles);
  }

  withStyleVariant(key: string | ThemeKey, selector: StyleSelector, style: StyleSpec): Theme {
    const name = themeKey(key);
    const current = this.styles.get(name);
    const selectorValue = styleSelectorValue(selector);
    const values = new Map(this.styles);
    values.set(name, {
      base: current?.base,
      variants: [
        ...(current?.variants ?? []).filter((variant) => !styleSelectorsEqual(variant.selector, selectorValue)),
        { selector: selectorValue, value: style.value },
      ],
    });
    return new Theme(values, this.colors, this.textStyles);
  }

  withColor(key: string | ThemeKey, color: ThemeColor): Theme {
    const name = themeKey(key);
    const current = this.colors.get(name);
    const values = new Map(this.colors);
    values.set(name, { base: color, variants: current?.variants ?? [] });
    return new Theme(this.styles, values, this.textStyles);
  }

  withColorVariant(key: string | ThemeKey, selector: StyleSelector, color: ThemeColor): Theme {
    const name = themeKey(key);
    const current = this.colors.get(name);
    const selectorValue = styleSelectorValue(selector);
    const values = new Map(this.colors);
    values.set(name, {
      base: current?.base,
      variants: [
        ...(current?.variants ?? []).filter((variant) => !styleSelectorsEqual(variant.selector, selectorValue)),
        { selector: selectorValue, value: color },
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
}

/** @internal Projects a Theme for the private native-boundary lowering. */
export function themeDefinitionFor(theme: Theme): ThemeDefinition {
  const data = themeData.get(theme);
  if (data === undefined) throw new TypeError("value is not a framework Theme");
  return {
    styles: Object.fromEntries(
      [...data.styles].map(([name, entry]): [string, ThemeStyleEntry] => [name, {
        ...(entry.base === undefined ? {} : { base: entry.base }),
        variants: entry.variants.map((variant) => ({
          selector: variant.selector,
          value: variant.value,
        })),
      }]),
    ),
    colors: Object.fromEntries([...data.colors].map(([name, entry]) => [name, {
      ...(entry.base === undefined ? {} : { base: entry.base }),
      variants: entry.variants.map((variant) => ({
        selector: variant.selector,
        value: variant.value,
      })),
    }])),
    textStyles: data.textStyles.map((entry) => ({
      selector: entry.selector,
      value: entry.value,
    })),
  };
}

function themeKey(key: string | ThemeKey): string {
  return typeof key === "string" ? validateThemeKey(key) : validateThemeKey(key.value);
}

function validateThemeKey(key: string): string {
  if (typeof key !== "string" || key.length === 0) throw new RangeError("theme key cannot be empty");
  return key;
}

function isUnconditional(selector: StyleSelectorValue): boolean {
  return selector.focused !== true
    && selector.focusWithin !== true
    && (selector.states === undefined || Object.keys(selector.states).length === 0);
}

function styleSelectorsEqual(left: StyleSelectorValue, right: StyleSelectorValue): boolean {
  if ((left.focused === true) !== (right.focused === true)
    || (left.focusWithin === true) !== (right.focusWithin === true)) return false;
  const leftStates = left.states ?? {};
  const rightStates = right.states ?? {};
  const leftKeys = Object.keys(leftStates);
  if (leftKeys.length !== Object.keys(rightStates).length) return false;
  return leftKeys.every((key) => leftStates[key] === rightStates[key]);
}

export type { StyleSelectorValue } from "./style.ts";
export { StyleSelector } from "./style.ts";
