//! Private presentation boundaries for semantic construction and lowering.
//!
//! `api` is the curated semantic construction facade. `ir` is the private
//! retained semantic tree. `layout` owns ordinary semantic compilation into
//! physical rows, `paint` resolves styles and decoration, and `wrap` owns
//! Unicode wrapping. Generic stream provenance is a sibling `stream` subsystem.

#[doc(hidden)]
pub mod api;
pub(crate) mod content;
#[doc(hidden)]
pub mod factory;
pub(crate) mod ir;
pub(crate) mod layout;
pub(crate) mod paint;
pub(crate) mod wrap;

/// The native crate's deliberately unsupported retained-value seam. The
/// public path is hidden from documentation; application code must use the
/// TypeScript facade instead of these opaque handles and operations.
#[doc(hidden)]
pub mod binding {
    #[cfg(feature = "native-host")]
    pub fn view_text(spans: Vec<TextSpan>, wrap: WrapMode, align: HorizontalAlign) -> View {
        super::factory::text_from_spans(spans, wrap, align)
    }

    #[cfg(feature = "native-host")]
    pub fn view_text_plain(text: impl Into<String>) -> View {
        view_text(
            vec![TextSpan::plain(text)],
            WrapMode::default(),
            HorizontalAlign::Start,
        )
    }

    #[cfg(feature = "native-host")]
    pub fn view_styled_text(spans: Vec<TextSpan>) -> View {
        view_text(spans, WrapMode::default(), HorizontalAlign::Start)
    }

    #[cfg(feature = "native-host")]
    pub fn view_spacer(rows: u16) -> View {
        crate::presentation::factory::spacer(rows)
    }

    #[cfg(feature = "native-host")]
    pub fn view_clamp_rows(view: View, max_rows: u16, overflow: OverflowIndicator) -> View {
        super::factory::clamp_rows(view, max_rows, overflow)
    }

