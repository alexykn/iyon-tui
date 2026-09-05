//! Typed semantic text construction backed by the canonical View IR.

use std::{fmt, str, sync::Arc};

use super::style::{
    BorderSpec, ColorSpec, Insets, OverflowIndicator, StyleFacts, StyleRef, StyleStateKey,
    StyleStateValue, StyleStates, TextAttribute,
};
use crate::presentation::ir::{
    Decoration, HeightRule, TextView, View, ViewKind, ViewNodeParts, WidthRule,
};

const INLINE_TEXT_CAPACITY: usize = 12;

/// Immutable native-owned UTF-8 storage shared by retained text clones.
#[derive(Debug)]
pub(crate) struct NativeUtf8Page {
    text: Box<str>,
}

pub(crate) enum TextStorage {
    Inline {
        bytes: [u8; INLINE_TEXT_CAPACITY],
        len: u8,
    },
    PageSlice {
        page: Arc<NativeUtf8Page>,
        offset: u32,
        len: u32,
    },
    Owned(String),
}

impl Clone for TextStorage {
    fn clone(&self) -> Self {
        match self {
            Self::Inline { bytes, len } => Self::Inline {
                bytes: *bytes,
                len: *len,
            },
            Self::PageSlice { page, offset, len } => Self::PageSlice {
                page: Arc::clone(page),
                offset: *offset,
                len: *len,
            },
            Self::Owned(text) => Self::Owned(text.clone()),
        }
    }
}

impl fmt::Debug for TextStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TextStorage")
            .field(&self.as_str())
            .finish()
    }
}

impl PartialEq for TextStorage {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl TextStorage {
    fn from_string(text: String) -> Self {
        let bytes = text.as_bytes();
        if bytes.len() <= INLINE_TEXT_CAPACITY {
            let mut inline = [0; INLINE_TEXT_CAPACITY];
            inline[..bytes.len()].copy_from_slice(bytes);
            return Self::Inline {
                bytes: inline,
                len: bytes.len() as u8,
            };
        }
        let len = text.len() as u32;
        Self::PageSlice {
            page: Arc::new(NativeUtf8Page {
                text: text.into_boxed_str(),
            }),
            offset: 0,
            len,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Inline { bytes, len } => unsafe {
                str::from_utf8_unchecked(&bytes[..*len as usize])
            },
            Self::PageSlice { page, offset, len } => {
                &page.text[*offset as usize..(*offset + *len) as usize]
            }
            Self::Owned(text) => text,
        }
    }
}

/// A semantic text span with optional text-cell styling.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub(crate) text: TextStorage,
    pub(crate) style: StyleRef,
    pub(crate) style_facts: StyleFacts,
}

/// One native-owned page shared by every span of a length-delimited ingress
/// buffer. The page is built once from an owned `String` (already valid
/// UTF-8 by construction); spans reference checked ranges instead of
/// rehydrating one `String` per span.
#[cfg(feature = "native-host")]
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct NativeTextPage {
    page: Arc<NativeUtf8Page>,
}

#[cfg(feature = "native-host")]
impl NativeTextPage {
    /// Moves an owned buffer into a single shared page. No copy: the
    /// caller's allocation becomes the page storage.
    pub fn new(text: String) -> Self {
        Self {
            page: Arc::new(NativeUtf8Page {
                text: text.into_boxed_str(),
            }),
        }
    }

    /// Borrows a checked range as a styled span. Returns `None` when the
    /// range is out of bounds or splits a UTF-8 sequence; interior validity
    /// holds by construction because the page was built from a `String`.
    pub fn span(&self, offset: u32, len: u32, style: StyleRef) -> Option<TextSpan> {
        let end = offset.checked_add(len)? as usize;
        let offset = offset as usize;
        self.page.text.get(offset..end)?;
        Some(TextSpan {
            text: TextStorage::PageSlice {
                page: Arc::clone(&self.page),
                offset: offset as u32,
                len,
            },
            style,
            style_facts: StyleFacts::default(),
        })
    }
}

impl TextSpan {
    #[must_use]
    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn text_mut(&mut self) -> &mut String {
        if !matches!(&self.text, TextStorage::Owned(_)) {
            self.text = TextStorage::Owned(self.text.as_str().to_owned());
        }
        match &mut self.text {
            TextStorage::Owned(text) => text,
            _ => unreachable!("text storage is materialized before mutable access"),
        }
    }

