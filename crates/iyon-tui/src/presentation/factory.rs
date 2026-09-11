//! Canonical internal retained-node factories.
//!
//! These functions are the only in-crate construction vocabulary used by
//! production lowering. They assemble the final retained record directly;
//! they are not a public builder facade.

use std::sync::Arc;

use super::api::text::{HorizontalAlign, WrapMode};
use super::ir::{
    ClampRowsView, ColumnChild, ColumnView, ContainerNode, Decoration, HangingView, HeightRule,
    PersistentSeq, RowChild, RowView, View, ViewKind, ViewNodeParts, WidthRule,
};
use super::{
    BorderSpec, ColorSpec, Insets, OverflowIndicator, StyleFacts, StyleRef, StyleSpec,
    StyleStateKey, StyleStateValue, StyleStates, TextAttribute, TextSpan, VerticalAlign,
};
use crate::component::{ComponentHandle, ComponentId};

pub(crate) fn text(value: impl Into<String>) -> View {
    text_from_spans(
        vec![TextSpan::plain(value)],
        WrapMode::default(),
        HorizontalAlign::Start,
    )
}

pub(crate) fn styled_text(spans: impl IntoIterator<Item = TextSpan>) -> View {
    text_from_spans(
        spans.into_iter().collect(),
        WrapMode::default(),
        HorizontalAlign::Start,
    )
}

pub(crate) fn text_from_spans(
    spans: Vec<TextSpan>,
    wrap: WrapMode,
    align: HorizontalAlign,
) -> View {
    text_from_spans_with_style(spans, wrap, align, StyleRef::default())
}

pub(crate) fn text_from_spans_with_style(
    spans: Vec<TextSpan>,
    wrap: WrapMode,
    align: HorizontalAlign,
    style: StyleRef,
) -> View {
    text_with_rules_cursor(
        spans,
        wrap,
        align,
        style,
        WidthRule::Fit,
        HeightRule::Fit,
        None,
    )
}

fn text_with_rules(
    spans: Vec<TextSpan>,
    wrap: WrapMode,
    align: HorizontalAlign,
    style: StyleRef,
    width: WidthRule,
    height: HeightRule,
) -> View {
    text_with_rules_cursor(spans, wrap, align, style, width, height, None)
}

fn text_with_rules_cursor(
    spans: Vec<TextSpan>,
    wrap: WrapMode,
    align: HorizontalAlign,
    style: StyleRef,
    width: WidthRule,
    height: HeightRule,
    cursor: Option<usize>,
) -> View {
    let mut decoration = Decoration::default();
    decoration.text_style = style;
    View::from_node(ViewNodeParts {
        width,
        height,
        decoration,
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Text(Arc::new(super::ir::TextView {
            spans: spans.into(),
            wrap,
            align,
            cursor: cursor.map(|byte_offset| super::ir::TextCursorAnchor { byte_offset }),
        })),
    })
}

pub(crate) fn text_with_style(
    value: impl Into<String>,
    wrap: WrapMode,
    align: HorizontalAlign,
    style: impl Into<StyleRef>,
) -> View {
    text_with_rules(
        vec![TextSpan::plain(value)],
        wrap,
        align,
        style.into(),
        WidthRule::Fit,
        HeightRule::Fit,
    )
}

pub(crate) fn text_with_style_fill(
    value: impl Into<String>,
    wrap: WrapMode,
    align: HorizontalAlign,
    style: impl Into<StyleRef>,
) -> View {
    text_with_rules(
        vec![TextSpan::plain(value)],
        wrap,
        align,
        style.into(),
        WidthRule::Fill,
        HeightRule::Fit,
    )
}

pub(crate) fn text_with_cursor(
    value: impl Into<String>,
    wrap: WrapMode,
    align: HorizontalAlign,
    byte_offset: usize,
) -> View {
    text_with_rules_cursor(
        vec![TextSpan::plain(value)],
        wrap,
        align,
        StyleRef::default(),
        WidthRule::Fit,
        HeightRule::Fit,
        Some(byte_offset),
    )
}

pub(crate) fn styled_text_fill(
    spans: impl IntoIterator<Item = TextSpan>,
    wrap: WrapMode,
    align: HorizontalAlign,
) -> View {
    text_with_rules(
        spans.into_iter().collect(),
        wrap,
        align,
        StyleRef::default(),
        WidthRule::Fill,
        HeightRule::Fit,
    )
}

