//! PERF-13-E direct content-data ABI.
//!
//! This is the only high-volume Source payload entrypoint. It shares the
//! environment-owned Source registry with the N-API control classes and never
//! performs projection, layout, paint, or callbacks while a payload call is in
//! progress.

use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::slice;

use iyon_tui::{ContentAnnotationRecord, ContentMutationResult, HostContentSource};

use crate::tui::content_environment_for_identity;

pub const CONTENT_ABI_MAGIC: u32 = 0x494f_4e31;
pub const CONTENT_ABI_VERSION: u32 = 1;
pub const CONTENT_ABI_SEMANTIC_VERSION: u32 = 1;
pub const CONTENT_ABI_ANNOTATION_LANES: u32 = 8;
pub const CONTENT_ABI_STATUS_VERSION: u32 = 1;
pub const CONTENT_ABI_REQUIRED_SYMBOLS: u32 = 6;
pub const CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN: u32 = 1;

const MAX_SOURCE_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_ANNOTATION_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

pub const CONTENT_STATUS_OK: u32 = 0;
pub const CONTENT_STATUS_INVALID_ARGUMENT: u32 = 1;
pub const CONTENT_STATUS_ABI_MISMATCH: u32 = 2;
pub const CONTENT_STATUS_WRONG_ENVIRONMENT: u32 = 3;
pub const CONTENT_STATUS_STALE_ENVIRONMENT: u32 = 4;
pub const CONTENT_STATUS_STALE_SOURCE: u32 = 5;
pub const CONTENT_STATUS_SOURCE_DISPOSED: u32 = 6;
pub const CONTENT_STATUS_SOURCE_IN_USE: u32 = 7;
pub const CONTENT_STATUS_INVALID_UTF8: u32 = 8;
pub const CONTENT_STATUS_INVALID_RANGE: u32 = 9;
pub const CONTENT_STATUS_UNKNOWN_ANNOTATION_KIND: u32 = 10;
pub const CONTENT_STATUS_INVALID_ANNOTATION_PAYLOAD: u32 = 11;
pub const CONTENT_STATUS_LIMIT_EXCEEDED: u32 = 12;
pub const CONTENT_STATUS_PAYLOAD_TOO_LARGE: u32 = 13;
pub const CONTENT_STATUS_RUNTIME_POISONED: u32 = 14;
pub const CONTENT_STATUS_INTERNAL_PANIC: u32 = 15;
pub const CONTENT_STATUS_SOURCE_SEALED: u32 = 16;
pub const CONTENT_STATUS_SOURCE_ALREADY_SEALED: u32 = 17;
pub const CONTENT_STATUS_RETENTION_OVERFLOW: u32 = 18;
pub const CONTENT_STATUS_INTERNAL_INVARIANT: u32 = 19;

// These are generated-schema fingerprints for the checked-in ABI v1 layout.
// They are represented as native u32 lanes so the metadata probe needs no
// pointer to process-owned strings.
pub const CONTENT_ABI_BUILD_FINGERPRINT: [u32; 8] = [
    0x494f_4e53,
    0x362d_5445,
    0x5854_2d31,
    0x0000_0000,
    0,
    0,
    0,
    0,
];
pub const CONTENT_ABI_SCHEMA_FINGERPRINT: [u32; 8] = [
    0x5045_5246,
    0x3133_2d47,
    0x434f_4e54,
    0x454e_542d,
    0x4142_4931,
    0,
    0,
    0,
];

/// Fixed metadata record. All fields are u32 lanes so the caller can inspect
/// it with a single `Uint32Array` without a C string or pointer lifetime.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IyonTuiPerf13AbiMetadataV1 {
    pub magic: u32,
    pub abi_version: u32,
    pub semantic_version: u32,
    pub pointer_width: u32,
    pub endian_marker: u32,
    pub metadata_size: u32,
    pub mutation_result_size: u32,
    pub mutation_result_align: u32,
    pub annotation_record_size: u32,
    pub annotation_record_align: u32,
    pub annotation_record_lanes: u32,
    pub status_table_version: u32,
    pub required_symbol_count: u32,
    pub reserved0: u32,
    pub build_fingerprint: [u32; 8],
    pub schema_fingerprint: [u32; 8],
    pub reserved: [u32; 2],
}

/// Fixed result record written by every Source mutation call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IyonTuiSourceMutationResultV1 {
    pub source_revision_lo: u32,
    pub source_revision_hi: u32,
    pub environment_wake_epoch_lo: u32,
    pub environment_wake_epoch_hi: u32,
    pub flags: u32,
    pub reserved0: u32,
}

