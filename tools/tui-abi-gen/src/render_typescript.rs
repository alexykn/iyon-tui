use crate::{
    model::{AbiDocument, ArgumentSpec, MaterializerFieldRole},
    render_manifest::{banner, typescript_bindings_header, typescript_calls_header},
};

pub fn abi_bindings(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = typescript_bindings_header(schema_hash, generator_hash);
    output.push_str("export type NativeAbiPointers = {\n");
    for function in &document.functions {
        output.push_str(&format!("  {}: Pointer;\n", camel_case(&function.name)));
    }
    output.push_str("};\n\n");
    output.push_str(
        "export function linkViewAbi(abi: NativeAbiPointers) {\n  return linkSymbols({\n",
    );
    for function in &document.functions {
        output.push_str(&format!(
            "    {}: {{ ptr: abi.{}, args: [{}], returns: {:?} }},\n",
            camel_case(&function.name),
            camel_case(&function.name),
            function
                .args
                .iter()
                .flat_map(ffi_args)
                .map(|item| format!("{item:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            ffi_return(function.return_type.as_str())
        ));
    }
    output.push_str("  } as const);\n}\n");
    output
}

pub fn conformance_bindings(
    document: &AbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = typescript_bindings_header(schema_hash, generator_hash);
    output.push_str("export type NativeAbiConformancePointers = {\n");
    for spec in &document.conformance {
        output.push_str(&format!("  {}: Pointer;\n", spec.name));
    }
    output.push_str("};\n\nexport function linkViewAbiConformance(abi: NativeAbiConformancePointers) {\n  return linkSymbols({\n");
    for spec in &document.conformance {
        let args = spec
            .args
            .iter()
            .map(|argument| format!("{argument:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "    {}: {{ ptr: abi.{}, args: [{}], returns: {:?} }},\n",
            spec.name, spec.name, args, spec.return_type
        ));
    }
    output.push_str("  } as const);\n}\n");
    output
}

/// PERF-12 T5 (§65/§66): generated semantic materializers. One monomorphic
/// explicit function per declared kind, children first, no reflection. The
/// T5 vertical slice is the spacer kind; buffer and reference lowerings land
/// with their owning tranches (T8/T6) and fail generation until then.
/// PERF-12 T5 (§65/§66): generated semantic materializers. One monomorphic
/// explicit function per declared kind, children first, no reflection. The
/// T5 vertical slice is the spacer kind; buffer and reference lowerings land
/// with their owning tranches (T8/T6) and fail generation until then.
pub fn materialize(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = typescript_calls_header(schema_hash, generator_hash);
    // The calls header already imports Pointer; add the builder imports.
    let mut builders = document
        .materializers
        .iter()
        .map(|m| camel_case(&m.rust_builder))
        .collect::<Vec<_>>();
    builders.sort();
    builders.dedup();
    output.push_str(&format!(
        "import {{ {} }} from \"./view_calls\";\n",
        builders.join(", ")
    ));
    output.push_str("import type { ViewAbiSymbols } from \"./view_calls\";\n\n");
    output.push_str("const ERROR_BIT = 0x8000_0000;\n\n");
    output
        .push_str("export interface MaterializeTx {\n  readonly symbols: ViewAbiSymbols;\n  readonly runtime: Pointer;\n}\n\n");
    output
        .push_str("function splitNodeId(id: number): [number, number] {\n  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];\n}\n\n");
    // §74: status decoding shared by every materializer caller. A failed
    // constructor returns its raw u32 status (high bit set or zero); a
    // success returns the minted NativeRef.
    output
        .push_str("export interface MaterializeStatus {\n  readonly ok: boolean;\n  readonly reference: number;\n  readonly status: number;\n}\n\n");
    output.push_str("export function decodeMaterializeStatus(result: number): MaterializeStatus {\n  if (result === 0 || (result & ERROR_BIT) !== 0) return { ok: false, reference: 0, status: result >>> 0 };\n  return { ok: true, reference: result, status: 0 };\n}\n\n");

    for materializer in &document.materializers {
        let kind_stem = materializer.bridge_kind.trim_start_matches("view");
        let node_interface = format!("Bridge{}MaterializeNode", pascal_case(kind_stem));
        output.push_str(&format!(
            "export interface {node_interface} {{\n  readonly id: number;\n"
        ));
        for field in &materializer.fields {
            let role = MaterializerFieldRole::parse(&field.role).expect("validated role");
            match role {
                MaterializerFieldRole::NodeIdLow | MaterializerFieldRole::NodeIdHigh => {}
                MaterializerFieldRole::Scalar => {
                    output.push_str(&format!("  readonly {}: number;\n", field.source));
                }
                MaterializerFieldRole::ChildRef
                | MaterializerFieldRole::StyleRef
                | MaterializerFieldRole::BaseRef => {
                    panic!(
                        "PERF-12: reference lowering lands in T6/T7; materializer {} declares {}",
                        materializer.name, field.role
                    );
                }
                MaterializerFieldRole::RefBuffer
                | MaterializerFieldRole::AuxBuffer
                | MaterializerFieldRole::ByteBuffer => {
                    panic!(
                        "PERF-12: buffer lowering lands in T8; materializer {} declares {}",
                        materializer.name, field.role
                    );
                }
            }
        }
        output.push_str("}\n\n");

        let builder_call = camel_case(&materializer.rust_builder);
        let args_list = materializer
            .fields
            .iter()
            .map(
                |field| match MaterializerFieldRole::parse(&field.role).expect("validated role") {
                    MaterializerFieldRole::NodeIdLow => "nodeIdLow".to_owned(),
                    MaterializerFieldRole::NodeIdHigh => "nodeIdHigh".to_owned(),
                    _ => format!("node.{}", field.source),
                },
            )
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "/** PERF-12 §74 status detail kind for this materializer: {:?}. */\n",
            materializer.status_detail
        ));
        output.push_str(&format!(
            "export const {}_STATUS_DETAIL = {:?} as const;\n\n",
            materializer.name.to_uppercase(),
            materializer.status_detail
        ));
        output.push_str(&format!(
            "export function materialize{}(node: {}, tx: MaterializeTx): number {{\n",
            pascal_case(materializer.name.as_str()),
            node_interface
        ));
        output.push_str("  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);\n");
        output.push_str(&format!(
            "  return {}(tx.symbols, tx.runtime, {});\n}}\n",
            builder_call, args_list
        ));
        if !materializer.fields.is_empty() {
            output.pop();
            output.push('\n');
        }
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

pub fn calls(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = typescript_calls_header(schema_hash, generator_hash);
    output
        .push_str("export type ViewAbiSymbols = ReturnType<typeof linkViewAbi>[\"symbols\"];\n\n");
    output.push_str("const ERROR_BIT = 0x8000_0000;\n\n");
    output.push_str("function checkedRef(result: number): number {\n  if (result === 0 || result >= ERROR_BIT) throw new Error(`native ABI status 0x${result.toString(16)}`);\n  return result;\n}\n\n");
    for function in &document.functions {
        output.push_str(&format!(
            "export function {}(symbols: ViewAbiSymbols, {}): {} {{\n",
            camel_case(&function.name),
            ts_arguments(&function.args, document),
            ts_return(function.return_type.as_str())
        ));
        let call_args = function
            .args
            .iter()
            .filter(|argument| argument.lowering != "buffer_length")
            .flat_map(call_argument_names)
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "  const result = symbols.{}({});\n",
            camel_case(&function.name),
            call_args
        ));
        if is_ref_result(function.return_type.as_str()) {
            output.push_str("  return checkedRef(result);\n");
        } else {
            output.push_str("  return result;\n");
        }
        output.push_str("}\n\n");
    }
    output
}

