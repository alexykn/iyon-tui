use std::collections::HashSet;

use thiserror::Error;

use crate::model::{
    UiAbiDocument, UiCodeSpec, UiControlCommandSpec, UiKindSpec, UiOpcodeSpec, UiPropertySpec,
    UiSectionSpec,
};

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    Invalid(String),
}

/// Validate the direct-occurrence schema independently from the historical
/// View ABI.  Keeping this finite and explicit is important: a permissive
/// property bag would move semantic ownership back into the transport.
pub fn validate_ui(document: &UiAbiDocument) -> Result<(), ValidationError> {
    if document.abi.name.is_empty() || !is_snake_case(&document.abi.name) {
        return invalid("UI ABI name must be a non-empty snake_case identifier");
    }
    if document.abi.version == 0 || document.abi.semantic_schema == 0 {
        return invalid("UI ABI version and semantic_schema must be non-zero");
    }
    if document.abi.minimum_bun != "1.4.0" || document.abi.qualified_bun != "1.4.0" {
        return invalid("UI ABI Bun versions must be exactly 1.4.0");
    }
    if document.abi.result_encoding != "u32_ack_v1" {
        return invalid("UI ABI result_encoding must be u32_ack_v1");
    }
    if document.abi.magic != 0x4959_5549
        || document.abi.batch_version != 1
        || document.abi.batch_header_words != 16
        || document.abi.ack_header_words != 8
        || document.abi.ack_words_per_created_handle != 4
        || document.abi.handle_words != 4
        || document.abi.local_handle_host != 0
        || document.abi.local_handle_generation != 0
        || document.abi.ack_status_error_bit != 0x8000_0000
        || document.abi.ack_reserved != 0
    {
        return invalid("UI ABI has unsupported version-one wire constants");
    }
    validate_ui_sections(&document.sections)?;
    validate_ui_kinds(&document.host_kinds)?;
    validate_ui_codes("control kind", &document.control_kinds, 1..=0xffff_ffff)?;
    validate_ui_codes("root role", &document.root_roles, 1..=0xffff_ffff)?;
    validate_ui_codes("handle kind", &document.handle_kinds, 1..=0xffff_ffff)?;
    validate_ui_codes("ownership mode", &document.ownership_modes, 1..=0xffff_ffff)?;
    validate_ui_codes("value kind", &document.value_kinds, 1..=0xffff_ffff)?;
    validate_ui_value_encodings(document)?;
    validate_ui_codes("effect", &document.effects, 0..=31)?;
    validate_ui_opcodes(&document.opcodes, &document.sections)?;
    validate_ui_control_commands(&document.control_commands, &document.control_kinds)?;
    validate_ui_configs(document)?;
    validate_ui_properties(document)?;
    Ok(())
}

fn validate_ui_sections(sections: &[UiSectionSpec]) -> Result<(), ValidationError> {
    const REQUIRED: [(&str, u32); 5] = [
        ("structure", 1),
        ("state", 2),
        ("content_control", 3),
        ("events", 4),
        ("content_descriptor", 5),
    ];
    if sections.len() != REQUIRED.len() {
        return invalid("UI ABI must declare exactly the version-one sections");
    }
    let mut names = HashSet::new();
    let mut codes = HashSet::new();
    for section in sections {
        if !is_snake_case(&section.name)
            || !names.insert(section.name.as_str())
            || !codes.insert(section.code)
        {
            return invalid(format!(
                "UI section {} is invalid or duplicated",
                section.name
            ));
        }
    }
    for (name, code) in REQUIRED {
        if !sections
            .iter()
            .any(|section| section.name == name && section.code == code)
        {
            return invalid(format!("UI section {name} must have code {code}"));
        }
    }
    Ok(())
}

fn validate_ui_kinds(kinds: &[UiKindSpec]) -> Result<(), ValidationError> {
    if kinds.is_empty() {
        return invalid("UI ABI must declare at least one host kind");
    }
    let mut names = HashSet::new();
    let mut codes = HashSet::new();
    for kind in kinds {
        if !is_pascal_case(&kind.name) {
            return invalid(format!("host kind {} must be PascalCase", kind.name));
        }
        if !names.insert(kind.name.as_str()) || !codes.insert(kind.code) {
            return invalid(format!("host kinds contain duplicate {}", kind.name));
        }
        if kind.code == 0 || kind.code > 32 {
            return invalid(format!("host kind {} code must be in 1..=32", kind.name));
        }
        if !matches!(kind.children.as_str(), "none" | "many" | "frames") {
            return invalid(format!(
                "host kind {} has unsupported child cardinality {}",
                kind.name, kind.children
            ));
        }
    }
    Ok(())
}

