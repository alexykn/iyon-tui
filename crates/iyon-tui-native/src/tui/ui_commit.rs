//! Qualified v1 occurrence UI ingress.
//!
//! The N-API entrypoint is a boundary decoder; semantic control transitions
//! live in the core occurrence control module, while this module coordinates
//! the host-local resource plan. It owns no renderer, frame, or View state.
//! JavaScript values are qualified and copied before the typed occurrence
//! transaction is prepared; only the already-owned acknowledgement is returned
//! after the core apply.

use std::{collections::HashMap, mem::size_of};

use napi::{
    Env, Error, Status,
    bindgen_prelude::{
        Array, ArrayBuffer, ClassInstance, FromNapiValue, JsValue, TypedArray, Uint32Array,
    },
};

use iyon_tui::binding::{
    Alignment, AlignmentAxis, BorderStyle, ColorSpec, ControlConfig, ControlKind, Edges,
    FunnelSpec, HostContentSource, HostKind, HostNamespace, Insets, LayerValue, LayoutMode,
    NodeRef, OwnershipMode, PropertyId, PropertyValue, ResourceRef, RootConfig, RootRole, SizeMode,
    StyleRef, StyleSpec, TextAttribute, TextAttributeSpec, UI_ACK_HEADER_WORDS,
    UI_ACK_WORDS_PER_CREATED_HANDLE, UI_BATCH_HEADER_WORDS, UI_BATCH_MAGIC, UI_BATCH_VERSION,
    UiCommit, UiHandle, UiOpcode, UiOperation, UiOperationResult, ValueKind, value_encoding,
    value_encoding_form,
};

use super::NativeTextSource;
use iyon_tui::binding::UiResourceOwner;

pub(crate) struct NativeUiState {
    pub(crate) resources: UiResourceOwner,
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NativeUiState>();
};

impl NativeUiState {
    pub(crate) fn new(
        namespace: HostNamespace,
        environment: iyon_tui::binding::TuiEnvironment,
    ) -> Self {
        Self {
            resources: UiResourceOwner::new(namespace, environment),
        }
    }

    pub(crate) fn namespace(&self) -> HostNamespace {
        self.resources.namespace
    }

    pub(crate) fn body_handle(&self) -> Result<UiHandle, String> {
        self.resources.body_handle()
    }

    pub(crate) fn close(&mut self) -> Result<(), String> {
        self.resources.close()
    }

    fn commit(
        &mut self,
        batch: UiCommit,
        sources: &[HostContentSource],
    ) -> Result<UiOperationResult, iyon_tui::binding::UiRejection> {
        let no_existing_history = HashMap::new();
        let output = self
            .resources
            .commit(batch, sources, &no_existing_history)?;
        HostContentSource::finish_prepared_wakes(output.wakes);
        Ok(output.result)
    }
}

const HEADER_WORDS: usize = UI_BATCH_HEADER_WORDS;

const OP_CREATE_NODE: u32 = UiOpcode::CreateNode as u32;
const OP_CREATE_ROOT: u32 = UiOpcode::CreateRoot as u32;
const OP_INSERT_BEFORE: u32 = UiOpcode::InsertBefore as u32;
const OP_DETACH: u32 = UiOpcode::Detach as u32;
const OP_RETIRE_SUBTREE: u32 = UiOpcode::RetireSubtree as u32;
const OP_ATTACH_PORT: u32 = UiOpcode::AttachPort as u32;
const OP_ATTACH_CONTROL: u32 = UiOpcode::AttachControl as u32;
const OP_CREATE_CONTROL: u32 = UiOpcode::CreateControl as u32;
const OP_DISPOSE_CONTROL: u32 = UiOpcode::DisposeControl as u32;
const OP_HISTORY_ACTION: u32 = UiOpcode::HistoryAction as u32;
const OP_RETIRE_ROOT: u32 = UiOpcode::RetireRoot as u32;
const OP_SET_DECLARED: u32 = UiOpcode::SetDeclared as u32;
const OP_RESET_DECLARED: u32 = UiOpcode::ResetDeclared as u32;
const OP_SET_OVERRIDE: u32 = UiOpcode::SetOverride as u32;
const OP_CLEAR_OVERRIDE: u32 = UiOpcode::ClearOverride as u32;
const OP_SET_HIDDEN: u32 = UiOpcode::SetHidden as u32;
const OP_SET_STYLE_STATE: u32 = UiOpcode::SetStyleState as u32;
const OP_CLEAR_STYLE_STATE: u32 = UiOpcode::ClearStyleState as u32;
const OP_CONTROL_COMMAND: u32 = UiOpcode::ControlCommand as u32;
const OP_CREATE_PORT: u32 = UiOpcode::CreatePort as u32;
const OP_CREATE_CONNECTOR: u32 = UiOpcode::CreateConnector as u32;
const OP_SELECT_CONNECTOR: u32 = UiOpcode::SelectConnector as u32;
const OP_DISPOSE_CONNECTOR: u32 = UiOpcode::DisposeConnector as u32;
const OP_DISPOSE_PORT: u32 = UiOpcode::DisposePort as u32;
const OP_SET_LITERAL_FUNNEL: u32 = UiOpcode::SetLiteralFunnel as u32;
const OP_SET_SUBSCRIPTIONS: u32 = UiOpcode::SetSubscriptions as u32;
const OP_REPLACE_LITERAL: u32 = UiOpcode::ReplaceLiteral as u32;
const OP_REPLACE_EDITOR: u32 = UiOpcode::ReplaceEditorContent as u32;

const MAX_WORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;
const MAX_LOCAL_COUNT: u32 = 1_048_576;
const SECTION_COUNT: usize = 5;

#[derive(Clone, Copy)]
struct Header {
    expected_revision: u64,
    local_count: u32,
    sections: [usize; SECTION_COUNT],
    metadata_len: usize,
    content_len: usize,
    source_count: usize,
}
pub(crate) fn commit_ui_v1(
    native_host: &super::NativeTuiHost,
    env: &Env,
    words: TypedArray,
    metadata: TypedArray,
    owned_content: TypedArray,
    sources: Array,
) -> napi::Result<Uint32Array> {
    let words = checked_u32_input(env, &words, "words")?;
    let metadata = checked_u8_input(env, &metadata, "metadata")?;
    let owned_content = checked_u8_input(env, &owned_content, "ownedContent")?;
    let ui_namespace = native_host
        .host
        .ui_namespace()
        .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))?;
    let header = decode_header(
        &words,
        metadata.len(),
        owned_content.len(),
        sources.len(),
        ui_namespace,
    )?;
    let environment = &native_host.ui_environment;
    let source_values = decode_sources(
        &sources,
        environment.environment_slot(),
        environment.environment_generation(),
        header.source_count,
    )?;
    let commit = decode_commit(&words, &header, &metadata, &owned_content)?;
    let decoded_creation_count = commit
        .operations()
        .iter()
        .filter(|operation| {
            matches!(
                operation,
                UiOperation::CreateNode { .. }
                    | UiOperation::CreateRoot { .. }
                    | UiOperation::CreatePort { .. }
                    | UiOperation::CreateConnector { .. }
                    | UiOperation::CreateControl { .. }
            )
        })
        .count();
    if decoded_creation_count != usize::try_from(header.local_count).unwrap_or(usize::MAX) {
        return Err(malformed(
            "local creation count does not match decoded records",
        ));
    }

    // The acknowledgement JS object is created before any host/core mutation.
    // The core's final word count is bounded by the header's local count.
    let ack_word_count = UI_ACK_HEADER_WORDS
        .checked_add(
            usize::try_from(header.local_count)
                .map_err(|_| malformed("local count"))?
                .checked_mul(UI_ACK_WORDS_PER_CREATED_HANDLE)
                .ok_or_else(|| malformed("ack size"))?,
        )
        .ok_or_else(|| malformed("ack size"))?;
    let mut acknowledgement = preallocate_acknowledgement(env, ack_word_count)?;

    // Decode/source qualification and all typed validation occur before this
    // call. The occurrence core performs its own final desired-state checks and
    // reserves all apply storage before installing anything.
    match native_host.host.commit_ui(commit, &source_values) {
        Ok(result) => copy_acknowledgement(&mut acknowledgement, &result),
        Err(rejection) => copy_acknowledgement(&mut acknowledgement, &rejection.result),
    }
    Ok(acknowledgement)
}

fn copy_acknowledgement(output: &mut Uint32Array, result: &UiOperationResult) {
    let target = unsafe { output.as_mut() };
    assert!(
        target.len() >= result.words.len(),
        "acknowledgement was preallocated from header"
    );
    target[..result.words.len()].copy_from_slice(&result.words);
}