const _: () = assert!(size_of::<IyonTuiPerf13AbiMetadataV1>() == 128);
const _: () = assert!(size_of::<IyonTuiSourceMutationResultV1>() == 24);
const _: () = assert!(align_of::<IyonTuiPerf13AbiMetadataV1>() == 4);
const _: () = assert!(align_of::<IyonTuiSourceMutationResultV1>() == 4);

fn status_for_diagnostic(diagnostic: &str) -> u32 {
    let code = diagnostic
        .split_once(':')
        .map_or(diagnostic, |(code, _)| code)
        .trim();
    match code {
        "INVALID_ARGUMENT" => CONTENT_STATUS_INVALID_ARGUMENT,
        "ABI_MISMATCH" => CONTENT_STATUS_ABI_MISMATCH,
        "WRONG_ENVIRONMENT" => CONTENT_STATUS_WRONG_ENVIRONMENT,
        "STALE_ENVIRONMENT" => CONTENT_STATUS_STALE_ENVIRONMENT,
        "STALE_SOURCE" | "STALE_HANDLE" => CONTENT_STATUS_STALE_SOURCE,
        "SOURCE_DISPOSED" => CONTENT_STATUS_SOURCE_DISPOSED,
        "SOURCE_IN_USE" => CONTENT_STATUS_SOURCE_IN_USE,
        "INVALID_UTF8" => CONTENT_STATUS_INVALID_UTF8,
        "INVALID_RANGE" => CONTENT_STATUS_INVALID_RANGE,
        "UNKNOWN_ANNOTATION_KIND" => CONTENT_STATUS_UNKNOWN_ANNOTATION_KIND,
        "INVALID_ANNOTATION_PAYLOAD" => CONTENT_STATUS_INVALID_ANNOTATION_PAYLOAD,
        "LIMIT_EXCEEDED" => CONTENT_STATUS_LIMIT_EXCEEDED,
        "PAYLOAD_TOO_LARGE" => CONTENT_STATUS_PAYLOAD_TOO_LARGE,
        "RUNTIME_POISONED" => CONTENT_STATUS_RUNTIME_POISONED,
        "SOURCE_SEALED" => CONTENT_STATUS_SOURCE_SEALED,
        "SOURCE_ALREADY_SEALED" => CONTENT_STATUS_SOURCE_ALREADY_SEALED,
        "SOURCE_RETENTION_OVERFLOW" => CONTENT_STATUS_RETENTION_OVERFLOW,
        "INTERNAL_INVARIANT" => CONTENT_STATUS_INTERNAL_INVARIANT,
        _ => CONTENT_STATUS_INTERNAL_INVARIANT,
    }
}

fn write_mutation_result(out: &mut IyonTuiSourceMutationResultV1, result: ContentMutationResult) {
    out.source_revision_lo = result.revision as u32;
    out.source_revision_hi = (result.revision >> 32) as u32;
    out.environment_wake_epoch_lo = result.environment_wake_epoch as u32;
    out.environment_wake_epoch_hi = (result.environment_wake_epoch >> 32) as u32;
    out.flags = if result.schedule_environment_drain {
        CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN
    } else {
        0
    };
    out.reserved0 = 0;
}

unsafe fn output_result<'a>(
    out: *mut IyonTuiSourceMutationResultV1,
) -> Result<&'a mut IyonTuiSourceMutationResultV1, u32> {
    if out.is_null() || (out as usize) % align_of::<IyonTuiSourceMutationResultV1>() != 0 {
        return Err(CONTENT_STATUS_INVALID_ARGUMENT);
    }
    // SAFETY: the caller owns the fixed output buffer for the duration of the
    // synchronous ABI call and the alignment/null checks above passed.
    let out = unsafe { &mut *out };
    *out = IyonTuiSourceMutationResultV1::default();
    Ok(out)
}

unsafe fn input_bytes<'a>(ptr: *const u8, length: u32, maximum: usize) -> Result<&'a [u8], u32> {
    if usize::try_from(length).map_or(true, |length| length > maximum) {
        return Err(CONTENT_STATUS_PAYLOAD_TOO_LARGE);
    }
    if length == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(CONTENT_STATUS_INVALID_ARGUMENT);
    }
    // SAFETY: direct FFI callers pass a live TypedArray for this synchronous
    // call. The explicit limit is checked before the borrowed slice is formed
    // and no pointer is retained after return.
    Ok(unsafe { slice::from_raw_parts(ptr, length as usize) })
}

