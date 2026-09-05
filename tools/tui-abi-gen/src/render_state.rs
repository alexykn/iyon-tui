//! L1-05 retained-state envelope renderers (§7.1).
//!
//! The `state_property` schema rows are the single source of truth for the
//! mask envelope shared by the TS packers and the Rust mask/lane tables.
//! TS validation (exact public error shape) stays hand-written in
//! `transport/state/control.ts`; the generated packers below only translate
//! already-normalized patches into masks plus value lanes. Value semantics
//! stay in hand-written readers on both sides.

use crate::{
    model::{AbiDocument, StatePropertySpec},
    render_manifest::banner,
};

/// Wake disposition bit shared by every state call (§7.2: a primitive wake
/// result, not a one-property JSON object).
const WAKE_DRAIN_BIT: u32 = 1;

const GLYPH_ORDER: [&str; 8] = [
    "top",
    "right",
    "bottom",
    "left",
    "topLeft",
    "topRight",
    "bottomLeft",
    "bottomRight",
];

fn domain_properties<'a>(document: &'a AbiDocument, domain: &str) -> Vec<&'a StatePropertySpec> {
    let mut properties: Vec<&StatePropertySpec> = document
        .state_properties
        .iter()
        .filter(|property| property.domain == domain)
        .collect();
    properties.sort_by_key(|property| property.id);
    properties
}

fn bit(id: u32) -> u32 {
    1u32 << id
}

fn all_mask(properties: &[&StatePropertySpec]) -> u32 {
    properties
        .iter()
        .fold(0, |mask, property| mask | bit(property.id))
}

fn nullable_mask(properties: &[&StatePropertySpec]) -> u32 {
    properties
        .iter()
        .filter(|property| property.nullable)
        .fold(0, |mask, property| mask | bit(property.id))
}

fn clearable_mask(properties: &[&StatePropertySpec]) -> u32 {
    properties
        .iter()
        .filter(|property| property.clearable)
        .fold(0, |mask, property| mask | bit(property.id))
}

fn word_count(properties: &[&StatePropertySpec]) -> u32 {
    properties.iter().map(|property| property.words).sum()
}

fn string_count(properties: &[&StatePropertySpec]) -> u32 {
    properties.iter().map(|property| property.strings).sum()
}

fn word_offset(properties: &[&StatePropertySpec], id: u32) -> u32 {
    properties
        .iter()
        .filter(|property| property.id < id)
        .map(|property| property.words)
        .sum()
}

fn string_offset(properties: &[&StatePropertySpec], id: u32) -> u32 {
    properties
        .iter()
        .filter(|property| property.id < id)
        .map(|property| property.strings)
        .sum()
}

/// Shared pack helpers: fixed sub-encodings for composite value kinds.
/// Codes mirror the Rust tables below (single source: the schema rows).
const TS_HELPERS: &str = r#"const TEXT_ATTR_ORDER = ["bold", "dim", "italic", "underline", "reversed", "strikethrough"] as const;

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

"#;

fn pascal(domain: &str) -> String {
    let mut output = String::new();
    for part in domain.split('_') {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            output.extend(first.to_uppercase());
            output.push_str(characters.as_str());
        }
    }
    output
}

/// TypeScript envelope packers for already-normalized patches plus the
/// mask/lane tables. Validation (exact public errors) stays hand-written;
/// these functions only pack.
pub fn typescript_envelope(
    document: &AbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = banner(schema_hash, generator_hash);
    output.push_str(&format!(
        "/** Primitive wake disposition bit: the environment must drain. */\nexport const STATE_WAKE_DRAIN = {};\n\n",
        WAKE_DRAIN_BIT
    ));
    output.push_str(
        "export interface StateEnvelope {\n  readonly setMask: number;\n  readonly nullMask: number;\n  readonly clearMask: number;\n  readonly words: readonly number[];\n  readonly strings: readonly string[];\n}\n\n",
    );
    output.push_str(TS_HELPERS);
    for domain in ["geometry", "presentation"] {
        let properties = domain_properties(document, domain);
        let upper = domain.to_uppercase();
        output.push_str(&format!(
            "export const {upper}_WORD_COUNT = {};\nexport const {upper}_STRING_COUNT = {};\nexport const {upper}_ALL_MASK = {:#x};\nexport const {upper}_NULLABLE_MASK = {:#x};\nexport const {upper}_CLEARABLE_MASK = {:#x};\n",
            word_count(&properties),
            string_count(&properties),
            all_mask(&properties),
            nullable_mask(&properties),
            clearable_mask(&properties),
        ));
        output.push_str(&format!(
            "export const {upper}_PROPERTY_IDS: Readonly<Record<string, number>> = {{\n"
        ));
        for property in &properties {
            output.push_str(&format!("  {}: {},\n", property.name, property.id));
        }
        output.push_str("};\n");
        output.push_str(&format!(
            "export const {upper}_CAPABILITIES: Readonly<Record<string, string>> = {{\n"
        ));
        for property in &properties {
            output.push_str(&format!(
                "  {}: {:?},\n",
                property.name, property.capability
            ));
        }
        output.push_str("};\n\n");
        output.push_str(&ts_set_packer(domain, &properties));
        output.push_str(&ts_clear_packer(domain, &properties));
    }
    output
}

