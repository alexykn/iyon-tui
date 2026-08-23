/* DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml. */
/* schema_blake3 = 3f4ebadaf333fb067cc4ffbde6266b7177216a3fa210cbd25e04992c5ae13332 */
/* generator_blake3 = 4eb8b57027886c4f8812e667ad51e61f3d6fcbdc4dbd0e1bc935b2aae8f6b29c */
#ifndef IYON_VIEW_ABI_H
#define IYON_VIEW_ABI_H

#include <stddef.h>
#include <stdint.h>

#define IYON_VIEW_ABI_NAME "iyon_tui_view"
#define IYON_VIEW_ABI_VERSION 1
#define IYON_VIEW_SEMANTIC_SCHEMA_VERSION 1
#define IYON_VIEW_MINIMUM_BUN "1.4.0"
#define IYON_VIEW_QUALIFIED_BUN "1.4.0"
#define IYON_VIEW_RESULT_ERROR_BIT UINT32_C(0x80000000)

typedef struct NativeViewRuntime NativeViewRuntime;
typedef struct NativeHost NativeHost;
typedef struct AxisChildInputV1 {
    uint32_t track_word;
    uint32_t child_ref;
} AxisChildInputV1;

typedef enum WrapMode {
    WrapMode_WordThenGrapheme = UINT32_C(1),
    WrapMode_Grapheme = UINT32_C(2),
    WrapMode_NoWrap = UINT32_C(3),
} WrapMode;

typedef enum HorizontalAlign {
    HorizontalAlign_Start = UINT32_C(1),
    HorizontalAlign_Center = UINT32_C(2),
    HorizontalAlign_End = UINT32_C(3),
} HorizontalAlign;

uint32_t iyon_runtime_noop_v1(NativeViewRuntime * runtime);

uint32_t iyon_view_status_detail_v1(NativeViewRuntime * runtime);

uint32_t iyon_view_render_ref_v1(NativeViewRuntime * runtime, uint32_t base);

int32_t iyon_host_render_ref_v1(NativeViewRuntime * runtime, NativeHost * host, uint32_t base);

uint32_t iyon_view_spacer_create_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t rows);

uint32_t iyon_view_text_layout_patch_root_v1(NativeViewRuntime * runtime, uint32_t base, uint32_t node_id_low, uint32_t node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_view_common_patch_root_v1(NativeViewRuntime * runtime, uint32_t base, uint32_t node_id_low, uint32_t node_id_high, uint32_t mask, uint32_t padding_tr, uint32_t padding_bl, uint32_t width_rule, uint32_t height_rule, uint32_t min_width, uint32_t max_width, uint32_t min_height, uint32_t max_height, uint32_t decoration_ref);

uint32_t iyon_view_axis_create_buffer_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t axis_kind, uint32_t gap, const AxisChildInputV1 * children, size_t children_capacity_bytes, uint32_t used_child_count);

uint32_t iyon_view_row_create_0_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap);

uint32_t iyon_view_row_create_1_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0);

uint32_t iyon_view_row_create_2_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0, uint32_t track1, uint32_t child1);

uint32_t iyon_view_row_create_3_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0, uint32_t track1, uint32_t child1, uint32_t track2, uint32_t child2);

uint32_t iyon_view_row_create_4_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0, uint32_t track1, uint32_t child1, uint32_t track2, uint32_t child2, uint32_t track3, uint32_t child3);

uint32_t iyon_view_column_create_0_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap);

uint32_t iyon_view_column_create_1_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0);

uint32_t iyon_view_column_create_2_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0, uint32_t track1, uint32_t child1);

uint32_t iyon_view_column_create_3_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0, uint32_t track1, uint32_t child1, uint32_t track2, uint32_t child2);

uint32_t iyon_view_column_create_4_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap, uint32_t track0, uint32_t child0, uint32_t track1, uint32_t child1, uint32_t track2, uint32_t child2, uint32_t track3, uint32_t child3);