fn preallocate_acknowledgement(env: &Env, word_count: usize) -> napi::Result<Uint32Array> {
    let byte_count = word_count
        .checked_mul(size_of::<u32>())
        .ok_or_else(|| malformed("ack size"))?;
    let mut backing = std::ptr::null_mut();
    let mut arraybuffer = std::ptr::null_mut();
    let status = unsafe {
        napi::sys::napi_create_arraybuffer(env.raw(), byte_count, &mut backing, &mut arraybuffer)
    };
    if status != napi::sys::Status::napi_ok {
        return Err(Error::new(
            Status::GenericFailure,
            "failed to allocate acknowledgement ArrayBuffer",
        ));
    }
    let mut typed = std::ptr::null_mut();
    let status = unsafe {
        napi::sys::napi_create_typedarray(env.raw(), 6, word_count, arraybuffer, 0, &mut typed)
    };
    if status != napi::sys::Status::napi_ok {
        return Err(Error::new(
            Status::GenericFailure,
            "failed to allocate acknowledgement Uint32Array",
        ));
    }
    unsafe { Uint32Array::from_napi_value(env.raw(), typed) }
        .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))
}

fn checked_u32_input(env: &Env, input: &TypedArray, name: &str) -> napi::Result<Vec<u32>> {
    let bytes = checked_typed_input(env, input, size_of::<u32>(), 6, MAX_WORD_BYTES, name)?;
    if bytes.len() % size_of::<u32>() != 0 {
        return Err(malformed(format!("{name} has a partial u32 lane")));
    }
    Ok(bytes
        .chunks_exact(size_of::<u32>())
        .map(|chunk| u32::from_ne_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect())
}

fn checked_u8_input(env: &Env, input: &TypedArray, name: &str) -> napi::Result<Vec<u8>> {
    let cap = if name == "metadata" {
        MAX_METADATA_BYTES
    } else {
        MAX_CONTENT_BYTES
    };
    checked_typed_input(env, input, size_of::<u8>(), 1, cap, name)
}

fn checked_typed_input(
    env: &Env,
    input: &TypedArray,
    element_size: usize,
    expected_type: i32,
    cap: usize,
    name: &str,
) -> napi::Result<Vec<u8>> {
    let checked =
        checked_typed_input_info(env, input.raw(), element_size, expected_type, cap, name)?;
    if checked.byte_len == 0 {
        return Ok(Vec::new());
    }
    Ok(unsafe { std::slice::from_raw_parts(checked.data as *const u8, checked.byte_len) }.to_vec())
}

struct CheckedTypedArray {
    data: *mut std::ffi::c_void,
    byte_len: usize,
}

fn checked_typed_input_info(
    env: &Env,
    value: napi::sys::napi_value,
    element_size: usize,
    expected_type: i32,
    cap: usize,
    name: &str,
) -> napi::Result<CheckedTypedArray> {
    let raw_env = env.raw();
    let mut array_type = 0i32;
    let mut length = 0usize;
    let mut data = std::ptr::null_mut();
    let mut backing = std::ptr::null_mut();
    let mut byte_offset = 0usize;
    let status = unsafe {
        napi::sys::napi_get_typedarray_info(
            raw_env,
            value,
            &mut array_type,
            &mut length,
            &mut data,
            &mut backing,
            &mut byte_offset,
        )
    };
    if status != napi::sys::Status::napi_ok {
        return Err(malformed(format!("{name} is not a valid typed array")));
    }
    if array_type != expected_type {
        return Err(malformed(format!("{name} has the wrong typed-array kind")));
    }
    let mut is_arraybuffer = false;
    let status = unsafe { napi::sys::napi_is_arraybuffer(raw_env, backing, &mut is_arraybuffer) };
    if status != napi::sys::Status::napi_ok || !is_arraybuffer {
        return Err(malformed(format!(
            "{name} must use a non-shared ArrayBuffer backing"
        )));
    }
    let backing_buffer = unsafe { ArrayBuffer::from_napi_value(raw_env, backing) }
        .map_err(|_| malformed(format!("{name} backing ArrayBuffer is unavailable")))?;
    if backing_buffer
        .is_detached()
        .map_err(|_| malformed(format!("{name} backing ArrayBuffer cannot be qualified")))?
    {
        return Err(malformed(format!("{name} uses a detached ArrayBuffer")));
    }
    let bytes = length
        .checked_mul(element_size)
        .ok_or_else(|| malformed(name))?;
    if bytes > cap {
        return Err(malformed(format!("{name} exceeds its UI byte cap")));
    }
    let span_end = byte_offset
        .checked_add(bytes)
        .ok_or_else(|| malformed(format!("{name} byte span overflow")))?;
    if span_end > backing_buffer.len() {
        return Err(malformed(format!(
            "{name} byte span exceeds its backing ArrayBuffer"
        )));
    }
    if bytes != 0 && data.is_null() {
        return Err(malformed(format!("{name} has no backing bytes")));
    }
    Ok(CheckedTypedArray {
        data,
        byte_len: bytes,
    })
}

fn decode_header(
    words: &[u32],
    metadata_len: usize,
    content_len: usize,
    source_len: u32,
    expected_namespace: u32,
) -> napi::Result<Header> {
    if words.len() < HEADER_WORDS {
        return Err(malformed("UI batch header is truncated"));
    }
    if words[0] != UI_BATCH_MAGIC || words[1] != UI_BATCH_VERSION {
        return Err(malformed("UI batch magic or version is invalid"));
    }
    let total = usize::try_from(words[2]).map_err(|_| malformed("word count"))?;
    if total != words.len() {
        return Err(malformed("UI batch word count is not exact"));
    }
    if words[15] != 0 {
        return Err(malformed("UI batch reserved header word is nonzero"));
    }
    if words[3] != expected_namespace {
        return Err(malformed("UI batch belongs to another host namespace"));
    }
    if words[6] > MAX_LOCAL_COUNT {
        return Err(malformed("local creation count exceeds host capacity"));
    }
    let section_words = [
        usize::try_from(words[7]).map_err(|_| malformed("section size"))?,
        usize::try_from(words[8]).map_err(|_| malformed("section size"))?,
        usize::try_from(words[9]).map_err(|_| malformed("section size"))?,
        usize::try_from(words[10]).map_err(|_| malformed("section size"))?,
        usize::try_from(words[11]).map_err(|_| malformed("section size"))?,
    ];
    let sum = section_words
        .iter()
        .try_fold(HEADER_WORDS, |sum, size| sum.checked_add(*size))
        .ok_or_else(|| malformed("section size overflow"))?;
    if sum != words.len() {
        return Err(malformed("UI batch sections do not cover the input"));
    }
    let metadata_count = usize::try_from(words[12]).map_err(|_| malformed("metadata size"))?;
    let content_count = usize::try_from(words[13]).map_err(|_| malformed("content size"))?;
    let source_count = usize::try_from(words[14]).map_err(|_| malformed("source count"))?;
    if metadata_count != metadata_len
        || content_count != content_len
        || source_count != source_len as usize
    {
        return Err(malformed("UI sidecar lengths do not match the header"));
    }
    Ok(Header {
        expected_revision: u64::from(words[4]) | (u64::from(words[5]) << 32),
        local_count: words[6],
        sections: section_words,
        metadata_len,
        content_len,
        source_count,
    })
}

fn decode_sources(
    sources: &Array,
    environment_slot: u32,
    environment_generation: u32,
    expected: usize,
) -> napi::Result<Vec<HostContentSource>> {
    if sources.len() as usize != expected {
        return Err(malformed(
            "source reference count does not match the header",
        ));
    }
    let mut values = Vec::with_capacity(expected);
    for index in 0..sources.len() {
        let source = sources
            .get::<ClassInstance<'_, NativeTextSource>>(index)
            .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))?
            .ok_or_else(|| malformed("source array contains a hole"))?;
        if !source.alive.load(std::sync::atomic::Ordering::Acquire) {
            return Err(malformed("source reference is disposed"));
        }
        if source.source.environment_slot() != environment_slot
            || source.source.environment_generation() != environment_generation
        {
            return Err(malformed("source belongs to another environment"));
        }
        if !source.source.is_live() {
            return Err(malformed("source reference is disposed"));
        }
        values.push(source.source.clone());
    }
    Ok(values)
}

fn decode_commit(
    words: &[u32],
    header: &Header,
    metadata: &[u8],
    content: &[u8],
) -> napi::Result<UiCommit> {
    let mut commit = UiCommit::new(header.expected_revision);
    let mut offset = HEADER_WORDS;
    for (section_index, section_size) in header.sections.iter().copied().enumerate() {
        let end = offset
            .checked_add(section_size)
            .ok_or_else(|| malformed("section overflow"))?;
        let mut cursor = Cursor {
            words,
            pos: offset,
            end,
        };
        while cursor.pos < cursor.end {
            let record = cursor.record()?;
            decode_record(
                section_index,
                record,
                &mut commit,
                header.local_count,
                metadata,
                content,
            )?;
        }
        if cursor.pos != end {
            return Err(malformed("section decoder did not consume exactly"));
        }
        offset = end;
    }
    Ok(commit)
}