pub fn benchmark_registry(
    document: &AbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = banner(schema_hash, generator_hash);
    output.push_str("export type GeneratedAbiBenchmarkCase = {\n  name: string;\n  family: string;\n  hotness: string;\n  benchmarkRegistration: string;\n  scalarArgs: number;\n  hasBuffer: boolean;\n  maxBufferBytes: number;\n  maxInputCount: number;\n};\n\n");
    output.push_str("export const generatedAbiCases: readonly GeneratedAbiBenchmarkCase[] = [\n");
    for function in &document.functions {
        let scalar_args: usize = function
            .args
            .iter()
            .filter(|argument| {
                !matches!(
                    argument.lowering.as_str(),
                    "buffer" | "pod_slice" | "buffer_length"
                )
            })
            .map(|argument| {
                if argument.lowering == "node_id_pair" {
                    2
                } else {
                    1
                }
            })
            .sum();
        let has_buffer = function
            .args
            .iter()
            .any(|argument| matches!(argument.lowering.as_str(), "buffer" | "pod_slice"));
        output.push_str(&format!("  {{ name: {:?}, family: {:?}, hotness: {:?}, benchmarkRegistration: {:?}, scalarArgs: {scalar_args}, hasBuffer: {has_buffer}, maxBufferBytes: {}, maxInputCount: {} }},\n", function.name, function.family, function.hotness, function.benchmark_registration, function.max_buffer_bytes, function.max_input_count));
    }
    output.push_str("];\n");
    output
}