fn ts_set_packer(domain: &str, properties: &[&StatePropertySpec]) -> String {
    let pascal = pascal(domain);
    let mut output = format!(
        "/** Packs an already-normalized {domain} patch into the mask envelope. */\nexport function encode{pascal}Envelope(patch: Record<string, unknown>): StateEnvelope {{\n  let setMask = 0;\n  let nullMask = 0;\n  const words = new Array<number>({word_count}).fill(0);\n  const strings = new Array<string>({string_count}).fill(\"\");\n",
        word_count = word_count(properties),
        string_count = string_count(properties),
    );
    for property in properties {
        output.push_str(&format!(
            "  if (patch[{name:?}] !== undefined) {{\n    setMask |= {:#x};\n",
            bit(property.id),
            name = property.name,
        ));
        output.push_str(&ts_pack_value(properties, property, "    "));
        output.push_str("  }\n");
    }
    output.push_str("  return { setMask, nullMask, clearMask: 0, words, strings };\n}\n\n");
    output
}

/// Emits the packing statements for one property. `value` holds the
/// already-normalized patch value; `words`/`strings` are the fixed lanes;
/// `nullMask` accumulates semantic-null bits. Guards below are unreachable
/// through the public API (the normalizer validates first) and exist so the
/// packer is total over its input type.
fn ts_pack_value(
    properties: &[&StatePropertySpec],
    property: &StatePropertySpec,
    indent: &str,
) -> String {
    let bit = bit(property.id);
    let word = word_offset(properties, property.id);
    let string = string_offset(properties, property.id);
    let fail = |what: &str| {
        format!(
            "throw new TypeError(`state envelope: {name} {what}`)",
            name = property.name
        )
    };
    let mut output = format!(
        "{indent}const value = patch[{name:?}];\n",
        name = property.name
    );
    if property.nullable {
        output.push_str(&format!(
            "{indent}if (value === null) {{\n{indent}  nullMask |= {bit:#x};\n{indent}}} else {{\n"
        ));
    }
    let body_indent = if property.nullable {
        format!("{indent}  ")
    } else {
        indent.to_owned()
    };
    let body = match property.value.as_str() {
        "size_mode" => {
            format!(
                "{indent}words[{word}] = value === \"fill\" ? 2 : value === \"fit\" ? 1 : (() => {{ {fail} }})();\n",
                indent = body_indent,
                fail = fail("must be fit or fill"),
            )
        }
        "u16" => {
            format!(
                "{indent}if (typeof value !== \"number\" || !Number.isInteger(value) || value < 0 || value > 65535) {fail};\n{indent}words[{word}] = value;\n",
                indent = body_indent,
                fail = fail("must be an integer from 0 to 65535"),
            )
        }
        "insets" => {
            let mut arm = format!(
                "{indent}const insetFields = value as {{ top?: unknown; right?: unknown; bottom?: unknown; left?: unknown }};\n",
                indent = body_indent,
            );
            for (index, field) in ["top", "right", "bottom", "left"].iter().enumerate() {
                arm.push_str(&format!(
                    "{indent}if (typeof insetFields.{field} !== \"number\" || !Number.isInteger(insetFields.{field}) || insetFields.{field} < 0 || insetFields.{field} > 65535) {fail};\n{indent}words[{slot}] = insetFields.{field};\n",
                    indent = body_indent,
                    slot = word + index as u32,
                    fail = fail(&format!("{field} must be an integer from 0 to 65535")),
                ));
            }
            arm
        }
        "alignment" => {
            format!(
                "{indent}words[{word}] = packAlignment(value);\n",
                indent = body_indent,
            )
        }
        "edges" => {
            format!(
                "{indent}packBorderEdges(value, words, {word});\n",
                indent = body_indent,
            )
        }
        "color" => {
            format!(
                "{indent}strings[{string}] = packColorString(value);\n",
                indent = body_indent,
            )
        }
        "border_style" => {
            format!(
                "{indent}words[{word}] = value === \"plain\" ? 1 : value === \"rounded\" ? 2 : value === \"double\" ? 3 : (() => {{ {fail} }})();\n",
                indent = body_indent,
                fail = fail("must be plain, rounded, double, or null"),
            )
        }
        "glyphs" => {
            let mut arm = format!(
                "{indent}const glyphFields = value as {{ top?: unknown; right?: unknown; bottom?: unknown; left?: unknown; topLeft?: unknown; topRight?: unknown; bottomLeft?: unknown; bottomRight?: unknown }};\n",
                indent = body_indent,
            );
            for (index, field) in GLYPH_ORDER.iter().enumerate() {
                arm.push_str(&format!(
                    "{indent}if (typeof glyphFields.{field} !== \"string\") {fail};\n{indent}strings[{slot}] = glyphFields.{field};\n",
                    indent = body_indent,
                    slot = string + index as u32,
                    fail = fail(&format!("glyph {field} must be a string")),
                ));
            }
            arm
        }
        "text_attrs" => {
            format!(
                "{indent}packTextAttributes(value, words, {word});\n",
                indent = body_indent,
            )
        }
        "style" => {
            format!(
                "{indent}packStyle(value, words, {word}, strings, {string});\n",
                indent = body_indent,
            )
        }
        other => panic!("state renderer has no TS packer for value kind {other}"),
    };
    output.push_str(&body);
    if property.nullable {
        output.push_str(&format!("{indent}}}\n"));
    }
    output
}

