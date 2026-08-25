import type { ColorNode, StyleNode } from "../ir.ts";
import { StyleSpec } from "./style.ts";
import type { TextSelector, TextSelectorNode } from "./text.ts";

export interface ThemeSelector {
  readonly focused?: boolean;
  readonly focusWithin?: boolean;
  readonly states?: Readonly<Record<string, string>>;
}

interface ThemeEntry<T> {
  readonly base?: T;
  readonly variants: readonly { readonly selector: ThemeSelector; readonly value: T }[];
}

interface TextThemeEntry {
  readonly selector: TextSelectorNode;
  readonly value: StyleNode;
}

export class ThemeKey {
  readonly kind = "theme-key" as const;
  constructor(readonly value: string) { if (value.length === 0) throw new RangeError("theme key cannot be empty"); }
}

export class Theme {
  readonly kind = "theme" as const;
  private constructor(
    private readonly styles: ReadonlyMap<string, ThemeEntry<StyleNode>>,
    private readonly colors: ReadonlyMap<string, ThemeEntry<ColorNode>>,
    private readonly textStyles: readonly TextThemeEntry[],
  ) {}
  static new(): Theme { return new Theme(new Map(), new Map(), []); }
  withStyle(key: string | ThemeKey, style: StyleSpec): Theme {
    const values = new Map(this.styles);
    values.set(themeKey(key), { base: style.value, variants: [] });
    return new Theme(values, this.colors, this.textStyles);
  }
  withStyleVariant(key: string | ThemeKey, selector: ThemeSelector, style: StyleSpec): Theme {
    const name = themeKey(key);
    const current = this.styles.get(name);
    const values = new Map(this.styles);
    values.set(name, { base: current?.base, variants: [...(current?.variants ?? []), { selector, value: style.value }] });
    return new Theme(values, this.colors, this.textStyles);
  }
  withColor(key: string | ThemeKey, color: ColorNode): Theme {
    const values = new Map(this.colors);
    values.set(themeKey(key), { base: color, variants: [] });
    return new Theme(this.styles, values, this.textStyles);
  }
  withColorVariant(key: string | ThemeKey, selector: ThemeSelector, color: ColorNode): Theme {
    const name = themeKey(key);
    const current = this.colors.get(name);
    const values = new Map(this.colors);
    values.set(name, { base: current?.base, variants: [...(current?.variants ?? []), { selector, value: color }] });
    return new Theme(this.styles, values, this.textStyles);
  }
  withTextStyle(selector: TextSelector, style: StyleSpec): Theme {
    return new Theme(this.styles, this.colors, [...this.textStyles, { selector: selector.value, value: style.value }]);
  }
  style(key: string | ThemeKey): StyleSpec { return new StyleSpec(this.styles.get(themeKey(key))?.base ?? { attributes: {} }); }
  color(key: string | ThemeKey): ColorNode | undefined { return this.colors.get(themeKey(key))?.base; }

  materialize(): object {
    return {
      styles: Object.fromEntries(this.styles),
      colors: Object.fromEntries(this.colors),
      textStyles: this.textStyles,
    };
  }

}

function themeKey(key: string | ThemeKey): string { return typeof key === "string" ? key : key.value; }
