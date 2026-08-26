use serde_json::Map;

use crate::{
    model::{AbiDocument, ArgumentSpec},
    render_manifest::c_header_preamble,
};

pub fn header(
    document: &AbiDocument,
    bridge_schema: &Map<String, serde_json::Value>,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = c_header_preamble(document, schema_hash, generator_hash);
    for pod in &document.pods {
        output.push_str(&format!("typedef struct {} {{\n", pod.name));
        for field in &pod.fields {
            output.push_str(&format!(
                "    {} {};\n",
                c_primitive_type(&field.type_name),
                field.name
            ));
        }
        output.push_str(&format!("}} {};\n\n", pod.name));
    }
    for enum_spec in &document.enums {
        output.push_str(&format!("typedef enum {} {{\n", enum_spec.name));
        for value in &enum_spec.values {
            let number = bridge_schema
                .get(&value.source_key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            output.push_str(&format!(
                "    {}_{} = UINT32_C({}),\n",
                enum_spec.name, value.name, number
            ));
        }
        output.push_str(&format!("}} {};\n\n", enum_spec.name));
    }
    for function in &document.functions {
        output.push_str(&format!(
            "{} iyon_{}_v1({});\n\n",
            c_return(function.return_type.as_str()),
            function.name,
            c_arguments(&function.args, document)
        ));
    }
    for spec in &document.conformance {
        output.push_str(&format!(
            "{} iyon_abi_conformance_{}_v1({});\n\n",
            c_return(spec.return_type.as_str()),
            spec.name,
            conformance_c_arguments(&spec.args)
        ));
    }
    output.push_str("#endif /* IYON_VIEW_ABI_H */\n");
    output
}

fn c_arguments(arguments: &[ArgumentSpec], document: &AbiDocument) -> String {
    arguments
        .iter()
        .flat_map(|argument| {
            if argument.lowering == "node_id_pair" {
                return vec![
                    format!("uint32_t {}_low", argument.name),
                    format!("uint32_t {}_high", argument.name),
                ];
            }
            vec![format!("{} {}", c_type(argument, document), argument.name)]
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn c_type(argument: &ArgumentSpec, document: &AbiDocument) -> String {
    match argument.lowering.as_str() {
        "runtime_ptr" => "NativeViewRuntime *".to_owned(),
        "host_ptr" => "NativeHost *".to_owned(),
        "buffer" if argument.type_name == "u32[]" => "const uint32_t *".to_owned(),
        "buffer" => "const uint8_t *".to_owned(),
        "pod_slice" => argument.type_name.strip_suffix("[]").map_or_else(
            || "const uint8_t *".to_owned(),
            |name| format!("const {name} *"),
        ),
        "buffer_length" => "size_t".to_owned(),
        "cstring_ephemeral" => "const char *".to_owned(),
        "i32" | "status_only" => "int32_t".to_owned(),
        "u8" => "uint8_t".to_owned(),
        "u16" => "uint16_t".to_owned(),
        "f32" => "float".to_owned(),
        "f64" => "double".to_owned(),
        _ if document
            .enums
            .iter()
            .any(|item| item.name == argument.type_name) =>
        {
            "uint32_t".to_owned()
        }
        _ => "uint32_t".to_owned(),
    }
}

fn conformance_c_arguments(arguments: &[String]) -> String {
    arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            let ty = match argument.as_str() {
                "ptr" => "void *",
                "buffer" => "const uint8_t *",
                "buffer_length" => "size_t",
                "cstring" => "const char *",
                "u8" => "uint8_t",
                "u16" => "uint16_t",
                "u32" => "uint32_t",
                "i32" => "int32_t",
                "f32" => "float",
                "f64" => "double",
                other => panic!("unsupported conformance type {other}"),
            };
            format!("{ty} a{index}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn c_primitive_type(type_name: &str) -> &'static str {
    match type_name {
        "u8" => "uint8_t",
        "u16" => "uint16_t",
        "u32" => "uint32_t",
        "i32" => "int32_t",
        "f32" => "float",
        "f64" => "double",
        _ => "uint8_t",
    }
}

fn c_return(return_type: &str) -> &'static str {
    match return_type {
        "i32" | "status_only" => "int32_t",
        "u32" | "ViewRefResult" | "PathRefResult" | "StyleRefResult" | "StyleAtomRefResult"
        | "native_ref_result" => "uint32_t",
        "f32" => "float",
        "f64" => "double",
        other => panic!("unsupported generated C return {other}"),
    }
}