unsafe fn input_records<'a>(
    ptr: *const IyonTuiAnnotationRecordV1,
    count: u32,
) -> Result<&'a [IyonTuiAnnotationRecordV1], u32> {
    if count == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() || (ptr as usize) % align_of::<IyonTuiAnnotationRecordV1>() != 0 {
        return Err(CONTENT_STATUS_INVALID_ARGUMENT);
    }
    if count > 16 * 1024 {
        return Err(CONTENT_STATUS_LIMIT_EXCEEDED);
    }
    // SAFETY: the caller passes count fixed-size records in a live TypedArray.
    Ok(unsafe { slice::from_raw_parts(ptr, count as usize) })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IyonTuiAnnotationRecordV1 {
    pub kind: u32,
    pub flags: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub aux0: u32,
    pub aux1: u32,
}

const _: () = assert!(size_of::<IyonTuiAnnotationRecordV1>() == 32);
const _: () = assert!(align_of::<IyonTuiAnnotationRecordV1>() == 4);

fn source_for_identity(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
) -> Result<HostContentSource, u32> {
    let environment = content_environment_for_identity(environment_slot, environment_generation)
        .map_err(|diagnostic| status_for_diagnostic(&diagnostic))?;
    environment
        .lookup_content_source(u64::from(source_slot), source_generation)
        .map_err(|error| status_for_diagnostic(&error.to_string()))
}

fn copy_records(records: &[IyonTuiAnnotationRecordV1]) -> Vec<ContentAnnotationRecord> {
    records
        .iter()
        .map(|record| ContentAnnotationRecord {
            kind: record.kind,
            flags: record.flags,
            start_byte: record.start_byte,
            end_byte: record.end_byte,
            payload_offset: record.payload_offset,
            payload_length: record.payload_length,
            aux0: record.aux0,
            aux1: record.aux1,
        })
        .collect()
}

