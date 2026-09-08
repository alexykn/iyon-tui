// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = c697c2a2064686fae3f39ee1c120ea7da763069faf1fdaff4434b95119a65817
// generator_blake3 = 21a374704490d608ade5c8894d0de2d01caec2c069c7f8db8839fb5a2832fe3d
/** Primitive wake disposition bit: the environment must drain. */
export const STATE_WAKE_DRAIN = 1;

export interface StateEnvelope {
  readonly setMask: number;
  readonly nullMask: number;
  readonly clearMask: number;
  readonly words: readonly number[];
  readonly strings: readonly string[];
}

const TEXT_ATTR_ORDER = ["bold", "dim", "italic", "underline", "reversed", "strikethrough"] as const;

function packAlignment(value: unknown): number {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("state envelope: alignment must be an object");
  }
  const candidate = value as { horizontal?: unknown; vertical?: unknown };
  const horizontal = candidate.horizontal === undefined
    ? 0
    : candidate.horizontal === "start" ? 1 : candidate.horizontal === "center" ? 2 : candidate.horizontal === "end" ? 3
    : (() => { throw new TypeError("state envelope: alignment horizontal is invalid"); })();
  const vertical = candidate.vertical === undefined
    ? 0
    : candidate.vertical === "top" ? 1 : candidate.vertical === "center" ? 2 : candidate.vertical === "bottom" ? 3
    : (() => { throw new TypeError("state envelope: alignment vertical is invalid"); })();
  if (horizontal === 0 && vertical === 0) throw new TypeError("state envelope: alignment must specify an axis");
  return horizontal | (vertical << 3);
}

function packBorderEdges(value: unknown, words: number[], offset: number): void {
  if (value === "all") {
    words[offset] = 1;
    words[offset + 1] = 0;
    return;
  }
  if (value === "topBottom") {
    words[offset] = 2;
    words[offset + 1] = 0;
    return;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("state envelope: borderEdges must be all, topBottom, or an edge object");
  }
  const candidate = value as Record<string, unknown>;
  let bits = 0;
  const edges = ["top", "right", "bottom", "left"] as const;
  for (let index = 0; index < edges.length; index += 1) {
    if (typeof candidate[edges[index]] !== "boolean") {
      throw new TypeError(`state envelope: border edge ${JSON.stringify(edges[index])} must be boolean`);
    }
    if (candidate[edges[index]] === true) bits |= 1 << index;
  }
  words[offset] = 0;
  words[offset + 1] = bits;
}

function packColorString(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    const candidate = value as { type?: unknown; value?: unknown };
    if (candidate.type === "ansi" && typeof candidate.value === "number") return `ansi:${candidate.value}`;
  }
  throw new TypeError("state envelope: color must be a string or ANSI color object");
}

function packTextAttributes(value: unknown, words: number[], offset: number): void {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("state envelope: textAttributes must be an object");
  }
  const candidate = value as Record<string, unknown>;
  let presence = 0;
  let values = 0;
  for (let index = 0; index < TEXT_ATTR_ORDER.length; index += 1) {
    const enabled = candidate[TEXT_ATTR_ORDER[index]];
    if (enabled === undefined) continue;
    if (typeof enabled !== "boolean") {
      throw new TypeError(`state envelope: text attribute ${JSON.stringify(TEXT_ATTR_ORDER[index])} must be boolean`);
    }
    presence |= 1 << index;
    if (enabled) values |= 1 << index;
  }
  words[offset] = presence;
  words[offset + 1] = values;
}

function packStyle(value: unknown, words: number[], wordOffset: number, strings: string[], stringOffset: number): void {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("state envelope: style must be an object");
  }
  const candidate = value as { theme?: unknown; foreground?: unknown; background?: unknown; attributes?: unknown };
  strings[stringOffset] = typeof candidate.theme === "string" ? candidate.theme : "";
  strings[stringOffset + 1] = candidate.foreground === undefined ? "" : packColorString(candidate.foreground);
  strings[stringOffset + 2] = candidate.background === undefined ? "" : packColorString(candidate.background);
  if (candidate.attributes === undefined) {
    words[wordOffset] = 0;
    words[wordOffset + 1] = 0;
    return;
  }
  packTextAttributes(candidate.attributes, words, wordOffset);
}

