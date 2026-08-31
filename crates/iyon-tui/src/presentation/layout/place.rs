//! Placement of prepared geometry into the retained layout tree.

use super::{
    measure::MeasuredKind,
    prepare::{PreparedChild, PreparedKind, PreparedNode},
    tree::{ChildDependency, LayoutContent, LayoutNode, LayoutNodeId, LayoutStyle},
};
use crate::{
    geometry::{Point, Rect},
    perf::{self, Counter},
    retained_state::{OccurrenceBox, state_node_kind},
};

pub(super) fn emit_prepared(
    prepared: &PreparedNode,
    origin: Point,
    inherited_clip: Rect,
    nodes: &mut Vec<LayoutNode>,
) -> LayoutNodeId {
    perf::inc(Counter::LayoutNodesEmitted);
    #[cfg(test)]
    super::record_emitted_node();
    let rect = Rect::new(
        origin.x,
        origin.y,
        prepared.size.width,
        prepared.size.height,
    );
    let node_clip = inherited_clip.intersection(rect).unwrap_or(Rect::new(
        inherited_clip.x,
        inherited_clip.y,
        0,
        0,
    ));
    let content_origin = Point {
        x: origin.x.saturating_add(prepared.content_offset_x),
        y: origin.y.saturating_add(prepared.content_offset_y),
    };
    let content_rect = Rect::new(
        content_origin.x,
        content_origin.y,
        prepared.core_size.width,
        prepared.core_size.height,
    )
    .intersection(rect)
    .unwrap_or(Rect::new(rect.x, rect.y, 0, 0));
    let content = layout_content(prepared);
    let child_clip = match prepared.kind {
        PreparedKind::RowViewport { .. } => {
            Rect::new(inherited_clip.x, 0, inherited_clip.width, u16::MAX)
        }
        _ => node_clip,
    };
    let id = LayoutNodeId(nodes.len());
    nodes.push(LayoutNode {
        view_id: prepared
            .measured
            .key
            .component_view
            .unwrap_or_else(|| prepared.measured.view.id()),
        paint_cacheable: prepared.measured.cacheable,
        occurrence: OccurrenceBox::from_effective(
            prepared.measured.view.state_attachment_id(),
            state_node_kind(prepared.measured.view.kind()),
            prepared.measured.view.width(),
            prepared.measured.view.height(),
            prepared.measured.base_gap,
            prepared.measured.base_alignment,
            prepared.measured.view.decoration().clone(),
            crate::retained_state::EffectiveGeometry {
                width: prepared.measured.width,
                height: prepared.measured.height,
                decoration: prepared.measured.effective_decoration.clone(),
                gap: prepared.measured.effective_gap,
                alignment: prepared.measured.effective_alignment,
            },
            prepared.measured.view.view_style_states().clone(),
            prepared.measured.effective_style_states.clone(),
        ),
        rect,
        content_rect,
        clip_rect: node_clip,
        component: prepared.measured.component,
        children: Vec::new(),
        child_dependencies: Vec::new(),
        style: LayoutStyle {
            component_scope: prepared.measured.component_scope,
            style_states: prepared.measured.effective_style_states.clone(),
            style_facts: prepared.measured.view.view_style_facts().clone(),
            decoration: prepared.measured.effective_decoration.clone(),
        },
        content,
    });
    let children = match &prepared.kind {
        PreparedKind::Leaf => Vec::new(),
        PreparedKind::Children(children) => {
            emit_children(children, content_origin, child_clip, nodes)
        }
        PreparedKind::Clamp { child } => emit_child(child, content_origin, child_clip, nodes),
        PreparedKind::RowViewport { child, .. } => {
            emit_child(child, content_origin, child_clip, nodes)
        }
    };
    nodes[id.0].children = children;
    let child_count = nodes[id.0].children.len();
    nodes[id.0].child_dependencies = child_dependencies(&prepared.measured.kind, child_count);
    debug_assert_eq!(
        nodes[id.0].children.len(),
        nodes[id.0].child_dependencies.len()
    );
    id
}

fn child_dependencies(kind: &MeasuredKind, child_count: usize) -> Vec<ChildDependency> {
    match kind {
        MeasuredKind::Container { .. }
        | MeasuredKind::ClampRows { .. }
        | MeasuredKind::Hanging { .. }
        | MeasuredKind::Grid { .. } => vec![ChildDependency::all(); child_count],
        MeasuredKind::Column { children, .. } => children
            .iter()
            .map(|child| {
                ChildDependency::new(
                    true,
                    !matches!(child.track, crate::presentation::ir::TrackSize::Fixed(_)),
                    true,
                    !matches!(child.track, crate::presentation::ir::TrackSize::Fixed(_)),
                )
            })
            .collect(),
        MeasuredKind::Row { children, .. } => {
            // Row content allocation probes child intrinsic widths during
            // measurement even when the eventual child rule is Fill. Keep
            // the dependency conservative until the track metadata is
            // carried into this retained record.
            vec![ChildDependency::all(); children.len()]
        }
        MeasuredKind::RowViewport {
            intrinsic_content_height,
            ..
        } => vec![ChildDependency::new(
            false,
            *intrinsic_content_height,
            true,
            true,
        )],
        MeasuredKind::Text { .. } | MeasuredKind::Spacer { .. } => Vec::new(),
    }
}

fn emit_children(
    children: &[PreparedChild],
    origin: Point,
    clip: Rect,
    nodes: &mut Vec<LayoutNode>,
) -> Vec<LayoutNodeId> {
    children
        .iter()
        .map(|child| {
            emit_prepared(
                &child.node,
                Point {
                    x: origin.x.saturating_add(child.x),
                    y: origin.y.saturating_add(child.y),
                },
                clip,
                nodes,
            )
        })
        .collect()
}

fn emit_child(
    child: &PreparedChild,
    origin: Point,
    clip: Rect,
    nodes: &mut Vec<LayoutNode>,
) -> Vec<LayoutNodeId> {
    vec![emit_prepared(
        &child.node,
        Point {
            x: origin.x.saturating_add(child.x),
            y: origin.y.saturating_add(child.y),
        },
        clip,
        nodes,
    )]
}

fn layout_content(prepared: &PreparedNode) -> LayoutContent {
    match (&prepared.measured.kind, &prepared.kind) {
        (MeasuredKind::Text { text, .. }, _) => LayoutContent::Text {
            text: (**text).clone(),
            width_rule: prepared.measured.width,
        },
        (MeasuredKind::Spacer { rows }, _) => LayoutContent::Spacer { rows: *rows },
        (MeasuredKind::ClampRows { overflow, .. }, PreparedKind::Clamp { .. }) => {
            LayoutContent::Clamp {
                overflow: (*overflow).clone(),
            }
        }
        (_, PreparedKind::RowViewport { skip_rows, .. }) => LayoutContent::RowViewport {
            skip_rows: *skip_rows,
        },
        _ => LayoutContent::Children,
    }
}