    #[cfg(feature = "native-host")]
    pub fn view_hanging(prefix: View, continuation_prefix: View, body: View) -> View {
        crate::presentation::factory::hanging(prefix, continuation_prefix, body)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_component(raw_id: u64) -> View {
        crate::presentation::factory::native_component(raw_id)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_content_host(port_id: u64) -> Result<View, String> {
        crate::presentation::factory::content_host(port_id)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_text_final(
        spans: Vec<TextSpan>,
        wrap: WrapMode,
        align: HorizontalAlign,
    ) -> View {
        super::factory::text_from_spans(spans, wrap, align)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_grid_final(
        columns: Vec<GridTrack>,
        column_gap: u16,
        row_gap: u16,
        rows: Vec<(GridTrack, Vec<(GridCellSpec, View)>)>,
    ) -> View {
        super::factory::grid(columns, column_gap, row_gap, rows)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_axis_from_children(
        horizontal: bool,
        gap: u16,
        children: Vec<(u32, View)>,
    ) -> Result<View, String> {
        View::native_axis_from_children(horizontal, gap, children)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_axis_set_child(
        base: View,
        index: usize,
        track_word: u32,
        child: View,
    ) -> Result<View, String> {
        base.native_axis_set_child(index, track_word, child)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_axis_splice(
        base: View,
        index: usize,
        remove_count: usize,
        inserted: Vec<(u32, View)>,
    ) -> Result<View, String> {
        base.native_axis_splice(index, remove_count, inserted)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_grid_set_cell(
        base: View,
        row: usize,
        column: usize,
        child: View,
    ) -> Result<View, String> {
        base.native_grid_set_cell(row, column, child)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_replace_at_path(
        base: View,
        steps: &[RetainedPathStep],
        axis_index: Option<usize>,
        track_word: u32,
        grid_row: Option<usize>,
        grid_column: Option<usize>,
        child: View,
    ) -> Result<(View, Vec<View>), String> {
        base.native_replace_at_path(steps, axis_index, track_word, grid_row, grid_column, child)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_patched(base: View, patch: &super::api::NativeCommonPatch) -> View {
        super::factory::native_patched(base, patch)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_container(base: View) -> View {
        super::factory::container(base)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_state_attachment_id(view: &View) -> Option<u64> {
        view.native_state_attachment_id()
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_state_capable(view: &View) -> bool {
        view.native_state_capable()
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_state_attachment_ids(view: &View) -> Result<Vec<u64>, String> {
        view.native_state_attachment_ids()
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_with_state_attachment(view: View, state_id: u64) -> Result<View, String> {
        view.native_with_state_attachment(state_id)
    }

    #[cfg(feature = "native-host")]
    pub fn view_native_with_content_attachment(view: View, port_id: u64) -> Result<View, String> {
        view.native_with_content_attachment(port_id)
    }

    #[cfg(feature = "native-host")]
    pub fn view_downgrade(view: &View) -> WeakView {
        view.downgrade()
    }

    #[cfg(feature = "native-host")]
    pub fn view_upgrade(view: &WeakView) -> Option<View> {
        view.upgrade()
    }

    #[cfg(feature = "native-host")]
    pub fn view_try_with_text_layout_patch(
        view: View,
        wrap: Option<WrapMode>,
        align: Option<HorizontalAlign>,
    ) -> Result<View, &'static str> {
        view.try_with_text_layout_patch(wrap, align)
    }

    #[cfg(feature = "native-host")]
    pub fn view_try_with_text_layout_patch_path(
        view: View,
        steps: &[RetainedPathStep],
        wrap: WrapMode,
        align: HorizontalAlign,
    ) -> Result<View, String> {
        view.try_with_text_layout_patch_path(steps, wrap, align)
    }

    #[cfg(feature = "native-host")]
    pub fn view_try_with_text_layout_patch_path_with_nodes(
        view: View,
        steps: &[RetainedPathStep],
        wrap: WrapMode,
        align: HorizontalAlign,
    ) -> Result<(View, Vec<View>), String> {
        view.try_with_text_layout_patch_path_with_nodes(steps, wrap, align)
    }

    #[cfg(feature = "native-host")]
    pub fn view_try_retained_child(view: &View, step: RetainedPathStep) -> Result<View, String> {
        view.try_retained_child(step)
    }

    #[cfg(feature = "native-host")]
    pub fn view_try_replace_retained_children(
        view: View,
        replacements: &[(RetainedPathStep, View)],
    ) -> Result<View, String> {
        view.try_replace_retained_children(replacements)
    }

    #[allow(unused_imports)]
    pub use super::api::grid::{GridCellSpec, GridTrack};
    #[allow(unused_imports)]
    pub use super::api::style::{
        AnsiColor, BorderEdges, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec, Insets,
        OverflowIndicator, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
        TextAttribute, TextAttributeSpec, ThemeColor, ThemeKey, VerticalAlign,
    };
    #[cfg(feature = "native-host")]
    #[allow(unused_imports)]
    pub use super::api::text::NativeTextPage;
    #[allow(unused_imports)]
    pub use super::api::text::{HorizontalAlign, TextSpan, WrapMode};
    #[cfg(feature = "native-host")]
    #[allow(unused_imports)]
    pub use super::api::view::NativeCommonPatch;
    #[cfg(feature = "native-host")]
    #[allow(unused_imports)]
    pub use super::ir::{RetainedPathStep, View, WeakView};

    pub fn grid_track_content() -> GridTrack {
        GridTrack::content()
    }

    pub fn grid_track_content_max(max: u16) -> GridTrack {
        GridTrack::content_max(max)
    }

    pub fn grid_track_fixed(size: u16) -> GridTrack {
        GridTrack::fixed(size)
    }

    pub fn grid_track_flex() -> GridTrack {
        GridTrack::flex()
    }

    pub fn grid_track_flex_max(max: u16) -> GridTrack {
        GridTrack::flex_max(max)
    }

    pub fn grid_cell_spec_new() -> GridCellSpec {
        GridCellSpec::new()
    }

    pub fn grid_cell_spec_column_span(spec: GridCellSpec, span: u16) -> GridCellSpec {
        spec.column_span(span)
    }

    pub fn grid_cell_spec_row_span(spec: GridCellSpec, span: u16) -> GridCellSpec {
        spec.row_span(span)
    }

    pub fn grid_cell_spec_horizontal_align(
        spec: GridCellSpec,
        align: HorizontalAlign,
    ) -> GridCellSpec {
        spec.horizontal_align(align)
    }

    pub fn grid_cell_spec_vertical_align(
        spec: GridCellSpec,
        align: super::api::style::VerticalAlign,
    ) -> GridCellSpec {
        spec.vertical_align(align)
    }

    pub fn text_span_plain(text: impl Into<String>) -> TextSpan {
        TextSpan::plain(text)
    }

    pub fn text_span_styled(
        text: impl Into<String>,
        style: impl Into<super::api::style::StyleRef>,
    ) -> TextSpan {
        TextSpan::styled(text, style)
    }
}

#[allow(unused_imports)]
pub(crate) use api::{
    BorderEdges, BorderGlyphError, BorderGlyphs, BorderSpec, BorderStyle, ColorSpec, GridCellSpec,
    GridTrack, HorizontalAlign, Insets, OverflowIndicator, StyleRef, StyleSpec, StyleStateKey,
    StyleStateValue, TextAttribute, TextAttributeSpec, TextSpan, ThemeKey, VerticalAlign, View,
    WrapMode,
};
pub(crate) use api::{StyleFacts, StyleStates};

// Retained IR types remain private implementation details of the semantic
// layout engine.
pub(crate) use content::{
    ContentDirty, ContentDirtyReason, ContentMeasurement, ContentProvider, ContentWindow,
    EmptyContentProvider, HistoryContentRows, PreparedProjectionTicket,
};
pub(crate) use ir::WidthRule;
