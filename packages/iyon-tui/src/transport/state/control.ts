import type {
  BorderEdges,
  BorderGlyphs,
  StyleRef,
  StyleSpec,
} from "../../api/presentation/style.ts";
import { validateTextAttribute } from "../../api/presentation/style.ts";
import type { ColorSpec } from "../../api/presentation/theme.ts";
import {
  semanticBorderFor,
  semanticColorFor,
  semanticStyleFor,
} from "../../api/presentation/semantic-style.ts";
import type {
  ViewStateBorderEdges,
  ViewStateGeometryPatch,
  ViewStateGeometryProperty,
  ViewStatePresentationPatch,
  ViewStatePresentationProperty,
} from "../../api/view/retained-state.ts";
import { insets, type Insets, type InsetsValue } from "../../api/view/geometry.ts";
import type { SemanticColor } from "../../api/view/semantic-node.ts";

/** Encodes a typed geometry patch for the private native control call. */
export function normalizeGeometryPatch(patch: ViewStateGeometryPatch): object {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) {
    throw new TypeError("ViewState geometry patch must be an object");
  }
  const normalized: Record<string, unknown> = {};
  for (const key of Object.keys(patch as object)) {
    if (!GEOMETRY_PROPERTIES.has(key as ViewStateGeometryProperty)) {
      throw new TypeError(`unknown ViewState geometry property ${JSON.stringify(key)}`);
    }
    const value = (patch as Record<string, unknown>)[key];
    if (value === undefined) continue;
    switch (key as ViewStateGeometryProperty) {
      case "width":
      case "height":
        if (value !== "fit" && value !== "fill") {
          throw new RangeError(`ViewState ${key} must be fit or fill`);
        }
        normalized[key] = value;
        break;
      case "padding":
        normalized.padding = normalizeInsets(value);
        break;
      case "minWidth":
      case "maxWidth":
      case "minHeight":
      case "maxHeight":
        normalized[key] = value === null ? null : validateU16(value, `ViewState ${key}`);
        break;
      case "gap":
        normalized.gap = validateU16(value, "ViewState gap");
        break;
      case "alignment":
        normalized.alignment = normalizeAlignment(value);
        break;
      case "borderEdges":
        normalized.borderEdges = normalizeBorderEdges(value);
        break;
    }
  }
  return normalized;
}

/** Normalizes a typed geometry clear list; omitted means clear the whole domain. */
export function normalizeClearGeometryProperties(
  properties: readonly ViewStateGeometryProperty[] | undefined,
): readonly string[] | undefined {
  if (properties === undefined) return undefined;
  if (!Array.isArray(properties)) throw new TypeError("ViewState geometry clear properties must be an array");
  const normalized: string[] = [];
  const seen = new Set<string>();
  for (const property of properties) {
    if (typeof property !== "string" || !GEOMETRY_PROPERTIES.has(property as ViewStateGeometryProperty)) {
      throw new TypeError(`unknown ViewState geometry clear property ${JSON.stringify(property)}`);
    }
    if (seen.has(property)) throw new RangeError(`duplicate ViewState geometry clear property ${JSON.stringify(property)}`);
    seen.add(property);
    normalized.push(property);
  }
  return normalized;
}

function normalizeInsets(value: unknown): InsetsValue {
  if (typeof value === "number") {
    const normalized = insets(value);
    for (const name of ["top", "right", "bottom", "left"] as const) {
      validateU16(normalized[name], `ViewState padding ${name}`);
    }
    return normalized;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("ViewState padding must be Insets or an InsetsValue");
  }
  const candidate = insets(value as Insets | InsetsValue);
  const normalized = {} as Record<keyof InsetsValue, number>;
  for (const name of ["top", "right", "bottom", "left"] as const) {
    normalized[name] = validateU16(candidate[name], `ViewState padding ${name}`);
  }
  return normalized;
}

function normalizeBorderEdges(value: unknown): BorderEdges | ViewStateBorderEdges | null {
  if (value === null || value === "all" || value === "topBottom") return value;
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError("ViewState borderEdges must be all, topBottom, an edge object, or null");
  }
  const candidate = value as Partial<Record<keyof ViewStateBorderEdges, unknown>>;
  for (const key of Object.keys(candidate)) {
    if (key !== "top" && key !== "right" && key !== "bottom" && key !== "left") {
      throw new TypeError(`unknown ViewState border edge ${JSON.stringify(key)}`);
    }
  }
  const normalized = {} as Record<keyof ViewStateBorderEdges, boolean>;
  for (const edge of ["top", "right", "bottom", "left"] as const) {
    if (typeof candidate[edge] !== "boolean") {
      throw new TypeError(`ViewState border edge ${JSON.stringify(edge)} must be boolean`);
    }
    normalized[edge] = candidate[edge];
  }
  return normalized;
}

function normalizeAlignment(value: unknown): object {
  if (value === "start" || value === "center" || value === "end") return { horizontal: value };
  if (value === "top" || value === "bottom") return { vertical: value };
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("ViewState alignment must be a known alignment or an alignment object");
  }
  const candidate = value as { horizontal?: unknown; vertical?: unknown };
  for (const key of Object.keys(candidate)) {
    if (key !== "horizontal" && key !== "vertical") {
      throw new TypeError(`unknown ViewState alignment field ${JSON.stringify(key)}`);
    }
  }
  const normalized: { horizontal?: string; vertical?: string } = {};
  if (candidate.horizontal !== undefined) {
    if (candidate.horizontal !== "start" && candidate.horizontal !== "center" && candidate.horizontal !== "end") {
      throw new RangeError("ViewState horizontal alignment is invalid");
    }
    normalized.horizontal = candidate.horizontal;
  }
  if (candidate.vertical !== undefined) {
    if (candidate.vertical !== "top" && candidate.vertical !== "center" && candidate.vertical !== "bottom") {
      throw new RangeError("ViewState vertical alignment is invalid");
    }
    normalized.vertical = candidate.vertical;
  }
  if (normalized.horizontal === undefined && normalized.vertical === undefined) {
    throw new RangeError("ViewState alignment must specify an axis");
  }
  return normalized;
}

