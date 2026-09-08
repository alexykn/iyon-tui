use std::collections::HashSet;

use serde_json::Map;
use thiserror::Error;

use crate::model::{
    AbiDocument, ConformanceSpec, EnumSpec, PodSpec, UiAbiDocument, UiCodeSpec,
    UiControlCommandSpec, UiKindSpec, UiOpcodeSpec, UiPropertySpec, UiSectionSpec,
};

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    Invalid(String),
}

pub fn validate(
    document: &AbiDocument,
    kind_codes: &Map<String, serde_json::Value>,
) -> Result<(), ValidationError> {
    if document.abi.name.is_empty() || !is_snake_case(&document.abi.name) {
        return invalid("abi.name must be a non-empty snake_case identifier");
    }
    if document.abi.version == 0 || document.abi.semantic_schema == 0 {
        return invalid("abi.version and abi.semantic_schema must be non-zero");
    }
    if document.abi.minimum_bun != "1.4.0" || document.abi.qualified_bun != "1.4.0" {
        return invalid(
            "abi.minimum_bun and abi.qualified_bun must be exactly 1.4.0 for Tranche 1",
        );
    }
    if document.abi.result_encoding != "u32_high_bit_status" {
        return invalid("abi.result_encoding must be u32_high_bit_status");
    }

    let mut handle_names = HashSet::new();
    for handle in &document.handles {
        if !is_pascal_case(&handle.name) {
            return invalid(format!("handle {} must be PascalCase", handle.name));
        }
        if !handle_names.insert(handle.name.as_str()) {
            return invalid(format!("duplicate handle {}", handle.name));
        }
        if handle.rust.is_empty() || handle.typescript.is_empty() || handle.lifetime.is_empty() {
            return invalid(format!("handle {} has an empty ABI property", handle.name));
        }
        if handle.kind.is_some() != handle.valid.is_some() {
            return invalid(format!(
                "handle {} must specify both kind and valid, or neither",
                handle.name
            ));
        }
    }

    let mut enum_names = HashSet::new();
    for enum_spec in &document.enums {
        validate_enum(enum_spec, kind_codes)?;
        if !enum_names.insert(enum_spec.name.as_str()) {
            return invalid(format!("duplicate enum {}", enum_spec.name));
        }
    }

    let mut pod_names = HashSet::new();
    for pod in &document.pods {
        validate_pod(pod)?;
        if !pod_names.insert(pod.name.as_str()) {
            return invalid(format!("duplicate POD struct {}", pod.name));
        }
    }

    let mut function_names = HashSet::new();
    for conformance in &document.conformance {
        validate_conformance(conformance)?;
    }

    for function in &document.functions {
        if !is_snake_case(&function.name) {
            return invalid(format!("function {} must be snake_case", function.name));
        }
        if !function_names.insert(function.name.as_str()) {
            return invalid(format!("duplicate function {}", function.name));
        }
        if function.family.is_empty()
            || function.hotness.is_empty()
            || function.implementation.is_empty()
            || function.ownership.is_empty()
            || function.borrow_duration.is_empty()
            || function.thread_affinity.is_empty()
            || function.benchmark_registration.is_empty()
        {
            return invalid(format!(
                "function {} has an empty ABI property",
                function.name
            ));
        }
        if !is_snake_case(&function.implementation) {
            return invalid(format!(
                "implementation {} must be snake_case",
                function.implementation
            ));
        }
        if function.borrow_duration != "call"
            || function.thread_affinity != "owner_thread"
            || function.max_input_count > 16 * 1024 * 1024
            || function.max_buffer_bytes > 16 * 1024 * 1024
            || function
                .arity_specializations
                .windows(2)
                .any(|window| window[0] >= window[1])
            || function
                .arity_specializations
                .iter()
                .any(|arity| *arity > 16)
        {
            return invalid(format!(
                "function {} has unsupported ownership, lifetime, thread, or bound policy",
                function.name
            ));
        }
        if !matches!(
            function.return_type.as_str(),
            "u32"
                | "i32"
                | "ViewRefResult"
                | "PathRefResult"
                | "StyleRefResult"
                | "StyleAtomRefResult"
                | "status_only"
                | "native_ref_result"
        ) {
            return invalid(format!(
                "function {} has unsupported return type {}",
                function.name, function.return_type
            ));
        }

        let mut argument_names = HashSet::new();
        let mut variable_buffers = 0;
        for argument in &function.args {
            if !is_snake_case(&argument.name) {
                return invalid(format!(
                    "argument {}.{} must be snake_case",
                    function.name, argument.name
                ));
            }
            if !argument_names.insert(argument.name.as_str()) {
                return invalid(format!(
                    "duplicate argument {}.{}",
                    function.name, argument.name
                ));
            }
            validate_type(&argument.type_name, document, function.name.as_str())?;
            if argument.lowering == "buffer_length" {
                let Some(length_of) = argument.buffer_length_of.as_deref() else {
                    return invalid(format!(
                        "buffer_length argument {}.{} must declare buffer_length_of",
                        function.name, argument.name
                    ));
                };
                if !argument_names.contains(length_of)
                    || !function.args.iter().any(|candidate| {
                        candidate.name == length_of
                            && matches!(candidate.lowering.as_str(), "buffer" | "pod_slice")
                    })
                {
                    return invalid(format!(
                        "{}.{} refers to unknown buffer {}",
                        function.name, argument.name, length_of
                    ));
                }
            } else if argument.buffer_length_of.is_some() {
                return invalid(format!(
                    "only buffer_length arguments may declare buffer_length_of: {}.{}",
                    function.name, argument.name
                ));
            }
            if !matches!(
                argument.lowering.as_str(),
                "u8" | "u16"
                    | "u32"
                    | "i32"
                    | "f32"
                    | "f64"
                    | "node_id_pair"
                    | "native_ref"
                    | "runtime_ptr"
                    | "host_ptr"
                    | "buffer"
                    | "buffer_length"
                    | "buffer_used"
                    | "cstring_ephemeral"
                    | "pod_slice"
                    | "status_only"
                    | "native_ref_result"
            ) {
                return invalid(format!(
                    "argument {}.{} has unsupported lowering {}",
                    function.name, argument.name, argument.lowering
                ));
            }
            validate_lowering(argument, document, function.name.as_str())?;
            if matches!(argument.lowering.as_str(), "buffer" | "pod_slice") {
                variable_buffers += 1;
            }
        }
        // PERF-12 T11 (§41): at most two variable buffers so the diff lane can
        // carry framed metadata words plus its UTF-8 byte payload in ONE call.
        // Each buffer still needs its own buffer_length/buffer_used pair and
        // the shared per-function max_buffer_bytes bound.
        if variable_buffers > 2 {
            return invalid(format!(
                "function {} has more than two variable buffers",
                function.name
            ));
        }
        if variable_buffers > 0 && function.max_buffer_bytes == 0 {
            return invalid(format!(
                "buffer function {} must declare max_buffer_bytes",
                function.name
            ));
        }
        let used_count = function
            .args
            .iter()
            .filter(|argument| argument.lowering == "buffer_used")
            .count();
        if variable_buffers > 0 && used_count != variable_buffers {
            return invalid(format!(
                "each buffer in {} must have exactly one buffer_used argument",
                function.name
            ));
        }
        // PERF-12 T11 (§41): multi-buffer functions must pair every
        // buffer_used with its target explicitly and unambiguously.
        if variable_buffers > 1 {
            let mut linked_targets = HashSet::new();
            for argument in &function.args {
                if argument.lowering != "buffer_used" {
                    continue;
                }
                let Some(target) = argument.buffer_used_of.as_deref() else {
                    return invalid(format!(
                        "buffer_used argument {}.{} must declare buffer_used_of on a multi-buffer function",
                        function.name, argument.name
                    ));
                };
                if !function.args.iter().any(|candidate| {
                    candidate.name == target
                        && matches!(candidate.lowering.as_str(), "buffer" | "pod_slice")
                }) {
                    return invalid(format!(
                        "buffer_used argument {}.{} refers to unknown buffer {}",
                        function.name, argument.name, target
                    ));
                }
                if !linked_targets.insert(target.to_owned()) {
                    return invalid(format!(
                        "buffer {} in {} has more than one buffer_used argument",
                        target, function.name
                    ));
                }
            }
            if linked_targets.len() != variable_buffers {
                return invalid(format!(
                    "every buffer in {} must have exactly one buffer_used argument",
                    function.name
                ));
            }
        }
        if variable_buffers == 0 && used_count != 0 {
            return invalid(format!(
                "buffer_used argument {} has no buffer",
                function.name
            ));
        }
        for argument in &function.args {
            if !matches!(argument.lowering.as_str(), "buffer" | "pod_slice") {
                continue;
            }
            let length_count = function
                .args
                .iter()
                .filter(|candidate| {
                    candidate.lowering == "buffer_length"
                        && candidate.buffer_length_of.as_deref() == Some(argument.name.as_str())
                })
                .count();
            if length_count != 1 {
                return invalid(format!(
                    "buffer argument {}.{} must have exactly one buffer_length pair",
                    function.name, argument.name
                ));
            }
            let Some(element_size) = buffer_element_size(argument, document) else {
                return invalid(format!(
                    "buffer argument {}.{} has no fixed element size",
                    function.name, argument.name
                ));
            };
            let required_bytes =
                u64::from(function.max_input_count).saturating_mul(u64::from(element_size));
            if required_bytes > function.max_buffer_bytes {
                return invalid(format!(
                    "buffer function {} permits {} bytes but max_buffer_bytes is {}",
                    function.name, required_bytes, function.max_buffer_bytes
                ));
            }
        }
    }

    validate_state_properties(document)?;

    Ok(())
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
            if form.name.is_empty()
                || form.name.chars().any(char::is_whitespace)
                || !form_names.insert(form.name.as_str())
                || form.word_count < encoding.min_words
                || form.word_count > encoding.max_words
                || form.mask.is_some_and(|mask| mask == 0)
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
    Ok(())
}

