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

/// PERF-12 T5/T7 (§65/§66): generated semantic materializers. One monomorphic
/// explicit function per declared kind, children first, no reflection. The
/// T5 vertical slice was the spacer kind; T7 adds fixed-arity axis kinds
/// (§22/§32), lowering `children` structurally through the constructor
/// family before the parent call. Buffer lowerings land with T8.
pub fn materialize(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = typescript_calls_header(schema_hash, generator_hash);
    // The calls header already imports Pointer; add the builder imports.
    let mut builders = document
        .materializers
        .iter()
        .flat_map(|materializer| match &materializer.fixed_arity_axis {
            Some(axis) => {
                let mut family = axis
                    .builders
                    .iter()
                    .map(|builder| builder.as_str())
                    .collect::<Vec<_>>();
                if let Some(buffer_builder) = &axis.buffer_builder {
                    family.push(buffer_builder.as_str());
                }
                family
            }
            None => vec![materializer.rust_builder.as_str()],
        })
        .map(camel_case)
        .collect::<Vec<_>>();
    builders.sort();
    builders.dedup();
    output.push_str(&format!(
        "import {{ {} }} from \"./view_calls\";\n",
        builders.join(", ")
    ));
    output.push_str("import type { ViewAbiSymbols } from \"./view_calls\";\n");
    // PERF-12 T7: axis materializers lower children through the runtime's
    // identity-first ensureNative (§22) and signal unsupported arities as
    // fast fallbacks (§32/§49). The import cycle is safe: the generated
    // module only calls into retained_dag at materialization time.
    let has_axis = document
        .materializers
        .iter()
        .any(|materializer| materializer.fixed_arity_axis.is_some());
    let has_axis_buffer = document.materializers.iter().any(|materializer| {
        materializer
            .fixed_arity_axis
            .as_ref()
            .is_some_and(|axis| axis.buffer_builder.is_some())
    });
    if has_axis {
        output.push_str(
            "import { BRIDGE_LAYOUT_CHILD_KIND, type BridgeLayoutChild } from \"../ir.ts\";\n",
        );
        output.push_str(
            "import { RetainedFastFallbackError, ensureNative } from \"../retained_dag.ts\";\n",
        );
        if has_axis_buffer {
            // PERF-12 T8 (§50): retained cap for one borrowed-buffer axis call.
            output.push_str("import { MAX_DIRECT_AXIS_REFS } from \"../native_view_policy.ts\";\n");
        }
        // Axis lowerings recurse into ensureNative, so the transaction type
        // is owned by the runtime module; re-exported here for callers.
        output.push_str("import type { MaterializeTx } from \"../retained_dag.ts\";\n");
        output.push_str("export type { MaterializeTx };\n\n");
    } else {
        output
            .push_str("export interface MaterializeTx {\n  readonly symbols: ViewAbiSymbols;\n  readonly runtime: Pointer;\n}\n\n");
    }
    output.push_str("const ERROR_BIT = 0x8000_0000;\n\n");
    output
        .push_str("function splitNodeId(id: number): [number, number] {\n  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];\n}\n\n");
    // §74: status decoding shared by every materializer caller. A failed
    // constructor returns its raw u32 status (high bit set or zero); a
    // success returns the minted NativeRef.
    output
        .push_str("export interface MaterializeStatus {\n  readonly ok: boolean;\n  readonly reference: number;\n  readonly status: number;\n}\n\n");
    output.push_str("export function decodeMaterializeStatus(result: number): MaterializeStatus {\n  if (result === 0 || (result & ERROR_BIT) !== 0) return { ok: false, reference: 0, status: result >>> 0 };\n  return { ok: true, reference: result, status: 0 };\n}\n\n");

    // PERF-12 T7 (§22/§32): layout-child → ABI track word lowering. Track
    // kind in the low byte (bridge-schema track* discriminants), u16 amount
    // in bits 8..24; word 0 is the implicit content track.
    if has_axis {
        output.push_str(
            "const TRACK_CONTENT_MAX = 2;\nconst TRACK_FIXED = 3;\nconst TRACK_FLEX = 4;\nconst TRACK_FLEX_MAX = 5;\n\n",
        );
        output.push_str(
            "function layoutTrackWord(child: BridgeLayoutChild): number {\n  switch (child.kind) {\n    case BRIDGE_LAYOUT_CHILD_KIND.normal:\n      return 0;\n    case BRIDGE_LAYOUT_CHILD_KIND.fixed:\n      return TRACK_FIXED | (child.size << 8);\n    case BRIDGE_LAYOUT_CHILD_KIND.flex:\n      return TRACK_FLEX | (1 << 8);\n    case BRIDGE_LAYOUT_CHILD_KIND.flexMax:\n      return TRACK_FLEX_MAX | (child.maxRows << 8);\n    case BRIDGE_LAYOUT_CHILD_KIND.contentMax:\n      return TRACK_CONTENT_MAX | (child.maxRows << 8);\n  }\n}\n\n",
        );
    }

    for materializer in &document.materializers {
        let kind_stem = materializer.bridge_kind.trim_start_matches("view");
        let node_interface = format!("Bridge{}MaterializeNode", pascal_case(kind_stem));
        let builder_call = camel_case(&materializer.rust_builder);

        // PERF-12 §74 status detail kind constant (shared shape for all
        // materializers).
        output.push_str(&format!(
            "/** PERF-12 §74 status detail kind for this materializer: {:?}. */\n",
            materializer.status_detail
        ));
        output.push_str(&format!(
            "export const {}_STATUS_DETAIL = {:?} as const;\n\n",
            materializer.name.to_uppercase(),
            materializer.status_detail
        ));

        if let Some(axis) = &materializer.fixed_arity_axis {
            // Fixed-arity axis kind (§22/§32): children lowered first through
            // ensureNative, then one monomorphic family constructor.
            output.push_str(&format!(
                "export interface {node_interface} {{\n  readonly id: number;\n  readonly gap: number;\n  readonly children: readonly BridgeLayoutChild[];\n}}\n\n"
            ));
            output.push_str(&format!(
                "export function materialize{}(node: {node_interface}, tx: MaterializeTx): number {{\n",
                pascal_case(materializer.name.as_str())
            ));
            output.push_str("  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);\n");
            output.push_str("  const children = node.children;\n");
            output.push_str("  switch (children.length) {\n");
            for (arity, builder_name) in axis.builders.iter().enumerate() {
                let call = camel_case(builder_name);
                let mut args = vec![
                    "tx.symbols".to_owned(),
                    "tx.runtime".to_owned(),
                    "nodeIdLow".to_owned(),
                    "nodeIdHigh".to_owned(),
                    "node.gap".to_owned(),
                ];
                for index in 0..arity {
                    args.push(format!("layoutTrackWord(children[{index}])"));
                    args.push(format!("ensureNative(children[{index}].child, tx)"));
                }
                output.push_str(&format!(
                    "    case {arity}: return {call}({});\n",
                    args.join(", ")
                ));
            }
            match &axis.buffer_builder {
                None => {
                    output.push_str(&format!(
                        "    default: throw new RetainedFastFallbackError(`{} arity ${{children.length}} exceeds fixed-arity specialization {}`);\n",
                        materializer.bridge_kind,
                        axis.builders.len() - 1
                    ));
                }
                Some(buffer_builder) => {
                    // PERF-12 T8 (§29/§32): borrowed-buffer lane. The reusable
                    // scratch holds (track_word, child_ref) pairs; native reads
                    // the storage only during this synchronous call and never
                    // retains a pointer (§29/§107/§116).
                    let axis_kind_literal = match materializer.bridge_kind.as_str() {
                        "viewRow" => 1u32,
                        "viewColumn" => 2u32,
                        other => panic!("validated axis kind {other}"),
                    };
                    let call = camel_case(buffer_builder);
                    output.push_str(&format!(
                        "    default: {{\n      // Single enforcement point: axisRefScratch refuses arities above the\n      // retained cap (Sections 30/50) and counts the fallback.\n      const scratch = tx.axisRefScratch(children.length);\n      let offset = 0;\n      for (let index = 0; index < children.length; index++) {{\n        const child = children[index];\n        scratch[offset++] = layoutTrackWord(child);\n        scratch[offset++] = ensureNative(child.child, tx);\n      }}\n      tx.noteRefWords(offset);\n      return {}(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, {}, node.gap, scratch, children.length);\n    }}\n",
                        call, axis_kind_literal
                    ));
                }
            }
            output.push_str("  }\n}\n\n");
            continue;
        }

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
                        "PERF-12: reference lowering lands in T7/T12; materializer {} declares {}",
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
            "export function materialize{}(node: {node_interface}, tx: MaterializeTx): number {{\n",
            pascal_case(materializer.name.as_str())
        ));
        output.push_str("  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);\n");
        output.push_str(&format!(
            "  return {builder_call}(tx.symbols, tx.runtime, {});\n}}\n\n",
            args_list
        ));
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