    #[must_use]
    pub fn style(&self) -> &StyleRef {
        &self.style
    }

    pub fn style_mut(&mut self) -> &mut StyleRef {
        &mut self.style
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: TextStorage::from_string(text.into()),
            style: StyleRef::default(),
            style_facts: StyleFacts::default(),
        }
    }

    pub fn styled(text: impl Into<String>, style: impl Into<StyleRef>) -> Self {
        Self {
            text: TextStorage::from_string(text.into()),
            style: style.into(),
            style_facts: StyleFacts::default(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn style_fact(
        mut self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        self.style_facts.set(key, value);
        self
    }

    pub(crate) fn with_style_facts(mut self, style_facts: StyleFacts) -> Self {
        self.style_facts = style_facts;
        self
    }
}

/// Text wrapping behavior for a typed text view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WrapMode {
    #[default]
    WordThenGrapheme,
    Grapheme,
    NoWrap,
}

/// Horizontal alignment inside an allocated text track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HorizontalAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Typed backend-neutral text construction backed by the crate's owned
/// semantic [`View`]. Ordinary properties preserve `Text`; structural
/// transforms return a general `View`.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    view: View,
}

impl View {
    /// Direct final text construction for native ingress: wrap and alignment
    /// initialize the `TextView` fields before root allocation, and the
    /// already-owned span vector moves into the payload with no second
    /// collection. Exactly one root per call.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_text_final(spans: Vec<TextSpan>, wrap: WrapMode, align: HorizontalAlign) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fit,
            height: HeightRule::Fit,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: None,
            kind: ViewKind::Text(Arc::new(TextView {
                spans: spans.into(),
                wrap,
                align,
                cursor: None,
            })),
        })
    }
}

impl Text {
    pub(super) fn plain(text: impl Into<String>) -> Self {
        Self::from_text_view(TextView::plain(text))
    }

    pub(super) fn styled(spans: impl IntoIterator<Item = TextSpan>) -> Self {
        Self::from_text_view(TextView {
            spans: spans.into_iter().collect::<Vec<_>>().into(),
            wrap: WrapMode::WordThenGrapheme,
            align: HorizontalAlign::Start,
            cursor: None,
        })
    }

    fn from_text_view(text: TextView) -> Self {
        Self {
            view: View::new_kind(ViewKind::Text(Arc::new(text))),
        }
    }

    pub(super) fn into_canonical_view(self) -> View {
        self.view
    }

    fn map_text(mut self, update: impl FnOnce(&mut TextView)) -> Self {
        self.view = self.view.map_text(update);
        self
    }

    #[must_use]
    pub fn wrap(self, wrap: WrapMode) -> Self {
        self.map_text(|text| text.wrap = wrap)
    }

    pub(crate) fn cursor_at(self, byte_offset: usize) -> Self {
        self.map_text(|text| {
            text.cursor = Some(crate::presentation::ir::TextCursorAnchor { byte_offset });
        })
    }

    #[must_use]
    pub fn no_wrap(self) -> Self {
        self.map_text(|text| text.wrap = WrapMode::NoWrap)
    }

    #[must_use]
    pub fn text_align(self, align: HorizontalAlign) -> Self {
        self.map_text(|text| text.align = align)
    }

    fn map_view(mut self, map: impl FnOnce(View) -> View) -> Self {
        self.view = map(self.view);
        self
    }

    #[must_use]
    pub fn style(self, style: impl Into<StyleRef>) -> Self {
        self.map_view(|view| view.style(style))
    }

    #[allow(dead_code)]
    pub(crate) fn style_fact(
        self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        self.map_view(|view| view.style_fact(key, value))
    }

    pub(crate) fn with_style_facts(self, facts: StyleFacts) -> Self {
        self.map_view(|view| view.with_style_facts(facts))
    }

    /// Sets the current text node's padding; repeated calls replace the prior value.
    #[must_use]
    pub fn style_state(
        mut self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        self.view = self.view.style_state(key, value);
        self
    }

    #[must_use]
    pub fn style_states(
        mut self,
        states: impl IntoIterator<Item = (StyleStateKey, StyleStateValue)>,
    ) -> Self {
        self.view = self.view.style_states(states);
        self
    }