uint32_t iyon_axis_builder_begin_v1(NativeViewRuntime * runtime, uint32_t axis_kind, uint32_t expected_children);

int32_t iyon_axis_builder_push_v1(NativeViewRuntime * runtime, uint32_t builder_ref, uint32_t track_word, uint32_t child_ref);

uint32_t iyon_axis_builder_finish_v1(NativeViewRuntime * runtime, uint32_t builder_ref, uint32_t node_id_low, uint32_t node_id_high, uint32_t gap);

int32_t iyon_axis_builder_abort_v1(NativeViewRuntime * runtime, uint32_t builder_ref);

uint32_t iyon_view_axis_set_child_v1(NativeViewRuntime * runtime, uint32_t base_axis_ref, uint32_t node_id_low, uint32_t node_id_high, uint32_t child_index, uint32_t track_word, uint32_t child_ref);

uint32_t iyon_view_axis_splice_buffer_v1(NativeViewRuntime * runtime, uint32_t base_axis_ref, uint32_t node_id_low, uint32_t node_id_high, uint32_t index, uint32_t remove_count, const AxisChildInputV1 * children, size_t children_capacity_bytes, uint32_t used_child_count);

uint32_t iyon_view_grid_set_cell_v1(NativeViewRuntime * runtime, uint32_t base_grid_ref, uint32_t node_id_low, uint32_t node_id_high, uint32_t row, uint32_t column, uint32_t child_ref);

uint32_t iyon_view_axis_set_child_path_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t path_depth, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t ancestor2_node_id_low, uint32_t ancestor2_node_id_high, uint32_t ancestor3_node_id_low, uint32_t ancestor3_node_id_high, uint32_t axis_index, uint32_t track_word, uint32_t child_ref);

uint32_t iyon_view_grid_create_buffer_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, uint32_t column_gap, uint32_t row_gap, const uint32_t * words, size_t words_capacity_bytes, uint32_t used_word_count);

uint32_t iyon_view_grid_set_cell_path_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t path_depth, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t ancestor2_node_id_low, uint32_t ancestor2_node_id_high, uint32_t ancestor3_node_id_low, uint32_t ancestor3_node_id_high, uint32_t grid_row, uint32_t grid_column, uint32_t child_ref);

int32_t iyon_view_release_many_v1(NativeViewRuntime * runtime, const uint32_t * refs, size_t refs_capacity_bytes, uint32_t used_ref_count);

uint32_t iyon_view_ref_for_node_id_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high);

uint32_t iyon_path_root_v1(NativeViewRuntime * runtime);

uint32_t iyon_path_child_v1(NativeViewRuntime * runtime, uint32_t parent_path_ref, uint32_t step_kind, uint32_t expected_view_kind, uint32_t selector);

uint32_t iyon_view_text_layout_patch_path_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t path_depth, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t ancestor2_node_id_low, uint32_t ancestor2_node_id_high, uint32_t ancestor3_node_id_low, uint32_t ancestor3_node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_layout_patch_path_d1_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_layout_patch_path_d2_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_layout_patch_path_d3_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t ancestor2_node_id_low, uint32_t ancestor2_node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_layout_patch_path_d4_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t path_ref, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t ancestor2_node_id_low, uint32_t ancestor2_node_id_high, uint32_t ancestor3_node_id_low, uint32_t ancestor3_node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_edit_txn_begin_v1(NativeViewRuntime * runtime, uint32_t base_root_ref, uint32_t expected_edit_count);

int32_t iyon_edit_txn_add_text_layout_v1(NativeViewRuntime * runtime, uint32_t txn_ref, uint32_t path_ref, uint32_t path_depth, uint32_t target_node_id_low, uint32_t target_node_id_high, uint32_t ancestor0_node_id_low, uint32_t ancestor0_node_id_high, uint32_t ancestor1_node_id_low, uint32_t ancestor1_node_id_high, uint32_t ancestor2_node_id_low, uint32_t ancestor2_node_id_high, uint32_t ancestor3_node_id_low, uint32_t ancestor3_node_id_high, uint32_t wrap, uint32_t align);

