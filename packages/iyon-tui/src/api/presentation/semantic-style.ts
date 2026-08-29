/**
 * Backend-neutral normalization for presentation values.
 *
 * The public style/theme objects are convenient immutable facades, while the
 * structural bridge uses a different representation (string color atoms,
 * numeric tags, and native packing). H3-A gives semantic View construction a
 * copied, frozen representation independent of that bridge. Retained
 * structural transport now consumes these semantic styles directly; structural
 * style-lowering remains for public/native boundary and cold compatibility
 * paths.
 */

import type { OverflowIndicator } from "../view/view.ts";
import { insets } from "../view/geometry.ts";
import type { Insets, InsetsValue } from "../view/geometry.ts";
import type {
  BorderGlyphs,
  BorderSpec,
  StyleRef,
  StyleSpec,
  StyleSpecValue,
  TextAttribute,
} from "./style.ts";
import { validateTextAttribute } from "./style.ts";
import type { ColorSpec, RgbColor } from "./theme.ts";
import type { ThemeKey } from "./theme-key.ts";
import type { TextSpan } from "../content/text.ts";
import type {
  SemanticBorder,
  SemanticColor,
  SemanticDecoration,
  SemanticInsets,
  SemanticOverflowIndicator,
  SemanticStyle,
  SemanticTextSpan,
} from "../view/semantic-node.ts";

const EMPTY_STYLE_VALUE: StyleSpecValue = { attributes: {} };

/** Copies a public ColorSpec into a backend-neutral semantic color. */
export function semanticColorFor(color: ColorSpec): SemanticColor {
  switch (color.type) {
    case "theme":
      return Object.freeze({ kind: "theme", key: themeKeyValue(color.key) });
    case "named":
      validateAnsiColor(color.value);
      return Object.freeze({ kind: "named", value: color.value });
    case "indexed":
      validateByte(color.value, "indexed ANSI color");
      return Object.freeze({ kind: "indexed", value: color.value });
    case "rgb":
      validateRgb(color);
      return Object.freeze({ kind: "rgb", r: color.r, g: color.g, b: color.b });
    default:
      return invalidColor(color);
  }
}

/** Copies a sparse or named public style into a semantic style record. */
export function semanticStyleFor(style: StyleRef | StyleSpec | StyleSpecValue): SemanticStyle {
  const theme = isStyleRef(style) ? style.themeKey?.value : undefined;
  const value = isStyleRef(style) ? style.local.value : isStyleSpec(style) ? style.value : style;
  return Object.freeze({
    ...(theme === undefined ? {} : { theme }),
    ...(value.foreground === undefined ? {} : { foreground: semanticColorFor(value.foreground) }),
    ...(value.background === undefined ? {} : { background: semanticColorFor(value.background) }),
    attributes: semanticAttributesFor(value.attributes),
  });
}

/** Copies and validates a public border description. */
export function semanticBorderFor(border: BorderSpec): SemanticBorder {
  validateBorder(border);
  const glyphs = border.glyphs === undefined ? undefined : Object.freeze({ ...border.glyphs });
  return Object.freeze({
    ...(glyphs === undefined ? {} : { glyphs }),
    ...(border.style === undefined ? {} : { style: border.style }),
    ...(border.edges === undefined ? {} : { edges: border.edges }),
    ...(border.color === undefined ? {} : { color: semanticColorFor(border.color) }),
  });
}

/** Copies a public TextSpan and its optional style into semantic records. */
export function semanticTextSpanFor(span: TextSpan): SemanticTextSpan {
  return Object.freeze({
    text: span.value.text,
    ...(span.value.style === undefined ? {} : { style: semanticStyleFor(span.value.style) }),
  });
}

/** Copies the public overflow indicator used by View.clampRows. */
export function semanticOverflowFor(overflow: OverflowIndicator): SemanticOverflowIndicator {
  switch (overflow.kind) {
    case "none":
      return Object.freeze({ kind: "none" });
    case "ellipsis":
      return Object.freeze({ kind: "ellipsis", style: semanticStyleFor(overflow.style) });
    case "footer":
      return Object.freeze({ kind: "footer", prefix: overflow.prefix, style: semanticStyleFor(overflow.style) });
    default:
      return invalidOverflow(overflow);
  }
}

