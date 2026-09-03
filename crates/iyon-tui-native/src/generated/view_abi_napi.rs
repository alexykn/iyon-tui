// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 7a24d016ae0beb92c3015bac0a6dd66f09ba93bb899e0271232d576bc586c2bd
// generator_blake3 = 8988baafda3c5ed74ab4450221eca5cd9a1b7fcd82dd351d571694bcb307ca65
// Generated safe N-API methods. The only unsafe operations are the
// private calls from typed N-API values into the validated semantic wrappers.
#[napi]
impl NativeViewAbiSession {
#[napi(js_name = "runtimeNoop")]
    pub fn runtime_noop(&self) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_runtime_noop_v1(runtime) })
    }

#[napi(js_name = "viewStatusDetail")]
    pub fn view_status_detail(&self) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_status_detail_v1(runtime) })
    }

#[napi(js_name = "viewRenderRef")]
    pub fn view_render_ref(&self, base: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_render_ref_v1(runtime, base) })
    }

#[napi(js_name = "hostRenderRef")]
    pub fn host_render_ref(&self, host: &NativeTuiHost, base: u32) -> napi::Result<i32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_host_render_ref_v1(runtime, host as *const NativeTuiHost as *mut NativeHost, base) })
    }

#[napi(js_name = "viewStateAttach")]
    pub fn view_state_attach(&self, base: u32, node_id_low: u32, node_id_high: u32, state_id_low: u32, state_id_high: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_state_attach_v1(runtime, base, node_id_low, node_id_high, state_id_low, state_id_high) })
    }

#[napi(js_name = "viewContentHostCreate")]
    pub fn view_content_host_create(&self, node_id_low: u32, node_id_high: u32, content_port_id_low: u32, content_port_id_high: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_content_host_create_v1(runtime, node_id_low, node_id_high, content_port_id_low, content_port_id_high) })
    }

#[napi(js_name = "viewSpacerCreate")]
    pub fn view_spacer_create(&self, node_id_low: u32, node_id_high: u32, rows: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_spacer_create_v1(runtime, node_id_low, node_id_high, rows) })
    }

#[napi(js_name = "viewTextLayoutPatchRoot")]
    pub fn view_text_layout_patch_root(&self, base: u32, node_id_low: u32, node_id_high: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_layout_patch_root_v1(runtime, base, node_id_low, node_id_high, wrap, align) })
    }

#[napi(js_name = "viewCommonPatchRoot")]
    pub fn view_common_patch_root(&self, base: u32, node_id_low: u32, node_id_high: u32, mask: u32, padding_tr: u32, padding_bl: u32, width_rule: u32, height_rule: u32, min_width: u32, max_width: u32, min_height: u32, max_height: u32, decoration_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_common_patch_root_v1(runtime, base, node_id_low, node_id_high, mask, padding_tr, padding_bl, width_rule, height_rule, min_width, max_width, min_height, max_height, decoration_ref) })
    }

#[napi(js_name = "viewAxisCreateBuffer")]
    pub fn view_axis_create_buffer(&self, node_id_low: u32, node_id_high: u32, axis_kind: u32, gap: u32, children: napi::bindgen_prelude::Uint32Array, used_child_count: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_axis_create_buffer_v1(runtime, node_id_low, node_id_high, axis_kind, gap, children.as_ref().as_ptr() as *const AxisChildInputV1, children.as_ref().len().saturating_mul(4), used_child_count) })
    }

#[napi(js_name = "viewRowCreate0")]
    pub fn view_row_create_0(&self, node_id_low: u32, node_id_high: u32, gap: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_row_create_0_v1(runtime, node_id_low, node_id_high, gap) })
    }

#[napi(js_name = "viewRowCreate1")]
    pub fn view_row_create_1(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_row_create_1_v1(runtime, node_id_low, node_id_high, gap, track0, child0) })
    }

#[napi(js_name = "viewRowCreate2")]
    pub fn view_row_create_2(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32, track1: u32, child1: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_row_create_2_v1(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1) })
    }

#[napi(js_name = "viewRowCreate3")]
    pub fn view_row_create_3(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32, track1: u32, child1: u32, track2: u32, child2: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_row_create_3_v1(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2) })
    }

