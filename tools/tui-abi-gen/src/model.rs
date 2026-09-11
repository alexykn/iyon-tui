use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("failed to read UI ABI schema {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse UI ABI schema {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: Box<serde_path_to_error::Error<toml::de::Error>>,
    },
    #[error("failed to parse UI ABI schema {path}: {source}")]
    ParseToml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse UI ABI schema document {path}: {source}")]
    ParseDocument {
        path: String,
        #[source]
        source: toml_edit::TomlError,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiAbiDocument {
    pub abi: UiAbiMetadata,
    #[serde(rename = "section", default)]
    pub sections: Vec<UiSectionSpec>,
    #[serde(rename = "host_kind", default)]
    pub host_kinds: Vec<UiKindSpec>,
    #[serde(rename = "control_kind", default)]
    pub control_kinds: Vec<UiCodeSpec>,
    #[serde(rename = "root_role", default)]
    pub root_roles: Vec<UiCodeSpec>,
    #[serde(rename = "handle_kind", default)]
    pub handle_kinds: Vec<UiCodeSpec>,
    #[serde(rename = "ownership_mode", default)]
    pub ownership_modes: Vec<UiCodeSpec>,
    #[serde(rename = "value_kind", default)]
    pub value_kinds: Vec<UiCodeSpec>,
    #[serde(rename = "effect", default)]
    pub effects: Vec<UiCodeSpec>,
    #[serde(rename = "opcode", default)]
    pub opcodes: Vec<UiOpcodeSpec>,
    #[serde(rename = "control_command", default)]
    pub control_commands: Vec<UiControlCommandSpec>,
    #[serde(rename = "config", default)]
    pub configs: Vec<UiConfigSpec>,
    #[serde(rename = "property", default)]
    pub properties: Vec<UiPropertySpec>,
    #[serde(rename = "value_encoding", default)]
    pub value_encodings: Vec<UiValueEncodingSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiAbiMetadata {
    pub name: String,
    pub version: u32,
    pub semantic_schema: u32,
    pub minimum_bun: String,
    pub qualified_bun: String,
    pub result_encoding: String,
    pub magic: u32,
    pub batch_version: u32,
    pub batch_header_words: u32,
    pub ack_header_words: u32,
    pub ack_words_per_created_handle: u32,
    pub handle_words: u32,
    pub local_handle_host: u32,
    pub local_handle_generation: u32,
    pub ack_status_error_bit: u32,
    pub ack_reserved: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiSectionSpec {
    pub name: String,
    pub code: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiKindSpec {
    pub name: String,
    pub code: u32,
    pub children: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiCodeSpec {
    pub name: String,
    pub code: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiOpcodeSpec {
    pub name: String,
    pub code: u32,
    pub section: String,
    pub operands: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiControlCommandSpec {
    pub name: String,
    pub code: u32,
    pub control_kind: String,
    pub operands: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiConfigSpec {
    pub name: String,
    pub owner: String,
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiPropertySpec {
    pub id: u32,
    pub domain: String,
    pub name: String,
    pub value: String,
    pub legal_kinds: Vec<String>,
    pub normalizer: String,
    pub default: String,
    pub reset: String,
    pub override_behavior: String,
    pub inheritance: String,
    pub effects: Vec<String>,
    pub realization: String,
    pub nullable: bool,
    pub clearable: bool,
    #[serde(default)]
    pub allowed_values: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiValueEncodingSpec {
    pub value_kind: String,
    pub encoding: String,
    pub min_words: u32,
    pub max_words: u32,
    #[serde(default)]
    pub metadata_words: u32,
    pub forms: Vec<UiValueFormSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiValueFormSpec {
    pub name: String,
    pub word_count: u32,
    #[serde(default)]
    pub tags: Vec<u32>,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub values: Vec<u32>,
    #[serde(default)]
    pub mask: Option<u32>,
    #[serde(default)]
    pub max_value: Option<u32>,
}

pub fn load_ui(
    path: &Path,
) -> Result<(UiAbiDocument, String, toml_edit::Document<String>), ModelError> {
    let path_display = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|source| ModelError::Read {
        path: path_display.clone(),
        source,
    })?;
    let syntax =
        toml_edit::Document::parse(source.clone()).map_err(|source| ModelError::ParseDocument {
            path: path_display.clone(),
            source,
        })?;
    let toml_deserializer =
        toml::Deserializer::parse(&source).map_err(|source| ModelError::ParseToml {
            path: path_display.clone(),
            source,
        })?;
    let mut track = serde_path_to_error::Track::new();
    let deserializer = serde_path_to_error::Deserializer::new(toml_deserializer, &mut track);
    let document =
        UiAbiDocument::deserialize(deserializer).map_err(|source| ModelError::Parse {
            path: path_display,
            source: Box::new(serde_path_to_error::Error::new(track.path(), source)),
        })?;
    Ok((document, source, syntax))
}
