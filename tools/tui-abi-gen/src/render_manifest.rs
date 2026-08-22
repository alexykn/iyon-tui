use askama::Template;
use serde_json::{Value, json};

use crate::model::AbiDocument;

const GENERATOR_SOURCES: &[&[u8]] = &[
    include_bytes!("main.rs"),
    include_bytes!("model.rs"),
    include_bytes!("validate.rs"),
    include_bytes!("render_rust.rs"),
    include_bytes!("render_typescript.rs"),
    include_bytes!("render_header.rs"),
    include_bytes!("render_manifest.rs"),
    include_bytes!("../templates/generated_banner.txt"),
    include_bytes!("../templates/generated_typescript_bindings_header.txt"),
    include_bytes!("../templates/generated_typescript_calls_header.txt"),
    include_bytes!("../templates/generated_c_header_preamble.txt"),
    include_bytes!("../templates/generated_reference_header.txt"),
];

#[derive(Template)]
#[template(path = "generated_banner.txt")]
struct GeneratedBanner<'a> {
    schema_hash: &'a str,
    generator_hash: &'a str,
}

#[derive(Template)]
#[template(path = "generated_typescript_bindings_header.txt")]
struct GeneratedTypescriptBindingsHeader {
    banner: String,
}

#[derive(Template)]
#[template(path = "generated_typescript_calls_header.txt")]
struct GeneratedTypescriptCallsHeader {
    banner: String,
}

#[derive(Template)]
#[template(path = "generated_c_header_preamble.txt")]
struct GeneratedCHeaderPreamble {
    banner: String,
    abi_name: String,
    abi_version: u32,
    semantic_schema: u32,
    minimum_bun: String,
    qualified_bun: String,
}

#[derive(Template)]
#[template(path = "generated_reference_header.txt")]
struct GeneratedReferenceHeader {
    banner: String,
    schema_hash: String,
    generator_hash: String,
    abi_name: String,
    abi_version: u32,
    semantic_schema: u32,
    minimum_bun: String,
    qualified_bun: String,
}

pub fn generator_hash() -> String {
    let mut hasher = blake3::Hasher::new();
    for source in GENERATOR_SOURCES {
        hasher.update(source);
    }
    hasher.finalize().to_hex().to_string()
}

pub fn banner(schema_hash: &str, generator_hash: &str) -> String {
    let rendered = GeneratedBanner {
        schema_hash,
        generator_hash,
    }
    .render()
    .expect("generated banner template is valid");
    if rendered.ends_with('\n') {
        rendered
    } else {
        format!("{rendered}\n")
    }
}

pub fn typescript_bindings_header(schema_hash: &str, generator_hash: &str) -> String {
    GeneratedTypescriptBindingsHeader {
        banner: banner(schema_hash, generator_hash),
    }
    .render()
    .expect("generated TypeScript bindings template is valid")
}

pub fn typescript_calls_header(schema_hash: &str, generator_hash: &str) -> String {
    GeneratedTypescriptCallsHeader {
        banner: banner(schema_hash, generator_hash),
    }
    .render()
    .expect("generated TypeScript calls template is valid")
}

pub fn c_header_preamble(
    document: &AbiDocument,
    schema_hash: &str,
    generator_hash: &str,
) -> String {
    let banner = banner(schema_hash, generator_hash)
        .replace("//", "/*")
        .replace('\n', " */\n");
    GeneratedCHeaderPreamble {
        banner,
        abi_name: document.abi.name.clone(),
        abi_version: document.abi.version,
        semantic_schema: document.abi.semantic_schema,
        minimum_bun: document.abi.minimum_bun.clone(),
        qualified_bun: document.abi.qualified_bun.clone(),
    }
    .render()
    .expect("generated C header template is valid")
}

pub fn reference_header(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    GeneratedReferenceHeader {
        banner: format!(
            "<!-- DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml. schema_blake3 = {schema_hash}; generator_blake3 = {generator_hash} -->\n\n"
        ),
        schema_hash: schema_hash.to_owned(),
        generator_hash: generator_hash.to_owned(),
        abi_name: document.abi.name.clone(),
        abi_version: document.abi.version,
        semantic_schema: document.abi.semantic_schema,
        minimum_bun: document.abi.minimum_bun.clone(),
        qualified_bun: document.abi.qualified_bun.clone(),
    }
    .render()
    .expect("generated Markdown reference template is valid")
}

