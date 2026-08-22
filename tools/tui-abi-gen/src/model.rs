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
        source: serde_path_to_error::Error<toml::de::Error>,
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
    #[error("failed to read bridge schema {path}: {source}")]
    BridgeRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse bridge schema {path}: {source}")]
    BridgeParse {
        path: String,
        #[source]
        source: serde_json::Error,
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
    #[serde(rename = "materializer", default)]
    pub materializers: Vec<MaterializerSpec>,
    #[serde(rename = "conformance", default)]
    pub conformance: Vec<ConformanceSpec>,
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
    pub fallback: String,
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
}

/// PERF-12 T5 (§63): strongly typed semantic materializer declaration.
/// Each spec lowers one BridgeViewNode kind into a sequence of generated FFI
/// constructor arguments, children first, with explicit lifetime policy.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializerSpec {
    pub name: String,
    pub bridge_kind: String,
    pub rust_builder: String,
    pub fallback: String,
    pub ownership: String,
    pub borrow_duration: String,
    pub thread_affinity: String,
    /// PERF-12 T5 (§74): `none`, `child_ref`, or `base_ref`. Declares which
    /// stale-ref detail kind the builder's failure status carries so the JS
    /// recovery path can identify the offending ref without probing.
    pub status_detail: String,
    pub benchmark_registration: String,
    pub result: MaterializerResultSpec,
    #[serde(rename = "field", default)]
    pub fields: Vec<MaterializerFieldSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializerFieldSpec {
    pub name: String,
    pub source: String,
    #[serde(rename = "type")]
    pub abi_type: String,
    pub role: String,
    /// Required for RefBuffer/AuxBuffer/ByteBuffer roles (§64): names the
    /// builder argument carrying the element length for this buffer.
    #[serde(default)]
    pub buffer_length_of: Option<String>,
    /// Required for buffer roles (§64): explicit upper bound in bytes.
    #[serde(default)]
    pub max_buffer_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializerResultSpec {
    pub kind: String,
}

/// Strongly typed view of MaterializerFieldSpec.role (§63).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterializerFieldRole {
    NodeIdLow,
    NodeIdHigh,
    Scalar,
    ChildRef,
    RefBuffer,
    AuxBuffer,
    ByteBuffer,
    StyleRef,
    BaseRef,
}

impl MaterializerFieldRole {
    pub fn parse(role: &str) -> Option<Self> {
        Some(match role {
            "node_id_low" => Self::NodeIdLow,
            "node_id_high" => Self::NodeIdHigh,
            "scalar" => Self::Scalar,
            "child_ref" => Self::ChildRef,
            "ref_buffer" => Self::RefBuffer,
            "aux_buffer" => Self::AuxBuffer,
            "byte_buffer" => Self::ByteBuffer,
            "style_ref" => Self::StyleRef,
            "base_ref" => Self::BaseRef,
            _ => return None,
        })
    }

    pub fn is_buffer(self) -> bool {
        matches!(self, Self::RefBuffer | Self::AuxBuffer | Self::ByteBuffer)
    }

    pub fn is_reference(self) -> bool {
        matches!(self, Self::ChildRef | Self::StyleRef | Self::BaseRef)
    }
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
        source: serde_path_to_error::Error::new(track.path(), source),
    })?;
    Ok((document, source, syntax))
}

pub fn load_bridge_schema(
    path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, ModelError> {
    let path_display = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|source| ModelError::BridgeRead {
        path: path_display.clone(),
        source,
    })?;
    serde_json::from_str(&source).map_err(|source| ModelError::BridgeParse {
        path: path_display,
        source,
    })
}