fn validate_ui_codes(
    family: &str,
    values: &[UiCodeSpec],
    allowed: std::ops::RangeInclusive<u32>,
) -> Result<(), ValidationError> {
    if values.is_empty() {
        return invalid(format!("UI ABI must declare at least one {family}"));
    }
    let mut names = HashSet::new();
    let mut codes = HashSet::new();
    for value in values {
        if !is_pascal_case(&value.name) {
            return invalid(format!("{family} {} must be PascalCase", value.name));
        }
        if !names.insert(value.name.as_str()) || !codes.insert(value.code) {
            return invalid(format!("{family} {} is duplicated", value.name));
        }
        if !allowed.contains(&value.code) {
            return invalid(format!("{family} {} has an out-of-range code", value.name));
        }
    }
    Ok(())
}

fn validate_ui_opcodes(
    opcodes: &[UiOpcodeSpec],
    sections: &[UiSectionSpec],
) -> Result<(), ValidationError> {
    if opcodes.is_empty() {
        return invalid("UI ABI must declare at least one opcode");
    }
    let mut names = HashSet::new();
    let mut codes = HashSet::new();
    let section_names: HashSet<&str> = sections
        .iter()
        .map(|section| section.name.as_str())
        .collect();
    for opcode in opcodes {
        if !is_pascal_case(&opcode.name) {
            return invalid(format!("UI opcode {} must be PascalCase", opcode.name));
        }
        if !names.insert(opcode.name.as_str()) || !codes.insert(opcode.code) {
            return invalid(format!("UI opcode {} is duplicated", opcode.name));
        }
        if opcode.code == 0 || opcode.code > 0xff {
            return invalid(format!("UI opcode {} must fit one byte", opcode.name));
        }
        if !section_names.contains(opcode.section.as_str()) {
            return invalid(format!(
                "UI opcode {} has unsupported section {}",
                opcode.name, opcode.section
            ));
        }
        if opcode.operands.is_empty()
            || opcode
                .operands
                .iter()
                .any(|operand| operand.is_empty() || operand.chars().any(char::is_whitespace))
        {
            return invalid(format!("UI opcode {} has invalid operands", opcode.name));
        }
    }
    Ok(())
}

fn validate_ui_control_commands(
    commands: &[UiControlCommandSpec],
    control_kinds: &[UiCodeSpec],
) -> Result<(), ValidationError> {
    if commands.is_empty() {
        return invalid("UI ABI must declare at least one control command");
    }
    let kind_names: HashSet<&str> = control_kinds
        .iter()
        .map(|kind| kind.name.as_str())
        .collect();
    let mut names = HashSet::new();
    let mut codes = HashSet::new();
    for command in commands {
        if !is_pascal_case(&command.name)
            || !names.insert(command.name.as_str())
            || !codes.insert(command.code)
        {
            return invalid(format!(
                "control command {} is invalid or duplicated",
                command.name
            ));
        }
        if command.code == 0 || command.code > 0xffff {
            return invalid(format!(
                "control command {} code is out of range",
                command.name
            ));
        }
        if !kind_names.contains(command.control_kind.as_str()) {
            return invalid(format!(
                "control command {} refers to unknown control kind {}",
                command.name, command.control_kind
            ));
        }
        if command
            .operands
            .iter()
            .any(|operand| operand.is_empty() || operand.chars().any(char::is_whitespace))
        {
            return invalid(format!(
                "control command {} has invalid operands",
                command.name
            ));
        }
    }
    Ok(())
}

fn validate_ui_configs(document: &UiAbiDocument) -> Result<(), ValidationError> {
    let control_kinds: HashSet<&str> = document
        .control_kinds
        .iter()
        .map(|kind| kind.name.as_str())
        .collect();
    let root_roles: HashSet<&str> = document
        .root_roles
        .iter()
        .map(|role| role.name.as_str())
        .collect();
    let mut names = HashSet::new();
    for config in &document.configs {
        if !is_pascal_case(&config.name) || !names.insert(config.name.as_str()) {
            return invalid(format!(
                "UI config {} is invalid or duplicated",
                config.name
            ));
        }
        match config.owner.as_str() {
            "control" if !control_kinds.contains(config.kind.as_str()) => {
                return invalid(format!(
                    "UI config {} refers to unknown control kind {}",
                    config.name, config.kind
                ));
            }
            "root" if !root_roles.contains(config.kind.as_str()) => {
                return invalid(format!(
                    "UI config {} refers to unknown root role {}",
                    config.name, config.kind
                ));
            }
            "control" | "root" => {}
            _ => return invalid(format!("UI config {} has an invalid owner", config.name)),
        }
        if !matches!(
            config.value.as_str(),
            "bool" | "u32" | "flow_boundary" | "unit_identity"
        ) {
            return invalid(format!(
                "UI config {} has an unsupported value kind {}",
                config.name, config.value
            ));
        }
    }
    Ok(())
}

