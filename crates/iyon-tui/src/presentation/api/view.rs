//! Semantic View construction and the open conversion boundary.

use std::sync::Arc;

use super::style::{StyleFacts, StyleRef, StyleStates};
use super::text::{HorizontalAlign, TextSpan, WrapMode};
use crate::presentation::ir::{Decoration, HeightRule, View, ViewKind, ViewNodeParts, WidthRule};

impl View {
    pub(crate) fn new_kind(kind: ViewKind) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fit,
            height: HeightRule::Fit,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            content_attachment: None,
            kind,
        })
    }

    /// Internal final text constructor used by semantic content lowering.
    /// Common text metadata and sparse style are installed before the single
    /// retained root allocation; source-backed spans remain range views.
    pub(crate) fn text_from_spans(
        spans: Vec<TextSpan>,
        wrap: WrapMode,
        align: HorizontalAlign,
        style: StyleRef,
    ) -> Self {
        let mut decoration = Decoration::default();
        decoration.text_style = style;
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fit,
            height: HeightRule::Fit,
            decoration,
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            content_attachment: None,
            kind: ViewKind::Text(Arc::new(crate::presentation::ir::TextView {
                spans: spans.into(),
                wrap,
                align,
                cursor: None,
            })),
        })
    }

    /// Applies text layout metadata while retaining the existing text payload.
    /// The canonical retained lowering uses this to keep span storage shared.
    #[doc(hidden)]
    #[must_use]
    pub(crate) fn with_text_layout(self, wrap: WrapMode, align: HorizontalAlign) -> Self {
        self.with_text_layout_patch(Some(wrap), Some(align))
    }

    /// Applies only the supplied text layout fields, preserving all others.
    #[doc(hidden)]
    #[must_use]
    pub(crate) fn with_text_layout_patch(
        self,
        wrap: Option<WrapMode>,
        align: Option<HorizontalAlign>,
    ) -> Self {
        self.map_text(|text| {
            if let Some(wrap) = wrap {
                text.wrap = wrap;
            }
            if let Some(align) = align {
                text.align = align;
            }
        })
    }

    /// Applies a retained text-layout patch without panicking on a non-text
    /// base. Generated native ABI calls use this checked boundary.
    #[doc(hidden)]
    pub(crate) fn try_with_text_layout_patch(
        self,
        wrap: Option<WrapMode>,
        align: Option<HorizontalAlign>,
    ) -> Result<Self, &'static str> {
        if !matches!(self.kind(), ViewKind::Text(_)) {
            return Err("text layout patch base is not text");
        }
        Ok(self.with_text_layout_patch(wrap, align))
    }
}
