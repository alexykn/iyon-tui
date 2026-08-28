import type {
  BorderSpec,
  ColorSpec,
  RgbColor,
  StyleSelectorValue,
  StyleSpecValue,
  TextSelectorValue,
  ThemeColor,
} from "../../types.ts";
import type {
  BorderNode,
  ColorNode,
  StyleNode,
  TextSpanNode,
} from "./ir.ts";
import { StyleRef, StyleSpec, validateTextAttribute } from "../../api/presentation/style.ts";
import type { TextSpan } from "../../api/content/text.ts";
import type { ThemeKey } from "../../api/presentation/theme-key.ts";

interface ThemeEntry<T> {
  readonly base?: T;
  readonly variants: readonly { readonly selector: StyleSelectorValue; readonly value: T }[];
}

interface ThemeDefinition {
  readonly styles: Readonly<Record<string, ThemeEntry<StyleSpecValue>>>;
  readonly colors: Readonly<Record<string, ThemeEntry<ThemeColor>>>;
  readonly textStyles: readonly { readonly selector: TextSelectorValue; readonly value: StyleSpecValue }[];
}

/** Lowers a public color value to the existing retained bridge atom. */
export function colorNodeFor(color: ColorSpec): ColorNode {
  switch (color.type) {
    case "theme":
      return `theme:${themeKeyValue(color.key)}`;
    case "named":
      validateAnsiColor(color.value);
      return color.value;
    case "indexed":
      validateByte(color.value, "indexed ANSI color");
      return { type: "ansi", value: color.value };
    case "rgb":
      validateRgb(color);
      return `#${hex(color.r)}${hex(color.g)}${hex(color.b)}`;
    default:
      return invalidColor(color);
  }
}

/** Lowers a resolved Theme color definition to the existing host value. */
function themeColorNodeFor(color: ThemeColor): ColorNode | { readonly type: "default" } {
  switch (color.type) {
    case "default":
      return { type: "default" };
    case "named":
      validateAnsiColor(color.value);
      return color.value;
    case "indexed":
      validateByte(color.value, "indexed ANSI color");
      return { type: "ansi", value: color.value };
    case "rgb":
      validateRgb(color);
      return `#${hex(color.r)}${hex(color.g)}${hex(color.b)}`;
    default:
      return invalidColor(color);
  }
}

/** Lowers a public sparse style or named style reference. */
export function styleNodeFor(style: StyleRef | StyleSpec | StyleSpecValue): StyleNode {
  const theme = isStyleRef(style) ? style.themeKey?.value : undefined;
  const value = isStyleRef(style) ? style.local.value : isStyleSpec(style) ? style.value : style;
  return {
    ...(theme === undefined ? {} : { theme }),
    ...(value.foreground === undefined ? {} : { foreground: colorNodeFor(value.foreground) }),
    ...(value.background === undefined ? {} : { background: colorNodeFor(value.background) }),
    attributes: styleAttributesFor(value.attributes),
  };
}

/** Lowers a public border description without changing the View ABI. */
export function borderNodeFor(border: BorderSpec): BorderNode {
  validateBorder(border);
  return {
    ...(border.glyphs === undefined ? {} : { glyphs: { ...border.glyphs } }),
    ...(border.style === undefined ? {} : { style: border.style }),
    ...(border.edges === undefined ? {} : { edges: border.edges }),
    ...(border.color === undefined ? {} : { color: colorNodeFor(border.color) }),
  };
}

function validateBorder(border: BorderSpec): void {
  if (typeof border !== "object" || border === null) {
    throw new TypeError("border must be an object");
  }
  if (border.style !== undefined && border.style !== "plain" && border.style !== "rounded" && border.style !== "double") {
    throw new RangeError(`unknown border style ${JSON.stringify(border.style)}`);
  }
  if (border.edges !== undefined && border.edges !== "all" && border.edges !== "topBottom") {
    throw new RangeError(`unknown border edges ${JSON.stringify(border.edges)}`);
  }
  if (border.glyphs === undefined) return;
  const glyphs = border.glyphs as unknown as Record<string, unknown>;
  for (const field of BORDER_GLYPH_FIELDS) {
    if (!Object.prototype.hasOwnProperty.call(glyphs, field) || typeof glyphs[field] !== "string") {
      throw new TypeError(`border glyph ${JSON.stringify(field)} must be a string`);
    }
  }
}