struct Record<'a> {
    opcode: u32,
    operands: &'a [u32],
    index: usize,
}

struct Cursor<'a> {
    words: &'a [u32],
    pos: usize,
    end: usize,
}

impl<'a> Cursor<'a> {
    fn record(&mut self) -> napi::Result<Record<'a>> {
        let index = self.pos;
        if self.end - self.pos < 2 {
            return Err(malformed("record header is truncated"));
        }
        let opcode = self.words[self.pos];
        let count =
            usize::try_from(self.words[self.pos + 1]).map_err(|_| malformed("record size"))?;
        if count < 2 || count > self.end - self.pos {
            return Err(malformed("record size is invalid"));
        }
        self.pos += count;
        Ok(Record {
            opcode,
            operands: &self.words[index + 2..index + count],
            index,
        })
    }
}

fn decode_record(
    section: usize,
    record: Record<'_>,
    commit: &mut UiCommit,
    local_count: u32,
    metadata: &[u8],
    content: &[u8],
) -> napi::Result<()> {
    let o = record.operands;
    let node = |at: usize| decode_node_ref(o, at, local_count);
    let resource = |at: usize, kind| decode_resource_ref(o, at, local_count, kind);
    match record.opcode {
        OP_CREATE_NODE if section == 0 => expect_len(o, 2, record.index).and_then(|_| {
            commit.push(UiOperation::CreateNode {
                local_ordinal: o[0],
                kind: HostKind::from_code(o[1])
                    .ok_or_else(|| malformed_at("host kind", record.index))?,
            });
            Ok(())
        }),
        OP_CREATE_ROOT if section == 0 => {
            expect_len(o, 8, record.index)?;
            let owner = decode_optional_node_ref(o, 2, local_count)?;
            let config = decode_root_config(
                RootRole::from_code(o[1]).ok_or_else(|| malformed_at("root role", record.index))?,
                metadata_slice(metadata, o[6], o[7])?,
                record.index,
            )?;
            commit.push(UiOperation::CreateRoot {
                local_ordinal: o[0],
                role: RootRole::from_code(o[1])
                    .ok_or_else(|| malformed_at("root role", record.index))?,
                owner,
            });
            commit.set_root_config(o[0], config);
            Ok(())
        }
        OP_INSERT_BEFORE if section == 0 => {
            expect_len(o, 12, record.index)?;
            commit.push(UiOperation::InsertBefore {
                parent: node(0)?,
                child: node(4)?,
                before: decode_optional_node_ref(o, 8, local_count)?,
            });
            Ok(())
        }
        OP_DETACH if section == 0 => {
            expect_len(o, 8, record.index)?;
            commit.push(UiOperation::Detach {
                parent: node(0)?,
                child: node(4)?,
            });
            Ok(())
        }
        OP_RETIRE_SUBTREE if section == 0 => {
            expect_len(o, 4, record.index)?;
            commit.push(UiOperation::RetireSubtree { root: node(0)? });
            Ok(())
        }
        OP_RETIRE_ROOT if section == 0 => {
            expect_len(o, 4, record.index)?;
            commit.push(UiOperation::RetireRoot { root: node(0)? });
            Ok(())
        }
        OP_ATTACH_PORT if section == 0 => {
            expect_len(o, 8, record.index)?;
            commit.push(UiOperation::AttachPort {
                node: node(0)?,
                port: decode_optional_resource_ref(o, 4, local_count, 2)?,
            });
            Ok(())
        }
        OP_ATTACH_CONTROL if section == 0 => {
            expect_len(o, 8, record.index)?;
            commit.push(UiOperation::AttachControl {
                node: node(0)?,
                control: decode_optional_resource_ref(o, 4, local_count, 4)?,
            });
            Ok(())
        }
        OP_CREATE_CONTROL if section == 0 => {
            expect_len(o, 9, record.index)?;
            let kind = ControlKind::from_code(o[1])
                .ok_or_else(|| malformed_at("control kind", record.index))?;
            let config =
                decode_control_config(kind, metadata_slice(metadata, o[7], o[8])?, record.index)?;
            commit.push(UiOperation::CreateControl {
                local_ordinal: o[0],
                kind,
                ownership: OwnershipMode::from_code(o[2])
                    .ok_or_else(|| malformed_at("ownership mode", record.index))?,
                owner: decode_optional_node_ref(o, 3, local_count)?,
            });
            commit.set_control_config(o[0], config);
            Ok(())
        }
        OP_DISPOSE_CONTROL if section == 0 => {
            expect_len(o, 4, record.index)?;
            commit.push(UiOperation::DisposeControl {
                control: resource(0, 4)?,
            });
            Ok(())
        }
        OP_HISTORY_ACTION if section == 0 => {
            expect_len(o, 5, record.index)?;
            commit.push(UiOperation::HistoryAction {
                root: node(0)?,
                action_id: o[4],
            });
            Ok(())
        }
        OP_CONTROL_COMMAND if section == 1 => {
            if o.len() < 5 {
                return Err(malformed_at("ControlCommand operands", record.index));
            }
            commit.push(UiOperation::ControlCommand {
                control: resource(0, 4)?,
                command_id: o[4],
                operands: o[5..].to_vec(),
            });
            Ok(())
        }
        OP_SET_DECLARED | OP_SET_OVERRIDE if section == 1 => {
            let node_ref = node(0)?;
            let property = decode_property(o.get(4).copied(), record.index)?;
            let (value, consumed) =
                decode_property_value(property, &o[5..], metadata, record.index)?;
            if 5 + consumed != o.len() {
                return Err(malformed_at("property operand count", record.index));
            }
            let operation = if record.opcode == OP_SET_DECLARED {
                UiOperation::SetDeclared {
                    node: node_ref,
                    property,
                    value,
                }
            } else {
                UiOperation::SetOverride {
                    node: node_ref,
                    property,
                    value,
                }
            };
            commit.push(operation);
            Ok(())
        }
        OP_RESET_DECLARED | OP_CLEAR_OVERRIDE if section == 1 => {
            expect_len(o, 5, record.index)?;
            let operation = if record.opcode == OP_RESET_DECLARED {
                UiOperation::ResetDeclared {
                    node: node(0)?,
                    property: decode_property(Some(o[4]), record.index)?,
                }
            } else {
                UiOperation::ClearOverride {
                    node: node(0)?,
                    property: decode_property(Some(o[4]), record.index)?,
                }
            };
            commit.push(operation);
            Ok(())
        }
        OP_SET_HIDDEN if section == 1 => {
            expect_len(o, 5, record.index)?;
            let hidden = bool_word(o[4], record.index)?;
            commit.push(UiOperation::SetHidden {
                node: node(0)?,
                hidden,
            });
            Ok(())
        }
        OP_SET_STYLE_STATE if section == 1 => {
            expect_len(o, 9, record.index)?;
            let layer = o[4];
            let key = metadata_string(metadata_slice(metadata, o[5], o[6])?, record.index)?;
            let value = metadata_string(metadata_slice(metadata, o[7], o[8])?, record.index)?;
            if layer > 1 {
                return Err(malformed_at("style-state layer", record.index));
            }
            commit.push(UiOperation::SetStyleState {
                node: node(0)?,
                layer,
                key,
                value,
            });
            Ok(())
        }
        OP_CLEAR_STYLE_STATE if section == 1 => {
            expect_len(o, 7, record.index)?;
            let key = metadata_string(metadata_slice(metadata, o[5], o[6])?, record.index)?;
            if o[4] > 1 {
                return Err(malformed_at("style-state layer", record.index));
            }
            commit.push(UiOperation::ClearStyleState {
                node: node(0)?,
                layer: o[4],
                key,
            });
            Ok(())
        }
        OP_SET_SUBSCRIPTIONS if section == 3 => {
            expect_len(o, 6, record.index)?;
            commit.push(UiOperation::SetSubscriptions {
                node: node(0)?,
                mask_low: o[4],
                mask_high: o[5],
            });
            Ok(())
        }
        OP_CREATE_PORT if section == 2 => {
            expect_len(o, 7, record.index)?;
            commit.push(UiOperation::CreatePort {
                local_ordinal: o[0],
                content_family: o[1],
                ownership: OwnershipMode::from_code(o[2])
                    .ok_or_else(|| malformed_at("ownership mode", record.index))?,
                owner: decode_optional_node_ref(o, 3, local_count)?,
            });
            Ok(())
        }
        OP_CREATE_CONNECTOR if section == 2 => {
            expect_len(o, 9, record.index)?;
            let source_index = o[1];
            let (kind, wrap, hyperlinks, smooth) =
                decode_funnel_metadata(metadata_slice(metadata, o[6], o[7])?, record.index)?;
            commit.push(UiOperation::CreateConnector {
                local_ordinal: o[0],
                source_index,
                port: resource(2, 2)?,
                ownership: OwnershipMode::from_code(o[8])
                    .ok_or_else(|| malformed_at("ownership mode", record.index))?,
            });
            commit.set_funnel_for_connector(
                o[0],
                FunnelSpec {
                    kind,
                    wrap,
                    hyperlinks,
                    smooth,
                },
            );
            Ok(())
        }
        OP_SELECT_CONNECTOR if section == 2 => {
            expect_len(o, 8, record.index)?;
            commit.push(UiOperation::SelectConnector {
                port: resource(0, 2)?,
                connector: decode_optional_resource_ref(o, 4, local_count, 3)?,
            });
            Ok(())
        }
        OP_DISPOSE_CONNECTOR if section == 2 => {
            expect_len(o, 4, record.index)?;
            commit.push(UiOperation::DisposeConnector {
                connector: resource(0, 3)?,
            });
            Ok(())
        }
        OP_DISPOSE_PORT if section == 2 => {
            expect_len(o, 4, record.index)?;
            commit.push(UiOperation::DisposePort {
                port: resource(0, 2)?,
            });
            Ok(())
        }
        OP_SET_LITERAL_FUNNEL if section == 2 => {
            expect_len(o, 6, record.index)?;
            let (kind, wrap, hyperlinks, smooth) =
                decode_funnel_metadata(metadata_slice(metadata, o[4], o[5])?, record.index)?;
            commit.push(UiOperation::SetLiteralFunnel {
                port: resource(0, 2)?,
                kind,
                wrap,
                hyperlinks,
                smooth,
            });
            Ok(())
        }
        OP_REPLACE_LITERAL if section == 4 => {
            expect_len(o, 9, record.index)?;
            let content_bytes = owned_content_slice(content, o[5], o[6])?.to_vec();
            let annotations = owned_content_slice(content, o[7], o[8])?.to_vec();
            commit.push(UiOperation::ReplaceLiteral {
                port: resource(0, 2)?,
                content_format: o[4],
                content: content_bytes,
                annotations,
            });
            Ok(())
        }
        OP_REPLACE_EDITOR if section == 4 => {
            expect_len(o, 8, record.index)?;
            let bytes = owned_content_slice(content, o[4], o[5])?.to_vec();
            commit.push(UiOperation::ReplaceEditorContent {
                control: resource(0, 4)?,
                content: bytes,
                expected_edit_revision: u64::from(o[6]) | (u64::from(o[7]) << 32),
            });
            Ok(())
        }
        _ => Err(malformed_at("opcode or section", record.index)),
    }
}

