mod model;
mod render_header;
mod render_manifest;
mod render_rust;
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
const BRIDGE_SCHEMA: &str = "packages/iyon-runtime/src/tui/bridge-schema.json";
const GENERATOR_OUTPUTS: &[&str] = &[
    "crates/iyon-native/src/generated/view_abi_types.rs",
    "crates/iyon-native/src/generated/view_abi_exports.rs",
    "crates/iyon-native/src/generated/view_abi_conformance.rs",
    "crates/iyon-native/src/generated/view_abi_table.rs",
    "crates/iyon-native/include/iyon_view_abi.h",
    "packages/iyon-runtime/src/tui/generated/view_abi.ts",
    "packages/iyon-runtime/src/tui/generated/view_abi_conformance.ts",
    "packages/iyon-runtime/src/tui/generated/view_calls.ts",
    "packages/iyon-runtime/src/tui/generated/view_materialize.ts",
    "packages/iyon-runtime/src/tui/generated/view_abi_manifest.json",
    "packages/iyon-runtime/tests/generated/view_abi_layout.test.ts",
    "packages/iyon-runtime/bench/generated/view_abi_cases.ts",
    "crates/iyon-native/tests/generated_view_abi.rs",
    "PERF-11-generated-abi-reference.md",
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
                .get("packages/iyon-runtime/src/tui/generated/view_abi_manifest.json")
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
                "name: {}\nsource_span: {:?}\nfamily: {}\nhotness: {}\nimplementation: {}\nfallback: {}\nownership: {}\nborrow_duration: {}\nthread_affinity: {}\nmay_allocate_native_memory: {}\nmutates_host_state: {}\nmax_buffer_bytes: {}\nmax_input_count: {}\narity_specializations: {:?}\nbenchmark_registration: {}\nreturn: {}",
                function_spec.name,
                source_span,
                function_spec.family,
                function_spec.hotness,
                function_spec.implementation,
                function_spec.fallback,
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
    let bridge_path = workspace.join(BRIDGE_SCHEMA);
    let bridge_schema = model::load_bridge_schema(&bridge_path)?;
    validate::validate(&document, &bridge_schema)?;
    let schema_hash = blake3::hash(schema_source.as_bytes()).to_hex().to_string();
    let generator_hash = render_manifest::generator_hash();
    let output_paths: Vec<&str> = GENERATOR_OUTPUTS.to_vec();
    let mut outputs = BTreeMap::new();
    outputs.insert(
        GENERATOR_OUTPUTS[0].to_owned(),
        render_rust::types(&document, &bridge_schema, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[1].to_owned(),
        render_rust::exports(&document, &bridge_schema, &schema_hash, &generator_hash),
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
        render_header::header(&document, &bridge_schema, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[5].to_owned(),
        render_typescript::abi_bindings(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[6].to_owned(),
        render_typescript::conformance_bindings(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[7].to_owned(),
        render_typescript::calls(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        GENERATOR_OUTPUTS[8].to_owned(),
        render_typescript::materialize(&document, &schema_hash, &generator_hash),
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
        assert!(outputs.contains_key("crates/iyon-native/src/generated/view_abi_types.rs"));
        insta::assert_snapshot!(
            outputs
                .get("packages/iyon-runtime/src/tui/generated/view_abi_manifest.json")
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

    #[test]
    fn validation_rejects_unknown_lowering() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        document.functions[0].args[0].lowering = "not_a_bun_ffi_type".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_incompatible_lowering() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        document.functions[0].args[0].lowering = "u32".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_missing_buffer_used() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        let used = document
            .functions
            .iter_mut()
            .flat_map(|function| function.args.iter_mut())
            .find(|argument| argument.lowering == "buffer_used")
            .expect("canonical buffer_used");
        used.lowering = "u32".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_unpaired_buffer_length() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        let length = document
            .functions
            .iter_mut()
            .flat_map(|function| function.args.iter_mut())
            .find(|argument| argument.lowering == "buffer_length")
            .expect("canonical buffer length");
        length.buffer_length_of = None;
        assert!(validate::validate(&document, &bridge).is_err());
    }

    fn first_materializer_mut(document: &mut model::AbiDocument) -> &mut model::MaterializerSpec {
        document
            .materializers
            .first_mut()
            .expect("canonical materializer slice")
    }

    #[test]
    fn canonical_schema_declares_the_t5_vertical_slice() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (document, _, _) = model::load(&schema).expect("canonical schema parses");
        // T5 shipped the spacer slice; T7 added the row/column fixed-arity
        // axis slices on top of it.
        assert_eq!(document.materializers.len(), 3);
        let spacer = &document.materializers[0];
        assert_eq!(spacer.name, "spacer");
        assert_eq!(spacer.bridge_kind, "viewSpacer");
        assert_eq!(spacer.rust_builder, "view_spacer_create");
        assert_eq!(spacer.borrow_duration, "call");
        assert_eq!(spacer.thread_affinity, "owner_thread");
    }

    #[test]
    fn validation_rejects_unknown_bridge_kind() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        first_materializer_mut(&mut document).bridge_kind = "viewDoesNotExist".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_missing_node_id_half() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        // §64: a u64 NodeId narrowed into a single u32 half fails generation.
        first_materializer_mut(&mut document)
            .fields
            .retain(|field| field.role != "node_id_high");
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_unknown_field_role() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        first_materializer_mut(&mut document).fields[2].role = "magic_ref".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_unbounded_buffer_field() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        let materializer = first_materializer_mut(&mut document);
        materializer.fields.push(model::MaterializerFieldSpec {
            name: "children".to_owned(),
            source: "children".to_owned(),
            abi_type: "ViewRef".to_owned(),
            role: "ref_buffer".to_owned(),
            buffer_length_of: None,
            max_buffer_bytes: None,
        });
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_non_call_borrow_duration() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        // §107: a constructor that can retain a borrowed buffer is illegal.
        first_materializer_mut(&mut document).borrow_duration = "session".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_owner_thread_violation() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        first_materializer_mut(&mut document).thread_affinity = "any_thread".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_duplicate_materializer_name() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        let clone = document.materializers[0].clone();
        document.materializers.push(clone);
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_missing_benchmark_registration() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        first_materializer_mut(&mut document).benchmark_registration = String::new();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_unknown_builder_function() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        first_materializer_mut(&mut document).rust_builder = "view_not_a_function".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn validation_rejects_invalid_conformance_signature() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        document.conformance[0].args[0] = "i32".to_owned();
        assert!(validate::validate(&document, &bridge).is_err());
    }

    #[test]
    fn canonical_schema_declares_the_t7_axis_slices() {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (document, _, _) = model::load(&schema).expect("canonical schema parses");
        assert_eq!(document.materializers.len(), 3);
        let row = document
            .materializers
            .iter()
            .find(|materializer| materializer.name == "row")
            .expect("row materializer");
        let axis = row.fixed_arity_axis.as_ref().expect("fixed-arity axis");
        assert_eq!(axis.builders.len(), 5);
        assert_eq!(axis.builders[0], "view_row_create_0");
        assert_eq!(axis.builders[4], "view_row_create_4");
    }

    /// Builds the canonical document, converts the spacer slice into a
    /// fixed-arity axis declaration, and applies `mutate` before validating.
    fn validate_mutated_axis(
        mutate: impl FnOnce(&mut model::MaterializerSpec),
    ) -> Result<(), validate::ValidationError> {
        let workspace = workspace_root().expect("workspace metadata");
        let schema = workspace.join(DEFAULT_SCHEMA);
        let (mut document, _, _) = model::load(&schema).expect("canonical schema parses");
        let bridge = model::load_bridge_schema(&workspace.join(BRIDGE_SCHEMA))
            .expect("bridge schema parses");
        let position = document
            .materializers
            .iter()
            .position(|materializer| materializer.name == "row")
            .expect("canonical row materializer");
        let materializer = &mut document.materializers[position];
        materializer.fixed_arity_axis = Some(model::MaterializerFixedArityAxisSpec {
            builders: [
                "view_row_create_0",
                "view_row_create_1",
                "view_row_create_2",
                "view_row_create_3",
                "view_row_create_4",
            ]
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
            buffer_builder: None,
        });
        mutate(materializer);
        validate::validate(&document, &bridge)
    }

    #[test]
    fn validation_accepts_well_formed_fixed_arity_axis() {
        if let Err(error) = validate_mutated_axis(|_| {}) {
            panic!("axis should validate: {error}");
        }
    }

    #[test]
    fn validation_rejects_axis_on_non_axis_kind() {
        assert!(
            validate_mutated_axis(|materializer| {
                materializer.bridge_kind = "viewSpacer".to_owned();
            })
            .is_err()
        );
    }

    #[test]
    fn validation_rejects_unknown_family_builder() {
        assert!(
            validate_mutated_axis(|materializer| {
                let axis = materializer.fixed_arity_axis.as_mut().expect("axis");
                axis.builders[2] = "view_not_a_function".to_owned();
            })
            .is_err()
        );
    }

    #[test]
    fn validation_rejects_duplicate_family_builder() {
        assert!(
            validate_mutated_axis(|materializer| {
                let axis = materializer.fixed_arity_axis.as_mut().expect("axis");
                axis.builders[3] = axis.builders[2].clone();
            })
            .is_err()
        );
    }

    #[test]
    fn validation_rejects_family_lifetime_disagreement() {
        // The family builders are runtime_owned/owner_thread; flipping the
        // materializer's thread affinity must fail generation (§69).
        assert!(
            validate_mutated_axis(|materializer| {
                materializer.thread_affinity = "any_thread".to_owned();
            })
            .is_err()
        );
    }

    #[test]
    fn validation_rejects_unknown_buffer_builder() {
        assert!(
            validate_mutated_axis(|materializer| {
                materializer
                    .fixed_arity_axis
                    .as_mut()
                    .expect("axis")
                    .buffer_builder = Some("view_not_a_function".to_owned());
            })
            .is_err()
        );
    }

    #[test]
    fn validation_rejects_buffer_builder_lifetime_disagreement() {
        // view_axis_create_buffer borrows for the call only; flipping the
        // materializer to a session borrow must fail generation (107).
        assert!(
            validate_mutated_axis(|materializer| {
                let axis = materializer.fixed_arity_axis.as_mut().expect("axis");
                axis.buffer_builder = Some("view_axis_create_buffer".to_owned());
                materializer.borrow_duration = "session".to_owned();
            })
            .is_err()
        );
    }

    #[test]
    fn validation_accepts_t8_buffer_lane() {
        assert!(
            validate_mutated_axis(|materializer| {
                materializer
                    .fixed_arity_axis
                    .as_mut()
                    .expect("axis")
                    .buffer_builder = Some("view_axis_create_buffer".to_owned());
            })
            .is_ok()
        );
    }

    #[test]
    fn validation_rejects_rust_builder_outside_family_base() {
        assert!(
            validate_mutated_axis(|materializer| {
                materializer.rust_builder = "view_row_create_2".to_owned();
            })
            .is_err()
        );
    }
}
