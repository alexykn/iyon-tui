//! Finite text values used by the native boundary and wrapping kernel.

use std::{fmt, str, sync::Arc};

use super::style::{StyleFacts, StyleRef};

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
    /// Borrowed source page retained by an immutable text span.
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