#[napi(js_name = "viewRowCreate4")]
    pub fn view_row_create_4(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32, track1: u32, child1: u32, track2: u32, child2: u32, track3: u32, child3: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_row_create_4_v1(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2, track3, child3) })
    }

#[napi(js_name = "viewColumnCreate0")]
    pub fn view_column_create_0(&self, node_id_low: u32, node_id_high: u32, gap: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_column_create_0_v1(runtime, node_id_low, node_id_high, gap) })
    }

#[napi(js_name = "viewColumnCreate1")]
    pub fn view_column_create_1(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_column_create_1_v1(runtime, node_id_low, node_id_high, gap, track0, child0) })
    }

#[napi(js_name = "viewColumnCreate2")]
    pub fn view_column_create_2(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32, track1: u32, child1: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_column_create_2_v1(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1) })
    }

#[napi(js_name = "viewColumnCreate3")]
    pub fn view_column_create_3(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32, track1: u32, child1: u32, track2: u32, child2: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_column_create_3_v1(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2) })
    }

#[napi(js_name = "viewColumnCreate4")]
    pub fn view_column_create_4(&self, node_id_low: u32, node_id_high: u32, gap: u32, track0: u32, child0: u32, track1: u32, child1: u32, track2: u32, child2: u32, track3: u32, child3: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_column_create_4_v1(runtime, node_id_low, node_id_high, gap, track0, child0, track1, child1, track2, child2, track3, child3) })
    }

#[napi(js_name = "axisBuilderBegin")]
    pub fn axis_builder_begin(&self, axis_kind: u32, expected_children: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_axis_builder_begin_v1(runtime, axis_kind, expected_children) })
    }

#[napi(js_name = "axisBuilderPush")]
    pub fn axis_builder_push(&self, builder_ref: u32, track_word: u32, child_ref: u32) -> napi::Result<i32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_axis_builder_push_v1(runtime, builder_ref, track_word, child_ref) })
    }

#[napi(js_name = "axisBuilderFinish")]
    pub fn axis_builder_finish(&self, builder_ref: u32, node_id_low: u32, node_id_high: u32, gap: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_axis_builder_finish_v1(runtime, builder_ref, node_id_low, node_id_high, gap) })
    }

#[napi(js_name = "axisBuilderAbort")]
    pub fn axis_builder_abort(&self, builder_ref: u32) -> napi::Result<i32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_axis_builder_abort_v1(runtime, builder_ref) })
    }

#[napi(js_name = "viewAxisSetChild")]
    pub fn view_axis_set_child(&self, base_axis_ref: u32, node_id_low: u32, node_id_high: u32, child_index: u32, track_word: u32, child_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_axis_set_child_v1(runtime, base_axis_ref, node_id_low, node_id_high, child_index, track_word, child_ref) })
    }

#[napi(js_name = "viewAxisSpliceBuffer")]
    pub fn view_axis_splice_buffer(&self, base_axis_ref: u32, node_id_low: u32, node_id_high: u32, index: u32, remove_count: u32, children: napi::bindgen_prelude::Uint32Array, used_child_count: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_axis_splice_buffer_v1(runtime, base_axis_ref, node_id_low, node_id_high, index, remove_count, children.as_ref().as_ptr() as *const AxisChildInputV1, children.as_ref().len().saturating_mul(4), used_child_count) })
    }

#[napi(js_name = "viewGridSetCell")]
    pub fn view_grid_set_cell(&self, base_grid_ref: u32, node_id_low: u32, node_id_high: u32, row: u32, column: u32, child_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_grid_set_cell_v1(runtime, base_grid_ref, node_id_low, node_id_high, row, column, child_ref) })
    }

#[napi(js_name = "viewAxisSetChildPath")]
    pub fn view_axis_set_child_path(&self, base_root_ref: u32, path_ref: u32, path_depth: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, ancestor2_node_id_low: u32, ancestor2_node_id_high: u32, ancestor3_node_id_low: u32, ancestor3_node_id_high: u32, axis_index: u32, track_word: u32, child_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_axis_set_child_path_v1(runtime, base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, axis_index, track_word, child_ref) })
    }