fn validate_ui_value_encodings(document: &UiAbiDocument) -> Result<(), ValidationError> {
    if document.value_encodings.len() != document.value_kinds.len() {
        return invalid("UI ABI must declare one encoding descriptor per value kind");
    }
    let kinds: HashSet<&str> = document
        .value_kinds
        .iter()
        .map(|kind| kind.name.as_str())
        .collect();
    let mut seen = HashSet::new();
    for encoding in &document.value_encodings {
        if !kinds.contains(encoding.value_kind.as_str())
            || !seen.insert(encoding.value_kind.as_str())
        {
            return invalid(format!(
                "UI value encoding {} is unknown or duplicated",
                encoding.value_kind
            ));
        }
        if encoding.encoding.is_empty()
            || encoding.encoding.chars().any(char::is_whitespace)
            || encoding.min_words == 0
            || encoding.min_words > encoding.max_words
            || encoding.metadata_words > encoding.max_words
            || encoding.forms.is_empty()
        {
            return invalid(format!(
                "UI value encoding {} has invalid bounds or name",
                encoding.value_kind
            ));
        }
        let mut form_names = HashSet::new();
        for form in &encoding.forms {
            let mut enum_names = HashSet::new();
            let mut tags = HashSet::new();
            if form.name.is_empty()
                || form.name.chars().any(char::is_whitespace)
                || !form_names.insert(form.name.as_str())
                || form.word_count < encoding.min_words
                || form.word_count > encoding.max_words
                || form.mask.is_some_and(|mask| mask == 0)
                || (!form.names.is_empty()
                    && (form.names.len() != form.values.len()
                        || form
                            .names
                            .iter()
                            .any(|name| name.is_empty() || !enum_names.insert(name.as_str())))
                    || form.tags.iter().any(|tag| !tags.insert(*tag)))
            {
                return invalid(format!(
                    "UI value encoding {} has an invalid or duplicated form",
                    encoding.value_kind
                ));
            }
        }
    }
    Ok(())
}

fn validate_ui_properties(document: &UiAbiDocument) -> Result<(), ValidationError> {
    if document.properties.is_empty() {
        return invalid("UI ABI must declare at least one property");
    }
    let kind_names: HashSet<String> = document
        .host_kinds
        .iter()
        .map(|kind| kind.name.clone())
        .collect();
    let value_names: HashSet<String> = document
        .value_kinds
        .iter()
        .map(|value| value.name.clone())
        .collect();
    let effect_names: HashSet<String> = document
        .effects
        .iter()
        .map(|effect| effect.name.clone())
        .collect();
    let mut ids = HashSet::new();
    let mut names: HashSet<String> = HashSet::new();
    let mut domains: HashSet<String> = HashSet::new();
    for property in &document.properties {
        validate_ui_property(
            property,
            &kind_names,
            &value_names,
            &effect_names,
            &mut ids,
            &mut names,
            &mut domains,
        )?;
    }
    if !domains.contains("geometry") || !domains.contains("presentation") {
        return invalid("UI ABI properties must cover geometry and presentation domains");
    }
    Ok(())
}

fn validate_ui_property(
    property: &UiPropertySpec,
    kind_names: &HashSet<String>,
    value_names: &HashSet<String>,
    effect_names: &HashSet<String>,
    ids: &mut HashSet<u32>,
    names: &mut HashSet<String>,
    domains: &mut HashSet<String>,
) -> Result<(), ValidationError> {
    if property.id == 0 {
        return invalid(format!("UI property {} has a zero id", property.name));
    }
    if !ids.insert(property.id) || !names.insert(property.name.clone()) {
        return invalid(format!("UI property {} is duplicated", property.name));
    }
    if property.name.is_empty()
        || property.name.chars().any(char::is_whitespace)
        || property.domain.is_empty()
    {
        return invalid(format!(
            "UI property {} has an invalid name/domain",
            property.name
        ));
    }
    domains.insert(property.domain.clone());
    if !value_names.contains(&property.value) {
        return invalid(format!(
            "UI property {} refers to unknown value kind {}",
            property.name, property.value
        ));
    }
    if property.legal_kinds.is_empty()
        || property
            .legal_kinds
            .iter()
            .any(|kind| !kind_names.contains(kind))
    {
        return invalid(format!(
            "UI property {} has an unknown or empty legal kind set",
            property.name
        ));
    }
    if property.normalizer.is_empty()
        || property.default.is_empty()
        || property.reset.is_empty()
        || property.override_behavior.is_empty()
        || property.inheritance.is_empty()
        || property.realization.is_empty()
        || property.effects.is_empty()
        || property
            .effects
            .iter()
            .any(|effect| !effect_names.contains(effect))
    {
        return invalid(format!(
            "UI property {} has incomplete metadata",
            property.name
        ));
    }
    let mut allowed = HashSet::new();
    if property.allowed_values.iter().any(|value| {
        value.is_empty() || value.chars().any(char::is_whitespace) || !allowed.insert(value)
    }) {
        return invalid(format!(
            "UI property {} has invalid or duplicate allowed values",
            property.name
        ));
    }
    Ok(())
}

fn is_snake_case(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && !value.starts_with('_')
        && !value.ends_with('_')
}

fn is_pascal_case(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn invalid(message: impl Into<String>) -> Result<(), ValidationError> {
    Err(ValidationError::Invalid(message.into()))
}
