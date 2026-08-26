//! Placement of prepared geometry into the retained layout tree.

use super::{
    measure::MeasuredKind,
    prepare::{PreparedChild, PreparedKind, PreparedNode},
    tree::{LayoutContent, LayoutNode, LayoutNodeId, LayoutStyle},
};
use crate::{
    geometry::{Point, Rect},
    perf::{self, Counter},
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
        rect,
        content_rect,
        clip_rect: node_clip,
        component: prepared.measured.component,
        children: Vec::new(),
        style: LayoutStyle {
            component_scope: prepared.measured.component_scope,
            style_states: prepared.measured.view.view_style_states().clone(),
            style_facts: prepared.measured.view.view_style_facts().clone(),
            decoration: prepared.measured.view.decoration().clone(),
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
    id
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
            width_rule: prepared.measured.view.width(),
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