export const GEOMETRY_WORD_COUNT = 14;
export const GEOMETRY_STRING_COUNT = 0;
export const GEOMETRY_ALL_MASK = 0x3ff;
export const GEOMETRY_NULLABLE_MASK = 0x278;
export const GEOMETRY_CLEARABLE_MASK = 0x3ff;
export const GEOMETRY_PROPERTY_IDS: Readonly<Record<string, number>> = {
  width: 0,
  height: 1,
  padding: 2,
  minWidth: 3,
  maxWidth: 4,
  minHeight: 5,
  maxHeight: 6,
  gap: 7,
  alignment: 8,
  borderEdges: 9,
};
export const GEOMETRY_CAPABILITIES: Readonly<Record<string, string>> = {
  width: "node-kind",
  height: "node-kind",
  padding: "node-kind",
  minWidth: "node-kind",
  maxWidth: "node-kind",
  minHeight: "node-kind",
  maxHeight: "node-kind",
  gap: "node-kind",
  alignment: "node-kind+axis",
  borderEdges: "node-kind",
};

/** Packs an already-normalized geometry patch into the mask envelope. */
export function encodeGeometryEnvelope(patch: Record<string, unknown>): StateEnvelope {
  let setMask = 0;
  let nullMask = 0;
  const words = new Array<number>(14).fill(0);
  const strings = new Array<string>(0).fill("");
  if (patch["width"] !== undefined) {
    setMask |= 0x1;
    const value = patch["width"];
    words[0] = value === "fill" ? 2 : value === "fit" ? 1 : (() => { throw new TypeError(`state envelope: width must be fit or fill`) })();
  }
  if (patch["height"] !== undefined) {
    setMask |= 0x2;
    const value = patch["height"];
    words[1] = value === "fill" ? 2 : value === "fit" ? 1 : (() => { throw new TypeError(`state envelope: height must be fit or fill`) })();
  }
  if (patch["padding"] !== undefined) {
    setMask |= 0x4;
    const value = patch["padding"];
    const insetFields = value as { top?: unknown; right?: unknown; bottom?: unknown; left?: unknown };
    if (typeof insetFields.top !== "number" || !Number.isInteger(insetFields.top) || insetFields.top < 0 || insetFields.top > 65535) throw new TypeError(`state envelope: padding top must be an integer from 0 to 65535`);
    words[2] = insetFields.top;
    if (typeof insetFields.right !== "number" || !Number.isInteger(insetFields.right) || insetFields.right < 0 || insetFields.right > 65535) throw new TypeError(`state envelope: padding right must be an integer from 0 to 65535`);
    words[3] = insetFields.right;
    if (typeof insetFields.bottom !== "number" || !Number.isInteger(insetFields.bottom) || insetFields.bottom < 0 || insetFields.bottom > 65535) throw new TypeError(`state envelope: padding bottom must be an integer from 0 to 65535`);
    words[4] = insetFields.bottom;
    if (typeof insetFields.left !== "number" || !Number.isInteger(insetFields.left) || insetFields.left < 0 || insetFields.left > 65535) throw new TypeError(`state envelope: padding left must be an integer from 0 to 65535`);
    words[5] = insetFields.left;
  }
  if (patch["minWidth"] !== undefined) {
    setMask |= 0x8;
    const value = patch["minWidth"];
    if (value === null) {
      nullMask |= 0x8;
    } else {
      if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 65535) throw new TypeError(`state envelope: minWidth must be an integer from 0 to 65535`);
      words[6] = value;
    }
  }
  if (patch["maxWidth"] !== undefined) {
    setMask |= 0x10;
    const value = patch["maxWidth"];
    if (value === null) {
      nullMask |= 0x10;
    } else {
      if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 65535) throw new TypeError(`state envelope: maxWidth must be an integer from 0 to 65535`);
      words[7] = value;
    }
  }
  if (patch["minHeight"] !== undefined) {
    setMask |= 0x20;
    const value = patch["minHeight"];
    if (value === null) {
      nullMask |= 0x20;
    } else {
      if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 65535) throw new TypeError(`state envelope: minHeight must be an integer from 0 to 65535`);
      words[8] = value;
    }
  }
  if (patch["maxHeight"] !== undefined) {
    setMask |= 0x40;
    const value = patch["maxHeight"];
    if (value === null) {
      nullMask |= 0x40;
    } else {
      if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 65535) throw new TypeError(`state envelope: maxHeight must be an integer from 0 to 65535`);
      words[9] = value;
    }
  }
  if (patch["gap"] !== undefined) {
    setMask |= 0x80;
    const value = patch["gap"];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 65535) throw new TypeError(`state envelope: gap must be an integer from 0 to 65535`);
    words[10] = value;
  }
  if (patch["alignment"] !== undefined) {
    setMask |= 0x100;
    const value = patch["alignment"];
    words[11] = packAlignment(value);
  }
  if (patch["borderEdges"] !== undefined) {
    setMask |= 0x200;
    const value = patch["borderEdges"];
    if (value === null) {
      nullMask |= 0x200;
    } else {
      packBorderEdges(value, words, 12);
    }
  }
  return { setMask, nullMask, clearMask: 0, words, strings };
}

