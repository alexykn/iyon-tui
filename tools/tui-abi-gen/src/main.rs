mod model;
mod render_header;
mod render_manifest;
mod render_rust;
mod render_state;
mod render_typescript;
mod validate;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use cargo_metadata::MetadataCommand;
use clap::{Parser, Subcommand};
use thiserror::Error;

use crate::{model::ModelError, validate::ValidationError};

const DEFAULT_SCHEMA: &str = "tools/tui-abi/view_abi.toml";
const KIND_CODES_SCHEMA: &str =
    "packages/iyon-tui/src/transport/abi/structural/schema/view-kind-codes.json";
const GENERATOR_OUTPUTS: &[&str] = &[
    "crates/iyon-tui-native/src/generated/view_abi_types.rs",
    "crates/iyon-tui-native/src/generated/view_abi_exports.rs",
    "crates/iyon-tui-native/src/generated/view_abi_conformance.rs",
    "crates/iyon-tui-native/src/generated/view_abi_table.rs",
    "crates/iyon-tui-native/src/generated/view_abi_napi.rs",
    "crates/iyon-tui-native/include/iyon_view_abi.h",
    "packages/iyon-tui/src/transport/abi/structural/generated/view_abi.ts",
    "packages/iyon-tui/src/transport/abi/structural/generated/view_abi_conformance.ts",
    "packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts",
    "packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json",
    "packages/iyon-tui/tests/generated/view_abi_layout.test.ts",
    "packages/iyon-tui/bench/generated/view_abi_cases.ts",
    "crates/iyon-tui-native/tests/generated_view_abi.rs",
    "docs/history/perf/PERF-11-generated-abi-reference.md",
    "crates/iyon-tui-native/src/generated/view_state_schema.rs",
    "packages/iyon-tui/src/transport/state/generated/state_envelope.ts",
];

#[derive(Debug, Parser)]
#[command(name = "tui-abi-gen", about = "Generate the Bun 1.4 native View ABI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Generate {
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        output_root: Option<PathBuf>,
    },
    Check {
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        output_root: Option<PathBuf>,
    },
    PrintManifest {
        #[arg(long)]
        input: Option<PathBuf>,
    },
    Explain {
        function: String,
        #[arg(long)]
        input: Option<PathBuf>,
    },
}