/// L1-05 retained-state property schema (§7.1). Bit ids must be dense per
/// domain starting at 0 so masks stay u32 and reordering rows never changes
/// an id; value kinds and capabilities come from fixed vocabularies so the
/// envelope renderers can match them exhaustively.
fn validate_state_properties(document: &AbiDocument) -> Result<(), ValidationError> {
    const DOMAINS: [&str; 2] = ["geometry", "presentation"];
    const VALUES: [&str; 10] = [
        "size_mode",
        "u16",
        "insets",
        "alignment",
        "edges",
        "color",
        "border_style",
        "glyphs",
        "text_attrs",
        "style",
    ];
    const CAPABILITIES: [&str; 2] = ["node-kind", "node-kind+axis"];
    for domain in DOMAINS {
        let mut properties: Vec<&crate::model::StatePropertySpec> = document
            .state_properties
            .iter()
            .filter(|property| property.domain == domain)
            .collect();
        if properties.is_empty() {
            return invalid(format!(
                "state domain `{domain}` must declare at least one property"
            ));
        }
        if properties.len() > 32 {
            return invalid(format!(
                "state domain `{domain}` declares {} properties but masks are u32",
                properties.len()
            ));
        }
        properties.sort_by_key(|property| property.id);
        let mut names = HashSet::new();
        for (index, property) in properties.iter().enumerate() {
            if property.id != index as u32 {
                return invalid(format!(
                    "state domain `{domain}` property ids must be dense from 0; expected id {index} for `{}`",
                    property.name
                ));
            }
            if property.name.is_empty() {
                return invalid(format!(
                    "state domain `{domain}` property id {} has an empty diagnostic name",
                    property.id
                ));
            }
            if !names.insert(property.name.as_str()) {
                return invalid(format!(
                    "state domain `{domain}` declares duplicate property `{}`",
                    property.name
                ));
            }
            if !VALUES.contains(&property.value.as_str()) {
                return invalid(format!(
                    "state property `{}.{}` has unknown native value kind `{}`",
                    domain, property.name, property.value
                ));
            }
            if !CAPABILITIES.contains(&property.capability.as_str()) {
                return invalid(format!(
                    "state property `{}.{}` has unknown capability rule `{}`",
                    domain, property.name, property.capability
                ));
            }
            let expected_lanes = match property.value.as_str() {
                "size_mode" | "u16" | "alignment" | "border_style" => (1, 0),
                "insets" => (4, 0),
                "edges" => (2, 0),
                "color" => (0, 1),
                "glyphs" => (0, 8),
                "text_attrs" => (2, 0),
                "style" => (2, 3),
                // `property.value` was checked against `VALUES` above. Keep
                // this arm explicit so a new value kind cannot accidentally
                // bypass the lane-shape check while this validator is being
                // updated.
                other => unreachable!("validated state value kind {other}"),
            };
            if (property.words, property.strings) != expected_lanes {
                return invalid(format!(
                    "state property `{}.{}` has the wrong lane shape: expected words={}, strings={}, got words={}, strings={}",
                    domain,
                    property.name,
                    expected_lanes.0,
                    expected_lanes.1,
                    property.words,
                    property.strings
                ));
            }
        }
    }
    let mut domains = HashSet::new();
    for property in &document.state_properties {
        if !DOMAINS.contains(&property.domain.as_str()) {
            return invalid(format!(
                "state property `{}` has unknown domain `{}`",
                property.name, property.domain
            ));
        }
        domains.insert(property.domain.as_str());
    }
    Ok(())
}