fn shouty(name: &str) -> String {
    let mut output = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            output.push('_');
        }
        output.extend(character.to_uppercase());
    }
    output
}

/// Rust mask/lane tables plus the envelope header check, one module per
/// domain. The hand-written decoder in `tui/view_state.rs` reads slots
/// through these offsets and validates headers through `check_envelope`;
/// value semantics stay in hand-written readers.
pub fn rust_schema(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = banner(schema_hash, generator_hash);
    // Included via `include!` inside a module body, so no inner attributes
    // (`#![...]`, `//!`) may appear here; the `mod` declaration carries the
    // outer `#[allow(dead_code)]` instead.
    output.push_str(
        "// L1-05 retained-state envelope tables (§7.1).\n//\n// Single source of truth: the `state_property` rows in\n// `tools/tui-abi/view_abi.toml`. Bit ids are dense per domain; the TS\n// envelope packers write the same lanes. Hand-written readers own value\n// semantics; these tables own masks, offsets, and codes.\n\n",
    );
    output.push_str(&format!(
        "/// Primitive wake disposition bit: the environment must drain.\npub const WAKE_SCHEDULE_ENVIRONMENT_DRAIN: u32 = {};\n\n",
        WAKE_DRAIN_BIT
    ));
    for domain in ["geometry", "presentation"] {
        let properties = domain_properties(document, domain);
        output.push_str(&format!("pub mod {domain} {{\n"));
        output.push_str(&format!(
            "    pub const PROPERTY_COUNT: u32 = {};\n    pub const ALL_MASK: u32 = {:#x};\n    pub const NULLABLE_MASK: u32 = {:#x};\n    pub const CLEARABLE_MASK: u32 = {:#x};\n    pub const WORD_COUNT: usize = {};\n    pub const STRING_COUNT: usize = {};\n",
            properties.len(),
            all_mask(&properties),
            nullable_mask(&properties),
            clearable_mask(&properties),
            word_count(&properties),
            string_count(&properties),
        ));
        for property in &properties {
            let upper = shouty(&property.name);
            output.push_str(&format!(
                "    pub const ID_{upper}: u32 = {};\n    pub const {upper}_NAME: &str = {:?};\n",
                property.id, property.name
            ));
            output.push_str(&format!(
                "    pub const {upper}_WORD_OFFSET: u32 = {};\n    pub const {upper}_WORD_LEN: u32 = {};\n    pub const {upper}_STRING_OFFSET: u32 = {};\n    pub const {upper}_STRING_LEN: u32 = {};\n",
                word_offset(&properties, property.id),
                property.words,
                string_offset(&properties, property.id),
                property.strings,
            ));
        }
        output.push_str(&rust_codes(domain));
        output.push_str(&rust_check_envelope(
            domain,
            all_mask(&properties),
            nullable_mask(&properties),
            clearable_mask(&properties),
            word_count(&properties),
            string_count(&properties),
        ));
        output.push_str("}\n\n");
    }
    output
}

