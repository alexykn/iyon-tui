//! Structured validation for stream snapshots and projected nodes.

use std::fmt;

use super::{
    node::StreamNode,
    projected::{ProjectedText, ProjectedTextLayout},
    snapshot::StreamSnapshot,
};

/// Public validation failures for externally constructed stream snapshots.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamValidationError {
    InvalidFrontier,
    FirstNodeDoesNotStartAtBase,
    GapOrOverlap,
    NodeBeyondSourceEnd,
    TrailingUncoveredSource,
    Projected(ProjectedValidationError),
    AtomicContainsComponent,
}

/// Public failures in projected source/display mapping.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedValidationError {
    InvalidContentRange,
    InvalidHangingPrefix,
    NonContiguousRun,
    EmptyRun,
    RunBeyondContent,
    InvalidVisibleRange,
    VisibleLengthMismatch,
    IncompleteSourceCoverage,
    HangingWidthMismatch,
}

impl fmt::Display for StreamValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid stream snapshot: {self:?}")
    }
}

impl std::error::Error for StreamValidationError {}

impl fmt::Display for ProjectedValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid projected text: {self:?}")
    }
}

impl std::error::Error for ProjectedValidationError {}

impl StreamSnapshot {
    pub(crate) fn validate(&self) -> Result<(), StreamValidationError> {
        if self.source_base > self.stable_through || self.stable_through > self.source_end {
            return Err(StreamValidationError::InvalidFrontier);
        }

        let mut expected = self.source_base;
        for (index, node) in self.view.nodes.iter().enumerate() {
            let owned = node.owned_range();
            if owned.start != expected {
                return Err(if index == 0 {
                    StreamValidationError::FirstNodeDoesNotStartAtBase
                } else {
                    StreamValidationError::GapOrOverlap
                });
            }
            if owned.end > self.source_end {
                return Err(StreamValidationError::NodeBeyondSourceEnd);
            }
            match node {
                StreamNode::Text(text) | StreamNode::ContinuousText(text) => {
                    validate_projected_text(text).map_err(StreamValidationError::Projected)?
                }
                StreamNode::Atomic { view, .. } if view.contains_component_identity() => {
                    return Err(StreamValidationError::AtomicContainsComponent);
                }
                StreamNode::Atomic { .. } => {}
            }
            expected = owned.end;
        }
        if expected != self.source_end {
            return Err(StreamValidationError::TrailingUncoveredSource);
        }
        Ok(())
    }
}

pub(crate) fn validate_projected_text(
    text: &ProjectedText,
) -> Result<(), ProjectedValidationError> {
    if text.content_range.start > text.content_range.end {
        return Err(ProjectedValidationError::InvalidContentRange);
    }
    let mut expected = match &text.layout {
        ProjectedTextLayout::Plain => text.content_range.start,
        ProjectedTextLayout::Hanging {
            body_column,
            prefix,
            prefix_source,
            show_prefix,
            ..
        } => {
            if prefix_source.start > prefix_source.end {
                return Err(ProjectedValidationError::InvalidHangingPrefix);
            }
            if *show_prefix {
                if prefix_source.start != text.content_range.start
                    || prefix_source.end > text.content_range.end
                {
                    return Err(ProjectedValidationError::InvalidHangingPrefix);
                }
                if crate::physical::text_cell_width(prefix.as_str()) != usize::from(*body_column) {
                    return Err(ProjectedValidationError::HangingWidthMismatch);
                }
                prefix_source.end
            } else {
                if prefix_source.end > text.content_range.start {
                    return Err(ProjectedValidationError::InvalidHangingPrefix);
                }
                text.content_range.start
            }
        }
    };

    for run in &text.runs {
        if run.owned.start != expected {
            return Err(ProjectedValidationError::NonContiguousRun);
        }
        if run.owned.start >= run.owned.end {
            return Err(ProjectedValidationError::EmptyRun);
        }
        if run.owned.end > text.content_range.end {
            return Err(ProjectedValidationError::RunBeyondContent);
        }
        if let Some(visible) = run.exact_visible {
            if visible.start < run.owned.start
                || visible.end > run.owned.end
                || visible.start > visible.end
            {
                return Err(ProjectedValidationError::InvalidVisibleRange);
            }
            if run.display.len() != visible.len() as usize {
                return Err(ProjectedValidationError::VisibleLengthMismatch);
            }
        }
        expected = run.owned.end;
    }
    if expected != text.content_range.end {
        return Err(ProjectedValidationError::IncompleteSourceCoverage);
    }
    Ok(())
}