const BORDER_GLYPH_FIELDS = [
  "top",
  "right",
  "bottom",
  "left",
  "topLeft",
  "topRight",
  "bottomLeft",
  "bottomRight",
] as const;

/** Lowers a public styled span to the existing text-span bridge record. */
export function textSpanNodeFor(span: TextSpan): TextSpanNode {
  return {
    text: span.value.text,
    ...(span.value.style === undefined ? {} : { style: styleNodeFor(span.value.style) }),
  };
}

export function materializeStyle(style: StyleSpecValue): StyleNode {
  return styleNodeFor(style);
}

function styleAttributesFor(attributes: StyleSpecValue["attributes"]): Readonly<Record<string, boolean>> {
  const lowered: Record<string, boolean> = {};
  for (const [name, enabled] of Object.entries(attributes)) {
    if (enabled === undefined) continue;
    validateTextAttribute(name);
    if (typeof enabled !== "boolean") throw new TypeError(`text attribute ${JSON.stringify(name)} must be boolean`);
    lowered[name] = enabled;
  }
  return lowered;
}

/** Lowers a public Theme definition only at the native host boundary. */
export function materializeTheme(theme: ThemeDefinition): object {
  return {
    styles: Object.fromEntries(Object.entries(theme.styles).map(([name, entry]) => [name, {
      ...(entry.base === undefined ? {} : { base: materializeStyle(entry.base) }),
      variants: entry.variants.map((variant) => ({
        selector: variant.selector,
        value: materializeStyle(variant.value),
      })),
    }])),
    colors: Object.fromEntries(Object.entries(theme.colors).map(([name, entry]) => [name, {
      ...(entry.base === undefined ? {} : { base: themeColorNodeFor(entry.base) }),
      variants: entry.variants.map((variant) => ({
        selector: variant.selector,
        value: themeColorNodeFor(variant.value),
      })),
    }])),
    textStyles: theme.textStyles.map((entry) => ({
      selector: entry.selector,
      value: materializeStyle(entry.value),
    })),
  };
}

function isStyleRef(style: StyleRef | StyleSpec | StyleSpecValue): style is StyleRef {
  return typeof style === "object"
    && style !== null
    && (style as { readonly kind?: unknown }).kind === "style-ref";
}

function isStyleSpec(style: StyleRef | StyleSpec | StyleSpecValue): style is StyleSpec {
  return typeof style === "object"
    && style !== null
    && (style as { readonly kind?: unknown }).kind === "style";
}

function themeKeyValue(key: string | ThemeKey): string {
  const value = typeof key === "string" ? key : key.value;
  if (typeof value !== "string" || value.length === 0) throw new RangeError("theme key cannot be empty");
  return value;
}

function validateAnsiColor(value: string): void {
  if (!ANSI_COLORS.has(value)) throw new RangeError(`unknown ANSI color ${JSON.stringify(value)}`);
}

function validateRgb(color: RgbColor): void {
  validateByte(color.r, "red channel");
  validateByte(color.g, "green channel");
  validateByte(color.b, "blue channel");
}

function validateByte(value: number, name: string): void {
  if (!Number.isInteger(value) || value < 0 || value > 255) {
    throw new RangeError(`${name} must be an integer from 0 to 255`);
  }
}

function hex(value: number): string {
  return value.toString(16).padStart(2, "0");
}

function invalidColor(value: never): never {
  throw new TypeError(`unknown color specification ${(value as { type?: unknown }).type ?? "<missing>"}`);
}

const ANSI_COLORS = new Set([
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "gray",
  "darkGray",
  "lightRed",
  "lightGreen",
  "lightYellow",
  "lightBlue",
  "lightMagenta",
  "lightCyan",
  "white",
]);