pub fn layout_test(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = banner(schema_hash, generator_hash);
    output.push_str(&format!(
        r#"import {{ expect, test }} from "bun:test";
import manifest from "../../src/tui/generated/view_abi_manifest.json";

test("generated ABI manifest is pinned and ordered", () => {{
  expect(manifest.schema_blake3).toBe("{}");
  expect(manifest.abi.version).toBe(1);
  expect(manifest.functions.map((item) => item.name)).toEqual([
"#,
        schema_hash,
    ));
    for function in &document.functions {
        output.push_str(&format!("    {:?},\n", function.name));
    }
    output.push_str("  ]);\n  expect(manifest.conformance.map((item) => item.name)).toEqual([\n");
    for spec in &document.conformance {
        output.push_str(&format!("    {:?},\n", spec.name));
    }
    output.push_str("  ]);\n});\n\ntest(\"generated ABI conformance signatures are pinned\", () => {\n  expect(manifest.conformance.map((item) => [item.name, item.return, item.args])).toEqual([\n");
    for spec in &document.conformance {
        let args = spec
            .args
            .iter()
            .map(|argument| format!("{:?}", argument))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "    [{:?}, {:?}, [{args}]],\n",
            spec.name, spec.return_type
        ));
    }
    output.push_str("  ]);\n});\n\ntest(\"generated ABI signatures and POD layouts are pinned\", () => {\n  expect(manifest.abi.qualified_bun).toBe(\"1.4.0\");\n  expect(manifest.abi.result_encoding).toBe(\"u32_high_bit_status\");\n  expect(manifest.pods.map((item) => [item.name, item.size, item.align])).toEqual([\n");
    for pod in &document.pods {
        output.push_str(&format!(
            "    [{:?}, {}, {}],\n",
            pod.name, pod.size, pod.align
        ));
    }
    output.push_str("  ]);\n  expect(manifest.functions.map((item) => item.args.map((arg) => arg.lowering))).toEqual([\n");
    for function in &document.functions {
        let lowerings = function
            .args
            .iter()
            .map(|argument| format!("{:?}", argument.lowering))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("    [{lowerings}],\n"));
    }
    output.push_str("  ]);\n});\n");
    output
}

fn ffi_args(argument: &ArgumentSpec) -> Vec<&'static str> {
    match argument.lowering.as_str() {
        "runtime_ptr" | "host_ptr" => vec!["ptr"],
        "native_ref" | "u32" | "buffer_used" | "native_ref_result" => vec!["u32"],
        "node_id_pair" => vec!["u32", "u32"],
        "i32" | "status_only" => vec!["i32"],
        "u8" => vec!["u8"],
        "u16" => vec!["u16"],
        "f32" => vec!["f32"],
        "f64" => vec!["f64"],
        "buffer" | "pod_slice" => vec!["buffer"],
        "buffer_length" => vec!["buffer_length"],
        "cstring_ephemeral" => vec!["cstring"],
        other => panic!("unsupported generated FFI lowering {other}"),
    }
}

fn ffi_return(return_type: &str) -> &'static str {
    match return_type {
        "i32" | "status_only" => "i32",
        "u32" | "ViewRefResult" | "PathRefResult" | "StyleRefResult" | "StyleAtomRefResult"
        | "native_ref_result" => "u32",
        "f32" => "f32",
        "f64" => "f64",
        other => panic!("unsupported generated FFI return {other}"),
    }
}

fn ts_arguments(arguments: &[ArgumentSpec], document: &AbiDocument) -> String {
    arguments
        .iter()
        .filter(|argument| argument.lowering != "buffer_length")
        .flat_map(|argument| {
            if argument.lowering == "node_id_pair" {
                vec![
                    format!("{}_low: number", argument.name),
                    format!("{}_high: number", argument.name),
                ]
            } else {
                vec![format!(
                    "{}: {}",
                    argument.name,
                    ts_type(argument, document)
                )]
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn call_argument_names(argument: &ArgumentSpec) -> Vec<String> {
    if argument.lowering == "node_id_pair" {
        return vec![
            format!("{}_low", argument.name),
            format!("{}_high", argument.name),
        ];
    }
    if matches!(argument.lowering.as_str(), "buffer" | "pod_slice") {
        return vec![argument.name.clone(), argument.name.clone()];
    }
    vec![argument.name.clone()]
}

fn ts_type(argument: &ArgumentSpec, document: &AbiDocument) -> &'static str {
    match argument.lowering.as_str() {
        "runtime_ptr" | "host_ptr" => "Pointer",
        "buffer" | "pod_slice" => "NodeJS.TypedArray | DataView",
        "cstring_ephemeral" => "string",
        _ if document
            .enums
            .iter()
            .any(|item| item.name == argument.type_name) =>
        {
            "number"
        }
        _ => "number",
    }
}

fn ts_return(return_type: &str) -> &'static str {
    match return_type {
        "i32" | "status_only" | "u32" | "ViewRefResult" | "PathRefResult" | "StyleRefResult"
        | "StyleAtomRefResult" | "native_ref_result" | "f32" | "f64" => "number",
        other => panic!("unsupported generated TS return {other}"),
    }
}

fn is_ref_result(return_type: &str) -> bool {
    matches!(
        return_type,
        "ViewRefResult"
            | "PathRefResult"
            | "StyleRefResult"
            | "StyleAtomRefResult"
            | "native_ref_result"
    )
}

fn camel_case(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase = false;
    for character in value.chars() {
        if character == '_' {
            uppercase = true;
            continue;
        }
        if uppercase {
            output.extend(character.to_uppercase());
            uppercase = false;
        } else {
            output.push(character);
        }
    }
    output
}

fn pascal_case(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