/**
 * Input shape for a semantic decoration. It deliberately contains public
 * semantic values, not bridge records or native style atoms.
 */
export interface SemanticDecorationInput {
  readonly padding?: number | Insets | InsetsValue;
  readonly background?: ColorSpec;
  readonly foreground?: ColorSpec;
  readonly border?: BorderSpec;
  readonly style?: StyleRef | StyleSpec | StyleSpecValue;
  readonly styleStates?: Readonly<Record<string, string>>;
  readonly width?: "fit" | "fill";
  readonly height?: "fit" | "fill";
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}

/** Copies all normalized decoration fields and owns mutable caller records. */
export function semanticDecorationFor(decoration: SemanticDecorationInput = {}): SemanticDecoration {
  const padding = decoration.padding === undefined ? undefined : semanticInsetsFor(decoration.padding);
  const styleStates = decoration.styleStates === undefined
    ? undefined
    : semanticStyleStatesFor(decoration.styleStates);
  if (decoration.width !== undefined && decoration.width !== "fit" && decoration.width !== "fill") {
    throw new RangeError(`unknown width mode ${JSON.stringify(decoration.width)}`);
  }
  if (decoration.height !== undefined && decoration.height !== "fit" && decoration.height !== "fill") {
    throw new RangeError(`unknown height mode ${JSON.stringify(decoration.height)}`);
  }
  for (const [name, value] of [
    ["minWidth", decoration.minWidth],
    ["maxWidth", decoration.maxWidth],
    ["minHeight", decoration.minHeight],
    ["maxHeight", decoration.maxHeight],
  ] as const) {
    if (value !== undefined) validateU16(value, name);
  }
  return Object.freeze({
    ...(padding === undefined ? {} : { padding }),
    ...(decoration.background === undefined ? {} : { background: semanticColorFor(decoration.background) }),
    ...(decoration.foreground === undefined ? {} : { foreground: semanticColorFor(decoration.foreground) }),
    ...(decoration.border === undefined ? {} : { border: semanticBorderFor(decoration.border) }),
    style: semanticStyleFor(decoration.style ?? EMPTY_STYLE_VALUE),
    ...(styleStates === undefined ? {} : { styleStates }),
    ...(decoration.width === undefined ? {} : { width: decoration.width }),
    ...(decoration.height === undefined ? {} : { height: decoration.height }),
    ...(decoration.minWidth === undefined ? {} : { minWidth: decoration.minWidth }),
    ...(decoration.maxWidth === undefined ? {} : { maxWidth: decoration.maxWidth }),
    ...(decoration.minHeight === undefined ? {} : { minHeight: decoration.minHeight }),
    ...(decoration.maxHeight === undefined ? {} : { maxHeight: decoration.maxHeight }),
  });
}

/** Returns an owned empty semantic style. */
export function semanticEmptyStyle(): SemanticStyle {
  return Object.freeze({ attributes: Object.freeze({}) });
}

/** Copies an already-normalized semantic style without sharing containers. */
export function semanticCloneStyle(style: SemanticStyle): SemanticStyle {
  return Object.freeze({
    ...(style.theme === undefined ? {} : { theme: style.theme }),
    ...(style.foreground === undefined ? {} : { foreground: cloneSemanticColor(style.foreground) }),
    ...(style.background === undefined ? {} : { background: cloneSemanticColor(style.background) }),
    attributes: Object.freeze({ ...style.attributes }),
  });
}

/** Applies the current sparse style merge semantics to semantic records. */
export function semanticMergeStyles(left: SemanticStyle, right: SemanticStyle): SemanticStyle {
  return Object.freeze({
    ...(right.theme === undefined && left.theme === undefined
      ? {}
      : { theme: right.theme ?? left.theme }),
    ...(right.foreground === undefined && left.foreground === undefined
      ? {}
      : { foreground: cloneSemanticColor(right.foreground ?? left.foreground!) }),
    ...(right.background === undefined && left.background === undefined
      ? {}
      : { background: cloneSemanticColor(right.background ?? left.background!) }),
    attributes: Object.freeze({ ...left.attributes, ...right.attributes }),
  });
}