pub(crate) fn row(children: Vec<View>, gap: u16) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Row(Arc::new(RowView {
            children: PersistentSeq::from_vec(
                children.into_iter().map(RowChild::content).collect(),
            ),
            gap,
            vertical_align: VerticalAlign::Top,
        })),
    })
}

pub(crate) fn row_specs<V: Into<View>>(
    children: Vec<(super::ir::TrackSize, V)>,
    gap: u16,
    vertical_align: VerticalAlign,
) -> View {
    let children = children
        .into_iter()
        .map(|(track, view)| RowChild {
            track,
            view: view.into(),
        })
        .collect();
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Row(Arc::new(RowView {
            children: PersistentSeq::from_vec(children),
            gap,
            vertical_align,
        })),
    })
}

pub(crate) fn column(children: Vec<View>, gap: u16) -> View {
    column_with_rules(children, gap, WidthRule::Fit, HeightRule::Fit)
}

fn column_with_rules(children: Vec<View>, gap: u16, width: WidthRule, height: HeightRule) -> View {
    View::from_node(ViewNodeParts {
        width,
        height,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(
                children.into_iter().map(ColumnChild::content).collect(),
            ),
            gap,
        })),
    })
}

pub(crate) fn column_specs<V: Into<View>>(
    children: Vec<(super::ir::TrackSize, V)>,
    gap: u16,
) -> View {
    let children = children
        .into_iter()
        .map(|(track, view)| ColumnChild {
            track,
            view: view.into(),
        })
        .collect();
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(children),
            gap,
        })),
    })
}

pub(crate) fn column_persistent(children: PersistentSeq<ColumnChild>, gap: u16) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Column(Arc::new(ColumnView { children, gap })),
    })
}

pub(crate) fn grid<V: Into<View>>(
    columns: Vec<super::api::grid::GridTrack>,
    column_gap: u16,
    row_gap: u16,
    rows: Vec<(
        super::api::grid::GridTrack,
        Vec<(super::api::grid::GridCellSpec, V)>,
    )>,
) -> View {
    View::grid_from_parts(
        columns,
        column_gap,
        row_gap,
        rows.into_iter()
            .map(|(track, cells)| {
                (
                    track,
                    cells
                        .into_iter()
                        .map(|(spec, view)| (spec, view.into()))
                        .collect(),
                )
            })
            .collect(),
    )
}

pub(crate) fn grid_cell_spec_new() -> super::api::grid::GridCellSpec {
    super::api::grid::GridCellSpec::new()
}

pub(crate) fn hanging(prefix: View, continuation_prefix: View, body: View) -> View {
    assert!(
        !continuation_prefix.contains_component_identity(),
        "hanging continuation_prefix cannot contain component identity: it is repeated"
    );
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Hanging(Arc::new(HangingView {
            prefix,
            continuation_prefix,
            body,
        })),
    })
}

pub(crate) fn spacer(rows: u16) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Spacer { rows },
    })
}

pub(crate) fn content_host(port_id: u64) -> Result<View, String> {
    if port_id == 0 {
        return Err("ContentPort identity must be positive".to_owned());
    }
    Ok(View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: Some(port_id),
        kind: ViewKind::ContentHost,
    }))
}

pub(crate) fn native_component(raw_id: u64) -> View {
    assert!(raw_id != 0, "native component identity must be non-zero");
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::ComponentSlot(super::ir::ComponentSlotNode {
            id: ComponentId::from_raw(raw_id),
        }),
    })
}

pub(crate) fn component<C>(handle: ComponentHandle<C>) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fit,
        height: HeightRule::Fit,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::ComponentSlot(super::ir::ComponentSlotNode { id: handle.id() }),
    })
}

pub(crate) fn container(view: View) -> View {
    let parts = ViewNodeParts::from_view(&view);
    View::from_node(ViewNodeParts {
        width: parts.width,
        height: parts.height,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::Container(Arc::new(ContainerNode { child: view })),
    })
}

pub(crate) fn clamp_rows(view: View, max_rows: u16, overflow: OverflowIndicator) -> View {
    let parts = ViewNodeParts::from_view(&view);
    View::from_node(ViewNodeParts {
        width: parts.width,
        height: parts.height,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::ClampRows(Arc::new(ClampRowsView {
            child: view,
            max_rows,
            overflow,
        })),
    })
}