    #[must_use]
    pub fn padding(self, padding: impl Into<Insets>) -> Self {
        self.map_view(|view| view.padding(padding))
    }

    /// Paints the text node's allocated surface, not its text-cell style.
    #[must_use]
    pub fn background(self, color: ColorSpec) -> Self {
        self.map_view(|view| view.background(color))
    }

    /// Sets inherited foreground intent for this text node.
    #[must_use]
    pub fn foreground(self, color: ColorSpec) -> Self {
        self.map_view(|view| view.foreground(color))
    }

    /// Replaces the text node's complete border specification.
    #[must_use]
    pub fn border(self, border: BorderSpec) -> Self {
        self.map_view(|view| view.border(border))
    }

    /// Sets sparse text-attribute intent, including explicit false.
    #[must_use]
    pub fn text_attribute(self, attribute: TextAttribute, enabled: bool) -> Self {
        self.map_view(|view| view.text_attribute(attribute, enabled))
    }

    #[must_use]
    pub fn bold(self) -> Self {
        self.text_attribute(TextAttribute::Bold, true)
    }

    #[must_use]
    pub fn dim(self) -> Self {
        self.text_attribute(TextAttribute::Dim, true)
    }

    #[must_use]
    pub fn italic(self) -> Self {
        self.text_attribute(TextAttribute::Italic, true)
    }

    #[must_use]
    pub fn underline(self) -> Self {
        self.text_attribute(TextAttribute::Underline, true)
    }

    #[must_use]
    pub fn reversed(self) -> Self {
        self.text_attribute(TextAttribute::Reversed, true)
    }

    #[must_use]
    pub fn strikethrough(self) -> Self {
        self.text_attribute(TextAttribute::Strikethrough, true)
    }

    #[must_use]
    pub fn container(self) -> View {
        self.into_canonical_view().container()
    }

    #[must_use]
    pub fn clamp_rows(self, max_rows: u16, overflow: OverflowIndicator) -> View {
        self.into_canonical_view().clamp_rows(max_rows, overflow)
    }

    #[must_use]
    pub fn fit_width(self) -> Self {
        self.map_view(View::fit_width)
    }

    #[must_use]
    pub fn fill_width(self) -> Self {
        self.map_view(View::fill_width)
    }

    #[must_use]
    pub fn fit_height(self) -> Self {
        self.map_view(View::fit_height)
    }

    #[must_use]
    pub fn fill_height(self) -> Self {
        self.map_view(View::fill_height)
    }
}

#[cfg(test)]
mod tests {
    use super::super::view::IntoView;
    use super::*;
    use crate::presentation::api::style::{ColorSpec, OverflowIndicator, StyleSpec, TextAttribute};
    use crate::presentation::ir::{Decoration, WidthRule};

    #[test]
    fn text_style_merges_node_intent_without_rewriting_spans() {
        let text = View::styled_text([
            TextSpan::plain("plain"),
            TextSpan::styled("bold", StyleSpec::new().bold()),
        ])
        .style(StyleSpec::new().foreground(ColorSpec::Ansi(1)))
        .style(StyleSpec::new().italic());

        assert_eq!(
            text.view.decoration().text_style.foreground,
            Some(ColorSpec::Ansi(1))
        );
        assert_eq!(
            text.view.decoration().text_style.attributes.italic,
            Some(true)
        );
        let ViewKind::Text(text_view) = text.view.kind() else {
            panic!("expected text view");
        };
        assert_eq!(text_view.spans[0].style, StyleSpec::default());
        assert_eq!(text_view.spans[1].style.attributes.bold, Some(true));

        let converted = text.into_view();
        assert!(matches!(converted.kind(), ViewKind::Text(_)));
    }

    #[test]
    fn typed_text_methods_update_only_canonical_text_payload() {
        let text = View::text("abcdef")
            .wrap(WrapMode::Grapheme)
            .text_align(HorizontalAlign::End)
            .style(StyleSpec::new().foreground(ColorSpec::Ansi(3)))
            .fill_width();

        assert_eq!(text.view.width(), WidthRule::Fill);
        assert_eq!(
            text.view.decoration().text_style.foreground,
            Some(ColorSpec::Ansi(3))
        );
        let ViewKind::Text(text_view) = text.view.kind() else {
            panic!("expected text view");
        };
        assert_eq!(text_view.wrap, WrapMode::Grapheme);
        assert_eq!(text_view.align, HorizontalAlign::End);
        assert_eq!(text_view.spans[0].style, StyleSpec::default());
    }