fn expect_len(values: &[u32], expected: usize, index: usize) -> napi::Result<()> {
    if values.len() == expected {
        Ok(())
    } else {
        Err(malformed_at("record operand count", index))
    }
}

fn bool_word(value: u32, index: usize) -> napi::Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(malformed_at("boolean", index)),
    }
}

fn decode_property(value: Option<u32>, index: usize) -> napi::Result<PropertyId> {
    PropertyId::from_raw(value.ok_or_else(|| malformed_at("property id", index))?)
        .ok_or_else(|| malformed_at("property id", index))
}

fn decode_node_ref(values: &[u32], at: usize, local_count: u32) -> napi::Result<NodeRef> {
    decode_ref(values, at, local_count, 1).map(|reference| match reference {
        RefValue::Node(value) => value,
        RefValue::Resource(_) => unreachable!(),
    })
}

fn decode_optional_node_ref(
    values: &[u32],
    at: usize,
    local_count: u32,
) -> napi::Result<Option<NodeRef>> {
    if is_null(values, at)? {
        Ok(None)
    } else {
        decode_node_ref(values, at, local_count).map(Some)
    }
}

fn decode_resource_ref(
    values: &[u32],
    at: usize,
    local_count: u32,
    kind: u32,
) -> napi::Result<ResourceRef> {
    decode_ref(values, at, local_count, kind).map(|reference| match reference {
        RefValue::Resource(value) => value,
        RefValue::Node(_) => unreachable!(),
    })
}

fn decode_optional_resource_ref(
    values: &[u32],
    at: usize,
    local_count: u32,
    kind: u32,
) -> napi::Result<Option<ResourceRef>> {
    if is_null(values, at)? {
        Ok(None)
    } else {
        decode_resource_ref(values, at, local_count, kind).map(Some)
    }
}

enum RefValue {
    Node(NodeRef),
    Resource(ResourceRef),
}

fn decode_ref(
    values: &[u32],
    at: usize,
    local_count: u32,
    expected_kind: u32,
) -> napi::Result<RefValue> {
    let host = *values.get(at).ok_or_else(|| malformed("handle"))?;
    let slot = *values.get(at + 1).ok_or_else(|| malformed("handle"))?;
    let generation = *values.get(at + 2).ok_or_else(|| malformed("handle"))?;
    let kind = *values.get(at + 3).ok_or_else(|| malformed("handle"))?;
    if kind != expected_kind {
        return Err(malformed("handle kind"));
    }
    if host == 0 {
        if generation != 0 || slot == 0 || slot > local_count {
            return Err(malformed("local handle"));
        }
        return Ok(if expected_kind == 1 {
            RefValue::Node(NodeRef::Local(slot))
        } else {
            RefValue::Resource(ResourceRef::Local(slot))
        });
    }
    let namespace = HostNamespace::new(host).ok_or_else(|| malformed("host namespace"))?;
    let handle = UiHandle::new(
        namespace,
        slot,
        generation,
        match expected_kind {
            1 => iyon_tui::binding::HandleKind::Node,
            2 => iyon_tui::binding::HandleKind::Port,
            3 => iyon_tui::binding::HandleKind::Connector,
            4 => iyon_tui::binding::HandleKind::Control,
            _ => return Err(malformed("handle kind")),
        },
    )
    .ok_or_else(|| malformed("handle"))?;
    Ok(if expected_kind == 1 {
        RefValue::Node(NodeRef::Existing(handle))
    } else {
        RefValue::Resource(ResourceRef::Existing(handle))
    })
}

fn is_null(values: &[u32], at: usize) -> napi::Result<bool> {
    Ok(values
        .get(at..at + 4)
        .ok_or_else(|| malformed("nullable handle"))?
        == [0, 0, 0, 0])
}

fn decode_control_config(
    kind: ControlKind,
    bytes: &[u8],
    record: usize,
) -> napi::Result<ControlConfig> {
    match kind {
        ControlKind::Editor => {
            let multiline = match bytes {
                [] => false,
                [0] => false,
                [1] => true,
                _ => return Err(malformed_at("Editor config", record)),
            };
            Ok(ControlConfig {
                multiline,
                animation_interval_ms: None,
            })
        }
        ControlKind::Scroll => {
            if !bytes.is_empty() {
                return Err(malformed_at("Scroll config", record));
            }
            Ok(ControlConfig::default())
        }
        ControlKind::Animation => {
            let interval_ms = match bytes {
                [] => None,
                [low, b1, b2, high] => {
                    let interval = u32::from_le_bytes([*low, *b1, *b2, *high]);
                    if interval == 0 {
                        return Err(malformed_at("Animation interval", record));
                    }
                    Some(interval)
                }
                _ => return Err(malformed_at("Animation config", record)),
            };
            Ok(ControlConfig {
                multiline: false,
                animation_interval_ms: interval_ms,
            })
        }
    }
}

fn decode_root_config(role: RootRole, bytes: &[u8], record: usize) -> napi::Result<RootConfig> {
    if bytes.is_empty() {
        return Ok(RootConfig::default());
    }
    if role != RootRole::LegacyHistoryUnit || bytes.len() != 10 {
        return Err(malformed_at("root config", record));
    }
    let flow_boundary = match bytes[0] {
        0 | 1 => u32::from(bytes[0]),
        _ => return Err(malformed_at("History flow boundary", record)),
    };
    let has_identity = match bytes[1] {
        0 => false,
        1 => true,
        _ => return Err(malformed_at("History unit identity marker", record)),
    };
    let identity = u64::from_le_bytes(
        bytes[2..10]
            .try_into()
            .expect("validated root config identity width"),
    );
    if !has_identity && identity != 0 {
        return Err(malformed_at("History unit identity", record));
    }
    Ok(RootConfig {
        flow_boundary,
        unit_identity: has_identity.then_some(identity),
    })
}

fn metadata_slice<'a>(metadata: &'a [u8], offset: u32, length: u32) -> napi::Result<&'a [u8]> {
    let start = usize::try_from(offset).map_err(|_| malformed("metadata offset"))?;
    let length = usize::try_from(length).map_err(|_| malformed("metadata length"))?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| malformed("metadata range"))?;
    metadata
        .get(start..end)
        .ok_or_else(|| malformed("metadata range"))
}