/** Packs an already-normalized geometry clear list into a clear mask. Unknown names cannot arrive here (the normalizer rejects them); the throw below is unreachable defense. */
export function encodeGeometryClearMask(properties: readonly string[]): number {
  let mask = 0;
  for (const property of properties) {
    const id = ({
      width: 0,
      height: 1,
      padding: 2,
      minWidth: 3,
      maxWidth: 4,
      minHeight: 5,
      maxHeight: 6,
      gap: 7,
      alignment: 8,
      borderEdges: 9,
    } as Record<string, number | undefined>)[property];
    if (id === undefined) throw new TypeError(`state envelope: unknown clear property ${JSON.stringify(property)}`);
    mask |= 1 << id;
  }
  return mask;
}

export const PRESENTATION_WORD_COUNT = 5;
export const PRESENTATION_STRING_COUNT = 14;
export const PRESENTATION_ALL_MASK = 0x7f;
export const PRESENTATION_NULLABLE_MASK = 0x5f;
export const PRESENTATION_CLEARABLE_MASK = 0x7f;
export const PRESENTATION_PROPERTY_IDS: Readonly<Record<string, number>> = {
  foreground: 0,
  background: 1,
  borderColor: 2,
  borderStyle: 3,
  borderGlyphs: 4,
  textAttributes: 5,
  style: 6,
};
export const PRESENTATION_CAPABILITIES: Readonly<Record<string, string>> = {
  foreground: "node-kind",
  background: "node-kind",
  borderColor: "node-kind",
  borderStyle: "node-kind",
  borderGlyphs: "node-kind",
  textAttributes: "node-kind",
  style: "node-kind",
};

