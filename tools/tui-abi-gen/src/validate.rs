use std::collections::HashSet;

use serde_json::Map;
use thiserror::Error;

use crate::model::{
    AbiDocument, ConformanceSpec, EnumSpec, MaterializerFieldRole, MaterializerFieldSpec,
    MaterializerFixedArityAxisSpec, MaterializerSpec, PodSpec,
};

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    Invalid(String),
}

pub fn validate(
    document: &AbiDocument,
    bridge_schema: &Map<String, serde_json::Value>,
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
        validate_enum(enum_spec, bridge_schema)?;
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

    validate_materializers(document, bridge_schema)?;
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
            || function.fallback.is_empty()
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
        if variable_buffers > 1 {
            return invalid(format!(
                "function {} has more than one variable buffer in tranche 1",
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
    bridge_schema: &Map<String, serde_json::Value>,
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
        let Some(number) = bridge_schema
            .get(&value.source_key)
            .and_then(serde_json::Value::as_u64)
        else {
            return invalid(format!(
                "enum {} value {} does not resolve integer bridge key {}",
                enum_spec.name, value.name, value.source_key
            ));
        };
        if number > u32::MAX as u64 {
            return invalid(format!("bridge key {} does not fit u32", value.source_key));
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
    (value + align - 1) / align * align
}

/// PERF-12 T5 (§64): generator validation for semantic materializer
/// declarations. Generation must fail on illegal lifetime declarations,
/// unknown kinds, narrowed NodeIds, unrepresented child fields, unbounded
/// buffers, and missing benchmark/conformance registration.
fn validate_materializers(
    document: &AbiDocument,
    bridge_schema: &Map<String, serde_json::Value>,
) -> Result<(), ValidationError> {
    let function_names: HashSet<&str> = document
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    let handle_names: HashSet<&str> = document
        .handles
        .iter()
        .map(|handle| handle.name.as_str())
        .collect();
    // Scalar ABI types that lower directly as engine-native call arguments.
    const SCALAR_TYPES: [&str; 6] = ["u32", "i32", "u64", "i64", "f32", "f64"];

    let mut materializer_names = HashSet::new();
    for materializer in &document.materializers {
        validate_materializer(
            document,
            materializer,
            &function_names,
            &handle_names,
            &SCALAR_TYPES,
            bridge_schema,
        )?;
        if !materializer_names.insert(materializer.name.as_str()) {
            return invalid(format!("duplicate materializer {}", materializer.name));
        }
    }
    Ok(())
}

fn validate_materializer(
    document: &AbiDocument,
    materializer: &MaterializerSpec,
    function_names: &HashSet<&str>,
    handle_names: &HashSet<&str>,
    scalar_types: &[&str; 6],
    bridge_schema: &Map<String, serde_json::Value>,
) -> Result<(), ValidationError> {
    if !is_snake_case(&materializer.name) {
        return invalid(format!(
            "materializer {} must be snake_case",
            materializer.name
        ));
    }
    if function_names.contains(materializer.name.as_str()) {
        return invalid(format!(
            "materializer {} collides with an ABI function name",
            materializer.name
        ));
    }
    // Unknown BridgeViewNode kind (§64): the kind must exist in the bridge
    // schema as a view-kind discriminant.
    let kind_declared = bridge_schema
        .get(&materializer.bridge_kind)
        .is_some_and(|value| value.is_i64() || value.is_u64());
    if !kind_declared || !materializer.bridge_kind.starts_with("view") {
        return invalid(format!(
            "materializer {} declares unknown BridgeViewNode kind {}",
            materializer.name, materializer.bridge_kind
        ));
    }
    // The rust builder must be a declared ABI function returning ViewRefResult.
    let builder_function = document
        .functions
        .iter()
        .find(|function| function.name == materializer.rust_builder);
    let Some(builder_function) = builder_function else {
        return invalid(format!(
            "materializer {} references unknown builder function {}",
            materializer.name, materializer.rust_builder
        ));
    };
    if builder_function.return_type != "ViewRefResult" {
        return invalid(format!(
            "materializer {} builder {} must return ViewRefResult",
            materializer.name, materializer.rust_builder
        ));
    }
    // §68/§69: checked-vs-timing policy stays explicit and materializers run
    // synchronously on the environment owner thread with call-scoped borrows.
    if materializer.ownership != builder_function.ownership {
        return invalid(format!(
            "materializer {} ownership must match its builder function",
            materializer.name
        ));
    }
    if materializer.borrow_duration != "call" {
        return invalid(format!(
            "materializer {} may not retain a borrowed pointer past the call (§107)",
            materializer.name
        ));
    }
    if materializer.thread_affinity != "owner_thread"
        || materializer.thread_affinity != builder_function.thread_affinity
    {
        return invalid(format!(
            "materializer {} must run on the environment owner thread (§69)",
            materializer.name
        ));
    }
    if !matches!(
        materializer.status_detail.as_str(),
        "none" | "child_ref" | "base_ref"
    ) {
        return invalid(format!(
            "materializer {} has unsupported status_detail {} (§74)",
            materializer.name, materializer.status_detail
        ));
    }
    if materializer.benchmark_registration.is_empty() {
        return invalid(format!(
            "materializer {} is missing benchmark/conformance registration",
            materializer.name
        ));
    }
    if materializer.fallback.is_empty() {
        return invalid(format!(
            "materializer {} has no fallback declaration",
            materializer.name
        ));
    }
    if materializer.result.kind != "view_ref" {
        return invalid(format!(
            "materializer {} result kind must be view_ref",
            materializer.name
        ));
    }

    // PERF-12 T7 (§22/§32): fixed-arity axis shape rules.
    if let Some(axis) = &materializer.fixed_arity_axis {
        validate_fixed_arity_axis(document, materializer, axis, scalar_types)?;
    }

    // §64: a u64 field must never be narrowed into one u32 - the full 53-bit
    // safe NodeId requires exactly one low half and one high half.
    let low_count = materializer
        .fields
        .iter()
        .filter(|field| field.role == "node_id_low")
        .count();
    let high_count = materializer
        .fields
        .iter()
        .filter(|field| field.role == "node_id_high")
        .count();
    if low_count != 1 || high_count != 1 {
        return invalid(format!(
            "materializer {} must declare exactly one node_id_low and one node_id_high field",
            materializer.name
        ));
    }

    let mut seen_fields = HashSet::new();
    for field in &materializer.fields {
        validate_materializer_field(field, handle_names, scalar_types)?;
        if !seen_fields.insert(field.role.as_str()) {
            return invalid(format!(
                "materializer {} declares duplicate role {}",
                materializer.name, field.role
            ));
        }
    }
    Ok(())
}

fn validate_materializer_field(
    field: &MaterializerFieldSpec,
    handle_names: &HashSet<&str>,
    scalar_types: &[&str; 6],
) -> Result<(), ValidationError> {
    let Some(role) = MaterializerFieldRole::parse(&field.role) else {
        return invalid(format!(
            "materializer field {} has unknown role {}",
            field.name, field.role
        ));
    };
    if field.source.is_empty() {
        return invalid(format!(
            "materializer field {} has an empty source",
            field.name
        ));
    }
    let type_ok = scalar_types.contains(&field.abi_type.as_str())
        || handle_names.contains(field.abi_type.as_str());
    if !type_ok {
        return invalid(format!(
            "materializer field {} has undeclared ABI type {}",
            field.name, field.abi_type
        ));
    }
    if role.is_buffer() {
        // §64: buffer without explicit bounded length fails generation.
        if field.buffer_length_of.is_none() || field.max_buffer_bytes.is_none() {
            return invalid(format!(
                "buffer field {} must declare buffer_length_of and max_buffer_bytes",
                field.name
            ));
        }
        if field
            .max_buffer_bytes
            .is_some_and(|limit| limit > 16 * 1024 * 1024)
        {
            return invalid(format!(
                "buffer field {} exceeds the 16 MiB scratch bound",
                field.name
            ));
        }
    } else {
        if field.buffer_length_of.is_some() || field.max_buffer_bytes.is_some() {
            return invalid(format!(
                "non-buffer field {} must not declare buffer bounds",
                field.name
            ));
        }
        if role.is_reference() && field.abi_type != "ViewRef" && field.abi_type != "StyleRef" {
            return invalid(format!(
                "reference field {} must lower through ViewRef or StyleRef",
                field.name
            ));
        }
    }
    Ok(())
}

/// PERF-12 T7 (§22/§32): validation for fixed-arity axis materializers.
/// The bridge kind must be a layout axis; the constructor family must exist,
/// return ViewRefResult, and agree with the materializer's lifetime policy;
/// the field list must be exactly the axis scalars (node id halves + gap) —
/// children are lowered structurally, never declared as fields.
fn validate_fixed_arity_axis(
    document: &AbiDocument,
    materializer: &MaterializerSpec,
    axis: &MaterializerFixedArityAxisSpec,
    scalar_types: &[&str],
) -> Result<(), ValidationError> {
    if !matches!(materializer.bridge_kind.as_str(), "viewRow" | "viewColumn") {
        return invalid(format!(
            "materializer {} declares fixed_arity_axis on non-axis kind {}",
            materializer.name, materializer.bridge_kind
        ));
    }
    if axis.builders.is_empty() || axis.builders.len() > 8 {
        return invalid(format!(
            "materializer {} fixed-arity family must contain 1 through 8 builders",
            materializer.name
        ));
    }
    if materializer.rust_builder != axis.builders[0] {
        return invalid(format!(
            "materializer {} rust_builder must be the arity-0 family builder {}",
            materializer.name, axis.builders[0]
        ));
    }
    // PERF-12 T8 (§29): the borrowed-buffer lane builder must exist, return
    // ViewRefResult, and agree with the materializer's lifetime policy.
    if let Some(buffer_builder) = &axis.buffer_builder {
        let Some(builder) = document
            .functions
            .iter()
            .find(|function| function.name == *buffer_builder)
        else {
            return invalid(format!(
                "materializer {} buffer builder {} is not a declared ABI function",
                materializer.name, buffer_builder
            ));
        };
        if builder.return_type != "ViewRefResult" {
            return invalid(format!(
                "materializer {} buffer builder {} must return ViewRefResult",
                materializer.name, buffer_builder
            ));
        }
        if builder.ownership != materializer.ownership
            || builder.thread_affinity != materializer.thread_affinity
            || builder.borrow_duration != materializer.borrow_duration
        {
            return invalid(format!(
                "materializer {} buffer builder {} lifetime policy disagrees with the materializer",
                materializer.name, buffer_builder
            ));
        }
        if axis.builders.contains(buffer_builder) {
            return invalid(format!(
                "materializer {} buffer builder {} must not duplicate a family builder",
                materializer.name, buffer_builder
            ));
        }
    }

    let mut seen = HashSet::new();
    for (arity, builder_name) in axis.builders.iter().enumerate() {
        let Some(builder) = document
            .functions
            .iter()
            .find(|function| function.name == *builder_name)
        else {
            return invalid(format!(
                "materializer {} family builder {} (arity {}) is not a declared ABI function",
                materializer.name, builder_name, arity
            ));
        };
        if builder.return_type != "ViewRefResult" {
            return invalid(format!(
                "materializer {} family builder {} must return ViewRefResult",
                materializer.name, builder_name
            ));
        }
        if builder.ownership != materializer.ownership
            || builder.thread_affinity != materializer.thread_affinity
        {
            return invalid(format!(
                "materializer {} family builder {} lifetime policy disagrees with the materializer",
                materializer.name, builder_name
            ));
        }
        if !seen.insert(builder_name.as_str()) {
            return invalid(format!(
                "materializer {} declares duplicate family builder {}",
                materializer.name, builder_name
            ));
        }
    }
    // Axis fields: exactly one node_id pair and exactly one scalar (gap).
    let low_count = materializer
        .fields
        .iter()
        .filter(|field| field.role == "node_id_low")
        .count();
    let high_count = materializer
        .fields
        .iter()
        .filter(|field| field.role == "node_id_high")
        .count();
    let scalar_fields: Vec<_> = materializer
        .fields
        .iter()
        .filter(|field| field.role == "scalar")
        .collect();
    if low_count != 1
        || high_count != 1
        || scalar_fields.len() != 1
        || materializer.fields.len() != 3
    {
        return invalid(format!(
            "materializer {} axis shape requires exactly node_id_low, node_id_high, and one gap scalar",
            materializer.name
        ));
    }
    let gap = scalar_fields[0];
    if gap.source != "gap" || !scalar_types.contains(&gap.abi_type.as_str()) {
        return invalid(format!(
            "materializer {} axis gap field must source 'gap' as a scalar ABI type",
            materializer.name
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