#[napi(js_name = "viewGridCreateBuffer")]
    pub fn view_grid_create_buffer(&self, node_id_low: u32, node_id_high: u32, column_gap: u32, row_gap: u32, words: napi::bindgen_prelude::Uint32Array, used_word_count: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_grid_create_buffer_v1(runtime, node_id_low, node_id_high, column_gap, row_gap, words.as_ref().as_ptr() as *const u32, words.as_ref().len().saturating_mul(4), used_word_count) })
    }

#[napi(js_name = "viewDiffCreateBuffer")]
    pub fn view_diff_create_buffer(&self, node_id_low: u32, node_id_high: u32, words: napi::bindgen_prelude::Uint32Array, used_word_count: u32, bytes: napi::bindgen_prelude::Uint8Array, used_byte_count: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_diff_create_buffer_v1(runtime, node_id_low, node_id_high, words.as_ref().as_ptr() as *const u32, words.as_ref().len().saturating_mul(4), used_word_count, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_byte_count) })
    }

#[napi(js_name = "viewHangingCreate")]
    pub fn view_hanging_create(&self, node_id_low: u32, node_id_high: u32, prefix_ref: u32, continuation_ref: u32, body_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_hanging_create_v1(runtime, node_id_low, node_id_high, prefix_ref, continuation_ref, body_ref) })
    }

#[napi(js_name = "viewContainerCreate")]
    pub fn view_container_create(&self, node_id_low: u32, node_id_high: u32, child_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_container_create_v1(runtime, node_id_low, node_id_high, child_ref) })
    }

#[napi(js_name = "viewClampCreate")]
    pub fn view_clamp_create(&self, node_id_low: u32, node_id_high: u32, child_ref: u32, max_rows: u32, overflow_kind: u32, overflow_style_ref: u32, prefix: String) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        let prefix_cstring = std::ffi::CString::new(prefix).map_err(|_| napi::Error::from_reason("prefix must not contain NUL"))?;
        Ok(unsafe { generated_exports::invoke_iyon_view_clamp_create_v1(runtime, node_id_low, node_id_high, child_ref, max_rows, overflow_kind, overflow_style_ref, prefix_cstring.as_ptr()) })
    }

#[napi(js_name = "viewComponentCreate")]
    pub fn view_component_create(&self, node_id_low: u32, node_id_high: u32, handle_low: u32, handle_high: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_component_create_v1(runtime, node_id_low, node_id_high, handle_low, handle_high) })
    }

#[napi(js_name = "viewDecoratedCreateBuffer")]
    pub fn view_decorated_create_buffer(&self, node_id_low: u32, node_id_high: u32, child_ref: u32, style_ref: u32, words: napi::bindgen_prelude::Uint32Array, used_word_count: u32, bytes: napi::bindgen_prelude::Uint8Array, used_byte_count: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_decorated_create_buffer_v1(runtime, node_id_low, node_id_high, child_ref, style_ref, words.as_ref().as_ptr() as *const u32, words.as_ref().len().saturating_mul(4), used_word_count, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_byte_count) })
    }

#[napi(js_name = "viewGridSetCellPath")]
    pub fn view_grid_set_cell_path(&self, base_root_ref: u32, path_ref: u32, path_depth: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, ancestor2_node_id_low: u32, ancestor2_node_id_high: u32, ancestor3_node_id_low: u32, ancestor3_node_id_high: u32, grid_row: u32, grid_column: u32, child_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_grid_set_cell_path_v1(runtime, base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, grid_row, grid_column, child_ref) })
    }

#[napi(js_name = "viewReleaseMany")]
    pub fn view_release_many(&self, refs: napi::bindgen_prelude::Uint32Array, used_ref_count: u32) -> napi::Result<i32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_release_many_v1(runtime, refs.as_ref().as_ptr() as *const u32, refs.as_ref().len().saturating_mul(4), used_ref_count) })
    }

#[napi(js_name = "viewRefForNodeId")]
    pub fn view_ref_for_node_id(&self, node_id_low: u32, node_id_high: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_ref_for_node_id_v1(runtime, node_id_low, node_id_high) })
    }

#[napi(js_name = "pathRoot")]
    pub fn path_root(&self) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_path_root_v1(runtime) })
    }

#[napi(js_name = "pathChild")]
    pub fn path_child(&self, parent_path_ref: u32, step_kind: u32, expected_view_kind: u32, selector: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_path_child_v1(runtime, parent_path_ref, step_kind, expected_view_kind, selector) })
    }