/** Packs an already-normalized presentation patch into the mask envelope. */
export function encodePresentationEnvelope(patch: Record<string, unknown>): StateEnvelope {
  let setMask = 0;
  let nullMask = 0;
  const words = new Array<number>(5).fill(0);
  const strings = new Array<string>(14).fill("");
  if (patch["foreground"] !== undefined) {
    setMask |= 0x1;
    const value = patch["foreground"];
    if (value === null) {
      nullMask |= 0x1;
    } else {
      strings[0] = packColorString(value);
    }
  }
  if (patch["background"] !== undefined) {
    setMask |= 0x2;
    const value = patch["background"];
    if (value === null) {
      nullMask |= 0x2;
    } else {
      strings[1] = packColorString(value);
    }
  }
  if (patch["borderColor"] !== undefined) {
    setMask |= 0x4;
    const value = patch["borderColor"];
    if (value === null) {
      nullMask |= 0x4;
    } else {
      strings[2] = packColorString(value);
    }
  }
  if (patch["borderStyle"] !== undefined) {
    setMask |= 0x8;
    const value = patch["borderStyle"];
    if (value === null) {
      nullMask |= 0x8;
    } else {
      words[0] = value === "plain" ? 1 : value === "rounded" ? 2 : value === "double" ? 3 : (() => { throw new TypeError(`state envelope: borderStyle must be plain, rounded, double, or null`) })();
    }
  }
  if (patch["borderGlyphs"] !== undefined) {
    setMask |= 0x10;
    const value = patch["borderGlyphs"];
    if (value === null) {
      nullMask |= 0x10;
    } else {
      const glyphFields = value as { top?: unknown; right?: unknown; bottom?: unknown; left?: unknown; topLeft?: unknown; topRight?: unknown; bottomLeft?: unknown; bottomRight?: unknown };
      if (typeof glyphFields.top !== "string") throw new TypeError(`state envelope: borderGlyphs glyph top must be a string`);
      strings[3] = glyphFields.top;
      if (typeof glyphFields.right !== "string") throw new TypeError(`state envelope: borderGlyphs glyph right must be a string`);
      strings[4] = glyphFields.right;
      if (typeof glyphFields.bottom !== "string") throw new TypeError(`state envelope: borderGlyphs glyph bottom must be a string`);
      strings[5] = glyphFields.bottom;
      if (typeof glyphFields.left !== "string") throw new TypeError(`state envelope: borderGlyphs glyph left must be a string`);
      strings[6] = glyphFields.left;
      if (typeof glyphFields.topLeft !== "string") throw new TypeError(`state envelope: borderGlyphs glyph topLeft must be a string`);
      strings[7] = glyphFields.topLeft;
      if (typeof glyphFields.topRight !== "string") throw new TypeError(`state envelope: borderGlyphs glyph topRight must be a string`);
      strings[8] = glyphFields.topRight;
      if (typeof glyphFields.bottomLeft !== "string") throw new TypeError(`state envelope: borderGlyphs glyph bottomLeft must be a string`);
      strings[9] = glyphFields.bottomLeft;
      if (typeof glyphFields.bottomRight !== "string") throw new TypeError(`state envelope: borderGlyphs glyph bottomRight must be a string`);
      strings[10] = glyphFields.bottomRight;
    }
  }
  if (patch["textAttributes"] !== undefined) {
    setMask |= 0x20;
    const value = patch["textAttributes"];
    packTextAttributes(value, words, 1);
  }
  if (patch["style"] !== undefined) {
    setMask |= 0x40;
    const value = patch["style"];
    if (value === null) {
      nullMask |= 0x40;
    } else {
      packStyle(value, words, 3, strings, 11);
    }
  }
  return { setMask, nullMask, clearMask: 0, words, strings };
}

/** Packs an already-normalized presentation clear list into a clear mask. Unknown names cannot arrive here (the normalizer rejects them); the throw below is unreachable defense. */
export function encodePresentationClearMask(properties: readonly string[]): number {
  let mask = 0;
  for (const property of properties) {
    const id = ({
      foreground: 0,
      background: 1,
      borderColor: 2,
      borderStyle: 3,
      borderGlyphs: 4,
      textAttributes: 5,
      style: 6,
    } as Record<string, number | undefined>)[property];
    if (id === undefined) throw new TypeError(`state envelope: unknown clear property ${JSON.stringify(property)}`);
    mask |= 1 << id;
  }
  return mask;
}