    #[test]
    fn typed_width_modifiers_preserve_text_and_last_write_wins() {
        let text = View::text("x")
            .fill_width()
            .no_wrap()
            .text_align(HorizontalAlign::End)
            .fit_width();

        assert_eq!(text.view.width(), WidthRule::Fit);
        assert!(matches!(text.view.kind(), ViewKind::Text(_)));
        assert_eq!(text.view.decoration(), &Decoration::default());
    }

    #[test]
    fn style_and_specific_text_properties_merge_as_sparse_patches() {
        let text = View::text("x")
            .bold()
            .style(StyleSpec::new().attribute(TextAttribute::Bold, false))
            .foreground(ColorSpec::ansi(1))
            .style(StyleSpec::new().italic())
            .bold();

        assert_eq!(
            text.view.decoration().text_style.foreground,
            Some(ColorSpec::ansi(1))
        );
        assert_eq!(
            text.view.decoration().text_style.attributes.bold,
            Some(true)
        );
        assert_eq!(
            text.view.decoration().text_style.attributes.italic,
            Some(true)
        );
    }

    #[test]
    fn structural_text_transforms_return_general_views() {
        let container = View::text("x").container();
        let clamp = View::text("x").clamp_rows(1, OverflowIndicator::None);

        assert!(matches!(container.kind(), ViewKind::Container(_)));
        assert!(matches!(clamp.kind(), ViewKind::ClampRows(_)));
    }

    #[test]
    fn small_spans_stay_inline_without_a_page() {
        let span = TextSpan::styled("hi", StyleRef::default());
        assert!(
            matches!(span.text, TextStorage::Inline { .. }),
            "short ingress text must keep the small-text fast case"
        );
        assert_eq!(span.text(), "hi");
    }

    #[cfg(feature = "native-host")]
    #[test]
    fn page_shared_spans_keep_old_roots_valid() {
        use super::NativeTextPage;
        use std::sync::Arc;

        let page = NativeTextPage::new("hello world".to_owned());
        let first = page
            .span(0, 5, StyleRef::default())
            .expect("in-bounds range");
        let second = page
            .span(6, 5, StyleRef::default())
            .expect("in-bounds range");
        assert_eq!(first.text(), "hello");
        assert_eq!(second.text(), "world");
        // Both spans borrow the single page: no per-span allocation.
        let (page_of_first, page_of_second) = match (&first.text, &second.text) {
            (
                TextStorage::PageSlice { page: first, .. },
                TextStorage::PageSlice { page: second, .. },
            ) => (Arc::clone(first), Arc::clone(second)),
            _ => panic!("length-delimited spans must borrow the shared page"),
        };
        assert!(Arc::ptr_eq(&page_of_first, &page_of_second));
        // An old root stays valid after subsequent construction, sharing the
        // page even once the ingress handle is gone.
        let old = View::native_text_final(
            vec![first],
            WrapMode::WordThenGrapheme,
            HorizontalAlign::Start,
        );
        drop(page);
        let _newer = View::native_text_final(
            vec![second],
            WrapMode::WordThenGrapheme,
            HorizontalAlign::Start,
        );
        let ViewKind::Text(retained) = old.kind() else {
            panic!("expected text view");
        };
        assert_eq!(retained.spans[0].text(), "hello");
    }

    #[cfg(feature = "native-host")]
    #[test]
    fn page_span_rejects_out_of_bounds_and_split_sequences() {
        use super::NativeTextPage;

        let page = NativeTextPage::new("héllo🌍".to_owned());
        assert!(page.span(0, 100, StyleRef::default()).is_none());
        assert!(page.span(1, 1, StyleRef::default()).is_none());
        assert_eq!(
            page.span(0, 1, StyleRef::default()).expect("ascii").text(),
            "h"
        );
        // Trailing newlines are ordinary text, preserved byte for byte.
        let lines = NativeTextPage::new("line\n".to_owned());
        assert_eq!(
            lines
                .span(0, 5, StyleRef::default())
                .expect("newline")
                .text(),
            "line\n"
        );
    }
}