/// Fixed sub-encoding codes shared with the TS pack helpers above.
fn rust_codes(domain: &str) -> String {
    let mut output = String::new();
    if domain == "geometry" {
        output.push_str(
            "    pub const SIZE_MODE_FIT: u32 = 1;\n    pub const SIZE_MODE_FILL: u32 = 2;\n    pub const ALIGN_H_START: u32 = 1;\n    pub const ALIGN_H_CENTER: u32 = 2;\n    pub const ALIGN_H_END: u32 = 3;\n    pub const ALIGN_V_TOP: u32 = 1;\n    pub const ALIGN_V_CENTER: u32 = 2;\n    pub const ALIGN_V_BOTTOM: u32 = 3;\n    pub const ALIGN_V_SHIFT: u32 = 3;\n    pub const EDGE_KIND_OBJECT: u32 = 0;\n    pub const EDGE_KIND_ALL: u32 = 1;\n    pub const EDGE_KIND_TOP_BOTTOM: u32 = 2;\n    pub const EDGE_BIT_TOP: u32 = 1;\n    pub const EDGE_BIT_RIGHT: u32 = 2;\n    pub const EDGE_BIT_BOTTOM: u32 = 4;\n    pub const EDGE_BIT_LEFT: u32 = 8;\n",
        );
    } else {
        output.push_str(
            "    pub const BORDER_STYLE_PLAIN: u32 = 1;\n    pub const BORDER_STYLE_ROUNDED: u32 = 2;\n    pub const BORDER_STYLE_DOUBLE: u32 = 3;\n    pub const TEXT_ATTR_BIT_BOLD: u32 = 1;\n    pub const TEXT_ATTR_BIT_DIM: u32 = 2;\n    pub const TEXT_ATTR_BIT_ITALIC: u32 = 4;\n    pub const TEXT_ATTR_BIT_UNDERLINE: u32 = 8;\n    pub const TEXT_ATTR_BIT_REVERSED: u32 = 16;\n    pub const TEXT_ATTR_BIT_STRIKETHROUGH: u32 = 32;\n",
        );
    }
    output
}

/// Envelope header rules (§7.1): no unknown bits, `null_mask ⊆ set_mask`,
/// null only where nullable, `clear_mask ∩ set_mask = ∅`, set calls carry
/// no clear mask and clear calls carry no set values, and lanes arrive at
/// exactly the schema lengths.
fn rust_check_envelope(
    domain: &str,
    all: u32,
    nullable: u32,
    clearable: u32,
    words: u32,
    strings: u32,
) -> String {
    format!(
        "    pub fn check_envelope(\n        set_mask: u32,\n        null_mask: u32,\n        clear_mask: u32,\n        for_set: bool,\n        words: usize,\n        strings: usize,\n    ) -> Result<(), String> {{\n        if set_mask & !{all:#x} != 0 {{\n            return Err(\"ViewState {domain} envelope sets unknown properties\".to_owned());\n        }}\n        if null_mask & !set_mask != 0 {{\n            return Err(\"ViewState {domain} envelope null mask escapes the set mask\".to_owned());\n        }}\n        if null_mask & !{nullable:#x} != 0 {{\n            return Err(\"ViewState {domain} envelope nulls a non-nullable property\".to_owned());\n        }}\n        if clear_mask & set_mask != 0 {{\n            return Err(\"ViewState {domain} envelope clears and sets the same properties\".to_owned());\n        }}\n        if for_set && clear_mask != 0 {{\n            return Err(\"ViewState {domain} set envelope carries a clear mask\".to_owned());\n        }}\n        if !for_set && (set_mask != 0 || null_mask != 0) {{\n            return Err(\"ViewState {domain} clear envelope carries set values\".to_owned());\n        }}\n        if clear_mask & !{all:#x} != 0 {{\n            return Err(\"ViewState {domain} envelope clears unknown properties\".to_owned());\n        }}\n        if clear_mask & !{clearable:#x} != 0 {{\n            return Err(\"ViewState {domain} envelope clears a non-clearable property\".to_owned());\n        }}\n        if for_set {{\n            if words != {words} {{\n                return Err(\"ViewState {domain} envelope words lane has the wrong length\".to_owned());\n            }}\n            if strings != {strings} {{\n                return Err(\"ViewState {domain} envelope strings lane has the wrong length\".to_owned());\n            }}\n        }} else if words != 0 || strings != 0 {{\n            return Err(\"ViewState {domain} clear envelope carries value lanes\".to_owned());\n        }}\n        Ok(())\n    }}\n"
    )
}

fn ts_clear_packer(domain: &str, properties: &[&StatePropertySpec]) -> String {
    let pascal = pascal(domain);
    let mut output = format!(
        "/** Packs an already-normalized {domain} clear list into a clear mask. Unknown names cannot arrive here (the normalizer rejects them); the throw below is unreachable defense. */\nexport function encode{pascal}ClearMask(properties: readonly string[]): number {{\n  let mask = 0;\n",
    );
    output.push_str("  for (const property of properties) {\n    const id = ({\n");
    for property in properties {
        output.push_str(&format!("      {}: {},\n", property.name, property.id));
    }
    output.push_str("    } as Record<string, number | undefined>)[property];\n");
    output.push_str(
        "    if (id === undefined) throw new TypeError(`state envelope: unknown clear property ${JSON.stringify(property)}`);\n    mask |= 1 << id;\n  }\n  return mask;\n}\n\n",
    );
    output
}