pub(crate) fn row_viewport(child: View, skip_rows: u16, visible_height: Option<u16>) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fill,
        height: HeightRule::Fill,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::RowViewport(Arc::new(super::ir::RowViewportView {
            child,
            skip_rows,
            visible_height,
            layout_height: None,
            intrinsic_content_height: visible_height.is_none(),
        })),
    })
}

pub(crate) fn row_viewport_default(child: View, skip_rows: u16) -> View {
    row_viewport(child, skip_rows, None)
}

pub(crate) fn bounded_row_viewport(child: View, height: u16) -> View {
    View::from_node(ViewNodeParts {
        width: WidthRule::Fill,
        height: HeightRule::Fill,
        decoration: Decoration::default(),
        style_states: StyleStates::default(),
        style_facts: StyleFacts::default(),
        content_attachment: None,
        kind: ViewKind::RowViewport(Arc::new(super::ir::RowViewportView {
            child,
            skip_rows: 0,
            visible_height: Some(height),
            layout_height: Some(height),
            intrinsic_content_height: false,
        })),
    })
}

pub(crate) fn style(view: View, style: impl Into<StyleRef>) -> View {
    let style = style.into();
    view.map_node(|node| {
        if style.theme.is_some() {
            node.decoration.text_style = style;
        } else {
            node.decoration.text_style.overlay(&style.local);
        }
    })
}

pub(crate) fn wrap(view: View, wrap: WrapMode, align: Option<HorizontalAlign>) -> View {
    view.map_text(|text| {
        text.wrap = wrap;
        if let Some(align) = align {
            text.align = align;
        }
    })
}

pub(crate) fn cursor_at(view: View, byte_offset: usize) -> View {
    view.map_text(|text| {
        text.cursor = Some(super::ir::TextCursorAnchor { byte_offset });
    })
}

pub(crate) fn fill_width(view: View) -> View {
    view.map_node(|node| node.width = WidthRule::Fill)
}

pub(crate) fn fill_height(view: View) -> View {
    view.map_node(|node| node.height = HeightRule::Fill)
}

pub(crate) fn fit_width(view: View) -> View {
    view.map_node(|node| node.width = WidthRule::Fit)
}

pub(crate) fn fit_height(view: View) -> View {
    view.map_node(|node| node.height = HeightRule::Fit)
}

pub(crate) fn padding(view: View, padding: impl Into<Insets>) -> View {
    view.map_node(|node| node.decoration.padding = padding.into())
}

pub(crate) fn background(view: View, color: ColorSpec) -> View {
    view.map_node(|node| node.decoration.surface_background = Some(color))
}

pub(crate) fn foreground(view: View, color: ColorSpec) -> View {
    view.map_node(|node| {
        node.decoration
            .text_style
            .overlay(&StyleSpec::new().foreground(color));
    })
}

pub(crate) fn border(view: View, border: BorderSpec) -> View {
    view.map_node(|node| node.decoration.border = Some(border))
}

pub(crate) fn text_attribute(view: View, attribute: TextAttribute, enabled: bool) -> View {
    view.map_node(|node| {
        node.decoration
            .text_style
            .overlay(&StyleSpec::new().attribute(attribute, enabled));
    })
}

pub(crate) fn style_states(
    view: View,
    states: impl IntoIterator<Item = (StyleStateKey, StyleStateValue)>,
) -> View {
    view.map_node(|node| {
        for (key, value) in states {
            node.style_states.set(key, value);
        }
    })
}

pub(crate) fn with_style_facts(view: View, facts: StyleFacts) -> View {
    view.map_node(|node| node.style_facts = facts)
}

pub(crate) fn style_fact(
    view: View,
    key: impl Into<StyleStateKey>,
    value: impl Into<StyleStateValue>,
) -> View {
    view.map_node(|node| node.style_facts.set(key, value))
}

pub(crate) fn style_state(
    view: View,
    key: impl Into<StyleStateKey>,
    value: impl Into<StyleStateValue>,
) -> View {
    view.map_node(|node| node.style_states.set(key, value))
}

pub(crate) fn min_width(view: View, width: u16) -> View {
    view.map_node(|node| node.decoration.bounds.width.min = width)
}

pub(crate) fn max_width(view: View, width: u16) -> View {
    view.map_node(|node| node.decoration.bounds.width.max = width)
}

pub(crate) fn min_height(view: View, height: u16) -> View {
    view.map_node(|node| node.decoration.bounds.height.min = height)
}

pub(crate) fn max_height(view: View, height: u16) -> View {
    view.map_node(|node| node.decoration.bounds.height.max = height)
}