pub fn manifest(
    document: &AbiDocument,
    schema_hash: &str,
    generator_hash: &str,
    output_paths: &[&str],
) -> String {
    let functions: Vec<Value> = document
        .functions
        .iter()
        .map(|function| {
            json!({
                "name": function.name,
                "family": function.family,
                "hotness": function.hotness,
                "implementation": function.implementation,
                "fallback": function.fallback,
                "ownership": function.ownership,
                "borrow_duration": function.borrow_duration,
                "thread_affinity": function.thread_affinity,
                "may_allocate_native_memory": function.may_allocate_native_memory,
                "mutates_host_state": function.mutates_host_state,
                "max_buffer_bytes": function.max_buffer_bytes,
                "max_input_count": function.max_input_count,
                "arity_specializations": function.arity_specializations,
                "benchmark_registration": function.benchmark_registration,
                "return": function.return_type,
                "args": function.args.iter().map(|argument| json!({
                    "name": argument.name,
                    "type": argument.type_name,
                    "lowering": argument.lowering,
                    "buffer_length_of": argument.buffer_length_of,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let enums: Vec<Value> = document
        .enums
        .iter()
        .map(|enum_spec| {
            json!({
                "name": enum_spec.name,
                "source": enum_spec.source,
                "repr": enum_spec.repr,
                "values": enum_spec.values.iter().map(|value| json!({
                    "name": value.name,
                    "source_key": value.source_key,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let materializers: Vec<Value> = document
        .materializers
        .iter()
        .map(|materializer| {
            json!({
                "name": materializer.name,
                "bridge_kind": materializer.bridge_kind,
                "rust_builder": materializer.rust_builder,
                "fallback": materializer.fallback,
                "ownership": materializer.ownership,
                "borrow_duration": materializer.borrow_duration,
                "thread_affinity": materializer.thread_affinity,
                "status_detail": materializer.status_detail,
                "benchmark_registration": materializer.benchmark_registration,
                "result": materializer.result,
                "fields": materializer.fields.iter().map(|field| json!({
                    "name": field.name,
                    "source": field.source,
                    "type": field.abi_type,
                    "role": field.role,
                    "buffer_length_of": field.buffer_length_of,
                    "max_buffer_bytes": field.max_buffer_bytes,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let conformance: Vec<Value> = document
        .conformance
        .iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "return": spec.return_type,
                "operation": spec.operation,
                "args": spec.args,
            })
        })
        .collect();
    let value = json!({
        "abi": {
            "name": document.abi.name,
            "version": document.abi.version,
            "semantic_schema": document.abi.semantic_schema,
            "minimum_bun": document.abi.minimum_bun,
            "qualified_bun": document.abi.qualified_bun,
            "result_encoding": document.abi.result_encoding,
        },
        "schema_blake3": schema_hash,
        "generator_blake3": generator_hash,
        "handles": document.handles,
        "enums": enums,
        "pods": document.pods,
        "functions": functions,
        "materializers": materializers,
        "conformance": conformance,
        "generated_outputs": output_paths,
    });
    serde_json::to_string_pretty(&value).expect("ABI manifest is serializable") + "\n"
}

pub fn human_reference(document: &AbiDocument, schema_hash: &str, generator_hash: &str) -> String {
    let mut output = reference_header(document, schema_hash, generator_hash);
    output.push_str(
        "## Handles\n\n| Name | Rust | TypeScript | Lifetime | Kind |\n|---|---|---|---|---|\n",
    );
    for handle in &document.handles {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            handle.name,
            handle.rust,
            handle.typescript,
            handle.lifetime,
            handle.kind.as_deref().unwrap_or("-")
        ));
    }
    output.push_str("\n## POD buffers\n\n| Name | Repr | Size | Align |\n|---|---|---:|---:|\n");
    for pod in &document.pods {
        output.push_str(&format!(
            "| `{}` | `{}` | {} | {} |\n",
            pod.name, pod.repr, pod.size, pod.align
        ));
    }
    output.push_str("\n## Enums\n\n");
    for enum_spec in &document.enums {
        output.push_str(&format!(
            "### `{}`\n\n| Value | Bridge key |\n|---|---|\n",
            enum_spec.name
        ));
        for value in &enum_spec.values {
            output.push_str(&format!("| `{}` | `{}` |\n", value.name, value.source_key));
        }
        output.push('\n');
    }
    output.push_str(
        "## Functions\n\n| Name | Family | Hotness | Return | Fallback | Thread | Allocates | Host mutation |\n|---|---|---|---|---|---|---|---|\n",
    );
    for function in &document.functions {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            function.name,
            function.family,
            function.hotness,
            function.return_type,
            function.fallback,
            function.thread_affinity,
            function.may_allocate_native_memory,
            function.mutates_host_state
        ));
    }
    output.push_str("\n## ABI conformance fixtures\n\n| Name | Return | Operation | Arguments |\n|---|---|---|---|\n");
    for spec in &document.conformance {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            spec.name,
            spec.return_type,
            spec.operation,
            spec.args.join(", ")
        ));
    }
    output.push_str("\n");
    output
}