/** Copies a semantic decoration, including all nested mutable containers. */
export function semanticCloneDecoration(decoration: SemanticDecoration): SemanticDecoration {
  const padding = decoration.padding === undefined ? undefined : Object.freeze({ ...decoration.padding });
  const border = decoration.border === undefined ? undefined : Object.freeze({
    ...(decoration.border.glyphs === undefined ? {} : { glyphs: Object.freeze({ ...decoration.border.glyphs }) }),
    ...(decoration.border.style === undefined ? {} : { style: decoration.border.style }),
    ...(decoration.border.edges === undefined ? {} : { edges: decoration.border.edges }),
    ...(decoration.border.color === undefined ? {} : { color: cloneSemanticColor(decoration.border.color) }),
  });
  const styleStates = decoration.styleStates === undefined
    ? undefined
    : Object.freeze({ ...decoration.styleStates });
  return Object.freeze({
    ...(padding === undefined ? {} : { padding }),
    ...(decoration.background === undefined ? {} : { background: cloneSemanticColor(decoration.background) }),
    ...(decoration.foreground === undefined ? {} : { foreground: cloneSemanticColor(decoration.foreground) }),
    ...(border === undefined ? {} : { border }),
    style: semanticCloneStyle(decoration.style),
    ...(styleStates === undefined ? {} : { styleStates }),
    ...(decoration.width === undefined ? {} : { width: decoration.width }),
    ...(decoration.height === undefined ? {} : { height: decoration.height }),
    ...(decoration.minWidth === undefined ? {} : { minWidth: decoration.minWidth }),
    ...(decoration.maxWidth === undefined ? {} : { maxWidth: decoration.maxWidth }),
    ...(decoration.minHeight === undefined ? {} : { minHeight: decoration.minHeight }),
    ...(decoration.maxHeight === undefined ? {} : { maxHeight: decoration.maxHeight }),
  });
}

function semanticInsetsFor(value: number | Insets | InsetsValue): SemanticInsets {
  const normalized = insets(value);
  for (const [name, part] of Object.entries(normalized)) validateU16(part, `inset ${name}`);
  return Object.freeze(normalized);
}

function semanticStyleStatesFor(states: Readonly<Record<string, string>>): Readonly<Record<string, string>> {
  for (const [key, value] of Object.entries(states)) {
    if (key.length === 0 || typeof value !== "string" || value.length === 0) {
      throw new RangeError("style state key and value cannot be empty");
    }
  }
  return Object.freeze({ ...states });
}

function semanticAttributesFor(
  attributes: StyleSpecValue["attributes"],
): Readonly<Record<TextAttribute, boolean>> {
  const normalized: Partial<Record<TextAttribute, boolean>> = {};
  for (const [name, enabled] of Object.entries(attributes)) {
    if (enabled === undefined) continue;
    validateTextAttribute(name);
    if (typeof enabled !== "boolean") {
      throw new TypeError(`text attribute ${JSON.stringify(name)} must be boolean`);
    }
    normalized[name as TextAttribute] = enabled;
  }
  return Object.freeze(normalized as Record<TextAttribute, boolean>);
}

function cloneSemanticColor(color: SemanticColor): SemanticColor {
  return Object.freeze({ ...color });
}

function validateBorder(border: BorderSpec): void {
  if (typeof border !== "object" || border === null) throw new TypeError("border must be an object");
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

const BORDER_GLYPH_FIELDS: readonly (keyof BorderGlyphs)[] = [
  "top",
  "right",
  "bottom",
  "left",
  "topLeft",
  "topRight",
  "bottomLeft",
  "bottomRight",
];

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

function validateU16(value: number, name: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 65535) {
    throw new RangeError(`${name} must be an integer from 0 to 65535`);
  }
  return value;
}

function invalidColor(value: never): never {
  throw new TypeError(`unknown color specification ${(value as { type?: unknown }).type ?? "<missing>"}`);
}

function invalidOverflow(value: never): never {
  throw new TypeError(`unknown overflow indicator kind ${(value as { kind?: unknown }).kind ?? "<missing>"}`);
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
