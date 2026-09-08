use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("failed to read ABI schema {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse ABI schema {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: Box<serde_path_to_error::Error<toml::de::Error>>,
    },
    #[error("failed to parse ABI schema {path}: {source}")]
    ParseToml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse ABI schema document {path}: {source}")]
    ParseDocument {
        path: String,
        #[source]
        source: toml_edit::TomlError,
    },
    #[error("failed to read kind codes schema {path}: {source}")]
    KindCodesRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse kind codes schema {path}: {source}")]
    KindCodesParse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to read UI ABI schema {path}: {source}")]
    UiRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse UI ABI schema {path}: {source}")]
    UiParse {
        path: String,
        #[source]
        source: Box<serde_path_to_error::Error<toml::de::Error>>,
    },
    #[error("failed to parse UI ABI schema {path}: {source}")]
    UiParseToml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse UI ABI schema document {path}: {source}")]
    UiParseDocument {
        path: String,
        #[source]
        source: toml_edit::TomlError,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbiDocument {
    pub abi: AbiMetadata,
    #[serde(rename = "handle", default)]
    pub handles: Vec<HandleSpec>,
    #[serde(rename = "enum", default)]
    pub enums: Vec<EnumSpec>,
    #[serde(rename = "pod", default)]
    pub pods: Vec<PodSpec>,
    #[serde(rename = "function", default)]
    pub functions: Vec<FunctionSpec>,
    #[serde(rename = "conformance", default)]
    pub conformance: Vec<ConformanceSpec>,
    #[serde(rename = "state_property", default)]
    pub state_properties: Vec<StatePropertySpec>,
}

/// The direct-occurrence UI batch schema.  It deliberately has its own
/// document type: the existing View ABI remains available while this schema
/// is introduced, but both documents are rendered by this one generator.
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
    pub values: Vec<u32>,
    #[serde(default)]
    pub mask: Option<u32>,
    #[serde(default)]
    pub max_value: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbiMetadata {
    pub name: String,
    pub version: u32,
    pub semantic_schema: u32,
    pub minimum_bun: String,
    pub qualified_bun: String,
    pub result_encoding: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HandleSpec {
    pub name: String,
    pub rust: String,
    pub typescript: String,
    pub nullable: bool,
    pub lifetime: String,
    #[serde(default)]
    pub valid: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumSpec {
    pub name: String,
    #[serde(default)]
    pub source: Option<String>,
    pub repr: String,
    #[serde(rename = "value", default)]
    pub values: Vec<EnumValueSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumValueSpec {
    pub name: String,
    pub source_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PodSpec {
    pub name: String,
    pub repr: String,
    pub size: u32,
    pub align: u32,
    #[serde(rename = "field", default)]
    pub fields: Vec<PodFieldSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PodFieldSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionSpec {
    pub name: String,
    pub family: String,
    pub hotness: String,
    pub implementation: String,
    pub ownership: String,
    pub borrow_duration: String,
    pub thread_affinity: String,
    pub may_allocate_native_memory: bool,
    pub mutates_host_state: bool,
    pub max_buffer_bytes: u64,
    pub max_input_count: u32,
    #[serde(default)]
    pub arity_specializations: Vec<u32>,
    pub benchmark_registration: String,
    #[serde(rename = "return")]
    pub return_type: String,
    #[serde(rename = "arg", default)]
    pub args: Vec<ArgumentSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArgumentSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub lowering: String,
    #[serde(default)]
    pub buffer_length_of: Option<String>,
    /// PERF-12 T11 (§41): required on `buffer_used` arguments of functions
    /// declaring more than one variable buffer; names the buffer whose used
    /// count this argument carries. Single-buffer functions infer the pair.
    #[serde(default)]
    pub buffer_used_of: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceSpec {
    pub name: String,
    #[serde(rename = "return")]
    pub return_type: String,
    pub operation: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// One L1-05 retained-state property (§7.1): stable per-domain bit id, TS
/// patch key and diagnostic name, native value kind, nullability, clear
/// support, capability rule, and value-lane layout (u32 words + strings).
/// The envelope packers (TS) and mask/lane tables (Rust) generate from
/// these rows; value semantics stay in hand-written readers.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatePropertySpec {
    pub domain: String,
    pub id: u32,
    pub name: String,
    pub value: String,
    pub nullable: bool,
    pub clearable: bool,
    pub capability: String,
    pub words: u32,
    pub strings: u32,
}

pub fn load(path: &Path) -> Result<(AbiDocument, String, toml_edit::Document<String>), ModelError> {
    let path_display = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|source| ModelError::Read {
        path: path_display.clone(),
        source,
    })?;

    // Parse with toml_edit first so the generator rejects syntax that the
    // serializer cannot understand while retaining declaration order and
    // source spans for explain/diagnostic output.
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
    let document = AbiDocument::deserialize(deserializer).map_err(|source| ModelError::Parse {
        path: path_display,
        source: Box::new(serde_path_to_error::Error::new(track.path(), source)),
    })?;
    Ok((document, source, syntax))
}

pub fn load_kind_codes(
    path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, ModelError> {
    let path_display = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|source| ModelError::KindCodesRead {
        path: path_display.clone(),
        source,
    })?;
    serde_json::from_str(&source).map_err(|source| ModelError::KindCodesParse {
        path: path_display,
        source,
    })
}

pub fn load_ui(
    path: &Path,
) -> Result<(UiAbiDocument, String, toml_edit::Document<String>), ModelError> {
    let path_display = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|source| ModelError::UiRead {
        path: path_display.clone(),
        source,
    })?;
    let syntax = toml_edit::Document::parse(source.clone()).map_err(|source| {
        ModelError::UiParseDocument {
            path: path_display.clone(),
            source,
        }
    })?;
    let toml_deserializer =
        toml::Deserializer::parse(&source).map_err(|source| ModelError::UiParseToml {
            path: path_display.clone(),
            source,
        })?;
    let mut track = serde_path_to_error::Track::new();
    let deserializer = serde_path_to_error::Deserializer::new(toml_deserializer, &mut track);
    let document =
        UiAbiDocument::deserialize(deserializer).map_err(|source| ModelError::UiParse {
            path: path_display,
            source: Box::new(serde_path_to_error::Error::new(track.path(), source)),
        })?;
    Ok((document, source, syntax))
}
