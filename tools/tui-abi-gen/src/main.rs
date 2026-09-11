mod model;
mod render_manifest;
mod render_ui;
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

const UI_SCHEMA: &str = "tools/tui-abi/ui_abi.toml";
const UI_GENERATOR_OUTPUTS: &[&str] = &[
    "crates/iyon-tui/src/occurrence/generated.rs",
    "packages/iyon-tui/src/transport/ui/generated/ui_schema.ts",
    "packages/iyon-tui/src/transport/ui/generated/ui_abi_manifest.json",
    "docs/architecture/generated/UI-ABI-REFERENCE.md",
];

#[derive(Debug, Parser)]
#[command(
    name = "tui-abi-gen",
    about = "Generate the direct-occurrence UI schema"
)]
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
    Model(#[from] model::ModelError),
    #[error(transparent)]
    Validation(#[from] validate::ValidationError),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("stale generated output: {0}")]
    Stale(String),
    #[error("unknown UI ABI item {0}")]
    UnknownItem(String),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), GeneratorError> {
    let cli = Cli::parse();
    let workspace = workspace_root()?;
    match cli.command {
        Command::Generate { input, output_root } => {
            let schema = input.unwrap_or_else(|| workspace.join(UI_SCHEMA));
            let root = output_root.unwrap_or_else(|| workspace.clone());
            write_outputs(&root, &render_outputs(&schema)?)
        }
        Command::Check { input, output_root } => {
            let schema = input.unwrap_or_else(|| workspace.join(UI_SCHEMA));
            let root = output_root.unwrap_or_else(|| workspace.clone());
            check_outputs(&root, &render_outputs(&schema)?)
        }
        Command::PrintManifest { input } => {
            let schema = input.unwrap_or_else(|| workspace.join(UI_SCHEMA));
            let outputs = render_outputs(&schema)?;
            print!(
                "{}",
                outputs
                    .get(UI_GENERATOR_OUTPUTS[2])
                    .expect("UI manifest is an authoritative generated output")
            );
            Ok(())
        }
        Command::Explain { function, input } => {
            let schema = input.unwrap_or_else(|| workspace.join(UI_SCHEMA));
            let (document, _, _) = model::load_ui(&schema)?;
            if let Some(opcode) = document.opcodes.iter().find(|item| item.name == function) {
                println!(
                    "name: {}\nsection: {}\ncode: {}\noperands: {:?}",
                    opcode.name, opcode.section, opcode.code, opcode.operands
                );
                return Ok(());
            }
            if let Some(command) = document
                .control_commands
                .iter()
                .find(|item| item.name == function)
            {
                println!(
                    "name: {}\ncontrol_kind: {}\ncode: {}\noperands: {:?}",
                    command.name, command.control_kind, command.code, command.operands
                );
                return Ok(());
            }
            Err(GeneratorError::UnknownItem(function))
        }
    }
}

fn workspace_root() -> Result<PathBuf, GeneratorError> {
    Ok(MetadataCommand::new()
        .no_deps()
        .exec()?
        .workspace_root
        .into_std_path_buf())
}

fn render_outputs(schema_path: &Path) -> Result<BTreeMap<String, String>, GeneratorError> {
    let (document, schema_source, _) = model::load_ui(schema_path)?;
    validate::validate_ui(&document)?;
    let schema_hash = blake3::hash(schema_source.as_bytes()).to_hex().to_string();
    let generator_hash = render_manifest::generator_hash();
    let mut outputs = BTreeMap::new();
    outputs.insert(
        UI_GENERATOR_OUTPUTS[0].to_owned(),
        render_ui::rust_schema(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        UI_GENERATOR_OUTPUTS[1].to_owned(),
        render_ui::typescript_schema(&document, &schema_hash, &generator_hash),
    );
    outputs.insert(
        UI_GENERATOR_OUTPUTS[2].to_owned(),
        render_ui::manifest(
            &document,
            &schema_hash,
            &generator_hash,
            UI_GENERATOR_OUTPUTS,
        ),
    );
    outputs.insert(
        UI_GENERATOR_OUTPUTS[3].to_owned(),
        render_ui::human_reference(&document, &schema_hash, &generator_hash),
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
    for (relative_path, expected) in outputs {
        let actual_path = root.join(relative_path);
        let actual = fs::read_to_string(&actual_path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                GeneratorError::Stale(relative_path.clone())
            } else {
                GeneratorError::Io {
                    path: actual_path.display().to_string(),
                    source,
                }
            }
        })?;
        if actual != *expected {
            return Err(GeneratorError::Stale(relative_path.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_ui_document() -> model::UiAbiDocument {
        let workspace = workspace_root().expect("workspace metadata");
        let (document, _, _) = model::load_ui(&workspace.join(UI_SCHEMA)).expect("UI schema");
        document
    }

    #[test]
    fn canonical_schema_renders_only_live_ui_outputs() {
        let workspace = workspace_root().expect("workspace metadata");
        let outputs =
            render_outputs(&workspace.join(UI_SCHEMA)).expect("canonical schema validates");
        assert_eq!(outputs.len(), UI_GENERATOR_OUTPUTS.len());
        assert!(outputs.contains_key("crates/iyon-tui/src/occurrence/generated.rs"));
        assert!(!outputs.keys().any(|path| path.contains("view_abi")));
        assert!(!outputs.keys().any(|path| path.contains("state_envelope")));
    }

    #[test]
    fn generated_output_paths_are_unique() {
        let unique = UI_GENERATOR_OUTPUTS
            .iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), UI_GENERATOR_OUTPUTS.len());
    }

    #[test]
    fn direct_ui_schema_covers_occurrence_wire_contract() {
        let document = canonical_ui_document();
        validate::validate_ui(&document).expect("UI schema validates");
        assert_eq!(document.host_kinds.len(), 5);
        assert_eq!(document.opcodes.len(), 28);
        assert_eq!(document.control_commands.len(), 24);
        assert_eq!(document.configs.len(), 4);
        assert_eq!(document.properties.len(), 18);
        assert_eq!(document.value_encodings.len(), 11);
    }

    #[test]
    fn ui_validation_rejects_duplicate_property_ids() {
        let mut document = canonical_ui_document();
        document.properties[1].id = document.properties[0].id;
        assert!(validate::validate_ui(&document).is_err());
    }

    #[test]
    fn ui_validation_rejects_unknown_property_effects() {
        let mut document = canonical_ui_document();
        document.properties[0].effects[0] = "PaintEverything".to_owned();
        assert!(validate::validate_ui(&document).is_err());
    }

    #[test]
    fn ui_validation_rejects_wire_constant_drift() {
        let mut document = canonical_ui_document();
        document.abi.batch_header_words += 1;
        assert!(validate::validate_ui(&document).is_err());
    }

    #[test]
    fn ui_validation_rejects_section_drift() {
        let mut document = canonical_ui_document();
        document.sections[0].code = 9;
        assert!(validate::validate_ui(&document).is_err());
    }

    #[test]
    fn ui_validation_rejects_unknown_control_command_kind() {
        let mut document = canonical_ui_document();
        document.control_commands[0].control_kind = "NotAControl".to_owned();
        assert!(validate::validate_ui(&document).is_err());
    }

    #[test]
    fn ui_validation_rejects_invalid_config_owner() {
        let mut document = canonical_ui_document();
        document.configs[0].owner = "props".to_owned();
        assert!(validate::validate_ui(&document).is_err());
    }
}