function validateU16(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 65535) {
    throw new RangeError(`${label} must be an integer from 0 to 65535`);
  }
  return value;
}

/** Encodes a typed presentation patch for the private native control call. */
export function normalizePresentationPatch(patch: ViewStatePresentationPatch): object {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) {
    throw new TypeError("ViewState presentation patch must be an object");
  }
  const normalized: Record<string, unknown> = {};
  for (const key of Object.keys(patch as object)) {
    if (!PRESENTATION_PROPERTIES.has(key as ViewStatePresentationProperty)) {
      throw new TypeError(`unknown ViewState presentation property ${JSON.stringify(key)}`);
    }
    const value = (patch as Record<string, unknown>)[key];
    if (value === undefined) continue;
    switch (key as ViewStatePresentationProperty) {
      case "foreground":
      case "background":
      case "borderColor":
        normalized[key] = value === null ? null : nativeColorFor(value as ColorSpec);
        break;
      case "borderStyle":
        if (value !== null && value !== "plain" && value !== "rounded" && value !== "double") {
          throw new RangeError(`unknown ViewState border style ${JSON.stringify(value)}`);
        }
        normalized[key] = value;
        break;
      case "borderGlyphs":
        normalized[key] = value === null ? null : normalizeBorderGlyphs(value);
        break;
      case "textAttributes":
        normalized[key] = normalizeTextAttributes(value);
        break;
      case "style":
        normalized[key] = value === null ? null : nativeStyleFor(value as StyleRef | StyleSpec);
        break;
    }
  }
  return normalized;
}

/** Normalizes a typed clear list; omitted means clear the whole domain. */
export function normalizeClearProperties(
  properties: readonly ViewStatePresentationProperty[] | undefined,
): readonly string[] | undefined {
  if (properties === undefined) return undefined;
  if (!Array.isArray(properties)) throw new TypeError("ViewState clear properties must be an array");
  const normalized: string[] = [];
  const seen = new Set<string>();
  for (const property of properties) {
    if (typeof property !== "string" || !PRESENTATION_PROPERTIES.has(property as ViewStatePresentationProperty)) {
      throw new TypeError(`unknown ViewState presentation property ${JSON.stringify(property)}`);
    }
    if (seen.has(property)) throw new RangeError(`duplicate ViewState clear property ${JSON.stringify(property)}`);
    seen.add(property);
    normalized.push(property);
  }
  return normalized;
}

function normalizeBorderGlyphs(value: unknown): object {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("ViewState borderGlyphs must be an object or null");
  }
  const glyphs = value as Partial<Record<keyof BorderGlyphs, unknown>>;
  const fields: (keyof BorderGlyphs)[] = [
    "top",
    "right",
    "bottom",
    "left",
    "topLeft",
    "topRight",
    "bottomLeft",
    "bottomRight",
  ];
  for (const field of fields) {
    if (typeof glyphs[field] !== "string") {
      throw new TypeError(`ViewState border glyph ${JSON.stringify(field)} must be a string`);
    }
  }
  const normalized = semanticBorderFor({ glyphs: glyphs as BorderGlyphs });
  return normalized.glyphs!;
}

function normalizeTextAttributes(value: unknown): object {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("ViewState textAttributes must be an object");
  }
  const normalized: Record<string, boolean> = {};
  for (const [name, enabled] of Object.entries(value)) {
    validateTextAttribute(name);
    if (typeof enabled !== "boolean") {
      throw new TypeError(`ViewState text attribute ${JSON.stringify(name)} must be boolean`);
    }
    normalized[name] = enabled;
  }
  return normalized;
}

function nativeColorFor(color: ColorSpec): string | object {
  return bridgeColor(semanticColorFor(color));
}

function bridgeColor(color: SemanticColor): string | object {
  switch (color.kind) {
    case "theme": return `theme:${color.key}`;
    case "named": return color.value;
    case "indexed": return { type: "ansi", value: color.value };
    case "rgb": return `#${hex(color.r)}${hex(color.g)}${hex(color.b)}`;
  }
}

function nativeStyleFor(style: StyleRef | StyleSpec): object {
  const normalized = semanticStyleFor(style);
  return {
    ...(normalized.theme === undefined ? {} : { theme: normalized.theme }),
    ...(normalized.foreground === undefined ? {} : { foreground: bridgeColor(normalized.foreground) }),
    ...(normalized.background === undefined ? {} : { background: bridgeColor(normalized.background) }),
    attributes: { ...normalized.attributes },
  };
}

function hex(value: number): string {
  return value.toString(16).padStart(2, "0");
}

const GEOMETRY_PROPERTIES = new Set<ViewStateGeometryProperty>([
  "width",
  "height",
  "padding",
  "minWidth",
  "maxWidth",
  "minHeight",
  "maxHeight",
  "gap",
  "alignment",
  "borderEdges",
]);

const PRESENTATION_PROPERTIES = new Set<ViewStatePresentationProperty>([
  "foreground",
  "background",
  "borderColor",
  "borderStyle",
  "borderGlyphs",
  "textAttributes",
  "style",
]);