#[napi(js_name = "viewTextLayoutPatchPath")]
    pub fn view_text_layout_patch_path(&self, base_root_ref: u32, path_ref: u32, path_depth: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, ancestor2_node_id_low: u32, ancestor2_node_id_high: u32, ancestor3_node_id_low: u32, ancestor3_node_id_high: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_layout_patch_path_v1(runtime, base_root_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align) })
    }

#[napi(js_name = "viewTextLayoutPatchPathD1")]
    pub fn view_text_layout_patch_path_d1(&self, base_root_ref: u32, path_ref: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_layout_patch_path_d1_v1(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, wrap, align) })
    }

#[napi(js_name = "viewTextLayoutPatchPathD2")]
    pub fn view_text_layout_patch_path_d2(&self, base_root_ref: u32, path_ref: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_layout_patch_path_d2_v1(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, wrap, align) })
    }

#[napi(js_name = "viewTextLayoutPatchPathD3")]
    pub fn view_text_layout_patch_path_d3(&self, base_root_ref: u32, path_ref: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, ancestor2_node_id_low: u32, ancestor2_node_id_high: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_layout_patch_path_d3_v1(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, wrap, align) })
    }

#[napi(js_name = "viewTextLayoutPatchPathD4")]
    pub fn view_text_layout_patch_path_d4(&self, base_root_ref: u32, path_ref: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, ancestor2_node_id_low: u32, ancestor2_node_id_high: u32, ancestor3_node_id_low: u32, ancestor3_node_id_high: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_layout_patch_path_d4_v1(runtime, base_root_ref, path_ref, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align) })
    }

#[napi(js_name = "editTxnBegin")]
    pub fn edit_txn_begin(&self, base_root_ref: u32, expected_edit_count: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_edit_txn_begin_v1(runtime, base_root_ref, expected_edit_count) })
    }

#[napi(js_name = "editTxnAddTextLayout")]
    pub fn edit_txn_add_text_layout(&self, txn_ref: u32, path_ref: u32, path_depth: u32, target_node_id_low: u32, target_node_id_high: u32, ancestor0_node_id_low: u32, ancestor0_node_id_high: u32, ancestor1_node_id_low: u32, ancestor1_node_id_high: u32, ancestor2_node_id_low: u32, ancestor2_node_id_high: u32, ancestor3_node_id_low: u32, ancestor3_node_id_high: u32, wrap: u32, align: u32) -> napi::Result<i32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_edit_txn_add_text_layout_v1(runtime, txn_ref, path_ref, path_depth, target_node_id_low, target_node_id_high, ancestor0_node_id_low, ancestor0_node_id_high, ancestor1_node_id_low, ancestor1_node_id_high, ancestor2_node_id_low, ancestor2_node_id_high, ancestor3_node_id_low, ancestor3_node_id_high, wrap, align) })
    }

#[napi(js_name = "editTxnCommitRender")]
    pub fn edit_txn_commit_render(&self, host: &NativeTuiHost, txn_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_edit_txn_commit_render_v1(runtime, host as *const NativeTuiHost as *mut NativeHost, txn_ref) })
    }

#[napi(js_name = "editTxnAbort")]
    pub fn edit_txn_abort(&self, txn_ref: u32) -> napi::Result<i32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_edit_txn_abort_v1(runtime, txn_ref) })
    }

#[napi(js_name = "styleAtomCreateCstring")]
    pub fn style_atom_create_cstring(&self, value: String) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        let value_cstring = std::ffi::CString::new(value).map_err(|_| napi::Error::from_reason("value must not contain NUL"))?;
        Ok(unsafe { generated_exports::invoke_iyon_style_atom_create_cstring_v1(runtime, value_cstring.as_ptr()) })
    }

#[napi(js_name = "styleCreateBits")]
    pub fn style_create_bits(&self, flags: u32, attribute_present: u32, attribute_true: u32, foreground_ref: u32, background_ref: u32, theme_atom_ref: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_style_create_bits_v1(runtime, flags, attribute_present, attribute_true, foreground_ref, background_ref, theme_atom_ref) })
    }

