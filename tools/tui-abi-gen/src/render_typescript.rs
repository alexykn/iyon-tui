use crate::{
    model::{AbiDocument, ArgumentSpec},
    render_manifest::{banner, typescript_bindings_header, typescript_calls_header},
};

pub fn abi_bindings(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = typescript_bindings_header(schema_hash, generator_hash);
    output.push_str("export interface NativeViewAbiMetadata {\n");
    output.push_str("  readonly abi_name: string;\n  readonly abi_version: number;\n  readonly semantic_version: number;\n  readonly schema_blake3: string;\n  readonly generator_blake3: string;\n  readonly generation: number;\n  readonly transport: \"napi\";\n  readonly function_count: number;\n}\n\n");
    output.push_str("export interface NativeViewAbiHandle {\n  /** S6 dispatch-granularity probe; not a semantic/public TUI operation. */\n  tuiPerfNapiBatchRuntimeNoop?(count: number): number;\n  metadata(): NativeViewAbiMetadata;\n");
    for function in &document.functions {
        let method = camel_case(&function.name);
        let arguments = function
            .args
            .iter()
            .filter(|argument| {
                !matches!(argument.lowering.as_str(), "runtime_ptr" | "buffer_length")
            })
            .flat_map(|argument| {
                if argument.lowering == "node_id_pair" {
                    vec![
                        format!("{}_low: number", argument.name),
                        format!("{}_high: number", argument.name),
                    ]
                } else {
                    vec![format!("{}: {}", argument.name, ts_napi_type(argument))]
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "  {}({}): {};\n",
            method,
            arguments,
            ts_return(function.return_type.as_str())
        ));
    }
    for spec in &document.conformance {
        let arguments = spec
            .args
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.as_str() != "buffer_length")
            .map(|(index, argument)| format!("a{}: {}", index, ts_conformance_type(argument)))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "  {}({}): {};\n",
            spec.name,
            arguments,
            ts_conformance_return_type(spec.return_type.as_str())
        ));
    }
    output.push_str("}\n");
    output
}

pub fn conformance_bindings(
    document: &AbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let mut output = typescript_bindings_header(schema_hash, generator_hash);
    output.push_str("import type { NativeViewAbiHandle } from \"./view_abi\";\n\n");
    output.push_str("export type NativeAbiConformanceSession = NativeViewAbiHandle;\n\n");
    for spec in &document.conformance {
        let arguments = spec
            .args
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.as_str() != "buffer_length")
            .map(|(index, argument)| format!("a{}: {}", index, ts_conformance_type(argument)))
            .collect::<Vec<_>>()
            .join(", ");
        let call_arguments = spec
            .args
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.as_str() != "buffer_length")
            .map(|(index, _)| format!("a{}", index))
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "export function {}(session: NativeAbiConformanceSession, {}): {} {{\n  return session.{}({});\n}}\n\n",
            spec.name,
            arguments,
            ts_conformance_return_type(spec.return_type.as_str()),
            spec.name,
            call_arguments
        ));
    }
    if output.ends_with("\n\n") {
        output.pop();
    }
    output
}

pub fn calls(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = typescript_calls_header(schema_hash, generator_hash);
    output.push_str("export type ViewAbiSymbols = NativeViewAbiHandle;\n\n");
    output.push_str("const ERROR_BIT = 0x8000_0000;\nconst CACHE_MISS = 0x8000_0004;\n\n");
    output.push_str("export class NativeAbiStatusError extends Error {\n  readonly status: number;\n  readonly detail: number;\n\n  constructor(status: number, detail: number) {\n    super(`native ABI status 0x${status.toString(16)}`);\n    this.name = \"NativeAbiStatusError\";\n    this.status = status;\n    this.detail = detail;\n  }\n}\n\n");
    output.push_str("function checkedRef(symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, result: number): number {\n  if (result === 0 || result >= ERROR_BIT) {\n    const detail = result === CACHE_MISS ? runtime.viewStatusDetail() : 0;\n    throw new NativeAbiStatusError(result, detail);\n  }\n  return result;\n}\n\n");
    for function in &document.functions {
        let arguments = ts_arguments(&function.args, document);
        let signature = if arguments.is_empty() {
            "symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle".to_owned()
        } else {
            format!("symbols: ViewAbiSymbols, runtime: NativeViewAbiHandle, {arguments}")
        };
        output.push_str(&format!(
            "export function {}({}): {} {{\n",
            camel_case(&function.name),
            signature,
            ts_return(function.return_type.as_str())
        ));
        let call_args = function
            .args
            .iter()
            .filter(|argument| {
                argument.lowering != "runtime_ptr" && argument.lowering != "buffer_length"
            })
            .flat_map(call_argument_names)
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "  const result = runtime.{}({});\n",
            camel_case(&function.name),
            call_args
        ));
        if is_ref_result(function.return_type.as_str()) {
            output.push_str("  return checkedRef(symbols, runtime, result);\n");
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
import manifest from "../../src/transport/abi/structural/generated/view_abi_manifest.json";

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

fn ts_arguments(arguments: &[ArgumentSpec], document: &AbiDocument) -> String {
    arguments
        .iter()
        .filter(|argument| !matches!(argument.lowering.as_str(), "runtime_ptr" | "buffer_length"))
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
    vec![argument.name.clone()]
}

fn ts_napi_type(argument: &ArgumentSpec) -> &'static str {
    match argument.lowering.as_str() {
        "host_ptr" => "NativeTuiHostContract",
        "buffer" | "pod_slice" if argument.type_name == "u8[]" => "Uint8Array",
        "buffer" | "pod_slice" => "Uint32Array",
        "cstring_ephemeral" => "string",
        _ => "number",
    }
}

fn ts_conformance_type(type_name: &str) -> &'static str {
    match type_name {
        "ptr" => "boolean",
        "buffer" => "Uint8Array",
        "u8" => "number",
        "u16" => "number",
        "u32" => "number",
        "i32" => "number",
        "f32" => "number",
        "f64" => "number",
        "buffer_length" => "number",
        "cstring" => "string",
        other => panic!("unsupported N-API conformance type {other}"),
    }
}

fn ts_conformance_return_type(return_type: &str) -> &'static str {
    match return_type {
        "f32" | "f64" | "i32" | "u32" => "number",
        other => panic!("unsupported N-API conformance return type {other}"),
    }
}

fn ts_type(argument: &ArgumentSpec, _document: &AbiDocument) -> &'static str {
    ts_napi_type(argument)
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
