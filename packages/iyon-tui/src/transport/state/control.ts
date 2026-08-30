import type {
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
  ViewStatePresentationPatch,
  ViewStatePresentationProperty,
} from "../../api/view/retained-state.ts";
import type { SemanticColor } from "../../api/view/semantic-node.ts";

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

const PRESENTATION_PROPERTIES = new Set<ViewStatePresentationProperty>([
  "foreground",
  "background",
  "borderColor",
  "borderStyle",
  "borderGlyphs",
  "textAttributes",
  "style",
]);
