// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = ec82466e117642ffc4009bd11199b7a24aa37f3476065fa34e8732d070dda2d4
// generator_blake3 = e6237f38757724691b7b739064c158573fc0f1dcd63ab16d537a85039e8d155a
// Generated C ABI wrappers. Semantic implementations are handwritten and linked below.
use super::{NativeViewRuntime, NativeHost, AxisChildInputV1};
pub mod generated_impls {
    use super::{AxisChildInputV1, NativeHost, NativeViewRuntime};
    unsafe extern "Rust" {
        pub fn runtime_noop_impl(runtime: *mut NativeViewRuntime) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_render_ref_impl(runtime: *mut NativeViewRuntime, base: u32) -> u32;
    }
    unsafe extern "Rust" {
        pub fn host_render_ref_impl(
            runtime: *mut NativeViewRuntime,
            host: *mut NativeHost,
            base: u32,
        ) -> i32;
    }
    unsafe extern "Rust" {
        pub fn view_spacer_create_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            rows: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_layout_patch_root_impl(
            runtime: *mut NativeViewRuntime,
            base: u32,
            node_id_low: u32,
            node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_common_patch_root_impl(
            runtime: *mut NativeViewRuntime,
            base: u32,
            node_id_low: u32,
            node_id_high: u32,
            mask: u32,
            padding_tr: u32,
            padding_bl: u32,
            width_rule: u32,
            height_rule: u32,
            min_width: u32,
            max_width: u32,
            min_height: u32,
            max_height: u32,
            decoration_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_axis_create_buffer_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            axis_kind: u32,
            gap: u32,
            children: *const AxisChildInputV1,
            children_capacity_bytes: usize,
            used_child_count: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_row_create_0_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_row_create_1_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_row_create_2_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
            track1: u32,
            child1: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_row_create_3_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
            track1: u32,
            child1: u32,
            track2: u32,
            child2: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_row_create_4_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
            track1: u32,
            child1: u32,
            track2: u32,
            child2: u32,
            track3: u32,
            child3: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_column_create_0_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_column_create_1_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_column_create_2_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
            track1: u32,
            child1: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_column_create_3_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
            track1: u32,
            child1: u32,
            track2: u32,
            child2: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_column_create_4_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
            track0: u32,
            child0: u32,
            track1: u32,
            child1: u32,
            track2: u32,
            child2: u32,
            track3: u32,
            child3: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn axis_builder_begin_impl(
            runtime: *mut NativeViewRuntime,
            axis_kind: u32,
            expected_children: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn axis_builder_push_impl(
            runtime: *mut NativeViewRuntime,
            builder_ref: u32,
            track_word: u32,
            child_ref: u32,
        ) -> i32;
    }
    unsafe extern "Rust" {
        pub fn axis_builder_finish_impl(
            runtime: *mut NativeViewRuntime,
            builder_ref: u32,
            node_id_low: u32,
            node_id_high: u32,
            gap: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn axis_builder_abort_impl(runtime: *mut NativeViewRuntime, builder_ref: u32) -> i32;
    }
    unsafe extern "Rust" {
        pub fn view_axis_set_child_impl(
            runtime: *mut NativeViewRuntime,
            base_axis_ref: u32,
            node_id_low: u32,
            node_id_high: u32,
            child_index: u32,
            track_word: u32,
            child_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_axis_splice_buffer_impl(
            runtime: *mut NativeViewRuntime,
            base_axis_ref: u32,
            node_id_low: u32,
            node_id_high: u32,
            index: u32,
            remove_count: u32,
            children: *const AxisChildInputV1,
            children_capacity_bytes: usize,
            used_child_count: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_grid_set_cell_impl(
            runtime: *mut NativeViewRuntime,
            base_grid_ref: u32,
            node_id_low: u32,
            node_id_high: u32,
            row: u32,
            column: u32,
            child_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_axis_set_child_path_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            path_depth: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            ancestor2_node_id_low: u32,
            ancestor2_node_id_high: u32,
            ancestor3_node_id_low: u32,
            ancestor3_node_id_high: u32,
            axis_index: u32,
            track_word: u32,
            child_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_grid_set_cell_path_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            path_depth: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            ancestor2_node_id_low: u32,
            ancestor2_node_id_high: u32,
            ancestor3_node_id_low: u32,
            ancestor3_node_id_high: u32,
            grid_row: u32,
            grid_column: u32,
            child_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_release_many_impl(
            runtime: *mut NativeViewRuntime,
            refs: *const u32,
            refs_capacity_bytes: usize,
            used_ref_count: u32,
        ) -> i32;
    }
    unsafe extern "Rust" {
        pub fn view_ref_for_node_id_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn path_root_impl(runtime: *mut NativeViewRuntime) -> u32;
    }
    unsafe extern "Rust" {
        pub fn path_child_impl(
            runtime: *mut NativeViewRuntime,
            parent_path_ref: u32,
            step_kind: u32,
            expected_view_kind: u32,
            selector: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_layout_patch_path_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            path_depth: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            ancestor2_node_id_low: u32,
            ancestor2_node_id_high: u32,
            ancestor3_node_id_low: u32,
            ancestor3_node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_layout_patch_path_d1_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_layout_patch_path_d2_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_layout_patch_path_d3_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            ancestor2_node_id_low: u32,
            ancestor2_node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_layout_patch_path_d4_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            path_ref: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            ancestor2_node_id_low: u32,
            ancestor2_node_id_high: u32,
            ancestor3_node_id_low: u32,
            ancestor3_node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn edit_txn_begin_impl(
            runtime: *mut NativeViewRuntime,
            base_root_ref: u32,
            expected_edit_count: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn edit_txn_add_text_layout_impl(
            runtime: *mut NativeViewRuntime,
            txn_ref: u32,
            path_ref: u32,
            path_depth: u32,
            target_node_id_low: u32,
            target_node_id_high: u32,
            ancestor0_node_id_low: u32,
            ancestor0_node_id_high: u32,
            ancestor1_node_id_low: u32,
            ancestor1_node_id_high: u32,
            ancestor2_node_id_low: u32,
            ancestor2_node_id_high: u32,
            ancestor3_node_id_low: u32,
            ancestor3_node_id_high: u32,
            wrap: u32,
            align: u32,
        ) -> i32;
    }
    unsafe extern "Rust" {
        pub fn edit_txn_commit_render_impl(
            runtime: *mut NativeViewRuntime,
            host: *mut NativeHost,
            txn_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn edit_txn_abort_impl(runtime: *mut NativeViewRuntime, txn_ref: u32) -> i32;
    }
    unsafe extern "Rust" {
        pub fn style_atom_create_cstring_impl(
            runtime: *mut NativeViewRuntime,
            value: *const ::core::ffi::c_char,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn style_create_bits_impl(
            runtime: *mut NativeViewRuntime,
            flags: u32,
            attribute_present: u32,
            attribute_true: u32,
            foreground_ref: u32,
            background_ref: u32,
            theme_atom_ref: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_cstring_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            text: *const ::core::ffi::c_char,
            style_ref: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_utf8_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            bytes: *const u8,
            bytes_capacity: usize,
            used_bytes: u32,
            style_ref: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_utf8_2_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            bytes: *const u8,
            bytes_capacity: usize,
            used_bytes: u32,
            span0_bytes: u32,
            style0: u32,
            span1_bytes: u32,
            style1: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_utf8_3_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            bytes: *const u8,
            bytes_capacity: usize,
            used_bytes: u32,
            span0_bytes: u32,
            style0: u32,
            span1_bytes: u32,
            style1: u32,
            span2_bytes: u32,
            style2: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_utf8_4_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            bytes: *const u8,
            bytes_capacity: usize,
            used_bytes: u32,
            span0_bytes: u32,
            style0: u32,
            span1_bytes: u32,
            style1: u32,
            span2_bytes: u32,
            style2: u32,
            span3_bytes: u32,
            style3: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_cstring_2_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            text0: *const ::core::ffi::c_char,
            style0: u32,
            text1: *const ::core::ffi::c_char,
            style1: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_cstring_3_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            text0: *const ::core::ffi::c_char,
            style0: u32,
            text1: *const ::core::ffi::c_char,
            style1: u32,
            text2: *const ::core::ffi::c_char,
            style2: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
    unsafe extern "Rust" {
        pub fn view_text_create_cstring_4_impl(
            runtime: *mut NativeViewRuntime,
            node_id_low: u32,
            node_id_high: u32,
            text0: *const ::core::ffi::c_char,
            style0: u32,
            text1: *const ::core::ffi::c_char,
            style1: u32,
            text2: *const ::core::ffi::c_char,
            style2: u32,
            text3: *const ::core::ffi::c_char,
            style3: u32,
            wrap: u32,
            align: u32,
        ) -> u32;
    }
}

#[cfg(feature = "fast-view-abi")]
#[allow(dead_code)]
fn generated_catch_unwind<T: Copy>(work: impl FnOnce() -> Result<T, T>, _panic_value: T) -> T {
    work().unwrap_or_else(|error| error)
}

#[cfg(not(feature = "fast-view-abi"))]
#[allow(dead_code)]
fn generated_catch_unwind<T: Copy>(work: impl FnOnce() -> Result<T, T>, panic_value: T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(result) => result.unwrap_or_else(|error| error),
        Err(_) => panic_value,
    }
}

#[allow(dead_code)]
fn generated_nonnull<T: Copy, P>(value: *mut P, error: T) -> Result<*mut P, T> {
    if value.is_null() {
        Err(error)
    } else {
        Ok(value)
    }
}

#[allow(dead_code)]
fn generated_nonnull_const<T: Copy, P>(value: *const P, error: T) -> Result<*const P, T> {
    if value.is_null() {
        Err(error)
    } else {
        Ok(value)
    }
}

#[allow(dead_code)]
fn generated_buffer<T: Copy, P>(
    value: *const P,
    capacity_bytes: usize,
    element_size: usize,
    maximum_bytes: u64,
    error: T,
) -> Result<*const P, T> {
    if capacity_bytes as u64 > maximum_bytes
        || capacity_bytes % element_size != 0
        || (capacity_bytes != 0
            && (value.is_null() || (value as usize) % ::core::mem::align_of::<P>() != 0))
    {
        Err(error)
    } else {
        Ok(value)
    }
}

#[allow(dead_code)]
fn generated_buffer_used<T: Copy>(
    used_count: u32,
    capacity_bytes: usize,
    element_size: usize,
    maximum_count: u32,
    error: T,
) -> Result<u32, T> {
    if used_count > maximum_count
        || (used_count as usize).saturating_mul(element_size) > capacity_bytes
    {
        Err(error)
    } else {
        Ok(used_count)
    }
}

#[allow(dead_code)]
fn generated_native_ref<T: Copy>(value: u32, error: T) -> Result<u32, T> {
    if value == 0 || value >= 0x8000_0000 {
        Err(error)
    } else {
        Ok(value)
    }
}

#[allow(dead_code)]
fn generated_node_id<T: Copy>(low: u32, high: u32, error: T) -> Result<(u32, u32), T> {
    if high > 0x001f_ffff || (high == 0 && low == 0) {
        Err(error)
    } else {
        Ok((low, high))
    }
}

#[allow(dead_code)]
fn generated_enum<T: Copy>(value: u32, allowed: &[u32], error: T) -> Result<u32, T> {
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(error)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_runtime_noop_v1(runtime: *mut NativeViewRuntime) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                Ok(unsafe { generated_impls::runtime_noop_impl(runtime) })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_render_ref_v1(
    runtime: *mut NativeViewRuntime,
    base: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base = generated_native_ref(base, 0x8000_0001u32)?;
                Ok(unsafe { generated_impls::view_render_ref_impl(runtime, base) })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_host_render_ref_v1(
    runtime: *mut NativeViewRuntime,
    host: *mut NativeHost,
    base: u32,
) -> i32 {
    generated_catch_unwind(
        || {
            (|| -> Result<i32, i32> {
                let runtime = generated_nonnull(runtime, -1i32)?;
                let host = generated_nonnull(host, -1i32)?;
                let base = generated_native_ref(base, -1i32)?;
                Ok(unsafe { generated_impls::host_render_ref_impl(runtime, host, base) })
            })()
        },
        -127i32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_spacer_create_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    rows: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_spacer_create_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        rows,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_layout_patch_root_v1(
    runtime: *mut NativeViewRuntime,
    base: u32,
    node_id_low: u32,
    node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base = generated_native_ref(base, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_layout_patch_root_impl(
                        runtime,
                        base,
                        node_id_low,
                        node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_common_patch_root_v1(
    runtime: *mut NativeViewRuntime,
    base: u32,
    node_id_low: u32,
    node_id_high: u32,
    mask: u32,
    padding_tr: u32,
    padding_bl: u32,
    width_rule: u32,
    height_rule: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
    decoration_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base = generated_native_ref(base, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_common_patch_root_impl(
                        runtime,
                        base,
                        node_id_low,
                        node_id_high,
                        mask,
                        padding_tr,
                        padding_bl,
                        width_rule,
                        height_rule,
                        min_width,
                        max_width,
                        min_height,
                        max_height,
                        decoration_ref,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_axis_create_buffer_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    axis_kind: u32,
    gap: u32,
    children: *const AxisChildInputV1,
    children_capacity_bytes: usize,
    used_child_count: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let children = generated_buffer(
                    children,
                    children_capacity_bytes,
                    8,
                    4194304,
                    0x8000_0002u32,
                )?;
                let used_child_count = generated_buffer_used(
                    used_child_count,
                    children_capacity_bytes,
                    8,
                    524288,
                    0x8000_0003u32,
                )?;
                Ok(unsafe {
                    generated_impls::view_axis_create_buffer_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        axis_kind,
                        gap,
                        children,
                        children_capacity_bytes,
                        used_child_count,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_row_create_0_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_row_create_0_impl(runtime, node_id_low, node_id_high, gap)
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_row_create_1_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_row_create_1_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_row_create_2_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                let child1 = generated_native_ref(child1, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_row_create_2_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                        track1,
                        child1,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_row_create_3_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
    track2: u32,
    child2: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                let child1 = generated_native_ref(child1, 0x8000_0001u32)?;
                let child2 = generated_native_ref(child2, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_row_create_3_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                        track1,
                        child1,
                        track2,
                        child2,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_row_create_4_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
    track2: u32,
    child2: u32,
    track3: u32,
    child3: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                let child1 = generated_native_ref(child1, 0x8000_0001u32)?;
                let child2 = generated_native_ref(child2, 0x8000_0001u32)?;
                let child3 = generated_native_ref(child3, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_row_create_4_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                        track1,
                        child1,
                        track2,
                        child2,
                        track3,
                        child3,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_column_create_0_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_column_create_0_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_column_create_1_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_column_create_1_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_column_create_2_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                let child1 = generated_native_ref(child1, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_column_create_2_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                        track1,
                        child1,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_column_create_3_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
    track2: u32,
    child2: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                let child1 = generated_native_ref(child1, 0x8000_0001u32)?;
                let child2 = generated_native_ref(child2, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_column_create_3_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                        track1,
                        child1,
                        track2,
                        child2,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_column_create_4_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
    track2: u32,
    child2: u32,
    track3: u32,
    child3: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child0 = generated_native_ref(child0, 0x8000_0001u32)?;
                let child1 = generated_native_ref(child1, 0x8000_0001u32)?;
                let child2 = generated_native_ref(child2, 0x8000_0001u32)?;
                let child3 = generated_native_ref(child3, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_column_create_4_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        gap,
                        track0,
                        child0,
                        track1,
                        child1,
                        track2,
                        child2,
                        track3,
                        child3,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_axis_builder_begin_v1(
    runtime: *mut NativeViewRuntime,
    axis_kind: u32,
    expected_children: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::axis_builder_begin_impl(runtime, axis_kind, expected_children)
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_axis_builder_push_v1(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
    track_word: u32,
    child_ref: u32,
) -> i32 {
    generated_catch_unwind(
        || {
            (|| -> Result<i32, i32> {
                let runtime = generated_nonnull(runtime, -1i32)?;
                let builder_ref = generated_native_ref(builder_ref, -1i32)?;
                let child_ref = generated_native_ref(child_ref, -1i32)?;
                Ok(unsafe {
                    generated_impls::axis_builder_push_impl(
                        runtime,
                        builder_ref,
                        track_word,
                        child_ref,
                    )
                })
            })()
        },
        -127i32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_axis_builder_finish_v1(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let builder_ref = generated_native_ref(builder_ref, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::axis_builder_finish_impl(
                        runtime,
                        builder_ref,
                        node_id_low,
                        node_id_high,
                        gap,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_axis_builder_abort_v1(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
) -> i32 {
    generated_catch_unwind(
        || {
            (|| -> Result<i32, i32> {
                let runtime = generated_nonnull(runtime, -1i32)?;
                let builder_ref = generated_native_ref(builder_ref, -1i32)?;
                Ok(unsafe { generated_impls::axis_builder_abort_impl(runtime, builder_ref) })
            })()
        },
        -127i32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_axis_set_child_v1(
    runtime: *mut NativeViewRuntime,
    base_axis_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    child_index: u32,
    track_word: u32,
    child_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_axis_ref = generated_native_ref(base_axis_ref, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child_ref = generated_native_ref(child_ref, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_axis_set_child_impl(
                        runtime,
                        base_axis_ref,
                        node_id_low,
                        node_id_high,
                        child_index,
                        track_word,
                        child_ref,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_axis_splice_buffer_v1(
    runtime: *mut NativeViewRuntime,
    base_axis_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    index: u32,
    remove_count: u32,
    children: *const AxisChildInputV1,
    children_capacity_bytes: usize,
    used_child_count: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_axis_ref = generated_native_ref(base_axis_ref, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let children = generated_buffer(
                    children,
                    children_capacity_bytes,
                    8,
                    4194304,
                    0x8000_0002u32,
                )?;
                let used_child_count = generated_buffer_used(
                    used_child_count,
                    children_capacity_bytes,
                    8,
                    524288,
                    0x8000_0003u32,
                )?;
                Ok(unsafe {
                    generated_impls::view_axis_splice_buffer_impl(
                        runtime,
                        base_axis_ref,
                        node_id_low,
                        node_id_high,
                        index,
                        remove_count,
                        children,
                        children_capacity_bytes,
                        used_child_count,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_grid_set_cell_v1(
    runtime: *mut NativeViewRuntime,
    base_grid_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    row: u32,
    column: u32,
    child_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_grid_ref = generated_native_ref(base_grid_ref, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let child_ref = generated_native_ref(child_ref, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_grid_set_cell_impl(
                        runtime,
                        base_grid_ref,
                        node_id_low,
                        node_id_high,
                        row,
                        column,
                        child_ref,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_axis_set_child_path_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    axis_index: u32,
    track_word: u32,
    child_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) = generated_node_id(
                    ancestor1_node_id_low,
                    ancestor1_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor2_node_id_low, ancestor2_node_id_high) = generated_node_id(
                    ancestor2_node_id_low,
                    ancestor2_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor3_node_id_low, ancestor3_node_id_high) = generated_node_id(
                    ancestor3_node_id_low,
                    ancestor3_node_id_high,
                    0x8000_0001u32,
                )?;
                let child_ref = generated_native_ref(child_ref, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_axis_set_child_path_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        path_depth,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        ancestor2_node_id_low,
                        ancestor2_node_id_high,
                        ancestor3_node_id_low,
                        ancestor3_node_id_high,
                        axis_index,
                        track_word,
                        child_ref,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_grid_set_cell_path_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    grid_row: u32,
    grid_column: u32,
    child_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) = generated_node_id(
                    ancestor1_node_id_low,
                    ancestor1_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor2_node_id_low, ancestor2_node_id_high) = generated_node_id(
                    ancestor2_node_id_low,
                    ancestor2_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor3_node_id_low, ancestor3_node_id_high) = generated_node_id(
                    ancestor3_node_id_low,
                    ancestor3_node_id_high,
                    0x8000_0001u32,
                )?;
                let child_ref = generated_native_ref(child_ref, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_grid_set_cell_path_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        path_depth,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        ancestor2_node_id_low,
                        ancestor2_node_id_high,
                        ancestor3_node_id_low,
                        ancestor3_node_id_high,
                        grid_row,
                        grid_column,
                        child_ref,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_release_many_v1(
    runtime: *mut NativeViewRuntime,
    refs: *const u32,
    refs_capacity_bytes: usize,
    used_ref_count: u32,
) -> i32 {
    generated_catch_unwind(
        || {
            (|| -> Result<i32, i32> {
                let runtime = generated_nonnull(runtime, -1i32)?;
                let refs = generated_buffer(refs, refs_capacity_bytes, 4, 524288, -2i32)?;
                let used_ref_count =
                    generated_buffer_used(used_ref_count, refs_capacity_bytes, 4, 131072, -3i32)?;
                Ok(unsafe {
                    generated_impls::view_release_many_impl(
                        runtime,
                        refs,
                        refs_capacity_bytes,
                        used_ref_count,
                    )
                })
            })()
        },
        -127i32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_ref_for_node_id_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_ref_for_node_id_impl(runtime, node_id_low, node_id_high)
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_path_root_v1(runtime: *mut NativeViewRuntime) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                Ok(unsafe { generated_impls::path_root_impl(runtime) })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_path_child_v1(
    runtime: *mut NativeViewRuntime,
    parent_path_ref: u32,
    step_kind: u32,
    expected_view_kind: u32,
    selector: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let parent_path_ref = generated_native_ref(parent_path_ref, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::path_child_impl(
                        runtime,
                        parent_path_ref,
                        step_kind,
                        expected_view_kind,
                        selector,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_layout_patch_path_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) = generated_node_id(
                    ancestor1_node_id_low,
                    ancestor1_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor2_node_id_low, ancestor2_node_id_high) = generated_node_id(
                    ancestor2_node_id_low,
                    ancestor2_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor3_node_id_low, ancestor3_node_id_high) = generated_node_id(
                    ancestor3_node_id_low,
                    ancestor3_node_id_high,
                    0x8000_0001u32,
                )?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_layout_patch_path_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        path_depth,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        ancestor2_node_id_low,
                        ancestor2_node_id_high,
                        ancestor3_node_id_low,
                        ancestor3_node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_layout_patch_path_d1_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_layout_patch_path_d1_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_layout_patch_path_d2_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) = generated_node_id(
                    ancestor1_node_id_low,
                    ancestor1_node_id_high,
                    0x8000_0001u32,
                )?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_layout_patch_path_d2_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_layout_patch_path_d3_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) = generated_node_id(
                    ancestor1_node_id_low,
                    ancestor1_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor2_node_id_low, ancestor2_node_id_high) = generated_node_id(
                    ancestor2_node_id_low,
                    ancestor2_node_id_high,
                    0x8000_0001u32,
                )?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_layout_patch_path_d3_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        ancestor2_node_id_low,
                        ancestor2_node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_layout_patch_path_d4_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    path_ref: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                let path_ref = generated_native_ref(path_ref, 0x8000_0001u32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, 0x8000_0001u32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) = generated_node_id(
                    ancestor0_node_id_low,
                    ancestor0_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) = generated_node_id(
                    ancestor1_node_id_low,
                    ancestor1_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor2_node_id_low, ancestor2_node_id_high) = generated_node_id(
                    ancestor2_node_id_low,
                    ancestor2_node_id_high,
                    0x8000_0001u32,
                )?;
                let (ancestor3_node_id_low, ancestor3_node_id_high) = generated_node_id(
                    ancestor3_node_id_low,
                    ancestor3_node_id_high,
                    0x8000_0001u32,
                )?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_layout_patch_path_d4_impl(
                        runtime,
                        base_root_ref,
                        path_ref,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        ancestor2_node_id_low,
                        ancestor2_node_id_high,
                        ancestor3_node_id_low,
                        ancestor3_node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_edit_txn_begin_v1(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    expected_edit_count: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let base_root_ref = generated_native_ref(base_root_ref, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::edit_txn_begin_impl(
                        runtime,
                        base_root_ref,
                        expected_edit_count,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_edit_txn_add_text_layout_v1(
    runtime: *mut NativeViewRuntime,
    txn_ref: u32,
    path_ref: u32,
    path_depth: u32,
    target_node_id_low: u32,
    target_node_id_high: u32,
    ancestor0_node_id_low: u32,
    ancestor0_node_id_high: u32,
    ancestor1_node_id_low: u32,
    ancestor1_node_id_high: u32,
    ancestor2_node_id_low: u32,
    ancestor2_node_id_high: u32,
    ancestor3_node_id_low: u32,
    ancestor3_node_id_high: u32,
    wrap: u32,
    align: u32,
) -> i32 {
    generated_catch_unwind(
        || {
            (|| -> Result<i32, i32> {
                let runtime = generated_nonnull(runtime, -1i32)?;
                let txn_ref = generated_native_ref(txn_ref, -1i32)?;
                let path_ref = generated_native_ref(path_ref, -1i32)?;
                let (target_node_id_low, target_node_id_high) =
                    generated_node_id(target_node_id_low, target_node_id_high, -1i32)?;
                let (ancestor0_node_id_low, ancestor0_node_id_high) =
                    generated_node_id(ancestor0_node_id_low, ancestor0_node_id_high, -1i32)?;
                let (ancestor1_node_id_low, ancestor1_node_id_high) =
                    generated_node_id(ancestor1_node_id_low, ancestor1_node_id_high, -1i32)?;
                let (ancestor2_node_id_low, ancestor2_node_id_high) =
                    generated_node_id(ancestor2_node_id_low, ancestor2_node_id_high, -1i32)?;
                let (ancestor3_node_id_low, ancestor3_node_id_high) =
                    generated_node_id(ancestor3_node_id_low, ancestor3_node_id_high, -1i32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], -1i32)?;
                let align = generated_enum(align, &[1, 2, 3], -1i32)?;
                Ok(unsafe {
                    generated_impls::edit_txn_add_text_layout_impl(
                        runtime,
                        txn_ref,
                        path_ref,
                        path_depth,
                        target_node_id_low,
                        target_node_id_high,
                        ancestor0_node_id_low,
                        ancestor0_node_id_high,
                        ancestor1_node_id_low,
                        ancestor1_node_id_high,
                        ancestor2_node_id_low,
                        ancestor2_node_id_high,
                        ancestor3_node_id_low,
                        ancestor3_node_id_high,
                        wrap,
                        align,
                    )
                })
            })()
        },
        -127i32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_edit_txn_commit_render_v1(
    runtime: *mut NativeViewRuntime,
    host: *mut NativeHost,
    txn_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let host = generated_nonnull(host, 0x8000_0001u32)?;
                let txn_ref = generated_native_ref(txn_ref, 0x8000_0001u32)?;
                Ok(unsafe { generated_impls::edit_txn_commit_render_impl(runtime, host, txn_ref) })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_edit_txn_abort_v1(
    runtime: *mut NativeViewRuntime,
    txn_ref: u32,
) -> i32 {
    generated_catch_unwind(
        || {
            (|| -> Result<i32, i32> {
                let runtime = generated_nonnull(runtime, -1i32)?;
                let txn_ref = generated_native_ref(txn_ref, -1i32)?;
                Ok(unsafe { generated_impls::edit_txn_abort_impl(runtime, txn_ref) })
            })()
        },
        -127i32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_style_atom_create_cstring_v1(
    runtime: *mut NativeViewRuntime,
    value: *const ::core::ffi::c_char,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let value = generated_nonnull_const(value, 0x8000_0001u32)?;
                Ok(unsafe { generated_impls::style_atom_create_cstring_impl(runtime, value) })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_style_create_bits_v1(
    runtime: *mut NativeViewRuntime,
    flags: u32,
    attribute_present: u32,
    attribute_true: u32,
    foreground_ref: u32,
    background_ref: u32,
    theme_atom_ref: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::style_create_bits_impl(
                        runtime,
                        flags,
                        attribute_present,
                        attribute_true,
                        foreground_ref,
                        background_ref,
                        theme_atom_ref,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_cstring_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text: *const ::core::ffi::c_char,
    style_ref: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let text = generated_nonnull_const(text, 0x8000_0001u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_cstring_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        text,
                        style_ref,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_utf8_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    bytes_capacity: usize,
    used_bytes: u32,
    style_ref: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let bytes = generated_buffer(bytes, bytes_capacity, 1, 16777216, 0x8000_0002u32)?;
                let used_bytes =
                    generated_buffer_used(used_bytes, bytes_capacity, 1, 16777216, 0x8000_0003u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_utf8_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        bytes,
                        bytes_capacity,
                        used_bytes,
                        style_ref,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_utf8_2_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    bytes_capacity: usize,
    used_bytes: u32,
    span0_bytes: u32,
    style0: u32,
    span1_bytes: u32,
    style1: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let bytes = generated_buffer(bytes, bytes_capacity, 1, 16777216, 0x8000_0002u32)?;
                let used_bytes =
                    generated_buffer_used(used_bytes, bytes_capacity, 1, 16777216, 0x8000_0003u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_utf8_2_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        bytes,
                        bytes_capacity,
                        used_bytes,
                        span0_bytes,
                        style0,
                        span1_bytes,
                        style1,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_utf8_3_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    bytes_capacity: usize,
    used_bytes: u32,
    span0_bytes: u32,
    style0: u32,
    span1_bytes: u32,
    style1: u32,
    span2_bytes: u32,
    style2: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let bytes = generated_buffer(bytes, bytes_capacity, 1, 16777216, 0x8000_0002u32)?;
                let used_bytes =
                    generated_buffer_used(used_bytes, bytes_capacity, 1, 16777216, 0x8000_0003u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_utf8_3_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        bytes,
                        bytes_capacity,
                        used_bytes,
                        span0_bytes,
                        style0,
                        span1_bytes,
                        style1,
                        span2_bytes,
                        style2,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_utf8_4_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    bytes: *const u8,
    bytes_capacity: usize,
    used_bytes: u32,
    span0_bytes: u32,
    style0: u32,
    span1_bytes: u32,
    style1: u32,
    span2_bytes: u32,
    style2: u32,
    span3_bytes: u32,
    style3: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let bytes = generated_buffer(bytes, bytes_capacity, 1, 16777216, 0x8000_0002u32)?;
                let used_bytes =
                    generated_buffer_used(used_bytes, bytes_capacity, 1, 16777216, 0x8000_0003u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_utf8_4_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        bytes,
                        bytes_capacity,
                        used_bytes,
                        span0_bytes,
                        style0,
                        span1_bytes,
                        style1,
                        span2_bytes,
                        style2,
                        span3_bytes,
                        style3,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_cstring_2_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text0: *const ::core::ffi::c_char,
    style0: u32,
    text1: *const ::core::ffi::c_char,
    style1: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let text0 = generated_nonnull_const(text0, 0x8000_0001u32)?;
                let text1 = generated_nonnull_const(text1, 0x8000_0001u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_cstring_2_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        text0,
                        style0,
                        text1,
                        style1,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_cstring_3_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text0: *const ::core::ffi::c_char,
    style0: u32,
    text1: *const ::core::ffi::c_char,
    style1: u32,
    text2: *const ::core::ffi::c_char,
    style2: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let text0 = generated_nonnull_const(text0, 0x8000_0001u32)?;
                let text1 = generated_nonnull_const(text1, 0x8000_0001u32)?;
                let text2 = generated_nonnull_const(text2, 0x8000_0001u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_cstring_3_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        text0,
                        style0,
                        text1,
                        style1,
                        text2,
                        style2,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_view_text_create_cstring_4_v1(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text0: *const ::core::ffi::c_char,
    style0: u32,
    text1: *const ::core::ffi::c_char,
    style1: u32,
    text2: *const ::core::ffi::c_char,
    style2: u32,
    text3: *const ::core::ffi::c_char,
    style3: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    generated_catch_unwind(
        || {
            (|| -> Result<u32, u32> {
                let runtime = generated_nonnull(runtime, 0x8000_0001u32)?;
                let (node_id_low, node_id_high) =
                    generated_node_id(node_id_low, node_id_high, 0x8000_0001u32)?;
                let text0 = generated_nonnull_const(text0, 0x8000_0001u32)?;
                let text1 = generated_nonnull_const(text1, 0x8000_0001u32)?;
                let text2 = generated_nonnull_const(text2, 0x8000_0001u32)?;
                let text3 = generated_nonnull_const(text3, 0x8000_0001u32)?;
                let wrap = generated_enum(wrap, &[1, 2, 3], 0x8000_0001u32)?;
                let align = generated_enum(align, &[1, 2, 3], 0x8000_0001u32)?;
                Ok(unsafe {
                    generated_impls::view_text_create_cstring_4_impl(
                        runtime,
                        node_id_low,
                        node_id_high,
                        text0,
                        style0,
                        text1,
                        style1,
                        text2,
                        style2,
                        text3,
                        style3,
                        wrap,
                        align,
                    )
                })
            })()
        },
        0x8000_00ffu32,
    )
}