unsafe fn run_payload_mutation(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    bytes_ptr: *const u8,
    bytes_len: u32,
    annotations_ptr: *const IyonTuiAnnotationRecordV1,
    annotation_count: u32,
    annotation_payload_ptr: *const u8,
    annotation_payload_len: u32,
    out: *mut IyonTuiSourceMutationResultV1,
    operation: impl FnOnce(
        &HostContentSource,
        &[u8],
        &[ContentAnnotationRecord],
        &[u8],
    ) -> Result<ContentMutationResult, String>,
) -> u32 {
    let Ok(out) = (unsafe { output_result(out) }) else {
        return CONTENT_STATUS_INVALID_ARGUMENT;
    };
    let source = match source_for_identity(
        environment_slot,
        environment_generation,
        source_slot,
        source_generation,
    ) {
        Ok(source) => source,
        Err(status) => return status,
    };
    let bytes = match unsafe { input_bytes(bytes_ptr, bytes_len, MAX_SOURCE_PAYLOAD_BYTES) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let records_slice = match unsafe { input_records(annotations_ptr, annotation_count) } {
        Ok(records) => records,
        Err(status) => return status,
    };
    let annotation_payload = match unsafe {
        input_bytes(
            annotation_payload_ptr,
            annotation_payload_len,
            MAX_ANNOTATION_PAYLOAD_BYTES,
        )
    } {
        Ok(payload) => payload,
        Err(status) => return status,
    };
    let records = copy_records(records_slice);
    match operation(&source, bytes, &records, annotation_payload) {
        Ok(result) => {
            write_mutation_result(out, result);
            CONTENT_STATUS_OK
        }
        Err(diagnostic) => status_for_diagnostic(&diagnostic),
    }
}

fn run_no_payload_mutation(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    out: *mut IyonTuiSourceMutationResultV1,
    operation: impl FnOnce(&HostContentSource) -> Result<ContentMutationResult, String>,
) -> u32 {
    let Ok(out) = (unsafe { output_result(out) }) else {
        return CONTENT_STATUS_INVALID_ARGUMENT;
    };
    let source = match source_for_identity(
        environment_slot,
        environment_generation,
        source_slot,
        source_generation,
    ) {
        Ok(source) => source,
        Err(status) => return status,
    };
    match operation(&source) {
        Ok(result) => {
            write_mutation_result(out, result);
            CONTENT_STATUS_OK
        }
        Err(diagnostic) => status_for_diagnostic(&diagnostic),
    }
}

fn guarded(work: impl FnOnce() -> u32) -> u32 {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(status) => status,
        Err(_) => CONTENT_STATUS_INTERNAL_PANIC,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_tui_perf13_abi_metadata_v1(
    out: *mut IyonTuiPerf13AbiMetadataV1,
    out_size: u32,
) -> u32 {
    guarded(|| {
        if out.is_null()
            || (out as usize) % align_of::<IyonTuiPerf13AbiMetadataV1>() != 0
            || out_size < size_of::<IyonTuiPerf13AbiMetadataV1>() as u32
        {
            return CONTENT_STATUS_INVALID_ARGUMENT;
        }
        let metadata = IyonTuiPerf13AbiMetadataV1 {
            magic: CONTENT_ABI_MAGIC,
            abi_version: CONTENT_ABI_VERSION,
            semantic_version: CONTENT_ABI_SEMANTIC_VERSION,
            pointer_width: size_of::<usize>() as u32,
            endian_marker: 0x0102_0304,
            metadata_size: size_of::<IyonTuiPerf13AbiMetadataV1>() as u32,
            mutation_result_size: size_of::<IyonTuiSourceMutationResultV1>() as u32,
            mutation_result_align: align_of::<IyonTuiSourceMutationResultV1>() as u32,
            annotation_record_size: size_of::<IyonTuiAnnotationRecordV1>() as u32,
            annotation_record_align: align_of::<IyonTuiAnnotationRecordV1>() as u32,
            annotation_record_lanes: CONTENT_ABI_ANNOTATION_LANES,
            status_table_version: CONTENT_ABI_STATUS_VERSION,
            required_symbol_count: CONTENT_ABI_REQUIRED_SYMBOLS,
            reserved0: 0,
            build_fingerprint: CONTENT_ABI_BUILD_FINGERPRINT,
            schema_fingerprint: CONTENT_ABI_SCHEMA_FINGERPRINT,
            reserved: [0, 0],
        };
        // SAFETY: null/alignment/size were checked above and the caller owns
        // the output buffer for this synchronous metadata call.
        unsafe { *out = metadata };
        CONTENT_STATUS_OK
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_tui_source_append_utf8_v1(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    bytes_ptr: *const u8,
    bytes_len: u32,
    annotations_ptr: *const IyonTuiAnnotationRecordV1,
    annotation_count: u32,
    annotation_payload_ptr: *const u8,
    annotation_payload_len: u32,
    out: *mut IyonTuiSourceMutationResultV1,
) -> u32 {
    guarded(|| unsafe {
        run_payload_mutation(
            environment_slot,
            environment_generation,
            source_slot,
            source_generation,
            bytes_ptr,
            bytes_len,
            annotations_ptr,
            annotation_count,
            annotation_payload_ptr,
            annotation_payload_len,
            out,
            |source, bytes, records, payload| {
                source
                    .append_utf8(bytes, records, payload)
                    .map_err(|error| error.to_string())
            },
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_tui_source_replace_utf8_v1(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    bytes_ptr: *const u8,
    bytes_len: u32,
    annotations_ptr: *const IyonTuiAnnotationRecordV1,
    annotation_count: u32,
    annotation_payload_ptr: *const u8,
    annotation_payload_len: u32,
    out: *mut IyonTuiSourceMutationResultV1,
) -> u32 {
    guarded(|| unsafe {
        run_payload_mutation(
            environment_slot,
            environment_generation,
            source_slot,
            source_generation,
            bytes_ptr,
            bytes_len,
            annotations_ptr,
            annotation_count,
            annotation_payload_ptr,
            annotation_payload_len,
            out,
            |source, bytes, records, payload| {
                source
                    .replace_utf8(bytes, records, payload)
                    .map_err(|error| error.to_string())
            },
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_tui_source_clear_v1(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    out: *mut IyonTuiSourceMutationResultV1,
) -> u32 {
    guarded(|| {
        run_no_payload_mutation(
            environment_slot,
            environment_generation,
            source_slot,
            source_generation,
            out,
            |source| source.clear().map_err(|error| error.to_string()),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_tui_source_seal_v1(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    out: *mut IyonTuiSourceMutationResultV1,
) -> u32 {
    guarded(|| {
        run_no_payload_mutation(
            environment_slot,
            environment_generation,
            source_slot,
            source_generation,
            out,
            |source| source.seal().map_err(|error| error.to_string()),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_tui_source_head_truncate_v1(
    environment_slot: u32,
    environment_generation: u32,
    source_slot: u32,
    source_generation: u32,
    offset_lo: u32,
    offset_hi: u32,
    out: *mut IyonTuiSourceMutationResultV1,
) -> u32 {
    guarded(|| {
        let offset = u64::from(offset_lo) | (u64::from(offset_hi) << 32);
        run_no_payload_mutation(
            environment_slot,
            environment_generation,
            source_slot,
            source_generation,
            out,
            |source| {
                source
                    .truncate_head(offset)
                    .map_err(|error| error.to_string())
            },
        )
    })
}