fn metadata_string(bytes: &[u8], record: usize) -> napi::Result<String> {
    let value = std::str::from_utf8(bytes).map_err(|_| malformed_at("metadata UTF-8", record))?;
    if value.is_empty() || value.contains('\0') {
        return Err(malformed_at("metadata string", record));
    }
    Ok(value.to_owned())
}

fn owned_content_slice<'a>(content: &'a [u8], offset: u32, length: u32) -> napi::Result<&'a [u8]> {
    metadata_slice(content, offset, length)
}

fn decode_funnel_metadata(bytes: &[u8], record: usize) -> napi::Result<(u32, u32, bool, bool)> {
    if bytes.is_empty() {
        return Ok((0, 0, true, false));
    }
    let value =
        std::str::from_utf8(bytes).map_err(|_| malformed_at("Funnel metadata UTF-8", record))?;
    let mut fields = value.split(':');
    let kind = match fields.next().unwrap_or_default() {
        "plain" => 0,
        "markdown" => 1,
        "diff" => 2,
        "ansi" => 3,
        _ => return Err(malformed_at("Funnel kind", record)),
    };
    let wrap = match fields.next().unwrap_or("word") {
        "word" => 0,
        "grapheme" => 1,
        "noWrap" => 2,
        _ => return Err(malformed_at("Funnel wrap", record)),
    };
    let hyperlinks = match fields.next().unwrap_or("1") {
        "0" => false,
        "1" => true,
        _ => return Err(malformed_at("Funnel hyperlinks", record)),
    };
    let smooth = match fields.next().unwrap_or("0") {
        "0" => false,
        "1" => true,
        _ => return Err(malformed_at("Funnel delivery", record)),
    };
    if fields.next().is_some() {
        return Err(malformed_at("Funnel metadata trailing fields", record));
    }
    Ok((kind, wrap, hyperlinks, smooth))
}