#[derive(Debug, Error)]
enum GeneratorError {
    #[error("unable to resolve the Cargo workspace: {0}")]
    Metadata(#[from] cargo_metadata::Error),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("stale generated output: {0}")]
    Stale(String),
    #[error("unknown ABI function {0}")]
    UnknownFunction(String),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", miette::miette!("{error}"));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), GeneratorError> {
    let cli = Cli::parse();
    let workspace = workspace_root()?;
    match cli.command {
        Command::Generate { input, output_root } => {
            let schema = input.unwrap_or_else(|| workspace.join(DEFAULT_SCHEMA));
            let root = output_root.unwrap_or_else(|| workspace.clone());
            write_outputs(&root, &render_outputs(&workspace, &schema)?)
        }
        Command::Check { input, output_root } => {
            let schema = input.unwrap_or_else(|| workspace.join(DEFAULT_SCHEMA));
            let root = output_root.unwrap_or_else(|| workspace.clone());
            check_outputs(&root, &render_outputs(&workspace, &schema)?)
        }
        Command::PrintManifest { input } => {
            let schema = input.unwrap_or_else(|| workspace.join(DEFAULT_SCHEMA));
            let outputs = render_outputs(&workspace, &schema)?;
            let manifest = outputs
                .get("packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json")
                .expect("manifest is an authoritative generated output");
            print!("{manifest}");
            Ok(())
        }
        Command::Explain { function, input } => {
            let schema = input.unwrap_or_else(|| workspace.join(DEFAULT_SCHEMA));
            let (document, _, syntax) = model::load(&schema)?;
            let function_spec = document
                .functions
                .iter()
                .find(|item| item.name == function)
                .ok_or(GeneratorError::UnknownFunction(function))?;
            let source_span = syntax
                .get("function")
                .and_then(toml_edit::Item::as_array_of_tables)
                .and_then(|functions| {
                    functions.iter().find(|table| {
                        table.get("name").and_then(toml_edit::Item::as_str)
                            == Some(function_spec.name.as_str())
                    })
                })
                .and_then(|table| table.get("name").and_then(toml_edit::Item::span));
            println!(
                "name: {}\nsource_span: {:?}\nfamily: {}\nhotness: {}\nimplementation: {}\nownership: {}\nborrow_duration: {}\nthread_affinity: {}\nmay_allocate_native_memory: {}\nmutates_host_state: {}\nmax_buffer_bytes: {}\nmax_input_count: {}\narity_specializations: {:?}\nbenchmark_registration: {}\nreturn: {}",
                function_spec.name,
                source_span,
                function_spec.family,
                function_spec.hotness,
                function_spec.implementation,
                function_spec.ownership,
                function_spec.borrow_duration,
                function_spec.thread_affinity,
                function_spec.may_allocate_native_memory,
                function_spec.mutates_host_state,
                function_spec.max_buffer_bytes,
                function_spec.max_input_count,
                function_spec.arity_specializations,
                function_spec.benchmark_registration,
                function_spec.return_type
            );
            for argument in &function_spec.args {
                println!(
                    "arg {}: {} ({})",
                    argument.name, argument.type_name, argument.lowering
                );
            }
            Ok(())
        }
    }
}

fn workspace_root() -> Result<PathBuf, GeneratorError> {
    let metadata = MetadataCommand::new().no_deps().exec()?;
    Ok(metadata.workspace_root.into_std_path_buf())
}

fn render_outputs(
    workspace: &Path,
    schema_path: &Path,
) -> Result<BTreeMap<String, String>, GeneratorError> {
    let (document, schema_source, _) = model::load(schema_path)?;
    let kind_codes_path = workspace.join(KIND_CODES_SCHEMA);
    let kind_codes = model::load_kind_codes(&kind_codes_path)?;
    validate::validate(&document, &kind_codes)?;
    let schema_hash = blake3::hash(schema_source.as_bytes()).to_hex().to_string();
    let generator_hash = render_manifest::generator_hash();
    let output_paths: Vec<&str> = GENERATOR_OUTPUTS.to_vec();
    let mut outputs = BTreeMap::new();
    outputs.insert(
        GENERATOR_OUTPUTS[0].to_owned(),
        render_rust::types(&document, &kind_codes, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[1].to_owned(),
        render_rust::exports(&document, &kind_codes, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[2].to_owned(),
        render_rust::conformance(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[3].to_owned(),
        render_rust::table(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[4].to_owned(),
        render_rust::napi_methods(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[5].to_owned(),
        render_header::header(&document, &kind_codes, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[6].to_owned(),
        render_typescript::abi_bindings(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[7].to_owned(),
        render_typescript::conformance_bindings(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[8].to_owned(),
        render_typescript::calls(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[9].to_owned(),
        render_manifest::manifest(&document, &schema_hash, &generator_hash, &output_paths),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[10].to_owned(),
        render_typescript::layout_test(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[11].to_owned(),
        render_typescript::benchmark_registry(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[12].to_owned(),
        render_rust::layout_tests(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[13].to_owned(),
        render_manifest::human_reference(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[14].to_owned(),
        render_state::rust_schema(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[15].to_owned(),
        render_state::typescript_envelope(&document, &schema_hash, &generator_hash),
    );
    Ok(outputs)
}

fn write_outputs(root: &Path, outputs: &BTreeMap<String, String>) -> Result<(), GeneratorError> {
    for (relative_path, content) in outputs {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| GeneratorError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }
        fs::write(&path, content).map_err(|source| GeneratorError::Io {
            path: path.display().to_string(),
            source,
        })?;
    }
    Ok(())
}

fn check_outputs(root: &Path, outputs: &BTreeMap<String, String>) -> Result<(), GeneratorError> {
    let temporary = tempfile::tempdir().map_err(|source| GeneratorError::Io {
        path: "temporary generator output".to_owned(),
        source,
    })?;
    write_outputs(temporary.path(), outputs)?;
    for relative_path in outputs.keys() {
        let expected_path = temporary.path().join(relative_path);
        let actual_path = root.join(relative_path);
        let expected = fs::read_to_string(&expected_path).map_err(|source| GeneratorError::Io {
            path: expected_path.display().to_string(),
            source,
        })?;
        let actual = match fs::read_to_string(&actual_path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(GeneratorError::Stale(relative_path.clone()));
            }
            Err(source) => {
                return Err(GeneratorError::Io {
                    path: actual_path.display().to_string(),
                    source,
                });
            }
        };
        if actual != expected {
            return Err(GeneratorError::Stale(relative_path.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_schema_renders_all_tranche_one_outputs() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let outputs = render_outputs(&workspace, &schema).expect("canonical schema validates");
        assert_eq!(outputs.len(), GENERATOR_OUTPUTS.len());
        assert!(outputs.contains_key("crates/iyon-tui-native/src/generated/view_abi_types.rs"));
        insta::assert_snapshot!(
            outputs
                .get("packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json")
                .expect("manifest output")
        );
    }

    #[test]
    fn generated_output_paths_are_unique() {
        let unique = GENERATOR_OUTPUTS
            .iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), GENERATOR_OUTPUTS.len());
    }

    fn canonical_document() -> (
        model::AbiDocument,
        serde_json::Map<String, serde_json::Value>,
    ) {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (document, _, _) = model::load(&schema).expect("canonical schema parses");
        let kind_codes = model::load_kind_codes(&workspace.join(KIND_CODES_SCHEMA))
            .expect("kind codes schema parses");
        (document, kind_codes)
    }

    #[test]
    fn validation_rejects_gapped_state_property_ids() {
        let (mut document, kind_codes) = canonical_document();
        document
            .state_properties
            .iter_mut()
            .find(|property| property.domain == "geometry" && property.id == 9)
            .expect("geometry bit 9 exists")
            .id = 10;
        assert!(validate::validate(&document, &kind_codes).is_err());
    }

    #[test]
    fn validation_rejects_unknown_state_value_kind() {
        let (mut document, kind_codes) = canonical_document();
        document
            .state_properties
            .iter_mut()
            .find(|property| property.domain == "presentation" && property.id == 0)
            .expect("presentation bit 0 exists")
            .value = "gradient".to_owned();
        assert!(validate::validate(&document, &kind_codes).is_err());
    }

    #[test]
    fn validation_rejects_state_lane_shapes_that_do_not_match_value_kinds() {
        let (document, kind_codes) = canonical_document();
        let expected = [
            ("size_mode", 1, 0),
            ("u16", 1, 0),
            ("insets", 4, 0),
            ("alignment", 1, 0),
            ("edges", 2, 0),
            ("color", 0, 1),
            ("border_style", 1, 0),
            ("glyphs", 0, 8),
            ("text_attrs", 2, 0),
            ("style", 2, 3),
        ];
        for (value_kind, words, strings) in expected {
            let property = document
                .state_properties
                .iter()
                .find(|property| property.value == value_kind)
                .expect("canonical schema covers every value kind");

            let mut bad_words = document.clone();
            let bad_words_value = if words == 8 { 7 } else { words + 1 };
            bad_words
                .state_properties
                .iter_mut()
                .find(|candidate| {
                    candidate.domain == property.domain && candidate.id == property.id
                })
                .expect("property remains addressable")
                .words = bad_words_value;
            assert!(
                validate::validate(&bad_words, &kind_codes).is_err(),
                "{value_kind} must reject words={bad_words_value}, strings={strings}"
            );

            let mut bad_strings = document.clone();
            let bad_strings_value = if strings == 8 { 7 } else { strings + 1 };
            bad_strings
                .state_properties
                .iter_mut()
                .find(|candidate| {
                    candidate.domain == property.domain && candidate.id == property.id
                })
                .expect("property remains addressable")
                .strings = bad_strings_value;
            assert!(
                validate::validate(&bad_strings, &kind_codes).is_err(),
                "{value_kind} must reject words={words}, strings={bad_strings_value}"
            );
        }
    }

    #[test]
    fn state_envelope_outputs_cover_both_domains() {
        let (document, _) = canonical_document();
        for (path, body) in [
            (
                "view_state_schema.rs",
                render_state::rust_schema(&document, "test", "test"),
            ),
            (
                "state_envelope.ts",
                render_state::typescript_envelope(&document, "test", "test"),
            ),
        ] {
            assert!(body.contains("geometry"), "{path} covers geometry");
            assert!(body.contains("presentation"), "{path} covers presentation");
            assert!(body.contains("borderEdges"), "{path} covers object lanes");
            assert!(
                body.contains("textAttributes"),
                "{path} covers attribute lanes"
            );
        }
    }

    #[test]
    fn validation_rejects_unknown_lowering() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let kind_codes = model::load_kind_codes(&workspace.join(KIND_CODES_SCHEMA))
            .expect("kind codes schema parses");
        document.functions[0].args[0].lowering = "not_a_bun_ffi_type".to_owned();
        assert!(validate::validate(&document, &kind_codes).is_err());
    }

    #[test]
    fn validation_rejects_incompatible_lowering() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let kind_codes = model::load_kind_codes(&workspace.join(KIND_CODES_SCHEMA))
            .expect("kind codes schema parses");
        document.functions[0].args[0].lowering = "u32".to_owned();
        assert!(validate::validate(&document, &kind_codes).is_err());
    }

    #[test]
    fn validation_rejects_missing_buffer_used() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let kind_codes = model::load_kind_codes(&workspace.join(KIND_CODES_SCHEMA))
            .expect("kind codes schema parses");
        let used = document
            .functions
            .iter_mut()
            .flat_map(|function| function.args.iter_mut())
            .find(|argument| argument.lowering == "buffer_used")
            .expect("canonical buffer_used");
        used.lowering = "u32".to_owned();
        assert!(validate::validate(&document, &kind_codes).is_err());
    }

    #[test]
    fn validation_rejects_unpaired_buffer_length() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let kind_codes = model::load_kind_codes(&workspace.join(KIND_CODES_SCHEMA))
            .expect("kind codes schema parses");
        let length = document
            .functions
            .iter_mut()
            .flat_map(|function| function.args.iter_mut())
            .find(|argument| argument.lowering == "buffer_length")
            .expect("canonical buffer length");
        length.buffer_length_of = None;
        assert!(validate::validate(&document, &kind_codes).is_err());
    }

    #[test]
    fn validation_rejects_invalid_conformance_signature() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let kind_codes = model::load_kind_codes(&workspace.join(KIND_CODES_SCHEMA))
            .expect("kind codes schema parses");
        document.conformance[0].args[0] = "i32".to_owned();
        assert!(validate::validate(&document, &kind_codes).is_err());
    }
}
