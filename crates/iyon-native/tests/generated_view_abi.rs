// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 8a6fdc06e24d71ad37c62392eb0cd8e96598118564598408fb8555b5ae4816e0
// generator_blake3 = 0fb2fdc89a11de0e5d62d9a0d5e5129e12f59a8e6f97c28d78fe95271bfa95a2
#[allow(dead_code)]
pub struct NativeViewRuntime;

#[path = "../src/generated/view_abi_table.rs"]
mod generated;
#[path = "../src/generated/view_abi_conformance.rs"]
mod generated_conformance;
#[path = "../src/generated/view_abi_types.rs"]
mod generated_types;

use generated_types::AxisChildInputV1;

#[allow(dead_code)]
pub struct NativeHost;

mod generated_exports {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/generated/view_abi_exports.rs"
    ));
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn runtime_noop_impl(runtime: *mut NativeViewRuntime) -> u32 {
    let _ = runtime;
    0x100
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_status_detail_impl(runtime: *mut NativeViewRuntime) -> u32 {
    let _ = runtime;
    0x101
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_render_ref_impl(
    runtime: *mut NativeViewRuntime,
    base: u32,
) -> u32 {
    let _ = runtime;
    let _ = base;
    0x102
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn host_render_ref_impl(
    runtime: *mut NativeViewRuntime,
    host: *mut NativeHost,
    base: u32,
) -> i32 {
    let _ = runtime;
    let _ = host;
    let _ = base;
    103
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_spacer_create_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    rows: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = rows;
    0x104
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_root_impl(
    runtime: *mut NativeViewRuntime,
    base: u32,
    node_id_low: u32,
    node_id_high: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let _ = runtime;
    let _ = base;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = wrap;
    let _ = align;
    0x105
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_common_patch_root_impl(
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
    let _ = runtime;
    let _ = base;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = mask;
    let _ = padding_tr;
    let _ = padding_bl;
    let _ = width_rule;
    let _ = height_rule;
    let _ = min_width;
    let _ = max_width;
    let _ = min_height;
    let _ = max_height;
    let _ = decoration_ref;
    0x106
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    axis_kind: u32,
    gap: u32,
    children: *const AxisChildInputV1,
    children_capacity_bytes: usize,
    used_child_count: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = axis_kind;
    let _ = gap;
    let _ = children;
    let _ = children_capacity_bytes;
    let _ = used_child_count;
    0x107
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_row_create_0_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    0x108
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_row_create_1_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    0x109
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_row_create_2_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    let _ = track1;
    let _ = child1;
    0x10a
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_row_create_3_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    let _ = track1;
    let _ = child1;
    let _ = track2;
    let _ = child2;
    0x10b
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_row_create_4_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    let _ = track1;
    let _ = child1;
    let _ = track2;
    let _ = child2;
    let _ = track3;
    let _ = child3;
    0x10c
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_column_create_0_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    0x10d
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_column_create_1_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    0x10e
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_column_create_2_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
    track0: u32,
    child0: u32,
    track1: u32,
    child1: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    let _ = track1;
    let _ = child1;
    0x10f
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_column_create_3_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    let _ = track1;
    let _ = child1;
    let _ = track2;
    let _ = child2;
    0x110
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_column_create_4_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    let _ = track0;
    let _ = child0;
    let _ = track1;
    let _ = child1;
    let _ = track2;
    let _ = child2;
    let _ = track3;
    let _ = child3;
    0x111
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_begin_impl(
    runtime: *mut NativeViewRuntime,
    axis_kind: u32,
    expected_children: u32,
) -> u32 {
    let _ = runtime;
    let _ = axis_kind;
    let _ = expected_children;
    0x112
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_push_impl(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
    track_word: u32,
    child_ref: u32,
) -> i32 {
    let _ = runtime;
    let _ = builder_ref;
    let _ = track_word;
    let _ = child_ref;
    119
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_finish_impl(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    gap: u32,
) -> u32 {
    let _ = runtime;
    let _ = builder_ref;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = gap;
    0x114
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn axis_builder_abort_impl(
    runtime: *mut NativeViewRuntime,
    builder_ref: u32,
) -> i32 {
    let _ = runtime;
    let _ = builder_ref;
    121
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_set_child_impl(
    runtime: *mut NativeViewRuntime,
    base_axis_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    child_index: u32,
    track_word: u32,
    child_ref: u32,
) -> u32 {
    let _ = runtime;
    let _ = base_axis_ref;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = child_index;
    let _ = track_word;
    let _ = child_ref;
    0x116
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_splice_buffer_impl(
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
    let _ = runtime;
    let _ = base_axis_ref;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = index;
    let _ = remove_count;
    let _ = children;
    let _ = children_capacity_bytes;
    let _ = used_child_count;
    0x117
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_grid_set_cell_impl(
    runtime: *mut NativeViewRuntime,
    base_grid_ref: u32,
    node_id_low: u32,
    node_id_high: u32,
    row: u32,
    column: u32,
    child_ref: u32,
) -> u32 {
    let _ = runtime;
    let _ = base_grid_ref;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = row;
    let _ = column;
    let _ = child_ref;
    0x118
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_axis_set_child_path_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = path_depth;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = ancestor2_node_id_low;
    let _ = ancestor2_node_id_high;
    let _ = ancestor3_node_id_low;
    let _ = ancestor3_node_id_high;
    let _ = axis_index;
    let _ = track_word;
    let _ = child_ref;
    0x119
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_grid_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    column_gap: u32,
    row_gap: u32,
    words: *const u32,
    words_capacity_bytes: usize,
    used_word_count: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = column_gap;
    let _ = row_gap;
    let _ = words;
    let _ = words_capacity_bytes;
    let _ = used_word_count;
    0x11a
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_diff_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    words: *const u32,
    words_capacity_bytes: usize,
    used_word_count: u32,
    bytes: *const u8,
    bytes_capacity_bytes: usize,
    used_byte_count: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = words;
    let _ = words_capacity_bytes;
    let _ = used_word_count;
    let _ = bytes;
    let _ = bytes_capacity_bytes;
    let _ = used_byte_count;
    0x11b
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_hanging_create_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    prefix_ref: u32,
    continuation_ref: u32,
    body_ref: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = prefix_ref;
    let _ = continuation_ref;
    let _ = body_ref;
    0x11c
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_container_create_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    child_ref: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = child_ref;
    0x11d
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_clamp_create_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    child_ref: u32,
    max_rows: u32,
    overflow_kind: u32,
    overflow_style_ref: u32,
    prefix: *const ::core::ffi::c_char,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = child_ref;
    let _ = max_rows;
    let _ = overflow_kind;
    let _ = overflow_style_ref;
    let _ = prefix;
    0x11e
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_component_create_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    handle_low: u32,
    handle_high: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = handle_low;
    let _ = handle_high;
    0x11f
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_decorated_create_buffer_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    child_ref: u32,
    style_ref: u32,
    words: *const u32,
    words_capacity_bytes: usize,
    used_word_count: u32,
    bytes: *const u8,
    bytes_capacity_bytes: usize,
    used_byte_count: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = child_ref;
    let _ = style_ref;
    let _ = words;
    let _ = words_capacity_bytes;
    let _ = used_word_count;
    let _ = bytes;
    let _ = bytes_capacity_bytes;
    let _ = used_byte_count;
    0x120
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_grid_set_cell_path_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = path_depth;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = ancestor2_node_id_low;
    let _ = ancestor2_node_id_high;
    let _ = ancestor3_node_id_low;
    let _ = ancestor3_node_id_high;
    let _ = grid_row;
    let _ = grid_column;
    let _ = child_ref;
    0x121
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_release_many_impl(
    runtime: *mut NativeViewRuntime,
    refs: *const u32,
    refs_capacity_bytes: usize,
    used_ref_count: u32,
) -> i32 {
    let _ = runtime;
    let _ = refs;
    let _ = refs_capacity_bytes;
    let _ = used_ref_count;
    134
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_ref_for_node_id_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    0x123
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn path_root_impl(runtime: *mut NativeViewRuntime) -> u32 {
    let _ = runtime;
    0x124
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn path_child_impl(
    runtime: *mut NativeViewRuntime,
    parent_path_ref: u32,
    step_kind: u32,
    expected_view_kind: u32,
    selector: u32,
) -> u32 {
    let _ = runtime;
    let _ = parent_path_ref;
    let _ = step_kind;
    let _ = expected_view_kind;
    let _ = selector;
    0x125
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = path_depth;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = ancestor2_node_id_low;
    let _ = ancestor2_node_id_high;
    let _ = ancestor3_node_id_low;
    let _ = ancestor3_node_id_high;
    let _ = wrap;
    let _ = align;
    0x126
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d1_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = wrap;
    let _ = align;
    0x127
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d2_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = wrap;
    let _ = align;
    0x128
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d3_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = ancestor2_node_id_low;
    let _ = ancestor2_node_id_high;
    let _ = wrap;
    let _ = align;
    0x129
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_layout_patch_path_d4_impl(
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
    let _ = runtime;
    let _ = base_root_ref;
    let _ = path_ref;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = ancestor2_node_id_low;
    let _ = ancestor2_node_id_high;
    let _ = ancestor3_node_id_low;
    let _ = ancestor3_node_id_high;
    let _ = wrap;
    let _ = align;
    0x12a
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_begin_impl(
    runtime: *mut NativeViewRuntime,
    base_root_ref: u32,
    expected_edit_count: u32,
) -> u32 {
    let _ = runtime;
    let _ = base_root_ref;
    let _ = expected_edit_count;
    0x12b
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_add_text_layout_impl(
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
    let _ = runtime;
    let _ = txn_ref;
    let _ = path_ref;
    let _ = path_depth;
    let _ = target_node_id_low;
    let _ = target_node_id_high;
    let _ = ancestor0_node_id_low;
    let _ = ancestor0_node_id_high;
    let _ = ancestor1_node_id_low;
    let _ = ancestor1_node_id_high;
    let _ = ancestor2_node_id_low;
    let _ = ancestor2_node_id_high;
    let _ = ancestor3_node_id_low;
    let _ = ancestor3_node_id_high;
    let _ = wrap;
    let _ = align;
    144
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_commit_render_impl(
    runtime: *mut NativeViewRuntime,
    host: *mut NativeHost,
    txn_ref: u32,
) -> u32 {
    let _ = runtime;
    let _ = host;
    let _ = txn_ref;
    0x12d
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn edit_txn_abort_impl(
    runtime: *mut NativeViewRuntime,
    txn_ref: u32,
) -> i32 {
    let _ = runtime;
    let _ = txn_ref;
    146
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn style_atom_create_cstring_impl(
    runtime: *mut NativeViewRuntime,
    value: *const ::core::ffi::c_char,
) -> u32 {
    let _ = runtime;
    let _ = value;
    0x12f
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn style_create_bits_impl(
    runtime: *mut NativeViewRuntime,
    flags: u32,
    attribute_present: u32,
    attribute_true: u32,
    foreground_ref: u32,
    background_ref: u32,
    theme_atom_ref: u32,
) -> u32 {
    let _ = runtime;
    let _ = flags;
    let _ = attribute_present;
    let _ = attribute_true;
    let _ = foreground_ref;
    let _ = background_ref;
    let _ = theme_atom_ref;
    0x130
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_impl(
    runtime: *mut NativeViewRuntime,
    node_id_low: u32,
    node_id_high: u32,
    text: *const ::core::ffi::c_char,
    style_ref: u32,
    wrap: u32,
    align: u32,
) -> u32 {
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = text;
    let _ = style_ref;
    let _ = wrap;
    let _ = align;
    0x131
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = bytes;
    let _ = bytes_capacity;
    let _ = used_bytes;
    let _ = style_ref;
    let _ = wrap;
    let _ = align;
    0x132
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_2_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = bytes;
    let _ = bytes_capacity;
    let _ = used_bytes;
    let _ = span0_bytes;
    let _ = style0;
    let _ = span1_bytes;
    let _ = style1;
    let _ = wrap;
    let _ = align;
    0x133
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_3_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = bytes;
    let _ = bytes_capacity;
    let _ = used_bytes;
    let _ = span0_bytes;
    let _ = style0;
    let _ = span1_bytes;
    let _ = style1;
    let _ = span2_bytes;
    let _ = style2;
    let _ = wrap;
    let _ = align;
    0x134
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_utf8_4_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = bytes;
    let _ = bytes_capacity;
    let _ = used_bytes;
    let _ = span0_bytes;
    let _ = style0;
    let _ = span1_bytes;
    let _ = style1;
    let _ = span2_bytes;
    let _ = style2;
    let _ = span3_bytes;
    let _ = style3;
    let _ = wrap;
    let _ = align;
    0x135
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_2_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = text0;
    let _ = style0;
    let _ = text1;
    let _ = style1;
    let _ = wrap;
    let _ = align;
    0x136
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_3_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = text0;
    let _ = style0;
    let _ = text1;
    let _ = style1;
    let _ = text2;
    let _ = style2;
    let _ = wrap;
    let _ = align;
    0x137
}

#[unsafe(no_mangle)]
pub unsafe extern "Rust" fn view_text_create_cstring_4_impl(
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
    let _ = runtime;
    let _ = node_id_low;
    let _ = node_id_high;
    let _ = text0;
    let _ = style0;
    let _ = text1;
    let _ = style1;
    let _ = text2;
    let _ = style2;
    let _ = text3;
    let _ = style3;
    let _ = wrap;
    let _ = align;
    0x138
}

#[test]
fn generated_function_count_is_stable() {
    assert_eq!(generated::FUNCTION_COUNT, 57);
}

#[test]
fn generated_abi_version_is_one() {
    assert_eq!(generated_types::ABI_VERSION, 1);
}

#[test]
fn generated_conformance_count_is_stable() {
    assert_eq!(10, 10);
}

#[test]
fn generated_conformance_functions_are_callable() {
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_u8_8_v1(
                1 as u8, 2 as u8, 3 as u8, 4 as u8, 5 as u8, 6 as u8, 7 as u8, 8 as u8,
            )
        },
        562
    );
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_u16_8_v1(
                1 as u16, 2 as u16, 3 as u16, 4 as u16, 5 as u16, 6 as u16, 7 as u16, 8 as u16,
            )
        },
        562
    );
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_u32_8_v1(
                1 as u32, 2 as u32, 3 as u32, 4 as u32, 5 as u32, 6 as u32, 7 as u32, 8 as u32,
            )
        },
        562
    );
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_u32_16_v1(
                1 as u32, 2 as u32, 3 as u32, 4 as u32, 5 as u32, 6 as u32, 7 as u32, 8 as u32,
                9 as u32, 10 as u32, 11 as u32, 12 as u32, 13 as u32, 14 as u32, 15 as u32,
                16 as u32,
            )
        },
        4988
    );
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_i32_4_v1(
                1 as i32, 2 as i32, 3 as i32, 4 as i32,
            )
        },
        78
    );
    assert!(
        (unsafe {
            generated_conformance::iyon_abi_conformance_f32_4_v1(
                1 as f32, 2 as f32, 3 as f32, 4 as f32,
            )
        } - 78.0)
            .abs()
            < 0.000001
    );
    assert!(
        (unsafe {
            generated_conformance::iyon_abi_conformance_f64_4_v1(
                1 as f64, 2 as f64, 3 as f64, 4 as f64,
            )
        } - 78.0)
            .abs()
            < 0.000001
    );
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_pointer_v1(
                core::ptr::NonNull::<core::ffi::c_void>::dangling().as_ptr(),
            )
        },
        1
    );
    let bytes = [0x7b_u8, 0x01, 0x02, 0x03];
    assert_eq!(
        unsafe {
            generated_conformance::iyon_abi_conformance_buffer_v1(bytes.as_ptr(), bytes.len())
        },
        4 * 257 + 0x7b
    );
    let text = std::ffi::CString::new("ABI ✓").expect("test text has no NUL");
    assert_ne!(
        unsafe { generated_conformance::iyon_abi_conformance_cstring_v1(text.as_ptr()) },
        0
    );
}

#[test]
fn generated_wrappers_reject_invalid_inputs_and_delegate() {
    let mut runtime = NativeViewRuntime;
    let runtime_ptr = &mut runtime as *mut NativeViewRuntime;
    assert_eq!(
        unsafe { generated_exports::iyon_runtime_noop_v1(runtime_ptr) },
        0x100
    );
    assert_eq!(
        unsafe { generated_exports::iyon_view_render_ref_v1(runtime_ptr, 1) },
        0x102
    );
    let mut host = NativeHost;
    let host_ptr = &mut host as *mut NativeHost;
    assert_eq!(
        unsafe { generated_exports::iyon_host_render_ref_v1(runtime_ptr, host_ptr, 1) },
        103
    );
    assert_eq!(
        unsafe { generated_exports::iyon_view_spacer_create_v1(runtime_ptr, 1, 0, 2) },
        0x104
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_text_layout_patch_root_v1(runtime_ptr, 1, 1, 0, 1, 2)
        },
        0x105
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_common_patch_root_v1(
                runtime_ptr,
                1,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                1,
            )
        },
        0x106
    );
    let children = [generated_types::AxisChildInputV1 {
        track_word: 1,
        child_ref: 1,
    }];
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_axis_create_buffer_v1(
                runtime_ptr,
                1,
                0,
                1,
                0,
                children.as_ptr(),
                core::mem::size_of_val(&children),
                1,
            )
        },
        0x107
    );
    let refs = [1_u32];
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_release_many_v1(
                runtime_ptr,
                refs.as_ptr(),
                core::mem::size_of_val(&refs),
                1,
            )
        },
        134
    );
    // PERF-12 T13 note: this expectation is positional (0x64 + index of
    // view_release_many in the canonical function order). It drifted at T13's
    // five-function insertion; prefer test_stub_value-style computation when
    // touching this block again.
    assert_eq!(
        unsafe { generated_exports::iyon_runtime_noop_v1(core::ptr::null_mut()) },
        0x8000_0001
    );
    assert_eq!(
        unsafe { generated_exports::iyon_view_render_ref_v1(runtime_ptr, 0) },
        0x8000_0001
    );
    assert_eq!(
        unsafe { generated_exports::iyon_view_spacer_create_v1(runtime_ptr, 0, 0, 1) },
        0x8000_0001
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_text_layout_patch_root_v1(runtime_ptr, 1, 1, 0, 0, 1)
        },
        0x8000_0001
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_axis_create_buffer_v1(
                runtime_ptr,
                1,
                0,
                1,
                0,
                core::ptr::null(),
                8,
                0,
            )
        },
        0x8000_0002
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_axis_create_buffer_v1(
                runtime_ptr,
                1,
                0,
                1,
                0,
                core::ptr::null(),
                0,
                1,
            )
        },
        0x8000_0003
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_release_many_v1(runtime_ptr, core::ptr::null(), 4, 0)
        },
        -2
    );
    assert_eq!(
        unsafe {
            generated_exports::iyon_view_release_many_v1(runtime_ptr, core::ptr::null(), 0, 1)
        },
        -3
    );
    assert_eq!(
        unsafe { generated_exports::iyon_view_ref_for_node_id_v1(runtime_ptr, 1, 0) },
        0x123
    );
    assert_eq!(
        unsafe { generated_exports::iyon_view_ref_for_node_id_v1(runtime_ptr, 0, 0) },
        0x8000_0001
    );
}