fn decode_property_value(
    property: PropertyId,
    words: &[u32],
    metadata: &[u8],
    index: usize,
) -> napi::Result<(LayerValue, usize)> {
    use iyon_tui::binding::ValueKind;
    let kind = iyon_tui::binding::property_descriptor(property).value_kind;
    let encoding = value_encoding(kind);
    if words.len() < encoding.min_words || words.len() > encoding.max_words {
        return Err(malformed_at("property encoding width", index));
    }
    let (value, consumed) = match kind {
        ValueKind::SizeMode => {
            let word = *words
                .first()
                .ok_or_else(|| malformed_at("size mode", index))?;
            let form = value_encoding_form(kind, "mode");
            if !form.values.contains(&word) {
                return Err(malformed_at("size mode", index));
            }
            let mode = match word {
                0 => SizeMode::Fit,
                1 => SizeMode::Fill,
                _ => return Err(malformed_at("size mode", index)),
            };
            (PropertyValue::SizeMode(mode), 1)
        }
        ValueKind::LayoutMode => {
            let word = *words
                .first()
                .ok_or_else(|| malformed_at("layout mode", index))?;
            let form = value_encoding_form(kind, "layout");
            if !form.values.contains(&word) {
                return Err(malformed_at("layout mode", index));
            }
            let mode = match word {
                0 => LayoutMode::Box,
                1 => LayoutMode::Row,
                2 => LayoutMode::Column,
                3 => LayoutMode::Grid,
                _ => return Err(malformed_at("layout mode", index)),
            };
            (PropertyValue::LayoutMode(mode), 1)
        }
        ValueKind::U16 => {
            let word = *words.first().ok_or_else(|| malformed_at("u16", index))?;
            let form = value_encoding_form(kind, "u16");
            if form.max_value.is_some_and(|max| word > max) {
                return Err(malformed_at("u16", index));
            }
            let value = u16::try_from(word).map_err(|_| malformed_at("u16", index))?;
            (PropertyValue::U16(value), 1)
        }
        ValueKind::Insets => {
            if words.len() < 4 {
                return Err(malformed_at("insets", index));
            }
            let values = words[..4]
                .iter()
                .copied()
                .map(|word| u16::try_from(word).map_err(|_| malformed_at("insets", index)))
                .collect::<Result<Vec<_>, _>>()?;
            (
                PropertyValue::Insets(Insets::new(values[0], values[1], values[2], values[3])),
                4,
            )
        }
        ValueKind::Alignment => {
            if words.len() < 2 {
                return Err(malformed_at("alignment", index));
            }
            (
                PropertyValue::Alignment(Alignment::new(
                    Some(axis(words[0], index)?),
                    Some(axis(words[1], index)?),
                )),
                2,
            )
        }
        ValueKind::Edges => {
            if words.len() < 4 {
                return Err(malformed_at("border edges", index));
            }
            let form = value_encoding_form(kind, "bool_x4");
            let values = words[..4]
                .iter()
                .copied()
                .map(|word| {
                    form.values
                        .contains(&word)
                        .then_some(word != 0)
                        .ok_or_else(|| malformed_at("border edges", index))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (
                PropertyValue::Edges(Edges::new(values[0], values[1], values[2], values[3])),
                4,
            )
        }
        ValueKind::Color => {
            let (color, consumed) = decode_color(words, index)?;
            (PropertyValue::Color(color), consumed)
        }
        ValueKind::BorderStyle => {
            let form = value_encoding_form(kind, "border");
            let style = match *words
                .first()
                .ok_or_else(|| malformed_at("border style", index))?
            {
                0 => BorderStyle::Plain,
                1 => BorderStyle::Rounded,
                2 => BorderStyle::Double,
                value if !form.values.contains(&value) => {
                    return Err(malformed_at("border style", index));
                }
                _ => return Err(malformed_at("border style", index)),
            };
            (PropertyValue::BorderStyle(style), 1)
        }
        ValueKind::TextAttributes => {
            let bits = *words
                .first()
                .ok_or_else(|| malformed_at("text attributes", index))?;
            let cleared = words.get(1).copied().unwrap_or(0);
            let form = value_encoding_form(kind, "set_or_clear");
            let mask = form
                .mask
                .ok_or_else(|| malformed_at("text attributes mask", index))?;
            if bits & !mask != 0 || cleared & !mask != 0 || bits & cleared != 0 {
                return Err(malformed_at("text attributes", index));
            }
            let mut style = TextAttributeSpec::new();
            for (bit, attribute) in [
                (1, TextAttribute::Bold),
                (2, TextAttribute::Dim),
                (4, TextAttribute::Italic),
                (8, TextAttribute::Underline),
                (16, TextAttribute::Reversed),
                (32, TextAttribute::Strikethrough),
            ] {
                if bits & bit != 0 {
                    style = style.attribute(attribute, true);
                } else if cleared & bit != 0 {
                    style = style.attribute(attribute, false);
                }
            }
            (PropertyValue::TextAttributes(style), words.len())
        }
        ValueKind::Style => {
            let style = decode_style(words, metadata, index)?;
            (PropertyValue::Style(style), words.len())
        }
        ValueKind::Glyphs => {
            let glyphs = decode_glyphs(words, metadata, index)?;
            (PropertyValue::Glyphs(glyphs), 16)
        }
    };
    Ok((LayerValue::Value(value), consumed))
}

fn decode_color(words: &[u32], index: usize) -> napi::Result<(ColorSpec, usize)> {
    let ansi = value_encoding_form(ValueKind::Color, "ansi");
    let rgb = value_encoding_form(ValueKind::Color, "rgb");
    let first = *words.first().ok_or_else(|| malformed_at("color", index))?;
    if words.len() == ansi.word_count {
        if ansi.max_value.is_some_and(|max| first > max) {
            return Err(malformed_at("color", index));
        }
        return Ok((
            ColorSpec::ansi(u8::try_from(first).map_err(|_| malformed_at("color", index))?),
            ansi.word_count,
        ));
    }
    if words.len() != rgb.word_count || rgb.tags.first().copied() != Some(first) {
        return Err(malformed_at("color encoding", index));
    }
    let red = u8::try_from(words[1]).map_err(|_| malformed_at("color red", index))?;
    let green = u8::try_from(words[2]).map_err(|_| malformed_at("color green", index))?;
    let blue = u8::try_from(words[3]).map_err(|_| malformed_at("color blue", index))?;
    Ok((ColorSpec::rgb(red, green, blue), rgb.word_count))
}

fn decode_style(words: &[u32], metadata: &[u8], index: usize) -> napi::Result<StyleRef> {
    let direct = value_encoding_form(ValueKind::Style, "direct");
    let themed = value_encoding_form(ValueKind::Style, "themed");
    if words.len() != direct.word_count && words.len() != themed.word_count {
        return Err(malformed_at("style", index));
    }
    let mut style = StyleSpec::new();
    if let Some(color) = decode_style_color(&words[..4], index, &direct.tags)? {
        style = style.foreground(color);
    }
    if let Some(color) = decode_style_color(&words[4..8], index, &direct.tags)? {
        style = style.background(color);
    }
    let set_bits = words[8];
    let clear_bits = words.get(9).copied().unwrap_or(0);
    let mask = direct
        .mask
        .ok_or_else(|| malformed_at("style mask", index))?;
    if set_bits & !mask != 0 || clear_bits & !mask != 0 || set_bits & clear_bits != 0 {
        return Err(malformed_at("style attributes", index));
    }
    for (bit, attribute) in [
        (1, TextAttribute::Bold),
        (2, TextAttribute::Dim),
        (4, TextAttribute::Italic),
        (8, TextAttribute::Underline),
        (16, TextAttribute::Reversed),
        (32, TextAttribute::Strikethrough),
    ] {
        if set_bits & bit != 0 {
            style = style.attribute(attribute, true);
        } else if clear_bits & bit != 0 {
            style = style.attribute(attribute, false);
        }
    }
    if words.len() == direct.word_count {
        return Ok(StyleRef::direct(style));
    }
    if words[10] == 0 && words[11] == 0 {
        return Ok(StyleRef::direct(style));
    }
    let theme = metadata_string(metadata_slice(metadata, words[10], words[11])?, index)?;
    Ok(StyleRef::themed(theme, style))
}

fn decode_style_color(
    words: &[u32],
    index: usize,
    tags: &[u32],
) -> napi::Result<Option<ColorSpec>> {
    let kind = words
        .first()
        .copied()
        .ok_or_else(|| malformed_at("style color", index))?;
    match tags.iter().position(|tag| *tag == kind) {
        Some(0) => {
            if words[1..].iter().any(|word| *word != 0) {
                return Err(malformed_at("style color unset payload", index));
            }
            Ok(None)
        }
        Some(1) => {
            if words[2] != 0 || words[3] != 0 {
                return Err(malformed_at("style ANSI color payload", index));
            }
            Ok(Some(ColorSpec::ansi(
                u8::try_from(words[1]).map_err(|_| malformed_at("style ANSI color", index))?,
            )))
        }
        Some(2) => Ok(Some(ColorSpec::rgb(
            u8::try_from(words[1]).map_err(|_| malformed_at("style red", index))?,
            u8::try_from(words[2]).map_err(|_| malformed_at("style green", index))?,
            u8::try_from(words[3]).map_err(|_| malformed_at("style blue", index))?,
        ))),
        _ => Err(malformed_at("style color kind", index)),
    }
}

fn decode_glyphs(
    words: &[u32],
    metadata: &[u8],
    index: usize,
) -> napi::Result<iyon_tui::binding::BorderGlyphs> {
    if words.len() < 16 {
        return Err(malformed_at("border glyphs", index));
    }
    let mut values = Vec::new();
    values
        .try_reserve(8)
        .map_err(|_| malformed_at("border glyph capacity", index))?;
    for pair in words[..16].chunks_exact(2) {
        values.push(metadata_string(
            metadata_slice(metadata, pair[0], pair[1])?,
            index,
        )?);
    }
    iyon_tui::binding::BorderGlyphs::new(
        values.remove(0),
        values.remove(0),
        values.remove(0),
        values.remove(0),
        values.remove(0),
        values.remove(0),
        values.remove(0),
        values.remove(0),
    )
    .map_err(|error| malformed_at(error.to_string(), index))
}

fn axis(value: u32, index: usize) -> napi::Result<AlignmentAxis> {
    match value {
        0 => Ok(AlignmentAxis::Start),
        1 => Ok(AlignmentAxis::Center),
        2 => Ok(AlignmentAxis::End),
        3 => Ok(AlignmentAxis::Top),
        4 => Ok(AlignmentAxis::Bottom),
        _ => Err(malformed_at("alignment axis", index)),
    }
}

fn malformed(message: impl Into<String>) -> Error {
    Error::new(
        Status::InvalidArg,
        format!("MALFORMED_UI: {}", message.into()),
    )
}
fn malformed_at(message: impl Into<String>, record: usize) -> Error {
    Error::new(
        Status::InvalidArg,
        format!("MALFORMED_UI: record {record}: {}", message.into()),
    )
}
fn unsupported(record: usize, message: impl Into<String>) -> Error {
    Error::new(
        Status::GenericFailure,
        format!("UNSUPPORTED_UI: record {record}: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use iyon_tui::binding::{
        HostKind, NodeRef, OwnershipMode, ResourceRef, TextSourceKind, TuiEnvironment,
    };

    #[test]
    fn property_decoder_preserves_finite_style_values_and_rejects_bad_lanes() {
        let (LayerValue::Value(PropertyValue::Insets(insets)), consumed) =
            decode_property_value(PropertyId::Padding, &[1, 2, 3, 4], &[], 0).unwrap()
        else {
            panic!("padding must decode as Insets");
        };
        assert_eq!(consumed, 4);
        assert_eq!(insets, Insets::new(1, 2, 3, 4));

        let (LayerValue::Value(PropertyValue::Edges(edges)), consumed) =
            decode_property_value(PropertyId::BorderEdges, &[1, 0, 1, 0], &[], 0).unwrap()
        else {
            panic!("border edges must decode as boolean lanes");
        };
        assert_eq!(consumed, 4);
        assert_eq!(edges, Edges::new(true, false, true, false));

        let (LayerValue::Value(PropertyValue::Color(color)), consumed) =
            decode_property_value(PropertyId::Foreground, &[0x8000_0001, 12, 34, 56], &[], 0)
                .unwrap()
        else {
            panic!("foreground must decode as Color");
        };
        assert_eq!(consumed, 4);
        assert_eq!(color, ColorSpec::rgb(12, 34, 56));

        let (LayerValue::Value(PropertyValue::TextAttributes(attributes)), consumed) =
            decode_property_value(PropertyId::TextAttributes, &[1 | 4 | 32], &[], 0).unwrap()
        else {
            panic!("text attributes must decode as a finite mask");
        };
        assert_eq!(consumed, 1);
        assert_eq!(
            attributes,
            TextAttributeSpec::new()
                .attribute(TextAttribute::Bold, true)
                .attribute(TextAttribute::Italic, true)
                .attribute(TextAttribute::Strikethrough, true)
        );

        let style_words = [1, 7, 0, 0, 2, 8, 9, 10, 1 | 16];
        let (LayerValue::Value(PropertyValue::Style(style)), consumed) =
            decode_property_value(PropertyId::Style, &style_words, &[], 0).unwrap()
        else {
            panic!("style must decode as Style");
        };
        assert_eq!(consumed, 9);
        assert_eq!(
            style,
            StyleRef::direct(
                StyleSpec::new()
                    .foreground(ColorSpec::ansi(7))
                    .background(ColorSpec::rgb(8, 9, 10))
                    .attribute(TextAttribute::Bold, true)
                    .attribute(TextAttribute::Reversed, true),
            )
        );

        let (LayerValue::Value(PropertyValue::Style(themed)), consumed) = decode_property_value(
            PropertyId::Style,
            &[0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 5],
            b"theme",
            0,
        )
        .unwrap() else {
            panic!("style must preserve named theme and cleared attributes");
        };
        assert_eq!(consumed, 12);
        assert_eq!(
            themed,
            StyleRef::themed(
                "theme",
                StyleSpec::new()
                    .attribute(TextAttribute::Bold, true)
                    .attribute(TextAttribute::Dim, false),
            )
        );

        let (LayerValue::Value(PropertyValue::TextAttributes(attributes)), consumed) =
            decode_property_value(PropertyId::TextAttributes, &[1, 2], &[], 0).unwrap()
        else {
            panic!("text attributes must preserve explicit clears");
        };
        assert_eq!(consumed, 2);
        assert_eq!(
            attributes,
            TextAttributeSpec::new()
                .attribute(TextAttribute::Bold, true)
                .attribute(TextAttribute::Dim, false)
        );

        let glyph_text = "─│─│┌┐└┘";
        let glyph_offsets = glyph_text
            .char_indices()
            .map(|(offset, character)| (offset as u32, character.len_utf8() as u32))
            .collect::<Vec<_>>();
        let glyph_words = glyph_offsets
            .iter()
            .flat_map(|(offset, length)| [*offset, *length])
            .collect::<Vec<_>>();
        let (LayerValue::Value(PropertyValue::Glyphs(glyphs)), consumed) = decode_property_value(
            PropertyId::BorderGlyphs,
            &glyph_words,
            glyph_text.as_bytes(),
            0,
        )
        .unwrap() else {
            panic!("border glyphs must decode from metadata");
        };
        assert_eq!(consumed, 16);
        assert_eq!(
            glyphs,
            iyon_tui::binding::BorderGlyphs::new("─", "│", "─", "│", "┌", "┐", "└", "┘").unwrap()
        );

        assert!(
            decode_property_value(
                PropertyId::Padding,
                &[u32::from(u16::MAX) + 1, 0, 0, 0],
                &[],
                0
            )
            .is_err()
        );
        assert!(decode_property_value(PropertyId::BorderEdges, &[2, 0, 0, 0], &[], 0).is_err());
        assert!(decode_property_value(PropertyId::TextAttributes, &[0x40], &[], 0).is_err());
        assert!(
            decode_property_value(PropertyId::Style, &[3, 0, 0, 0, 0, 0, 0, 0, 0], &[], 0).is_err()
        );
        assert!(
            decode_property_value(PropertyId::BorderGlyphs, &glyph_words, b"invalid", 0).is_err()
        );
    }

    fn mount_literal(
        state: &mut NativeUiState,
        environment: &TuiEnvironment,
    ) -> (
        HostContentSource,
        iyon_tui::binding::UiHandle,
        iyon_tui::binding::UiHandle,
    ) {
        mount_literal_with_replacements(state, environment, &[b"old"])
    }

    fn mount_literal_with_replacements(
        state: &mut NativeUiState,
        environment: &TuiEnvironment,
        replacements: &[&[u8]],
    ) -> (
        HostContentSource,
        iyon_tui::binding::UiHandle,
        iyon_tui::binding::UiHandle,
    ) {
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .expect("test Source");
        let body = state.body_handle().expect("open UI body handle");
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        batch.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        batch.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(ResourceRef::Local(2)),
        });
        let result = state.commit(batch, &[]).expect("literal Port mount");
        let node = result.acknowledgement.created[0];
        let port = result.acknowledgement.created[1];
        let mut literal = UiCommit::new(1);
        for replacement in replacements {
            literal.push(UiOperation::ReplaceLiteral {
                port: ResourceRef::Existing(port),
                content_format: 1,
                content: (*replacement).to_vec(),
                annotations: Vec::new(),
            });
        }
        state.commit(literal, &[]).expect("literal replacement");
        (source, node, port)
    }

    #[test]
    fn native_state_coalesces_initial_literal_replacements_and_releases_once() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(58).expect("namespace"),
            environment.clone(),
        );
        let (_external_source, node, port) = mount_literal_with_replacements(
            &mut state,
            &environment,
            &[b"first", b"second", b"final"],
        );
        let port_key = port.resource_key().expect("Port key");
        let identity = state
            .resources
            .literal_sources
            .get(&port_key)
            .copied()
            .expect("private Source identity");
        let private_source = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source");
        assert_eq!(private_source.snapshot().unwrap().text(), "final");
        assert_eq!(private_source.stats().unwrap().revision, 1);

        let mut funnel_and_replace = UiCommit::new(2);
        funnel_and_replace.push(UiOperation::SetLiteralFunnel {
            port: ResourceRef::Existing(port),
            kind: 2,
            wrap: 1,
            hyperlinks: false,
            smooth: true,
        });
        funnel_and_replace.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Existing(port),
            content_format: 1,
            content: b"funnel-after".to_vec(),
            annotations: Vec::new(),
        });
        state
            .commit(funnel_and_replace, &[])
            .expect("Funnel and replacement");
        assert_eq!(private_source.snapshot().unwrap().text(), "funnel-after");
        assert_eq!(private_source.stats().unwrap().revision, 2);
        assert_eq!(state.resources.connectors.get(&port_key).unwrap().1.kind, 2);
        assert_eq!(state.resources.connectors.get(&port_key).unwrap().1.wrap, 1);
        assert!(
            !state
                .resources
                .connectors
                .get(&port_key)
                .unwrap()
                .1
                .hyperlinks
        );
        assert!(state.resources.connectors.get(&port_key).unwrap().1.smooth);

        let mut retire = UiCommit::new(3);
        retire.push(UiOperation::AttachPort {
            node: NodeRef::Existing(node),
            port: None,
        });
        state.commit(retire, &[]).expect("retire literal Port");
        private_source
            .dispose()
            .expect("the coalesced literal membership is released once");
        assert!(
            environment
                .lookup_content_source(identity.id, identity.generation)
                .is_err()
        );
    }

    #[test]
    fn wire_order_prepares_initial_literal_funnel_before_replacement() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(64).expect("namespace"),
            environment.clone(),
        );
        let namespace = state.namespace().get();
        let structure = vec![
            0x01, 4, 1, 1, 0x03, 14, namespace, 1, 1, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0x06, 10, 0, 1, 0,
            1, 0, 2, 0, 2,
        ];
        let content_control = vec![0x20, 9, 2, 1, 1, 0, 1, 0, 1, 0x25, 8, 0, 2, 0, 2, 0, 17];
        let descriptor = vec![0x40, 11, 0, 2, 0, 2, 1, 0, 6, 6, 0];
        let metadata = b"diff:grapheme:0:1";
        let content = b"custom";
        let words = [
            vec![
                iyon_tui::binding::UI_BATCH_MAGIC,
                iyon_tui::binding::UI_BATCH_VERSION,
                u32::try_from(
                    iyon_tui::binding::UI_BATCH_HEADER_WORDS
                        + structure.len()
                        + content_control.len()
                        + descriptor.len(),
                )
                .unwrap(),
                namespace,
                0,
                0,
                2,
                u32::try_from(structure.len()).unwrap(),
                0,
                u32::try_from(content_control.len()).unwrap(),
                0,
                u32::try_from(descriptor.len()).unwrap(),
                u32::try_from(metadata.len()).unwrap(),
                u32::try_from(content.len()).unwrap(),
                0,
                0,
            ],
            structure,
            content_control,
            descriptor,
        ]
        .concat();
        let words = words.into_iter().collect::<Vec<_>>();
        let header = decode_header(&words, metadata.len(), content.len(), 0, namespace)
            .expect("wire header");
        let commit = decode_commit(&words, &header, metadata, content).expect("wire decode");
        state.commit(commit, &[]).expect("initial literal commit");

        let port = state
            .resources
            .literal_sources
            .keys()
            .next()
            .copied()
            .expect("literal Port");
        let funnel = state
            .resources
            .connectors
            .get(&port)
            .expect("literal Connector")
            .1;
        assert_eq!(funnel.kind, 2);
        assert_eq!(funnel.wrap, 1);
        assert!(!funnel.hyperlinks);
        assert!(funnel.smooth);
        let identity = state.resources.literal_sources[&port];
        let source = environment
            .lookup_content_source(identity.id, identity.generation)
            .expect("private Source");
        assert_eq!(source.snapshot().unwrap().text(), "custom");
    }

    #[test]
    fn literal_binding_cleanup_waits_for_connector_selection() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(67).expect("namespace"),
            environment.clone(),
        );
        let (source, _node, port) = mount_literal(&mut state, &environment);
        let port_key = port.resource_key().expect("Port key");
        let private_identity = state.resources.literal_sources[&port_key];
        let mut candidate = UiCommit::new(2);
        candidate.push(UiOperation::CreateConnector {
            local_ordinal: 1,
            source_index: 0,
            port: ResourceRef::Existing(port),
            ownership: OwnershipMode::Explicit,
        });
        let candidate_result = state
            .commit(candidate, std::slice::from_ref(&source))
            .expect("unselected Connector candidate");
        let connector = candidate_result.acknowledgement.created[0];
        assert!(
            environment
                .lookup_content_source(private_identity.id, private_identity.generation)
                .is_ok()
        );

        let mut select = UiCommit::new(3);
        select.push(UiOperation::SelectConnector {
            port: ResourceRef::Existing(port),
            connector: Some(ResourceRef::Existing(connector)),
        });
        state
            .commit(select, std::slice::from_ref(&source))
            .expect("selected Connector replaces literal binding");
        assert!(
            environment
                .lookup_content_source(private_identity.id, private_identity.generation)
                .is_err()
        );
    }

    #[test]
    fn native_state_close_releases_private_literal_resources() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(61).expect("namespace"),
            environment.clone(),
        );
        let (_external_source, _node, port) = mount_literal(&mut state, &environment);
        let identity = *state
            .resources
            .private_sources
            .get(&port.resource_key().expect("Port key"))
            .expect("private Source identity");
        assert!(
            environment
                .lookup_content_source(identity.id, identity.generation)
                .is_ok()
        );
        state.close().expect("native host close cleanup");
        assert!(
            environment
                .lookup_content_source(identity.id, identity.generation)
                .is_err()
        );
        assert!(state.body_handle().is_err());
        assert!(state.commit(UiCommit::new(1), &[]).is_err());
        state.close().expect("closed UI state is idempotent");
    }

    #[test]
    fn native_state_late_literal_failure_preserves_private_source() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(55).expect("namespace"),
            environment.clone(),
        );
        let (_external_source, _node, port) = mount_literal(&mut state, &environment);
        let port_key = port.resource_key().expect("Port key");
        let identity = state
            .resources
            .literal_sources
            .get(&port_key)
            .copied()
            .expect("private Source identity");
        let before = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source")
            .snapshot()
            .expect("private snapshot");

        let mut invalid = UiCommit::new(2);
        invalid.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Existing(port),
            content_format: 1,
            content: b"new".to_vec(),
            annotations: Vec::new(),
        });
        invalid.push(UiOperation::SetLiteralFunnel {
            port: ResourceRef::Existing(port),
            kind: 99,
            wrap: 0,
            hyperlinks: true,
            smooth: false,
        });
        assert!(state.commit(invalid, &[]).is_err());
        let after = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source")
            .snapshot()
            .expect("private snapshot");
        assert_eq!(before.text(), after.text());
        assert_eq!(before.revision, after.revision);
    }

    #[test]
    fn native_state_invalid_annotation_range_preserves_literal_source() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(57).expect("namespace"),
            environment.clone(),
        );
        let (_source, _node, port) = mount_literal(&mut state, &environment);
        let port_key = port.resource_key().expect("Port key");
        let identity = state
            .resources
            .literal_sources
            .get(&port_key)
            .copied()
            .expect("private Source identity");
        let before = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source")
            .snapshot()
            .expect("private snapshot");

        let mut annotations = Vec::new();
        annotations.extend_from_slice(&1u32.to_le_bytes());
        annotations.extend_from_slice(&1u32.to_le_bytes());
        annotations.extend_from_slice(&99u32.to_le_bytes());
        annotations.extend_from_slice(&100u32.to_le_bytes());
        annotations.extend_from_slice(&[0u8; 20]);
        let mut invalid = UiCommit::new(2);
        invalid.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Existing(port),
            content_format: 1,
            content: b"new".to_vec(),
            annotations,
        });
        assert!(state.commit(invalid, &[]).is_err());
        let after = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source")
            .snapshot()
            .expect("private snapshot");
        assert_eq!(before.text(), after.text());
        assert_eq!(before.revision, after.revision);
    }

    #[test]
    fn invalid_literal_annotation_is_not_hidden_by_a_later_valid_replacement() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(63).expect("namespace"),
            environment.clone(),
        );
        let (_source, _node, port) = mount_literal(&mut state, &environment);
        let port_key = port.resource_key().expect("Port key");
        let identity = state
            .resources
            .literal_sources
            .get(&port_key)
            .copied()
            .expect("private Source identity");
        let before = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source")
            .snapshot()
            .expect("private snapshot");

        let mut annotations = Vec::new();
        annotations.extend_from_slice(&1u32.to_le_bytes());
        annotations.extend_from_slice(&1u32.to_le_bytes());
        annotations.extend_from_slice(&99u32.to_le_bytes());
        annotations.extend_from_slice(&100u32.to_le_bytes());
        annotations.extend_from_slice(&0u32.to_le_bytes());
        annotations.extend_from_slice(&7u32.to_le_bytes());
        annotations.extend_from_slice(&[0u8; 8]);
        annotations.extend_from_slice(b"ns\x00name");

        let mut replacements = UiCommit::new(2);
        replacements.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Existing(port),
            content_format: 1,
            content: b"new".to_vec(),
            annotations,
        });
        replacements.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Existing(port),
            content_format: 1,
            content: b"later".to_vec(),
            annotations: Vec::new(),
        });
        assert!(state.commit(replacements, &[]).is_err());

        let after = environment
            .lookup_content_source(u64::from(identity.id), identity.generation)
            .expect("private Source")
            .snapshot()
            .expect("private snapshot");
        assert_eq!(before.text(), after.text());
        assert_eq!(before.revision, after.revision);
        assert!(after.source_end > after.source_base);
    }

    #[test]
    fn native_state_late_disposal_and_control_failures_preserve_state() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .expect("Source");
        let mut state = NativeUiState::new(
            HostNamespace::new(56).expect("namespace"),
            environment.clone(),
        );
        let mut create = UiCommit::new(0);
        create.push(UiOperation::CreatePort {
            local_ordinal: 1,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 2,
            source_index: 0,
            port: ResourceRef::Local(1),
            ownership: OwnershipMode::Explicit,
        });
        let result = state
            .commit(create, std::slice::from_ref(&source))
            .expect("Source Connector mount");
        let port = result.acknowledgement.created[0];
        let connector = result.acknowledgement.created[1];
        assert!(source.dispose().is_err());

        let mut invalid = UiCommit::new(1);
        invalid.push(UiOperation::DisposeConnector {
            connector: ResourceRef::Existing(connector),
        });
        invalid.push(UiOperation::CreateConnector {
            local_ordinal: 1,
            source_index: 1,
            port: ResourceRef::Existing(port),
            ownership: OwnershipMode::Explicit,
        });
        assert!(state.commit(invalid, &[]).is_err());
        assert!(source.dispose().is_err());

        let mut control_failure = UiCommit::new(1);
        control_failure.push(UiOperation::CreateControl {
            local_ordinal: 1,
            kind: iyon_tui::binding::ControlKind::Editor,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        control_failure.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        control_failure.push(UiOperation::CreateConnector {
            local_ordinal: 3,
            source_index: 1,
            port: ResourceRef::Local(2),
            ownership: OwnershipMode::Explicit,
        });
        assert!(state.commit(control_failure, &[]).is_err());
        assert!(state.resources.controls.is_empty());
        assert!(source.dispose().is_err());
    }

    #[test]
    fn native_state_explicit_connector_and_port_dispose_release_source_once() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Stream)
            .expect("Source");
        let mut state = NativeUiState::new(
            HostNamespace::new(59).expect("namespace"),
            environment.clone(),
        );
        let mut create = UiCommit::new(0);
        create.push(UiOperation::CreatePort {
            local_ordinal: 1,
            content_family: 1,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        create.push(UiOperation::CreateConnector {
            local_ordinal: 2,
            source_index: 0,
            port: ResourceRef::Local(1),
            ownership: OwnershipMode::Explicit,
        });
        let result = state
            .commit(create, std::slice::from_ref(&source))
            .expect("Source Connector mount");
        let port = result.acknowledgement.created[0];
        let connector = result.acknowledgement.created[1];
        assert!(source.dispose().is_err());

        let mut dispose = UiCommit::new(1);
        dispose.push(UiOperation::DisposeConnector {
            connector: ResourceRef::Existing(connector),
        });
        dispose.push(UiOperation::DisposePort {
            port: ResourceRef::Existing(port),
        });
        state.commit(dispose, &[]).expect("explicit disposal");
        source.dispose().expect("Source membership released once");
    }

    #[test]
    fn native_state_prepares_typed_control_transitions_in_one_batch() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(60).expect("namespace"),
            environment.clone(),
        );
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateControl {
            local_ordinal: 1,
            kind: ControlKind::Editor,
            ownership: OwnershipMode::Explicit,
            owner: None,
        });
        batch.push(UiOperation::ControlCommand {
            control: ResourceRef::Local(1),
            command_id: 1,
            operands: vec!['x' as u32],
        });
        batch.push(UiOperation::ReplaceEditorContent {
            control: ResourceRef::Local(1),
            content: b"controlled".to_vec(),
            expected_edit_revision: 1,
        });
        state.commit(batch, &[]).expect("typed control transitions");
        let control = state
            .resources
            .controls
            .values()
            .next()
            .expect("installed control state");
        assert_eq!(control.kind(), ControlKind::Editor);
        assert_eq!(control.editor_text(), Some("controlled"));
        assert_eq!(control.editor_revision(), Some(2));

        let mut invalid = UiCommit::new(1);
        invalid.push(UiOperation::ControlCommand {
            control: ResourceRef::Existing(
                state
                    .resources
                    .controls
                    .keys()
                    .next()
                    .unwrap()
                    .handle(state.namespace()),
            ),
            command_id: 1,
            operands: vec!['y' as u32],
        });
        invalid.push(UiOperation::ReplaceEditorContent {
            control: ResourceRef::Existing(
                state
                    .resources
                    .controls
                    .keys()
                    .next()
                    .unwrap()
                    .handle(state.namespace()),
            ),
            content: b"stale".to_vec(),
            expected_edit_revision: 1,
        });
        assert!(state.commit(invalid, &[]).is_err());
        let control = state.resources.controls.values().next().unwrap();
        assert_eq!(control.editor_text(), Some("controlled"));
        assert_eq!(control.editor_revision(), Some(2));
    }

    #[test]
    fn native_state_allocates_history_identity_during_root_preparation() {
        let environment = TuiEnvironment::new();
        let mut state = NativeUiState::new(
            HostNamespace::new(62).expect("namespace"),
            environment.clone(),
        );
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: RootRole::LegacyHistoryUnit,
            owner: None,
        });
        let result = state.commit(batch, &[]).expect("history root preparation");
        let root = result.acknowledgement.created[0].node_key().unwrap();
        let config = state.resources.root_configs.get(&root).unwrap();
        assert_eq!(config.flow_boundary, 0);
        assert!(config.unit_identity.is_some());
    }
}
