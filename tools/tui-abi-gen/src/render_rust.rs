use std::io::Write;
use std::process::{Command, Stdio};

use quote::quote;
use serde_json::Map;

use crate::{
    model::{AbiDocument, ArgumentSpec},
    render_manifest::banner,
};

pub fn types(
    document: &AbiDocument,
    bridge_schema: &Map<String, serde_json::Value>,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = banner(schema_hash, generator_hash);
    output.push_str(
        "#![allow(dead_code)]\n\n//! Canonical pointer-free ABI types and constants.\n\n",
    );
    output.push_str(&format!(
        "pub const SCHEMA_BLAKE3: &str = {:?};\npub const GENERATOR_BLAKE3: &str = {:?};\n\n",
        schema_hash, generator_hash
    ));
    output.push_str(&format!("pub const ABI_NAME: &str = {:?};\npub const ABI_VERSION: u32 = {};\npub const SEMANTIC_SCHEMA_VERSION: u32 = {};\npub const MINIMUM_BUN: &str = {:?};\npub const QUALIFIED_BUN: &str = {:?};\npub const RESULT_ERROR_BIT: u32 = 0x8000_0000;\n\n", document.abi.name, document.abi.version, document.abi.semantic_schema, document.abi.minimum_bun, document.abi.qualified_bun));
    output.push_str("pub type ViewRefResult = u32;\npub type StyleRefResult = u32;\npub type StyleAtomRefResult = u32;\n\n");
    for pod in &document.pods {
        output.push_str("#[repr(C)]\n#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]\n");
        output.push_str(&format!("pub struct {} {{\n", pod.name));
        for field in &pod.fields {
            output.push_str(&format!(
                "    pub {}: {},\n",
                field.name,
                rust_type(&field.type_name)
            ));
        }
        output.push_str("}\n\n");
        output.push_str(&format!(
            "static_assertions::const_assert_eq!(::core::mem::size_of::<{}>(), {});\nstatic_assertions::const_assert_eq!(::core::mem::align_of::<{}>(), {});\n\n",
            pod.name, pod.size, pod.name, pod.align
        ));
    }
    output.push_str("#[repr(C)]\n#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]\npub struct NativeViewAbiHeader {\n    pub magic: u32,\n    pub abi_version: u32,\n    pub semantic_version: u32,\n    pub alive: u32,\n}\n\nstatic_assertions::const_assert_eq!(::core::mem::size_of::<NativeViewAbiHeader>(), 16);\n\n");
    for enum_spec in &document.enums {
        output.push_str("#[repr(u32)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
        output.push_str(&format!("pub enum {} {{\n", enum_spec.name));
        for value in &enum_spec.values {
            let number = bridge_schema
                .get(&value.source_key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            output.push_str(&format!("    {} = {},\n", value.name, number));
        }
        output.push_str("}\n\n");
        for value in &enum_spec.values {
            let number = bridge_schema
                .get(&value.source_key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            output.push_str(&format!(
                "static_assertions::const_assert_eq!({}::{} as u32, {});\n",
                enum_spec.name, value.name, number
            ));
        }
        output.push('\n');
    }
    format_rust(output)
}

pub fn exports(
    document: &AbiDocument,
    bridge_schema: &Map<String, serde_json::Value>,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut source = banner(schema_hash, generator_hash);
    source.push_str(
        "// Generated C ABI wrappers. Semantic implementations are handwritten and linked below.\n",
    );
    source.push_str(&format!("{}\n", export_imports(document)));
    source.push_str("pub mod generated_impls {\n");
    source.push_str(&format!("    {}\n", export_imports(document)));

    for function in &document.functions {
        source.push_str(&format!(
            "    unsafe extern \"Rust\" {{\n        pub fn {}({}) -> {};\n    }}\n",
            function.implementation,
            rust_arguments(&function.args, document),
            rust_type(function.return_type.as_str())
        ));
    }
    source.push_str("}\n\n");
    source.push_str("#[cfg(feature = \"fast-view-abi\")]\n#[allow(dead_code)]\nfn generated_catch_unwind<T: Copy>(work: impl FnOnce() -> Result<T, T>, _panic_value: T) -> T {\n    work().unwrap_or_else(|error| error)\n}\n\n#[cfg(not(feature = \"fast-view-abi\"))]\n#[allow(dead_code)]\nfn generated_catch_unwind<T: Copy>(work: impl FnOnce() -> Result<T, T>, panic_value: T) -> T {\n    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {\n        Ok(result) => result.unwrap_or_else(|error| error),\n        Err(_) => panic_value,\n    }\n}\n\n#[allow(dead_code)]\nfn generated_nonnull<T: Copy, P>(value: *mut P, error: T) -> Result<*mut P, T> {\n    if value.is_null() { Err(error) } else { Ok(value) }\n}\n\n#[allow(dead_code)]\nfn generated_nonnull_const<T: Copy, P>(value: *const P, error: T) -> Result<*const P, T> {\n    if value.is_null() { Err(error) } else { Ok(value) }\n}\n\n#[allow(dead_code)]\nfn generated_buffer<T: Copy, P>(value: *const P, capacity_bytes: usize, element_size: usize, maximum_bytes: u64, error: T) -> Result<*const P, T> {\n    if capacity_bytes as u64 > maximum_bytes\n        || capacity_bytes % element_size != 0\n        || (capacity_bytes != 0 && (value.is_null() || (value as usize) % ::core::mem::align_of::<P>() != 0))\n    {\n        Err(error)\n    } else {\n        Ok(value)\n    }\n}\n\n#[allow(dead_code)]\nfn generated_buffer_used<T: Copy>(used_count: u32, capacity_bytes: usize, element_size: usize, maximum_count: u32, error: T) -> Result<u32, T> {\n    if used_count > maximum_count || (used_count as usize).saturating_mul(element_size) > capacity_bytes {\n        Err(error)\n    } else {\n        Ok(used_count)\n    }\n}\n\n#[allow(dead_code)]\nfn generated_native_ref<T: Copy>(value: u32, error: T) -> Result<u32, T> {\n    if value == 0 || value >= 0x8000_0000 {\n        Err(error)\n    } else {\n        Ok(value)\n    }\n}\n\n#[allow(dead_code)]\nfn generated_node_id<T: Copy>(low: u32, high: u32, error: T) -> Result<(u32, u32), T> {\n    if high > 0x001f_ffff || (high == 0 && low == 0) {\n        Err(error)\n    } else {\n        Ok((low, high))\n    }\n}\n\n#[allow(dead_code)]\nfn generated_enum<T: Copy>(value: u32, allowed: &[u32], error: T) -> Result<u32, T> {\n    if allowed.contains(&value) { Ok(value) } else { Err(error) }\n}\n\n");
    for function in &document.functions {
        let result_type = rust_type(function.return_type.as_str());
        let panic_error = error_literal(function, "panic");
        source.push_str("#[unsafe(no_mangle)]\n");
        source.push_str(&format!(
            "pub unsafe extern \"C\" fn iyon_{}_v1({}) -> {} {{\n    generated_catch_unwind(|| {{\n        (|| -> Result<{}, {}> {{\n",
            function.name,
            rust_arguments(&function.args, document),
            result_type,
            result_type,
            result_type
        ));
        source.push_str(&validation_statements(function, document, bridge_schema));
        source.push_str(&format!(
            "            Ok(unsafe {{ generated_impls::{}({}) }})\n        }})()\n    }},\n        {}\n    )\n}}\n\n",
            function.implementation,
            rust_call_arguments(&function.args),
            panic_error
        ));
    }
    format_rust(source)
}

pub fn conformance(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = banner(schema_hash, generator_hash);
    for spec in &document.conformance {
        let args = spec
            .args
            .iter()
            .enumerate()
            .map(|(index, type_name)| format!("a{index}: {}", conformance_rust_type(type_name)))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn iyon_abi_conformance_{}_v1({}) -> {} {{\n",
            spec.name,
            args,
            conformance_rust_type(&spec.return_type)
        ));
        match spec.operation.as_str() {
            "position_weighted_sum" => output.push_str(&format!(
                "    {}\n",
                weighted_sum_expression(spec)
            )),
            "pointer_probe" => output.push_str(
                "    if a0.is_null() { 0 } else { 1 }\n",
            ),
            "buffer_probe" => output.push_str(
                "    if a0.is_null() { u32::MAX } else { (a1 as u32).wrapping_mul(257).wrapping_add(unsafe { *a0 as u32 }) }\n",
            ),
            "cstring_hash" => output.push_str(
                "    if a0.is_null() { 0 } else { unsafe { ::core::ffi::CStr::from_ptr(a0) }.to_bytes().iter().fold(2166136261u32, |hash, byte| hash.wrapping_mul(16777619).wrapping_add(u32::from(*byte))) }\n",
            ),
            operation => panic!("unsupported conformance operation {operation}"),
        }
        output.push_str("}\n\n");
    }
    format_rust(output)
}

fn conformance_rust_type(type_name: &str) -> &'static str {
    match type_name {
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "i32" => "i32",
        "f32" => "f32",
        "f64" => "f64",
        "ptr" => "*mut ::core::ffi::c_void",
        "buffer" => "*const u8",
        "buffer_length" => "usize",
        "cstring" => "*const ::core::ffi::c_char",
        other => panic!("unsupported conformance type {other}"),
    }
}

fn weighted_sum_expression(spec: &crate::model::ConformanceSpec) -> String {
    const WEIGHTS: [u32; 16] = [3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59];
    let terms = spec
        .args
        .iter()
        .enumerate()
        .map(|(index, type_name)| match type_name.as_str() {
            "u8" | "u16" => format!("u32::from(a{index}).wrapping_mul({})", WEIGHTS[index]),
            "u32" | "i32" => format!("a{index}.wrapping_mul({})", WEIGHTS[index]),
            "f32" => format!("a{index} * {}.0", WEIGHTS[index]),
            "f64" => format!("a{index} * {}.0", WEIGHTS[index]),
            other => panic!("unsupported weighted conformance type {other}"),
        })
        .collect::<Vec<_>>();
    let operator = match spec.args.first().map(String::as_str) {
        Some("f32" | "f64") => " + ",
        Some("u8" | "u16" | "u32" | "i32") => ".wrapping_add(",
        _ => panic!("weighted conformance requires at least one scalar argument"),
    };
    if operator == " + " {
        terms.join(operator)
    } else {
        let mut expression = terms[0].clone();
        for term in terms.iter().skip(1) {
            expression = format!("{expression}.wrapping_add({term})");
        }
        expression
    }
}

pub fn table(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = banner(schema_hash, generator_hash);
    output.push_str("#![allow(dead_code)]\n\n#[derive(Clone, Copy, Debug)]\npub struct FunctionDescriptor {\n    pub name: &'static str,\n    pub symbol: &'static str,\n    pub family: &'static str,\n    pub hotness: &'static str,\n    pub fallback: &'static str,\n    pub ownership: &'static str,\n    pub borrow_duration: &'static str,\n    pub thread_affinity: &'static str,\n    pub may_allocate_native_memory: bool,\n    pub mutates_host_state: bool,\n    pub max_buffer_bytes: u64,\n    pub max_input_count: u32,\n    pub benchmark_registration: &'static str,\n}\n\n");
    output.push_str("pub static FUNCTIONS: &[FunctionDescriptor] = &[\n");
    for function in &document.functions {
        output.push_str(&format!(
            "    FunctionDescriptor {{\n        name: {:?},\n        symbol: {:?},\n        family: {:?},\n        hotness: {:?},\n        fallback: {:?},\n        ownership: {:?},\n        borrow_duration: {:?},\n        thread_affinity: {:?},\n        may_allocate_native_memory: {},\n        mutates_host_state: {},\n        max_buffer_bytes: {},\n        max_input_count: {},\n        benchmark_registration: {:?},\n    }},\n",
            function.name,
            format!("iyon_{}_v1", function.name),
            function.family,
            function.hotness,
            function.fallback,
            function.ownership,
            function.borrow_duration,
            function.thread_affinity,
            function.may_allocate_native_memory,
            function.mutates_host_state,
            function.max_buffer_bytes,
            function.max_input_count,
            function.benchmark_registration
        ));
    }
    output.push_str("];\n\npub const FUNCTION_COUNT: usize = FUNCTIONS.len();\n");
    output
}

fn export_imports(document: &AbiDocument) -> String {
    let mut names = vec!["NativeViewRuntime"];
    if document.functions.iter().any(|function| {
        function
            .args
            .iter()
            .any(|argument| argument.lowering == "host_ptr")
    }) {
        names.push("NativeHost");
    }
    names.extend(
        document
            .pods
            .iter()
            .filter(|pod| {
                document.functions.iter().any(|function| {
                    function.args.iter().any(|argument| {
                        argument.lowering == "pod_slice"
                            && argument.type_name.strip_suffix("[]") == Some(pod.name.as_str())
                    })
                })
            })
            .map(|pod| pod.name.as_str()),
    );
    if names.len() == 1 {
        format!("use super::{};", names[0])
    } else {
        format!("use super::{{{}}};", names.join(", "))
    }
}

fn error_literal(function: &crate::model::FunctionSpec, kind: &str) -> String {
    if function.return_type == "i32" || function.return_type == "status_only" {
        return match kind {
            "panic" => "-127i32".to_owned(),
            _ => "-1i32".to_owned(),
        };
    }
    match kind {
        "panic" => "0x8000_00ffu32".to_owned(),
        _ => "0x8000_0001u32".to_owned(),
    }
}

fn validation_statements(
    function: &crate::model::FunctionSpec,
    document: &AbiDocument,
    bridge_schema: &Map<String, serde_json::Value>,
) -> String {
    let error = error_literal(function, "invalid");
    let buffer_error = if function.return_type == "i32" || function.return_type == "status_only" {
        "-2i32"
    } else {
        "0x8000_0002u32"
    };
    let count_error = if function.return_type == "i32" || function.return_type == "status_only" {
        "-3i32"
    } else {
        "0x8000_0003u32"
    };
    let mut output = String::new();
    let mut node_id_pairs = std::collections::HashSet::new();
    for (index, argument) in function.args.iter().enumerate() {
        if argument.lowering == "node_id_pair" {
            output.push_str(&format!(
                "            let ({}_low, {}_high) = generated_node_id({}_low, {}_high, {})?;\n",
                argument.name, argument.name, argument.name, argument.name, error
            ));
            continue;
        }
        if let Some(base) = argument.name.strip_suffix("_low") {
            let high_name = format!("{base}_high");
            if base.contains("node_id")
                && function.args.get(index + 1).is_some_and(|candidate| {
                    candidate.name == high_name
                        && candidate.lowering == "u32"
                        && argument.lowering == "u32"
                })
                && node_id_pairs.insert(base.to_owned())
            {
                output.push_str(&format!(
                    "            let ({}, {}) = generated_node_id({}, {}, {})?;\n",
                    argument.name, high_name, argument.name, high_name, error
                ));
                continue;
            }
        }
        match argument.lowering.as_str() {
            "runtime_ptr" | "host_ptr" => output.push_str(&format!(
                "            let {} = generated_nonnull({}, {})?;\n",
                argument.name, argument.name, error
            )),
            "native_ref" => output.push_str(&format!(
                "            let {} = generated_native_ref({}, {})?;\n",
                argument.name, argument.name, error
            )),
            "buffer" | "pod_slice" => {
                let capacity = function
                    .args
                    .iter()
                    .find(|candidate| {
                        candidate.lowering == "buffer_length"
                            && candidate.buffer_length_of.as_deref() == Some(argument.name.as_str())
                    })
                    .map(|candidate| candidate.name.as_str())
                    .expect("validated buffer_length pair");
                let element_size = buffer_element_size(argument, document)
                    .expect("validated fixed-size buffer element")
                    .to_string();
                output.push_str(&format!(
                    "            let {} = generated_buffer({}, {}, {}, {}, {})?;\n",
                    argument.name,
                    argument.name,
                    capacity,
                    element_size,
                    function.max_buffer_bytes,
                    buffer_error
                ));
            }
            "buffer_length" => {}
            "cstring_ephemeral" => output.push_str(&format!(
                "            let {} = generated_nonnull_const({}, {})?;\n",
                argument.name, argument.name, error
            )),
            "buffer_used" => {
                let buffer = if let Some(target) = argument.buffer_used_of.as_deref() {
                    // PERF-12 T11 (§41): explicit pairing on multi-buffer
                    // functions; validated to name a real buffer exactly once.
                    function
                        .args
                        .iter()
                        .find(|candidate| {
                            candidate.name == target
                                && matches!(candidate.lowering.as_str(), "buffer" | "pod_slice")
                        })
                        .expect("validated buffer_used_of target")
                } else {
                    function
                        .args
                        .iter()
                        .find(|candidate| {
                            matches!(candidate.lowering.as_str(), "buffer" | "pod_slice")
                        })
                        .expect("validated buffer_used pair")
                };
                let capacity = function
                    .args
                    .iter()
                    .find(|candidate| {
                        candidate.lowering == "buffer_length"
                            && candidate.buffer_length_of.as_deref() == Some(buffer.name.as_str())
                    })
                    .map(|candidate| candidate.name.as_str())
                    .expect("validated buffer_length pair");
                let element_size = buffer_element_size(buffer, document)
                    .expect("validated fixed-size buffer element")
                    .to_string();
                output.push_str(&format!(
                    "            let {} = generated_buffer_used({}, {}, {}, {}, {})?;\n",
                    argument.name,
                    argument.name,
                    capacity,
                    element_size,
                    function.max_input_count,
                    count_error
                ));
            }
            _ if document
                .enums
                .iter()
                .any(|item| item.name == argument.type_name) =>
            {
                let values = document
                    .enums
                    .iter()
                    .find(|item| item.name == argument.type_name)
                    .into_iter()
                    .flat_map(|enum_spec| enum_spec.values.iter())
                    .map(|value| {
                        bridge_schema
                            .get(&value.source_key)
                            .and_then(serde_json::Value::as_u64)
                            .expect("validated bridge enum value")
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!(
                    "            let {} = generated_enum({}, &[{}], {})?;\n",
                    argument.name, argument.name, values, error
                ));
            }
            _ => {}
        }
    }
    output
}

fn buffer_element_size(argument: &ArgumentSpec, document: &AbiDocument) -> Option<u32> {
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

fn rust_arguments(arguments: &[ArgumentSpec], document: &AbiDocument) -> String {
    let mut rendered = Vec::new();
    for argument in arguments {
        if argument.lowering == "node_id_pair" {
            rendered.push(format!("{}_low: u32", argument.name));
            rendered.push(format!("{}_high: u32", argument.name));
        } else {
            rendered.push(format!(
                "{}: {}",
                argument.name,
                rust_type_for_argument(argument, document)
            ));
        }
    }
    rendered.join(", ")
}

fn rust_type_for_argument(argument: &ArgumentSpec, document: &AbiDocument) -> String {
    match argument.lowering.as_str() {
        "runtime_ptr" => "*mut NativeViewRuntime".to_owned(),
        "host_ptr" => "*mut NativeHost".to_owned(),
        "native_ref" | "node_id_pair" | "native_ref_result" => "u32".to_owned(),
        "buffer" if argument.type_name == "u32[]" => "*const u32".to_owned(),
        "buffer" => "*const u8".to_owned(),
        "pod_slice" => argument
            .type_name
            .strip_suffix("[]")
            .map_or_else(|| "*const u8".to_owned(), |name| format!("*const {name}")),
        "buffer_length" => "usize".to_owned(),
        "cstring_ephemeral" => "*const ::core::ffi::c_char".to_owned(),
        _ => rust_type(&type_name(argument, document)),
    }
}

fn type_name(argument: &ArgumentSpec, document: &AbiDocument) -> String {
    if document
        .handles
        .iter()
        .any(|handle| handle.name == argument.type_name)
        || document
            .enums
            .iter()
            .any(|enum_spec| enum_spec.name == argument.type_name)
    {
        return "u32".to_owned();
    }
    argument.type_name.clone()
}

fn rust_call_arguments(arguments: &[ArgumentSpec]) -> String {
    arguments
        .iter()
        .flat_map(|argument| {
            if argument.lowering == "node_id_pair" {
                return vec![
                    format!("{}_low", argument.name),
                    format!("{}_high", argument.name),
                ];
            }
            vec![argument.name.clone()]
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn rust_type(type_name: &str) -> String {
    match type_name {
        "ViewRefResult" | "PathRefResult" | "StyleRefResult" | "StyleAtomRefResult"
        | "native_ref_result" | "u32" => "u32".to_owned(),
        "i32" | "status_only" => "i32".to_owned(),
        "u8" => "u8".to_owned(),
        "u16" => "u16".to_owned(),
        "f32" => "f32".to_owned(),
        "f64" => "f64".to_owned(),
        other => other.to_owned(),
    }
}

pub fn layout_tests(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = banner(schema_hash, generator_hash);
    let pod_imports = document
        .pods
        .iter()
        .filter(|pod| {
            document.functions.iter().any(|function| {
                function.args.iter().any(|argument| {
                    argument.lowering == "pod_slice"
                        && argument.type_name.strip_suffix("[]") == Some(pod.name.as_str())
                })
            })
        })
        .map(|pod| pod.name.as_str())
        .collect::<Vec<_>>();
    let generated_root_imports = if pod_imports.is_empty() {
        String::new()
    } else {
        format!("use generated_types::{{{}}};\n\n", pod_imports.join(", "))
    };
    let host_decl = if document.functions.iter().any(|function| {
        function
            .args
            .iter()
            .any(|argument| argument.lowering == "host_ptr")
    }) {
        "#[allow(dead_code)]\npub struct NativeHost;\n\n"
    } else {
        ""
    };
    output.push_str(&format!("#[allow(dead_code)]\npub struct NativeViewRuntime;\n\n#[path = \"../src/generated/view_abi_table.rs\"]\nmod generated;\n#[path = \"../src/generated/view_abi_types.rs\"]\nmod generated_types;\n#[path = \"../src/generated/view_abi_conformance.rs\"]\nmod generated_conformance;\n\n{generated_root_imports}{host_decl}mod generated_exports {{\n    include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/generated/view_abi_exports.rs\"));\n}}\n\n"));
    for (index, function) in document.functions.iter().enumerate() {
        output.push_str(&format!(
            "#[unsafe(no_mangle)]\npub unsafe extern \"Rust\" fn {}({}) -> {} {{\n",
            function.implementation,
            rust_arguments(&function.args, document),
            rust_type(function.return_type.as_str())
        ));
        for argument in &function.args {
            output.push_str(&format!("    let _ = {};\n", argument.name));
        }
        output.push_str(&format!(
            "    {}\n}}\n\n",
            test_stub_value(function.return_type.as_str(), index)
        ));
    }
    output.push_str("#[test]\nfn generated_function_count_is_stable() {\n");
    output.push_str(&format!(
        "    assert_eq!(generated::FUNCTION_COUNT, {});\n",
        document.functions.len()
    ));
    output.push_str(
        "}\n\n#[test]\nfn generated_abi_version_is_one() {\n    assert_eq!(generated_types::ABI_VERSION, 1);\n}\n\n",
    );
    output.push_str(&format!(
        "#[test]\nfn generated_conformance_count_is_stable() {{\n    assert_eq!({}, {});\n}}\n",
        document.conformance.len(),
        document.conformance.len()
    ));
    output.push_str("\n#[test]\nfn generated_conformance_functions_are_callable() {\n");
    for spec in &document.conformance {
        output.push_str(&conformance_test_call(spec));
    }
    output.push_str("}\n\n#[test]\nfn generated_wrappers_reject_invalid_inputs_and_delegate() {\n    let mut runtime = NativeViewRuntime;\n    let runtime_ptr = &mut runtime as *mut NativeViewRuntime;\n");
    if document.functions.len() >= 7 {
        // Stub returns are positional (test_stub_value: u32/ViewRefResult ->
        // 0x100 + index, i32/f32/f64 -> 100 + index). These expectations must
        // track the canonical function order; they drifted when T12 inserted
        // view_status_detail and were re-derived for T11's diff insertion.
        output.push_str("    assert_eq!(unsafe { generated_exports::iyon_runtime_noop_v1(runtime_ptr) }, 0x100);\n    assert_eq!(unsafe { generated_exports::iyon_view_render_ref_v1(runtime_ptr, 1) }, 0x102);\n    let mut host = NativeHost;\n    let host_ptr = &mut host as *mut NativeHost;\n    assert_eq!(unsafe { generated_exports::iyon_host_render_ref_v1(runtime_ptr, host_ptr, 1) }, 103);\n    assert_eq!(unsafe { generated_exports::iyon_view_spacer_create_v1(runtime_ptr, 1, 0, 2) }, 0x104);\n    assert_eq!(unsafe { generated_exports::iyon_view_text_layout_patch_root_v1(runtime_ptr, 1, 1, 0, 1, 2) }, 0x105);\n    assert_eq!(unsafe { generated_exports::iyon_view_common_patch_root_v1(runtime_ptr, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1) }, 0x106);\n    let children = [generated_types::AxisChildInputV1 { track_word: 1, child_ref: 1 }];\n    assert_eq!(unsafe { generated_exports::iyon_view_axis_create_buffer_v1(runtime_ptr, 1, 0, 1, 0, children.as_ptr(), core::mem::size_of_val(&children), 1) }, 0x107);\n    let refs = [1_u32];\n    assert_eq!(unsafe { generated_exports::iyon_view_release_many_v1(runtime_ptr, refs.as_ptr(), core::mem::size_of_val(&refs), 1) }, 129);\n    assert_eq!(unsafe { generated_exports::iyon_runtime_noop_v1(core::ptr::null_mut()) }, 0x8000_0001);\n    assert_eq!(unsafe { generated_exports::iyon_view_render_ref_v1(runtime_ptr, 0) }, 0x8000_0001);\n    assert_eq!(unsafe { generated_exports::iyon_view_spacer_create_v1(runtime_ptr, 0, 0, 1) }, 0x8000_0001);\n    assert_eq!(unsafe { generated_exports::iyon_view_text_layout_patch_root_v1(runtime_ptr, 1, 1, 0, 0, 1) }, 0x8000_0001);\n    assert_eq!(unsafe { generated_exports::iyon_view_axis_create_buffer_v1(runtime_ptr, 1, 0, 1, 0, core::ptr::null(), 8, 0) }, 0x8000_0002);\n    assert_eq!(unsafe { generated_exports::iyon_view_axis_create_buffer_v1(runtime_ptr, 1, 0, 1, 0, core::ptr::null(), 0, 1) }, 0x8000_0003);\n    assert_eq!(unsafe { generated_exports::iyon_view_release_many_v1(runtime_ptr, core::ptr::null(), 4, 0) }, -2);\n    assert_eq!(unsafe { generated_exports::iyon_view_release_many_v1(runtime_ptr, core::ptr::null(), 0, 1) }, -3);\n");
    }
    if let Some(index) = document
        .functions
        .iter()
        .position(|function| function.name == "view_ref_for_node_id")
    {
        // Positional stub value (0x100 + index) so insertions cannot drift it.
        output.push_str(&format!(
            "    assert_eq!(unsafe {{ generated_exports::iyon_view_ref_for_node_id_v1(runtime_ptr, 1, 0) }}, 0x{:x});\n    assert_eq!(unsafe {{ generated_exports::iyon_view_ref_for_node_id_v1(runtime_ptr, 0, 0) }}, 0x8000_0001);\n",
            0x100 + index
        ));
    }
    output.push_str("}\n");
    format_rust(output)
}

fn test_stub_value(return_type: &str, index: usize) -> String {
    match return_type {
        "i32" | "status_only" => format!("{}", 100 + index as i32),
        "f32" => format!("{}.0_f32", 100 + index),
        "f64" => format!("{}.0_f64", 100 + index),
        _ => format!("0x{:x}", 0x100 + index),
    }
}

fn conformance_test_call(spec: &crate::model::ConformanceSpec) -> String {
    let symbol = format!(
        "generated_conformance::iyon_abi_conformance_{}_v1",
        spec.name
    );
    match spec.operation.as_str() {
        "position_weighted_sum" => {
            let args = spec
                .args
                .iter()
                .enumerate()
                .map(|(index, type_name)| format!("{} as {}", index + 1, type_name))
                .collect::<Vec<_>>()
                .join(", ");
            let expected = spec
                .args
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    (index as u32 + 1)
                        * [3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59][index]
                })
                .sum::<u32>();
            if matches!(spec.return_type.as_str(), "f32" | "f64") {
                format!(
                    "    assert!((unsafe {{ {symbol}({args}) }} - {expected}.0).abs() < 0.000001);\n"
                )
            } else {
                format!("    assert_eq!(unsafe {{ {symbol}({args}) }}, {expected});\n")
            }
        }
        "pointer_probe" => format!(
            "    assert_eq!(unsafe {{ {symbol}(core::ptr::NonNull::<core::ffi::c_void>::dangling().as_ptr()) }}, 1);\n"
        ),
        "buffer_probe" => format!(
            "    let bytes = [0x7b_u8, 0x01, 0x02, 0x03];\n    assert_eq!(unsafe {{ {symbol}(bytes.as_ptr(), bytes.len()) }}, 4 * 257 + 0x7b);\n"
        ),
        "cstring_hash" => format!(
            "    let text = std::ffi::CString::new(\"ABI ✓\").expect(\"test text has no NUL\");\n    assert_ne!(unsafe {{ {symbol}(text.as_ptr()) }}, 0);\n"
        ),
        operation => panic!("unsupported conformance test operation {operation}"),
    }
}

fn format_rust(source: String) -> String {
    let body_start = ["\n//!", "\n#[", "\npub "]
        .iter()
        .filter_map(|marker| source.find(marker).map(|index| index + 1))
        .min();
    let Some(body_start) = body_start else {
        return source;
    };
    let (prefix, body) = source.split_at(body_start);
    let formatted = rustfmt_body(body).unwrap_or_else(|| {
        syn::parse_file(body)
            .map(|file| {
                let _tokens: proc_macro2::TokenStream = quote!(#file);
                prettyplease::unparse(&file)
            })
            .unwrap_or_else(|_| body.to_owned())
    });
    format!("{prefix}{}\n", formatted.trim_end())
}

fn rustfmt_body(body: &str) -> Option<String> {
    let mut process = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    process.stdin.take()?.write_all(body.as_bytes()).ok()?;
    let output = process.wait_with_output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}