fn validate_conformance(conformance: &ConformanceSpec) -> Result<(), ValidationError> {
    if !is_snake_case(&conformance.name) {
        return invalid(format!(
            "conformance {} must be snake_case",
            conformance.name
        ));
    }
    if !matches!(
        conformance.return_type.as_str(),
        "u32" | "i32" | "f32" | "f64"
    ) {
        return invalid(format!(
            "conformance {} has unsupported return type {}",
            conformance.name, conformance.return_type
        ));
    }
    if !matches!(
        conformance.operation.as_str(),
        "position_weighted_sum" | "pointer_probe" | "buffer_probe" | "cstring_hash"
    ) {
        return invalid(format!(
            "conformance {} has unsupported operation {}",
            conformance.name, conformance.operation
        ));
    }
    if conformance.args.len() > 16 {
        return invalid(format!(
            "conformance {} exceeds the representative maximum arity",
            conformance.name
        ));
    }
    match conformance.operation.as_str() {
        "position_weighted_sum" => {
            if conformance.args.is_empty()
                || conformance.args.iter().any(|arg| {
                    !matches!(arg.as_str(), "u8" | "u16" | "u32" | "i32" | "f32" | "f64")
                })
                || conformance.args.windows(2).any(|args| args[0] != args[1])
                || conformance.return_type
                    != match conformance.args[0].as_str() {
                        "i32" => "i32",
                        "f32" => "f32",
                        "f64" => "f64",
                        _ => "u32",
                    }
            {
                return invalid(format!(
                    "conformance {} has an invalid weighted scalar signature",
                    conformance.name
                ));
            }
        }
        "pointer_probe" => {
            if conformance.args.len() != 1
                || conformance.args[0] != "ptr"
                || conformance.return_type != "u32"
            {
                return invalid(format!(
                    "conformance {} must be ptr -> u32",
                    conformance.name
                ));
            }
        }
        "buffer_probe" => {
            if conformance.args.len() != 2
                || conformance.args[0] != "buffer"
                || conformance.args[1] != "buffer_length"
                || conformance.return_type != "u32"
            {
                return invalid(format!(
                    "conformance {} must be buffer + buffer_length -> u32",
                    conformance.name
                ));
            }
        }
        "cstring_hash" => {
            if conformance.args.len() != 1
                || conformance.args[0] != "cstring"
                || conformance.return_type != "u32"
            {
                return invalid(format!(
                    "conformance {} must be cstring -> u32",
                    conformance.name
                ));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn validate_enum(
    enum_spec: &EnumSpec,
    kind_codes: &Map<String, serde_json::Value>,
) -> Result<(), ValidationError> {
    if !is_pascal_case(&enum_spec.name) {
        return invalid(format!("enum {} must be PascalCase", enum_spec.name));
    }
    if enum_spec.repr != "u32" {
        return invalid(format!("enum {} must use u32 in tranche 1", enum_spec.name));
    }
    if enum_spec.values.is_empty() {
        return invalid(format!(
            "enum {} must define at least one value",
            enum_spec.name
        ));
    }
    let mut names = HashSet::new();
    for value in &enum_spec.values {
        if !is_pascal_case(&value.name) || !names.insert(value.name.as_str()) {
            return invalid(format!(
                "enum {} has an invalid or duplicate value {}",
                enum_spec.name, value.name
            ));
        }
        let Some(number) = kind_codes
            .get(&value.source_key)
            .and_then(serde_json::Value::as_u64)
        else {
            return invalid(format!(
                "enum {} value {} does not resolve integer kind-codes key {}",
                enum_spec.name, value.name, value.source_key
            ));
        };
        if number > u32::MAX as u64 {
            return invalid(format!(
                "kind-codes key {} does not fit u32",
                value.source_key
            ));
        }
    }
    Ok(())
}

fn validate_type(
    type_name: &str,
    document: &AbiDocument,
    function_name: &str,
) -> Result<(), ValidationError> {
    let primitive = matches!(type_name, "u8" | "u16" | "u32" | "i32" | "f32" | "f64");
    let builtin = matches!(type_name, "u8[]" | "u32[]" | "NodeId" | "string");
    let pod = type_name
        .strip_suffix("[]")
        .is_some_and(|name| document.pods.iter().any(|item| item.name == name));
    let handle = document.handles.iter().any(|item| item.name == type_name);
    let enum_type = document.enums.iter().any(|item| item.name == type_name);
    if !(primitive || builtin || pod || handle || enum_type) {
        return invalid(format!(
            "function {} refers to unknown type {}",
            function_name, type_name
        ));
    }
    Ok(())
}

fn validate_lowering(
    argument: &crate::model::ArgumentSpec,
    document: &AbiDocument,
    function_name: &str,
) -> Result<(), ValidationError> {
    let type_name = argument.type_name.as_str();
    let enum_or_u32 = type_name == "u32"
        || document.enums.iter().any(|item| item.name == type_name)
        || document
            .handles
            .iter()
            .any(|item| item.name == type_name && item.rust == "u32");
    let valid = match argument.lowering.as_str() {
        "u8" => type_name == "u8",
        "u16" => type_name == "u16",
        "u32" | "buffer_used" | "buffer_length" => enum_or_u32,
        "i32" | "status_only" => type_name == "i32",
        "f32" => type_name == "f32",
        "f64" => type_name == "f64",
        "node_id_pair" => type_name == "NodeId",
        "native_ref" => document.handles.iter().any(|handle| {
            handle.name == type_name && handle.rust == "u32" && handle.kind.is_some()
        }),
        "runtime_ptr" => type_name == "RuntimePtr",
        "host_ptr" => type_name == "HostPtr",
        "buffer" => type_name == "u32[]" || type_name.ends_with("[]"),
        "pod_slice" => type_name
            .strip_suffix("[]")
            .is_some_and(|name| document.pods.iter().any(|pod| pod.name == name)),
        "cstring_ephemeral" => type_name == "string",
        "native_ref_result" => enum_or_u32,
        _ => false,
    };
    if valid {
        return Ok(());
    }
    invalid(format!(
        "argument {}.{} type {} is incompatible with lowering {}",
        function_name, argument.name, argument.type_name, argument.lowering
    ))
}

fn buffer_element_size(
    argument: &crate::model::ArgumentSpec,
    document: &AbiDocument,
) -> Option<u32> {
    if argument.type_name == "u8[]" {
        return Some(1);
    }
    if argument.type_name == "u32[]" {
        return Some(4);
    }
    let pod_name = argument.type_name.strip_suffix("[]")?;
    document
        .pods
        .iter()
        .find(|pod| pod.name == pod_name)
        .map(|pod| pod.size)
}

fn validate_pod(pod: &PodSpec) -> Result<(), ValidationError> {
    if !is_pascal_case(&pod.name) || pod.repr != "C" {
        return invalid(format!("POD {} must be PascalCase and repr = C", pod.name));
    }
    if pod.size == 0 || pod.align == 0 || !pod.align.is_power_of_two() {
        return invalid(format!("POD {} has invalid size/alignment", pod.name));
    }
    if pod.fields.is_empty() {
        return invalid(format!("POD {} must define fields", pod.name));
    }
    let mut names = HashSet::new();
    let mut offset = 0u32;
    let mut max_align = 1u32;
    for field in &pod.fields {
        if !is_snake_case(&field.name) || !names.insert(field.name.as_str()) {
            return invalid(format!(
                "POD {} has an invalid or duplicate field {}",
                pod.name, field.name
            ));
        }
        let Some((size, align)) = primitive_layout(&field.type_name) else {
            return invalid(format!(
                "POD {} field {} must be a fixed-width primitive",
                pod.name, field.name
            ));
        };
        offset = align_up(offset, align).saturating_add(size);
        max_align = max_align.max(align);
    }
    let expected_size = align_up(offset, max_align);
    if expected_size != pod.size || max_align != pod.align {
        return invalid(format!(
            "POD {} declares size/alignment {}/{} but fields require {}/{}",
            pod.name, pod.size, pod.align, expected_size, max_align
        ));
    }
    Ok(())
}

fn primitive_layout(type_name: &str) -> Option<(u32, u32)> {
    match type_name {
        "u8" => Some((1, 1)),
        "u16" => Some((2, 2)),
        "u32" | "i32" | "f32" => Some((4, 4)),
        "f64" => Some((8, 8)),
        _ => None,
    }
}

fn align_up(value: u32, align: u32) -> u32 {
    value.div_ceil(align) * align
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