#[napi(js_name = "viewTextCreateCstring")]
    pub fn view_text_create_cstring(&self, node_id_low: u32, node_id_high: u32, text: String, style_ref: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        let text_cstring = std::ffi::CString::new(text).map_err(|_| napi::Error::from_reason("text must not contain NUL"))?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_cstring_v1(runtime, node_id_low, node_id_high, text_cstring.as_ptr(), style_ref, wrap, align) })
    }

#[napi(js_name = "viewTextCreateUtf8")]
    pub fn view_text_create_utf8(&self, node_id_low: u32, node_id_high: u32, bytes: napi::bindgen_prelude::Uint8Array, used_bytes: u32, style_ref: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_utf8_v1(runtime, node_id_low, node_id_high, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_bytes, style_ref, wrap, align) })
    }

#[napi(js_name = "viewTextCreateUtf82")]
    pub fn view_text_create_utf8_2(&self, node_id_low: u32, node_id_high: u32, bytes: napi::bindgen_prelude::Uint8Array, used_bytes: u32, span0_bytes: u32, style0: u32, span1_bytes: u32, style1: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_utf8_2_v1(runtime, node_id_low, node_id_high, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_bytes, span0_bytes, style0, span1_bytes, style1, wrap, align) })
    }

#[napi(js_name = "viewTextCreateUtf83")]
    pub fn view_text_create_utf8_3(&self, node_id_low: u32, node_id_high: u32, bytes: napi::bindgen_prelude::Uint8Array, used_bytes: u32, span0_bytes: u32, style0: u32, span1_bytes: u32, style1: u32, span2_bytes: u32, style2: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_utf8_3_v1(runtime, node_id_low, node_id_high, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_bytes, span0_bytes, style0, span1_bytes, style1, span2_bytes, style2, wrap, align) })
    }

#[napi(js_name = "viewTextCreateUtf84")]
    pub fn view_text_create_utf8_4(&self, node_id_low: u32, node_id_high: u32, bytes: napi::bindgen_prelude::Uint8Array, used_bytes: u32, span0_bytes: u32, style0: u32, span1_bytes: u32, style1: u32, span2_bytes: u32, style2: u32, span3_bytes: u32, style3: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_utf8_4_v1(runtime, node_id_low, node_id_high, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_bytes, span0_bytes, style0, span1_bytes, style1, span2_bytes, style2, span3_bytes, style3, wrap, align) })
    }

#[napi(js_name = "viewTextCreateCstring2")]
    pub fn view_text_create_cstring_2(&self, node_id_low: u32, node_id_high: u32, text0: String, style0: u32, text1: String, style1: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        let text0_cstring = std::ffi::CString::new(text0).map_err(|_| napi::Error::from_reason("text0 must not contain NUL"))?;
        let text1_cstring = std::ffi::CString::new(text1).map_err(|_| napi::Error::from_reason("text1 must not contain NUL"))?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_cstring_2_v1(runtime, node_id_low, node_id_high, text0_cstring.as_ptr(), style0, text1_cstring.as_ptr(), style1, wrap, align) })
    }

#[napi(js_name = "viewTextCreateCstring3")]
    pub fn view_text_create_cstring_3(&self, node_id_low: u32, node_id_high: u32, text0: String, style0: u32, text1: String, style1: u32, text2: String, style2: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        let text0_cstring = std::ffi::CString::new(text0).map_err(|_| napi::Error::from_reason("text0 must not contain NUL"))?;
        let text1_cstring = std::ffi::CString::new(text1).map_err(|_| napi::Error::from_reason("text1 must not contain NUL"))?;
        let text2_cstring = std::ffi::CString::new(text2).map_err(|_| napi::Error::from_reason("text2 must not contain NUL"))?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_cstring_3_v1(runtime, node_id_low, node_id_high, text0_cstring.as_ptr(), style0, text1_cstring.as_ptr(), style1, text2_cstring.as_ptr(), style2, wrap, align) })
    }

