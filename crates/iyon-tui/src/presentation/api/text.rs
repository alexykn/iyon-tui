//! Typed semantic text construction backed by the canonical View IR.

use std::{fmt, str, sync::Arc};

use super::style::{StyleFacts, StyleRef, StyleStateKey, StyleStateValue};

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
    /// Borrowed semantic Source page retained by an immutable text View.
    /// Unlike `Owned`, this representation does not copy a raw projection
    /// merely to lower it into terminal layout.
    SourcePage {
        page: Arc<str>,
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
            Self::SourcePage { page, offset, len } => Self::SourcePage {
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
            Self::SourcePage { page, offset, len } => {
                &page[*offset as usize..(*offset + *len) as usize]
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

    pub(crate) fn plain(text: impl Into<String>) -> Self {
        Self {
            text: TextStorage::from_string(text.into()),
            style: StyleRef::default(),
            style_facts: StyleFacts::default(),
        }
    }

    pub(crate) fn styled(text: impl Into<String>, style: impl Into<StyleRef>) -> Self {
        Self {
            text: TextStorage::from_string(text.into()),
            style: style.into(),
            style_facts: StyleFacts::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn source_page_ptr(&self) -> Option<usize> {
        match &self.text {
            TextStorage::SourcePage { page, .. } => Some(Arc::as_ptr(page) as *const () as usize),
            _ => None,
        }
    }

    /// Internal page-backed source lowering.  The page owner is retained by
    /// the resulting semantic View, so its range remains valid across cache
    /// eviction and later Source snapshots.
    pub(crate) fn from_source_page(page: Arc<str>, offset: u32, len: u32, style: StyleRef) -> Self {
        Self {
            text: TextStorage::SourcePage { page, offset, len },
            style,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let (page_of_first, page_of_second) = match (&first.text, &second.text) {
            (
                TextStorage::PageSlice { page: first, .. },
                TextStorage::PageSlice { page: second, .. },
            ) => (Arc::clone(first), Arc::clone(second)),
            _ => panic!("length-delimited spans must borrow the shared page"),
        };
        assert!(Arc::ptr_eq(&page_of_first, &page_of_second));

        let old = crate::presentation::factory::text_from_spans(
            vec![first],
            WrapMode::WordThenGrapheme,
            HorizontalAlign::Start,
        );
        drop(page);
        let _newer = crate::presentation::factory::text_from_spans(
            vec![second],
            WrapMode::WordThenGrapheme,
            HorizontalAlign::Start,
        );
        let crate::presentation::ir::ViewKind::Text(retained) = old.kind() else {
            panic!("expected text view");
        };
        assert_eq!(retained.spans[0].text(), "hello");
    }

    #[cfg(feature = "native-host")]
    #[test]
    fn page_span_rejects_out_of_bounds_and_split_sequences() {
        let page = NativeTextPage::new("héllo🌍".to_owned());
        assert!(page.span(0, 100, StyleRef::default()).is_none());
        assert!(page.span(1, 1, StyleRef::default()).is_none());
        assert_eq!(
            page.span(0, 1, StyleRef::default()).expect("ascii").text(),
            "h"
        );
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
