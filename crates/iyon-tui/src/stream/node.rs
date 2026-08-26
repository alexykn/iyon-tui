//! Semantic stream nodes and static lowering.

use crate::presentation::{
    api::{IntoView, TextSpan},
    ir::{View, WidthRule},
};

use super::{
    StreamOffset, StreamRange,
    projected::{
        ExactTerminator, ProjectedText, ProjectedTextLayout, slice_projected_text,
        slice_projected_text_to,
    },
};

/// A semantic presentation node with truthful provenance.
///
/// Exact text is statically constrained to [`TextView`] plus an optional typed
/// structural terminator, making arbitrary hidden source unrepresentable.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StreamNode {
    Text(ProjectedText),
    /// Text chunks from one append-only source may be joined for visual
    /// compilation even when retention splits their semantic ownership.
    ContinuousText(ProjectedText),

    Atomic {
        range: StreamRange,
        view: View,
    },
}

impl StreamNode {
    pub(crate) fn projected_text(text: ProjectedText) -> Self {
        Self::Text(text)
    }

    pub(crate) fn exact_text(text_range: StreamRange, spans: Vec<TextSpan>) -> Self {
        Self::Text(ProjectedText::identity_with_terminator(
            text_range,
            ExactTerminator::None,
            spans,
        ))
    }

    pub(crate) fn continuous_exact_text(text_range: StreamRange, spans: Vec<TextSpan>) -> Self {
        Self::ContinuousText(ProjectedText::identity_with_terminator(
            text_range,
            ExactTerminator::None,
            spans,
        ))
    }

    pub(crate) fn exact_line(
        text_range: StreamRange,
        spans: Vec<TextSpan>,
        has_newline: bool,
    ) -> Self {
        Self::Text(ProjectedText::identity_with_terminator(
            text_range,
            if has_newline {
                ExactTerminator::HardNewline
            } else {
                ExactTerminator::None
            },
            spans,
        ))
    }

    pub(crate) fn atomic(range: StreamRange, view: View) -> Self {
        assert!(
            !view.contains_component_identity(),
            "stream atomic view cannot contain component identity"
        );
        Self::Atomic { range, view }
    }

    /// The full monotonic source range owned by this node (including any typed structural terminator).
    pub(crate) fn owned_range(&self) -> StreamRange {
        match self {
            Self::Text(text) | Self::ContinuousText(text) => text.owned_range(),
            Self::Atomic { range, .. } => *range,
        }
    }
}

/// V1 linear stream view: an ordered sequence of provenance-bearing semantic blocks.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct StreamView {
    pub(crate) nodes: Vec<StreamNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamSliceError {
    InvalidRange,
    IllegalCheckpoint,
    AtomicBoundary,
}

impl StreamView {
    pub(crate) fn new(nodes: Vec<StreamNode>) -> Self {
        Self { nodes }
    }

    /// Lowers the stream's exact/atomic presentation into the ordinary static
    /// view vocabulary without changing visible content. Structural source
    /// terminators remain provenance metadata and are intentionally not emitted.
    pub(crate) fn into_static_view(self) -> View {
        let mut children = Vec::new();
        let mut pending_continuous = None;
        for node in self.nodes {
            match node {
                StreamNode::ContinuousText(text) => {
                    if let Some(pending) = pending_continuous.as_mut()
                        && merge_continuous_text(pending, &text)
                    {
                        continue;
                    }
                    if let Some(pending) = pending_continuous.take() {
                        children.push(render_projected_text(pending));
                    }
                    pending_continuous = Some(text);
                }
                StreamNode::Text(text) => {
                    if let Some(pending) = pending_continuous.take() {
                        children.push(render_projected_text(pending));
                    }
                    children.push(render_projected_text(text));
                }
                StreamNode::Atomic { view, .. } => {
                    if let Some(pending) = pending_continuous.take() {
                        children.push(render_projected_text(pending));
                    }
                    children.push(view);
                }
            }
        }
        if let Some(pending) = pending_continuous {
            children.push(render_projected_text(pending));
        }

        View::vertical(|column| {
            column.children(children);
        })
    }

    #[cfg(test)]
    pub(crate) fn semantic_slice(&self, range: StreamRange) -> Result<Self, StreamSliceError> {
        semantic_slice_nodes(self.nodes.iter(), range)
    }