uint32_t iyon_edit_txn_commit_render_v1(NativeViewRuntime * runtime, NativeHost * host, uint32_t txn_ref);

int32_t iyon_edit_txn_abort_v1(NativeViewRuntime * runtime, uint32_t txn_ref);

uint32_t iyon_style_atom_create_cstring_v1(NativeViewRuntime * runtime, const char * value);

uint32_t iyon_style_create_bits_v1(NativeViewRuntime * runtime, uint32_t flags, uint32_t attribute_present, uint32_t attribute_true, uint32_t foreground_ref, uint32_t background_ref, uint32_t theme_atom_ref);

uint32_t iyon_view_text_create_cstring_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const char * text, uint32_t style_ref, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_utf8_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const uint8_t * bytes, size_t bytes_capacity, uint32_t used_bytes, uint32_t style_ref, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_utf8_2_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const uint8_t * bytes, size_t bytes_capacity, uint32_t used_bytes, uint32_t span0_bytes, uint32_t style0, uint32_t span1_bytes, uint32_t style1, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_utf8_3_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const uint8_t * bytes, size_t bytes_capacity, uint32_t used_bytes, uint32_t span0_bytes, uint32_t style0, uint32_t span1_bytes, uint32_t style1, uint32_t span2_bytes, uint32_t style2, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_utf8_4_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const uint8_t * bytes, size_t bytes_capacity, uint32_t used_bytes, uint32_t span0_bytes, uint32_t style0, uint32_t span1_bytes, uint32_t style1, uint32_t span2_bytes, uint32_t style2, uint32_t span3_bytes, uint32_t style3, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_cstring_2_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const char * text0, uint32_t style0, const char * text1, uint32_t style1, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_cstring_3_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const char * text0, uint32_t style0, const char * text1, uint32_t style1, const char * text2, uint32_t style2, uint32_t wrap, uint32_t align);

uint32_t iyon_view_text_create_cstring_4_v1(NativeViewRuntime * runtime, uint32_t node_id_low, uint32_t node_id_high, const char * text0, uint32_t style0, const char * text1, uint32_t style1, const char * text2, uint32_t style2, const char * text3, uint32_t style3, uint32_t wrap, uint32_t align);

uint32_t iyon_abi_conformance_u8_8_v1(uint8_t a0, uint8_t a1, uint8_t a2, uint8_t a3, uint8_t a4, uint8_t a5, uint8_t a6, uint8_t a7);

uint32_t iyon_abi_conformance_u16_8_v1(uint16_t a0, uint16_t a1, uint16_t a2, uint16_t a3, uint16_t a4, uint16_t a5, uint16_t a6, uint16_t a7);

uint32_t iyon_abi_conformance_u32_8_v1(uint32_t a0, uint32_t a1, uint32_t a2, uint32_t a3, uint32_t a4, uint32_t a5, uint32_t a6, uint32_t a7);

uint32_t iyon_abi_conformance_u32_16_v1(uint32_t a0, uint32_t a1, uint32_t a2, uint32_t a3, uint32_t a4, uint32_t a5, uint32_t a6, uint32_t a7, uint32_t a8, uint32_t a9, uint32_t a10, uint32_t a11, uint32_t a12, uint32_t a13, uint32_t a14, uint32_t a15);

int32_t iyon_abi_conformance_i32_4_v1(int32_t a0, int32_t a1, int32_t a2, int32_t a3);

float iyon_abi_conformance_f32_4_v1(float a0, float a1, float a2, float a3);

double iyon_abi_conformance_f64_4_v1(double a0, double a1, double a2, double a3);

uint32_t iyon_abi_conformance_pointer_v1(void * a0);

uint32_t iyon_abi_conformance_buffer_v1(const uint8_t * a0, size_t a1);

uint32_t iyon_abi_conformance_cstring_v1(const char * a0);

#endif /* IYON_VIEW_ABI_H */
