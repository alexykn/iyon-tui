use std::fmt::Write;

use serde_json::json;

use crate::model::{UiAbiDocument, UiCodeSpec, UiKindSpec};

pub fn rust_schema(document: &UiAbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = format!(
        "// DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml.\n// schema_blake3 = {schema_hash}\n// generator_blake3 = {generator_hash}\n\n#![allow(dead_code)]\n\n"
    );
    writeln!(
        output,
        "pub const UI_ABI_NAME: &str = {:?};\npub const UI_ABI_VERSION: u32 = {};\npub const UI_SEMANTIC_SCHEMA_VERSION: u32 = {};\npub const UI_MINIMUM_BUN: &str = {:?};\npub const UI_QUALIFIED_BUN: &str = {:?};\npub const UI_BATCH_MAGIC: u32 = {magic};\npub const UI_BATCH_VERSION: u32 = {batch_version};\npub const UI_BATCH_HEADER_WORDS: usize = {batch_header_words};\npub const UI_ACK_HEADER_WORDS: usize = {ack_header_words};\npub const UI_ACK_WORDS_PER_CREATED_HANDLE: usize = {ack_words_per_created_handle};\npub const UI_HANDLE_WORDS: usize = {handle_words};\npub const UI_LOCAL_HANDLE_HOST: u32 = {local_handle_host};\npub const UI_LOCAL_HANDLE_GENERATION: u32 = {local_handle_generation};\npub const UI_ACK_STATUS_ERROR_BIT: u32 = {ack_status_error_bit};\npub const UI_ACK_RESERVED: u32 = {ack_reserved};\n",
        document.abi.name,
        document.abi.version,
        document.abi.semantic_schema,
        document.abi.minimum_bun,
        document.abi.qualified_bun,
        magic = rust_hex(document.abi.magic),
        batch_version = document.abi.batch_version,
        batch_header_words = document.abi.batch_header_words,
        ack_header_words = document.abi.ack_header_words,
        ack_words_per_created_handle = document.abi.ack_words_per_created_handle,
        handle_words = document.abi.handle_words,
        local_handle_host = document.abi.local_handle_host,
        local_handle_generation = document.abi.local_handle_generation,
        ack_status_error_bit = rust_hex(document.abi.ack_status_error_bit),
        ack_reserved = document.abi.ack_reserved,
    )
    .expect("writing generated UI constants cannot fail");
    render_rust_code_enum(&mut output, "HostKind", &document.host_kinds);
    output.push_str(
        "impl HostKind {\n    pub const fn accepts_children(self) -> bool {\n        match self {\n",
    );
    for kind in &document.host_kinds {
        writeln!(
            output,
            "            Self::{} => {},",
            kind.name,
            kind.children != "none"
        )
        .expect("writing generated host child cardinality cannot fail");
    }
    output.push_str("        }\n    }\n\n    pub const fn child_cardinality(self) -> &'static str {\n        match self {\n");
    for kind in &document.host_kinds {
        writeln!(
            output,
            "            Self::{} => {:?},",
            kind.name, kind.children
        )
        .expect("writing generated host cardinality metadata cannot fail");
    }
    output.push_str("        }\n    }\n}\n\n");
    render_rust_code_enum(&mut output, "ControlKind", &document.control_kinds);
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct ControlCommandDescriptor {\n    pub name: &'static str,\n    pub code: u32,\n    pub control_kind: ControlKind,\n    pub operands: &'static [&'static str],\n}\n\npub const CONTROL_COMMAND_DESCRIPTORS: &[ControlCommandDescriptor] = &[\n",
    );
    for command in &document.control_commands {
        writeln!(
            output,
            "    ControlCommandDescriptor {{ name: {:?}, code: {}, control_kind: ControlKind::{}, operands: &[{}] }},",
            command.name,
            command.code,
            command.control_kind,
            command
                .operands
                .iter()
                .map(|operand| format!("{operand:?}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing generated control command cannot fail");
    }
    output.push_str(
        "];\n\npub fn control_command_descriptor(code: u32) -> Option<&'static ControlCommandDescriptor> {\n    CONTROL_COMMAND_DESCRIPTORS.iter().find(|command| command.code == code)\n}\n\n",
    );
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct UiConfigDescriptor {\n    pub name: &'static str,\n    pub owner: &'static str,\n    pub kind: &'static str,\n    pub value: &'static str,\n}\n\npub const UI_CONFIG_DESCRIPTORS: &[UiConfigDescriptor] = &[\n",
    );
    for config in &document.configs {
        writeln!(
            output,
            "    UiConfigDescriptor {{ name: {:?}, owner: {:?}, kind: {:?}, value: {:?} }},",
            config.name, config.owner, config.kind, config.value,
        )
        .expect("writing generated UI config cannot fail");
    }
    output.push_str("];\n\n");
    render_rust_code_enum(&mut output, "RootRole", &document.root_roles);
    render_rust_code_enum(&mut output, "HandleKind", &document.handle_kinds);
    render_rust_code_enum(&mut output, "OwnershipMode", &document.ownership_modes);
    render_rust_code_enum(&mut output, "ValueKind", &document.value_kinds);
    output.push_str(
        "#[derive(Clone, Copy, Debug)]\npub struct ValueEncodingDescriptor {\n    pub value_kind: ValueKind,\n    pub encoding: &'static str,\n    pub min_words: usize,\n    pub max_words: usize,\n    pub metadata_words: usize,\n    pub forms: &'static [ValueEncodingForm],\n}\n\n#[derive(Clone, Copy, Debug)]\npub struct ValueEncodingForm {\n    pub name: &'static str,\n    pub word_count: usize,\n    pub tags: &'static [u32],\n    pub values: &'static [u32],\n    pub mask: Option<u32>,\n    pub max_value: Option<u32>,\n}\n\npub const VALUE_ENCODING_DESCRIPTORS: &[ValueEncodingDescriptor] = &[\n",
    );
    for encoding in &document.value_encodings {
        writeln!(
            output,
            "    ValueEncodingDescriptor {{ value_kind: ValueKind::{}, encoding: {:?}, min_words: {}, max_words: {}, metadata_words: {}, forms: &[{}] }},",
            encoding.value_kind,
            encoding.encoding,
            encoding.min_words,
            encoding.max_words,
            encoding.metadata_words,
            encoding
                .forms
                .iter()
                .map(|form| format!(
                    "ValueEncodingForm {{ name: {:?}, word_count: {}, tags: &{:?}, values: &{:?}, mask: {:?}, max_value: {:?} }}",
                    form.name,
                    form.word_count,
                    form.tags,
                    form.values,
                    form.mask,
                    form.max_value,
                ))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing generated value encoding cannot fail");
    }
    output.push_str(
        "];\n\npub fn value_encoding(value_kind: ValueKind) -> &'static ValueEncodingDescriptor {\n    VALUE_ENCODING_DESCRIPTORS.iter().find(|descriptor| descriptor.value_kind as u32 == value_kind as u32).expect(\"validated value encoding descriptor\")\n}\n\npub fn value_encoding_form(value_kind: ValueKind, name: &str) -> &'static ValueEncodingForm {\n    value_encoding(value_kind).forms.iter().find(|form| form.name == name).expect(\"validated value encoding form\")\n}\n\n",
    );

    output.push_str(
        "#[repr(u32)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum Effect {\n",
    );
    for effect in &document.effects {
        writeln!(output, "    {} = {},", effect.name, effect.code)
            .expect("writing generated effect cannot fail");
    }
    output.push_str("}\n\n#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]\npub struct EffectMask(pub u32);\n\nimpl EffectMask {\n    pub const NONE: Self = Self(0);\n    pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }\n    pub const fn union(self, other: Self) -> Self { Self(self.0 | other.0) }\n    pub const fn is_empty(self) -> bool { self.0 == 0 }\n}\n\n");
    for effect in &document.effects {
        writeln!(
            output,
            "pub const EFFECT_{}: EffectMask = EffectMask(1 << {});",
            upper_snake(&effect.name),
            effect.code
        )
        .expect("writing generated effect mask cannot fail");
    }
    output.push('\n');

    output.push_str(
        "#[repr(u32)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum UiSection {\n",
    );
    for section in &document.sections {
        writeln!(output, "    {} = {},", pascal(&section.name), section.code)
            .expect("writing generated UI section cannot fail");
    }
    output.push_str("}\n\n");
    output.push_str(
        "#[repr(u32)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum UiOpcode {\n",
    );
    for opcode in &document.opcodes {
        writeln!(output, "    {} = 0x{:02x},", opcode.name, opcode.code)
            .expect("writing generated opcode cannot fail");
    }
    output.push_str("}\n\n");
    output.push_str("#[derive(Clone, Copy, Debug)]\npub struct OpcodeDescriptor {\n    pub opcode: UiOpcode,\n    pub section: UiSection,\n    pub name: &'static str,\n    pub operands: &'static [&'static str],\n}\n\npub const OPCODE_DESCRIPTORS: &[OpcodeDescriptor] = &[\n");
    for opcode in &document.opcodes {
        writeln!(
            output,
            "    OpcodeDescriptor {{ opcode: UiOpcode::{}, section: UiSection::{}, name: {:?}, operands: &[{}] }},",
            opcode.name,
            pascal(&opcode.section),
            opcode.name,
            opcode
                .operands
                .iter()
                .map(|operand| format!("{:?}", operand))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing generated opcode descriptor cannot fail");
    }
    output.push_str("];\n\n");

    output.push_str(
        "#[repr(u32)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum PropertyId {\n",
    );
    for property in &document.properties {
        writeln!(
            output,
            "    {} = 0x{:04x},",
            pascal(&property.name),
            property.id
        )
        .expect("writing generated property id cannot fail");
    }
    output.push_str("}\n\nimpl PropertyId {\n    pub const ALL: &[Self] = &[\n");
    for property in &document.properties {
        writeln!(output, "        Self::{},", pascal(&property.name))
            .expect("writing generated property list cannot fail");
    }
    output.push_str("    ];\n\n    pub const fn from_raw(value: u32) -> Option<Self> {\n        match value {\n");
    for property in &document.properties {
        writeln!(
            output,
            "            0x{:04x} => Some(Self::{}),",
            property.id,
            pascal(&property.name)
        )
        .expect("writing generated property lookup cannot fail");
    }
    output.push_str("            _ => None,\n        }\n    }\n\n    pub const fn index(self) -> usize {\n        match self {\n");
    for (index, property) in document.properties.iter().enumerate() {
        writeln!(
            output,
            "            Self::{} => {},",
            pascal(&property.name),
            index
        )
        .expect("writing generated property index cannot fail");
    }
    output.push_str("        }\n    }\n}\n\n");

    output.push_str("#[derive(Clone, Copy, Debug)]\npub struct PropertyDescriptor {\n    pub id: PropertyId,\n    pub name: &'static str,\n    pub domain: &'static str,\n    pub value_kind: ValueKind,\n    pub legal_kinds: &'static [HostKind],\n    pub normalizer: &'static str,\n    pub default: &'static str,\n    pub reset: &'static str,\n    pub override_behavior: &'static str,\n    pub inheritance: &'static str,\n    pub effects: EffectMask,\n    pub realization: &'static str,\n    pub nullable: bool,\n    pub clearable: bool,\n}\n\npub const PROPERTY_DESCRIPTORS: &[PropertyDescriptor] = &[\n");
    for property in &document.properties {
        writeln!(
            output,
            "    PropertyDescriptor {{ id: PropertyId::{}, name: {:?}, domain: {:?}, value_kind: ValueKind::{}, legal_kinds: &[{}], normalizer: {:?}, default: {:?}, reset: {:?}, override_behavior: {:?}, inheritance: {:?}, effects: {}, realization: {:?}, nullable: {}, clearable: {} }},",
            pascal(&property.name),
            property.name,
            property.domain,
            property.value,
            property
                .legal_kinds
                .iter()
                .map(|kind| format!("HostKind::{kind}"))
                .collect::<Vec<_>>()
                .join(", "),
            property.normalizer,
            property.default,
            property.reset,
            property.override_behavior,
            property.inheritance,
            effect_mask_expression(document, &property.effects),
            property.realization,
            property.nullable,
            property.clearable,
        )
        .expect("writing generated property descriptor cannot fail");
    }
    output.push_str("];\n\npub const PROPERTY_COUNT: usize = PROPERTY_DESCRIPTORS.len();\n\npub const fn property_descriptor(id: PropertyId) -> &'static PropertyDescriptor {\n    &PROPERTY_DESCRIPTORS[id.index()]\n}\n");
    crate::render_rust::format_rust(output)
}

fn render_rust_code_enum(output: &mut String, enum_name: &str, values: &[impl CodeValue]) {
    writeln!(
        output,
        "#[repr(u32)]\n#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]\npub enum {enum_name} {{"
    )
    .expect("writing generated enum cannot fail");
    for value in values {
        writeln!(output, "    {} = {},", value.name(), value.code())
            .expect("writing generated enum value cannot fail");
    }
    output.push_str("}\n\n");
    writeln!(
        output,
        "impl {enum_name} {{\n    pub const fn from_code(value: u32) -> Option<Self> {{\n        match value {{"
    )
    .expect("writing generated enum conversion cannot fail");
    for value in values {
        writeln!(
            output,
            "            {} => Some(Self::{}),",
            value.code(),
            value.name()
        )
        .expect("writing generated enum conversion arm cannot fail");
    }
    output.push_str("            _ => None,\n        }\n    }\n\n    pub const fn code(self) -> u32 { self as u32 }\n");
    output.push_str("}\n\n");
}

trait CodeValue {
    fn name(&self) -> &str;
    fn code(&self) -> u32;
}

impl CodeValue for UiCodeSpec {
    fn name(&self) -> &str {
        &self.name
    }

    fn code(&self) -> u32 {
        self.code
    }
}

impl CodeValue for UiKindSpec {
    fn name(&self) -> &str {
        &self.name
    }

    fn code(&self) -> u32 {
        self.code
    }
}

pub fn typescript_schema(
    document: &UiAbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = ui_banner(schema_hash, generator_hash);
    output.push_str("/** Generated direct-occurrence UI schema; do not edit. */\n");
    writeln!(
        output,
        "export const UI_ABI_NAME = {:?} as const;\nexport const UI_ABI_VERSION = {} as const;\nexport const UI_SEMANTIC_SCHEMA_VERSION = {} as const;\nexport const UI_BATCH_MAGIC = 0x{magic:08x} as const;\nexport const UI_BATCH_VERSION = {batch_version} as const;\nexport const UI_BATCH_HEADER_WORDS = {batch_header_words} as const;\nexport const UI_ACK_HEADER_WORDS = {ack_header_words} as const;\nexport const UI_ACK_WORDS_PER_CREATED_HANDLE = {ack_words_per_created_handle} as const;\nexport const UI_HANDLE_WORDS = {handle_words} as const;\nexport const UI_LOCAL_HANDLE_HOST = {local_handle_host} as const;\nexport const UI_LOCAL_HANDLE_GENERATION = {local_handle_generation} as const;\nexport const UI_ACK_STATUS_ERROR_BIT = 0x{ack_status_error_bit:08x} as const;\n",
        document.abi.name,
        document.abi.version,
        document.abi.semantic_schema,
        magic = document.abi.magic,
        batch_version = document.abi.batch_version,
        batch_header_words = document.abi.batch_header_words,
        ack_header_words = document.abi.ack_header_words,
        ack_words_per_created_handle = document.abi.ack_words_per_created_handle,
        handle_words = document.abi.handle_words,
        local_handle_host = document.abi.local_handle_host,
        local_handle_generation = document.abi.local_handle_generation,
        ack_status_error_bit = document.abi.ack_status_error_bit,
    )
    .expect("writing generated TypeScript constants cannot fail");
    render_typescript_codes(&mut output, "HOST_KINDS", &document.host_kinds);
    output.push_str("export const HOST_KIND_CHILDREN = {\n");
    for kind in &document.host_kinds {
        writeln!(
            output,
            "  {}: {:?},",
            lower_camel(&kind.name),
            kind.children
        )
        .expect("writing generated TypeScript host cardinality cannot fail");
    }
    output.push_str("} as const;\n\n");
    render_typescript_codes(&mut output, "CONTROL_KINDS", &document.control_kinds);
    output.push_str(
        "export interface UiControlCommandDescriptor {\n  readonly name: string;\n  readonly code: number;\n  readonly controlKind: ControlKindName;\n  readonly operands: readonly string[];\n}\n\nexport const UI_CONTROL_COMMAND_DESCRIPTORS: readonly UiControlCommandDescriptor[] = [\n",
    );
    for command in &document.control_commands {
        writeln!(
            output,
            "  {{ name: {:?}, code: {}, controlKind: {:?}, operands: [{}] }},",
            command.name,
            command.code,
            command.control_kind,
            command
                .operands
                .iter()
                .map(|operand| format!("{operand:?}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing generated TS control command cannot fail");
    }
    output.push_str("];\n\n");
    output.push_str(
        "export interface UiConfigDescriptor {\n  readonly name: string;\n  readonly owner: \"control\" | \"root\";\n  readonly kind: string;\n  readonly value: string;\n}\n\nexport const UI_CONFIG_DESCRIPTORS: readonly UiConfigDescriptor[] = [\n",
    );
    for config in &document.configs {
        writeln!(
            output,
            "  {{ name: {:?}, owner: {:?}, kind: {:?}, value: {:?} }},",
            config.name, config.owner, config.kind, config.value,
        )
        .expect("writing generated TS UI config cannot fail");
    }
    output.push_str("];\n\n");
    render_typescript_codes(&mut output, "ROOT_ROLES", &document.root_roles);
    render_typescript_codes(&mut output, "HANDLE_KINDS", &document.handle_kinds);
    render_typescript_codes(&mut output, "OWNERSHIP_MODES", &document.ownership_modes);
    render_typescript_codes(&mut output, "VALUE_KINDS", &document.value_kinds);
    output.push_str("export interface UiValueEncodingDescriptor {\n  readonly valueKind: ValueKindName;\n  readonly encoding: string;\n  readonly minWords: number;\n  readonly maxWords: number;\n  readonly metadataWords: number;\n  readonly forms: readonly UiValueEncodingForm[];\n}\n\nexport interface UiValueEncodingForm {\n  readonly name: string;\n  readonly wordCount: number;\n  readonly tags: readonly number[];\n  readonly values: readonly number[];\n  readonly mask: number | undefined;\n  readonly maxValue: number | undefined;\n}\n\nexport const UI_VALUE_ENCODING_DESCRIPTORS: readonly UiValueEncodingDescriptor[] = [\n");
    for encoding in &document.value_encodings {
        writeln!(
            output,
            "  {{ valueKind: {:?}, encoding: {:?}, minWords: {}, maxWords: {}, metadataWords: {}, forms: [{}] }},",
            encoding.value_kind,
            encoding.encoding,
            encoding.min_words,
            encoding.max_words,
            encoding.metadata_words,
            encoding
                .forms
                .iter()
                .map(|form| format!(
                    "{{ name: {:?}, wordCount: {}, tags: {:?}, values: {:?}, mask: {}, maxValue: {} }}",
                    form.name,
                    form.word_count,
                    form.tags,
                    form.values,
                    form.mask
                        .map_or_else(|| "undefined".to_owned(), |value| value.to_string()),
                    form.max_value
                        .map_or_else(|| "undefined".to_owned(), |value| value.to_string()),
                ))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing generated TS value encoding cannot fail");
    }
    output.push_str("];\n\nexport function uiValueEncoding(valueKind: ValueKindName): UiValueEncodingDescriptor {\n  const descriptor = UI_VALUE_ENCODING_DESCRIPTORS.find((value) => value.valueKind === valueKind);\n  if (!descriptor) throw new Error(\"unknown UI value encoding: \" + valueKind);\n  return descriptor;\n}\n\nexport function uiValueEncodingForm(valueKind: ValueKindName, name: string): UiValueEncodingForm {\n  const descriptor = uiValueEncoding(valueKind).forms.find((value) => value.name === name);\n  if (!descriptor) throw new Error(\"unknown UI value encoding form: \" + valueKind + \"/\" + name);\n  return descriptor;\n}\n\n");
    render_typescript_codes(&mut output, "EFFECTS", &document.effects);
    output.push_str("export const UI_SECTIONS = {\n");
    for section in &document.sections {
        writeln!(
            output,
            "  {}: {},",
            lower_camel(&section.name),
            section.code
        )
        .expect("writing generated TypeScript UI section cannot fail");
    }
    output.push_str("} as const;\n\n");
    output.push_str("export const UI_OPCODES = {\n");
    for opcode in &document.opcodes {
        writeln!(
            output,
            "  {}: 0x{:02x},",
            lower_camel(&opcode.name),
            opcode.code
        )
        .expect("writing generated TypeScript opcode cannot fail");
    }
    output.push_str("} as const;\n\nexport interface UiOpcodeDescriptor {\n  readonly name: string;\n  readonly code: number;\n  readonly section: keyof typeof UI_SECTIONS;\n  readonly operands: readonly string[];\n}\n\nexport const UI_OPCODE_DESCRIPTORS: readonly UiOpcodeDescriptor[] = [\n");
    for opcode in &document.opcodes {
        writeln!(
            output,
            "  {{ name: {:?}, code: 0x{:02x}, section: {:?}, operands: [{}] }},",
            opcode.name,
            opcode.code,
            lower_camel(&opcode.section),
            opcode
                .operands
                .iter()
                .map(|operand| format!("{operand:?}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing generated TypeScript opcode descriptor cannot fail");
    }
    output.push_str("];\n\n");
    output.push_str("export const UI_PROPERTIES = {\n");
    for property in &document.properties {
        writeln!(output, "  {}: 0x{:04x},", property.name, property.id)
            .expect("writing generated TypeScript property id cannot fail");
    }
    output.push_str("} as const;\nexport type UiPropertyName = keyof typeof UI_PROPERTIES;\nexport type UiPropertyId = typeof UI_PROPERTIES[UiPropertyName];\n\nexport interface UiPropertyDescriptor {\n  readonly id: number;\n  readonly name: UiPropertyName;\n  readonly domain: string;\n  readonly valueKind: ValueKindName;\n  readonly legalKinds: readonly HostKindName[];\n  readonly normalizer: string;\n  readonly default: string;\n  readonly reset: string;\n  readonly overrideBehavior: string;\n  readonly inheritance: string;\n  readonly effects: readonly EffectName[];\n  readonly realization: string;\n  readonly nullable: boolean;\n  readonly clearable: boolean;\n}\n\nexport const UI_PROPERTY_DESCRIPTORS: readonly UiPropertyDescriptor[] = [\n");
    for property in &document.properties {
        writeln!(
            output,
            "  {{ id: 0x{:04x}, name: {:?}, domain: {:?}, valueKind: {:?}, legalKinds: [{}], normalizer: {:?}, default: {:?}, reset: {:?}, overrideBehavior: {:?}, inheritance: {:?}, effects: [{}], realization: {:?}, nullable: {}, clearable: {} }},",
            property.id,
            property.name,
            property.domain,
            property.value,
            property
                .legal_kinds
                .iter()
                .map(|kind| format!("{kind:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            property.normalizer,
            property.default,
            property.reset,
            property.override_behavior,
            property.inheritance,
            property
                .effects
                .iter()
                .map(|effect| format!("{effect:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            property.realization,
            property.nullable,
            property.clearable,
        )
        .expect("writing generated TypeScript property descriptor cannot fail");
    }
    output.push_str("];\n\nexport function uiPropertyDescriptor(id: number): UiPropertyDescriptor | undefined {\n  return UI_PROPERTY_DESCRIPTORS.find((property) => property.id === id);\n}\n");
    output
}

fn render_typescript_codes(output: &mut String, name: &str, values: &[impl CodeValue]) {
    writeln!(output, "export const {name} = {{").expect("writing generated TS enum cannot fail");
    for value in values {
        writeln!(output, "  {}: {},", lower_camel(value.name()), value.code())
            .expect("writing generated TS enum value cannot fail");
    }
    output.push_str("} as const;\n");
    let type_name = typescript_type_name(name);
    writeln!(
        output,
        "export type {type_name} = typeof {name}[keyof typeof {name}];\n\n"
    )
    .expect("writing generated TS enum type cannot fail");
    let name_type = format!("{type_name}Name");
    writeln!(
        output,
        "export type {name_type} = {};
",
        values
            .iter()
            .map(|value| format!("{:?}", value.name()))
            .collect::<Vec<_>>()
            .join(" | ")
    )
    .expect("writing generated TS enum name type cannot fail");
}

pub fn manifest(
    document: &UiAbiDocument,
    schema_hash: &str,
    generator_hash: &str,
    output_paths: &[&str],
) -> String {
    let value = json!({
        "abi": {
            "name": document.abi.name,
            "version": document.abi.version,
            "semantic_schema": document.abi.semantic_schema,
            "minimum_bun": document.abi.minimum_bun,
            "qualified_bun": document.abi.qualified_bun,
            "result_encoding": document.abi.result_encoding,
            "magic": document.abi.magic,
            "batch_version": document.abi.batch_version,
            "batch_header_words": document.abi.batch_header_words,
            "ack_header_words": document.abi.ack_header_words,
            "ack_words_per_created_handle": document.abi.ack_words_per_created_handle,
            "handle_words": document.abi.handle_words,
            "local_handle_host": document.abi.local_handle_host,
            "local_handle_generation": document.abi.local_handle_generation,
            "ack_status_error_bit": document.abi.ack_status_error_bit,
            "ack_reserved": document.abi.ack_reserved,
        },
        "sections": document.sections,
        "schema_blake3": schema_hash,
        "generator_blake3": generator_hash,
        "host_kinds": document.host_kinds,
        "control_kinds": document.control_kinds,
        "root_roles": document.root_roles,
        "handle_kinds": document.handle_kinds,
        "ownership_modes": document.ownership_modes,
        "value_kinds": document.value_kinds,
        "value_encodings": document.value_encodings,
        "effects": document.effects,
        "opcodes": document.opcodes,
        "control_commands": document.control_commands,
        "configs": document.configs,
        "properties": document.properties,
        "generated_outputs": output_paths,
    });
    serde_json::to_string_pretty(&value).expect("UI ABI manifest is serializable") + "\n"
}

pub fn human_reference(
    document: &UiAbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = format!(
        "<!-- DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml. schema_blake3 = {schema_hash}; generator_blake3 = {generator_hash} -->\n\n# Direct UI ABI\n\n"
    );
    output.push_str("## Wire constants\n\n| Name | Value |\n|---|---:|\n");
    for (name, value) in [
        ("magic", document.abi.magic),
        ("batch_version", document.abi.batch_version),
        ("batch_header_words", document.abi.batch_header_words),
        ("ack_header_words", document.abi.ack_header_words),
        (
            "ack_words_per_created_handle",
            document.abi.ack_words_per_created_handle,
        ),
        ("handle_words", document.abi.handle_words),
        ("local_handle_host", document.abi.local_handle_host),
        (
            "local_handle_generation",
            document.abi.local_handle_generation,
        ),
        ("ack_status_error_bit", document.abi.ack_status_error_bit),
        ("ack_reserved", document.abi.ack_reserved),
    ] {
        writeln!(output, "| {name} | {value} |").expect("writing UI ABI constants cannot fail");
    }
    output.push_str("\n## Sections\n\n| Name | Code |\n|---|---:|\n");
    for section in &document.sections {
        writeln!(output, "| {} | {} |", section.name, section.code)
            .expect("writing UI ABI sections cannot fail");
    }
    output.push_str("## Host kinds\n\n| Name | Code | Children |\n|---|---:|---|\n");
    for kind in &document.host_kinds {
        writeln!(
            output,
            "| {} | {} | {} |",
            kind.name, kind.code, kind.children
        )
        .expect("writing UI ABI reference cannot fail");
    }
    output.push_str(
        "\n## Value encodings\n\n| Value kind | Encoding | Min words | Max words | Metadata words | Forms |\n|---|---|---:|---:|---:|---|\n",
    );
    for encoding in &document.value_encodings {
        writeln!(
            output,
            "| {} | {} | {} | {} | {} | {} |",
            encoding.value_kind,
            encoding.encoding,
            encoding.min_words,
            encoding.max_words,
            encoding.metadata_words,
            encoding
                .forms
                .iter()
                .map(|form| form.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
        .expect("writing UI ABI value encoding reference cannot fail");
    }
    output.push_str(
        "\n## Control commands\n\n| Name | Code | Control kind | Operands |\n|---|---:|---|---|\n",
    );
    for command in &document.control_commands {
        writeln!(
            output,
            "| {} | 0x{:x} | {} | {} |",
            command.name,
            command.code,
            command.control_kind,
            command.operands.join(", "),
        )
        .expect("writing UI ABI control command reference cannot fail");
    }
    output.push_str(
        "\n## Configuration fields\n\n| Name | Owner | Kind | Value |\n|---|---|---|---|\n",
    );
    for config in &document.configs {
        writeln!(
            output,
            "| {} | {} | {} | {} |",
            config.name, config.owner, config.kind, config.value,
        )
        .expect("writing UI ABI config reference cannot fail");
    }
    output.push_str("\n## Opcodes\n\n| Name | Code | Section | Operands |\n|---|---:|---|---|\n");
    for opcode in &document.opcodes {
        writeln!(
            output,
            "| {} | 0x{:02x} | {} | {} |",
            opcode.name,
            opcode.code,
            opcode.section,
            opcode.operands.join(", ")
        )
        .expect("writing UI ABI opcode reference cannot fail");
    }
    output.push_str("\n## Property coverage\n\n| ID | Domain | Name | Value | Legal kinds | Effects | Normalizer | Nullable | Clearable |\n|---:|---|---|---|---|---|---|---|---|\n");
    for property in &document.properties {
        writeln!(
            output,
            "| 0x{:08x} | {} | {} | {} | {} | {} | {} | {} | {} |",
            property.id,
            property.domain,
            property.name,
            property.value,
            property.legal_kinds.join(", "),
            property.effects.join(", "),
            property.normalizer,
            property.nullable,
            property.clearable,
        )
        .expect("writing UI ABI property reference cannot fail");
    }
    output.push('\n');
    output
}

fn effect_mask_expression(document: &UiAbiDocument, effects: &[String]) -> String {
    let mut expression = String::from("EffectMask::NONE");
    for effect in effects {
        let code = document
            .effects
            .iter()
            .find(|candidate| candidate.name == *effect)
            .expect("validated UI effect name")
            .code;
        expression.push_str(&format!(".union(EffectMask(1 << {code}))"));
    }
    expression
}

fn ui_banner(schema_hash: &str, generator_hash: &str) -> String {
    format!(
        "// DO NOT EDIT. Generated from tools/tui-abi/ui_abi.toml.\n// schema_blake3 = {schema_hash}\n// generator_blake3 = {generator_hash}\n\n"
    )
}

fn rust_hex(value: u32) -> String {
    format!("0x{:04x}_{:04x}", value >> 16, value & 0xffff)
}

fn pascal(value: &str) -> String {
    let mut output = String::new();
    for part in value.split('_') {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            output.extend(first.to_uppercase());
            output.push_str(chars.as_str());
        }
    }
    output
}

fn lower_camel(value: &str) -> String {
    let pascal = pascal(value);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn upper_snake(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() && index != 0 {
            output.push('_');
        }
        output.push(character.to_ascii_uppercase());
    }
    output
}

fn typescript_type_name(value: &str) -> &str {
    match value {
        "HOST_KINDS" => "HostKind",
        "CONTROL_KINDS" => "ControlKind",
        "ROOT_ROLES" => "RootRole",
        "HANDLE_KINDS" => "HandleKind",
        "OWNERSHIP_MODES" => "OwnershipMode",
        "VALUE_KINDS" => "ValueKind",
        "EFFECTS" => "Effect",
        _ => "UiCode",
    }
}