    pub(crate) fn suffix_from(&self, offset: StreamOffset) -> Self {
        let mut nodes = Vec::new();
        for node in &self.nodes {
            let range = node.owned_range();
            if range.end <= offset {
                continue;
            }
            if range.start >= offset {
                nodes.push(node.clone());
                continue;
            }
            match node {
                StreamNode::Text(text) => {
                    nodes.push(StreamNode::Text(slice_projected_text(text, offset)));
                }
                StreamNode::ContinuousText(text) => {
                    nodes.push(StreamNode::ContinuousText(slice_projected_text(
                        text, offset,
                    )));
                }
                StreamNode::Atomic { .. } => {
                    panic!("stream suffix cuts an indivisible atomic node")
                }
            }
        }
        Self::new(nodes)
    }

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Single exact text block.
    #[cfg(test)]
    pub(crate) fn exact_text(range: StreamRange, spans: Vec<TextSpan>) -> Self {
        Self {
            nodes: vec![StreamNode::exact_text(range, spans)],
        }
    }

    /// Single atomic view.
    #[cfg(test)]
    pub(crate) fn atomic(range: StreamRange, view: View) -> Self {
        Self {
            nodes: vec![StreamNode::atomic(range, view)],
        }
    }
}

fn merge_continuous_text(left: &mut ProjectedText, right: &ProjectedText) -> bool {
    if left.terminator != ExactTerminator::None
        || right.terminator != ExactTerminator::None
        || left.content_range.end != right.content_range.start
        || left.width != right.width
        || left.wrap != right.wrap
        || left.align != right.align
        || left.layout != right.layout
    {
        return false;
    }
    left.content_range.end = right.content_range.end;
    left.runs.extend(right.runs.iter().cloned());
    true
}

fn render_projected_text(text: ProjectedText) -> View {
    let body = View::styled_text(
        text.runs
            .into_iter()
            .filter(|run| !run.display.is_empty())
            .map(|run| TextSpan::styled(run.display, run.style).with_style_facts(run.style_facts)),
    );
    let body = match &text.layout {
        ProjectedTextLayout::Plain => match text.width {
            WidthRule::Fit => body.fit_width(),
            WidthRule::Fill => body.fill_width(),
        },
        ProjectedTextLayout::Hanging { .. } => body.fill_width(),
    };
    match text.layout {
        ProjectedTextLayout::Plain => body.into_view(),
        ProjectedTextLayout::Hanging {
            body_column,
            prefix,
            prefix_style,
            prefix_facts,
            show_prefix,
            ..
        } => View::horizontal(|row| {
            row.fixed(
                body_column,
                if show_prefix {
                    View::styled_text(vec![
                        TextSpan::styled(prefix, prefix_style).with_style_facts(prefix_facts),
                    ])
                    .no_wrap()
                } else {
                    View::text("").fill_width()
                },
            );
            row.flex(body);
        }),
    }
}

pub(crate) fn semantic_slice_nodes<'a>(
    nodes: impl Iterator<Item = &'a StreamNode>,
    range: StreamRange,
) -> Result<StreamView, StreamSliceError> {
    if range.start > range.end {
        return Err(StreamSliceError::InvalidRange);
    }
    let mut sliced_nodes = Vec::new();
    for node in nodes {
        let owned = node.owned_range();
        if owned.end <= range.start || owned.start >= range.end {
            continue;
        }
        if range.start <= owned.start && owned.end <= range.end {
            sliced_nodes.push(node.clone());
            continue;
        }
        let continuous = matches!(node, StreamNode::ContinuousText(_));
        let text = match node {
            StreamNode::Text(text) | StreamNode::ContinuousText(text) => text,
            StreamNode::Atomic { .. } => return Err(StreamSliceError::AtomicBoundary),
        };
        let start = range.start.max(owned.start);
        let end = range.end.min(owned.end);
        if start > text.content_range.start
            && !super::projected::projected_checkpoint_is_legal(text, start)
        {
            return Err(StreamSliceError::IllegalCheckpoint);
        }
        let sliced = if start > text.content_range.start {
            slice_projected_text(text, start)
        } else {
            text.clone()
        };
        let sliced = if end < sliced.owned_range().end {
            slice_projected_text_to(&sliced, end)
                .map_err(|_| StreamSliceError::IllegalCheckpoint)?
        } else {
            sliced
        };
        sliced_nodes.push(if continuous {
            StreamNode::ContinuousText(sliced)
        } else {
            StreamNode::Text(sliced)
        });
    }
    Ok(StreamView::new(sliced_nodes))
}
