<!-- DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml. schema_blake3 = ec82466e117642ffc4009bd11199b7a24aa37f3476065fa34e8732d070dda2d4; generator_blake3 = e6237f38757724691b7b739064c158573fc0f1dcd63ab16d537a85039e8d155a -->

# PERF-11 generated ABI reference

> This file is generated. Do not edit it directly.

- Schema BLAKE3: `ec82466e117642ffc4009bd11199b7a24aa37f3476065fa34e8732d070dda2d4`
- Generator BLAKE3: `e6237f38757724691b7b739064c158573fc0f1dcd63ab16d537a85039e8d155a`
- ABI: `iyon_tui_view` v1
- Semantic schema: v1
- Minimum Bun: `1.4.0`
- Qualified Bun: `1.4.0`
## Handles

| Name | Rust | TypeScript | Lifetime | Kind |
|---|---|---|---|---|
| `RuntimePtr` | `*mut NativeViewRuntime` | `Pointer` | `environment` | `-` |
| `HostPtr` | `*mut NativeHost` | `Pointer` | `host` | `-` |
| `ViewRef` | `u32` | `number` | `runtime` | `view` |
| `PathRef` | `u32` | `number` | `runtime` | `path` |
| `StyleRef` | `u32` | `number` | `runtime` | `style` |
| `StyleAtomRef` | `u32` | `number` | `runtime` | `style_atom` |
| `BuilderRef` | `u32` | `number` | `runtime` | `builder` |
| `EditTxnRef` | `u32` | `number` | `runtime` | `edit_txn` |

## POD buffers

| Name | Repr | Size | Align |
|---|---|---:|---:|
| `AxisChildInputV1` | `C` | 8 | 4 |

## Enums

### `WrapMode`

| Value | Bridge key |
|---|---|
| `WordThenGrapheme` | `wrapWordThenGrapheme` |
| `Grapheme` | `wrapGrapheme` |
| `NoWrap` | `wrapNoWrap` |

### `HorizontalAlign`

| Value | Bridge key |
|---|---|
| `Start` | `horizontalStart` |
| `Center` | `horizontalCenter` |
| `End` | `horizontalEnd` |

## Functions

| Name | Family | Hotness | Return | Fallback | Thread | Allocates | Host mutation |
|---|---|---|---|---|---|---|---|
| `runtime_noop` | `runtime` | `probe` | `u32` | `none` | `owner_thread` | `false` | `false` |
| `view_render_ref` | `render_ref` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `false` | `false` |
| `host_render_ref` | `render_ref` | `critical` | `i32` | `none` | `owner_thread` | `false` | `true` |
| `view_spacer_create` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_layout_patch_root` | `scalar_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_common_patch_root` | `scalar_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_axis_create_buffer` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_row_create_0` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_row_create_1` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_row_create_2` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_row_create_3` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_row_create_4` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_column_create_0` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_column_create_1` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_column_create_2` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_column_create_3` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_column_create_4` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `axis_builder_begin` | `builder` | `warm` | `native_ref_result` | `v4` | `owner_thread` | `true` | `false` |
| `axis_builder_push` | `builder` | `warm` | `status_only` | `v4` | `owner_thread` | `true` | `false` |
| `axis_builder_finish` | `builder` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `axis_builder_abort` | `builder` | `cold` | `status_only` | `none` | `owner_thread` | `false` | `false` |
| `view_axis_set_child` | `structural_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_axis_splice_buffer` | `structural_patch` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_grid_set_cell` | `structural_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_axis_set_child_path` | `structural_path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_grid_set_cell_path` | `structural_path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_release_many` | `lifecycle` | `cold` | `i32` | `none` | `owner_thread` | `false` | `false` |
| `view_ref_for_node_id` | `exact_lookup` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `false` | `false` |
| `path_root` | `path` | `warm` | `PathRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `path_child` | `path` | `warm` | `PathRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_layout_patch_path` | `path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_layout_patch_path_d1` | `path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_layout_patch_path_d2` | `path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_layout_patch_path_d3` | `path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_layout_patch_path_d4` | `path_patch` | `critical` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `edit_txn_begin` | `edit_transaction` | `critical` | `native_ref_result` | `v4` | `owner_thread` | `true` | `false` |
| `edit_txn_add_text_layout` | `edit_transaction` | `critical` | `status_only` | `v4` | `owner_thread` | `true` | `false` |
| `edit_txn_commit_render` | `edit_transaction` | `critical` | `native_ref_result` | `v4` | `owner_thread` | `true` | `true` |
| `edit_txn_abort` | `edit_transaction` | `cold` | `status_only` | `none` | `owner_thread` | `false` | `false` |
| `style_atom_create_cstring` | `style_atom` | `warm` | `StyleAtomRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `style_create_bits` | `style_atom` | `warm` | `StyleRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_cstring` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_utf8` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_utf8_2` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_utf8_3` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_utf8_4` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_cstring_2` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_cstring_3` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |
| `view_text_create_cstring_4` | `constructor` | `warm` | `ViewRefResult` | `v4` | `owner_thread` | `true` | `false` |

## ABI conformance fixtures

| Name | Return | Operation | Arguments |
|---|---|---|---|
| `u8_8` | `u32` | `position_weighted_sum` | `u8, u8, u8, u8, u8, u8, u8, u8` |
| `u16_8` | `u32` | `position_weighted_sum` | `u16, u16, u16, u16, u16, u16, u16, u16` |
| `u32_8` | `u32` | `position_weighted_sum` | `u32, u32, u32, u32, u32, u32, u32, u32` |
| `u32_16` | `u32` | `position_weighted_sum` | `u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32` |
| `i32_4` | `i32` | `position_weighted_sum` | `i32, i32, i32, i32` |
| `f32_4` | `f32` | `position_weighted_sum` | `f32, f32, f32, f32` |
| `f64_4` | `f64` | `position_weighted_sum` | `f64, f64, f64, f64` |
| `pointer` | `u32` | `pointer_probe` | `ptr` |
| `buffer` | `u32` | `buffer_probe` | `buffer, buffer_length` |
| `cstring` | `u32` | `cstring_hash` | `cstring` |