#[napi(js_name = "viewTextCreateCstring4")]
    pub fn view_text_create_cstring_4(&self, node_id_low: u32, node_id_high: u32, text0: String, style0: u32, text1: String, style1: u32, text2: String, style2: u32, text3: String, style3: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        let text0_cstring = std::ffi::CString::new(text0).map_err(|_| napi::Error::from_reason("text0 must not contain NUL"))?;
        let text1_cstring = std::ffi::CString::new(text1).map_err(|_| napi::Error::from_reason("text1 must not contain NUL"))?;
        let text2_cstring = std::ffi::CString::new(text2).map_err(|_| napi::Error::from_reason("text2 must not contain NUL"))?;
        let text3_cstring = std::ffi::CString::new(text3).map_err(|_| napi::Error::from_reason("text3 must not contain NUL"))?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_cstring_4_v1(runtime, node_id_low, node_id_high, text0_cstring.as_ptr(), style0, text1_cstring.as_ptr(), style1, text2_cstring.as_ptr(), style2, text3_cstring.as_ptr(), style3, wrap, align) })
    }

#[napi(js_name = "viewTextCreateBuffer")]
    pub fn view_text_create_buffer(&self, node_id_low: u32, node_id_high: u32, words: napi::bindgen_prelude::Uint32Array, used_word_count: u32, bytes: napi::bindgen_prelude::Uint8Array, used_byte_count: u32, wrap: u32, align: u32) -> napi::Result<u32> {
        let runtime = self.runtime_ptr()?;
        Ok(unsafe { generated_exports::invoke_iyon_view_text_create_buffer_v1(runtime, node_id_low, node_id_high, words.as_ref().as_ptr() as *const u32, words.as_ref().len().saturating_mul(4), used_word_count, bytes.as_ref().as_ptr() as *const u8, bytes.as_ref().len().saturating_mul(1), used_byte_count, wrap, align) })
    }

#[napi(js_name = "u8_8")]
    pub fn u8_8(&self, a0: u8, a1: u8, a2: u8, a3: u8, a4: u8, a5: u8, a6: u8, a7: u8) -> napi::Result<u32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_u8_8_v1(a0, a1, a2, a3, a4, a5, a6, a7) })
    }

#[napi(js_name = "u16_8")]
    pub fn u16_8(&self, a0: u16, a1: u16, a2: u16, a3: u16, a4: u16, a5: u16, a6: u16, a7: u16) -> napi::Result<u32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_u16_8_v1(a0, a1, a2, a3, a4, a5, a6, a7) })
    }

#[napi(js_name = "u32_8")]
    pub fn u32_8(&self, a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32) -> napi::Result<u32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_u32_8_v1(a0, a1, a2, a3, a4, a5, a6, a7) })
    }

#[napi(js_name = "u32_16")]
    pub fn u32_16(&self, a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32, a9: u32, a10: u32, a11: u32, a12: u32, a13: u32, a14: u32, a15: u32) -> napi::Result<u32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_u32_16_v1(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15) })
    }

#[napi(js_name = "i32_4")]
    pub fn i32_4(&self, a0: i32, a1: i32, a2: i32, a3: i32) -> napi::Result<i32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_i32_4_v1(a0, a1, a2, a3) })
    }

#[napi(js_name = "f32_4")]
    pub fn f32_4(&self, a0: f64, a1: f64, a2: f64, a3: f64) -> napi::Result<f64> {
        Ok((unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_f32_4_v1(a0 as f32, a1 as f32, a2 as f32, a3 as f32) }) as f64)
    }

#[napi(js_name = "f64_4")]
    pub fn f64_4(&self, a0: f64, a1: f64, a2: f64, a3: f64) -> napi::Result<f64> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_f64_4_v1(a0, a1, a2, a3) })
    }

#[napi(js_name = "pointer")]
    pub fn pointer(&self, a0: bool) -> napi::Result<u32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_pointer_v1(if a0 { std::ptr::NonNull::<u8>::dangling().as_ptr() as *mut ::core::ffi::c_void } else { std::ptr::null_mut() }) })
    }

#[napi(js_name = "buffer")]
    pub fn buffer(&self, a0: napi::bindgen_prelude::Uint8Array) -> napi::Result<u32> {
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_buffer_v1(a0.as_ref().as_ptr(), a0.as_ref().len()) })
    }

#[napi(js_name = "cstring")]
    pub fn cstring(&self, a0: String) -> napi::Result<u32> {
        let a0_cstring = std::ffi::CString::new(a0).map_err(|_| napi::Error::from_reason("cstring must not contain NUL"))?;
        Ok(unsafe { super::generated_view_abi_conformance::invoke_iyon_abi_conformance_cstring_v1(a0_cstring.as_ptr()) })
    }

}
