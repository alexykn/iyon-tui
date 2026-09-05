use std::{fmt, ops::Range, sync::Arc};

use crate::stream::{StreamOffset, StreamRange};

use super::{Block, TextIrError, TextRun};

/// Exact, unclaimed text at the root of a text projection.
#[derive(Clone, Debug, Default)]
pub struct RawText {
    page: Arc<str>,
    start: u32,
    len: u32,
}

impl PartialEq for RawText {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}
impl Eq for RawText {}

impl RawText {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        let page = text.into();
        let len = page.len() as u32;
        Self {
            page,
            start: 0,
            len,
        }
    }

    pub(crate) fn from_page_slice(page: Arc<str>, start: u32, len: u32) -> Self {
        Self { page, start, len }
    }

    pub(crate) fn page(&self) -> &Arc<str> {
        &self.page
    }

    pub(crate) fn page_start(&self) -> u32 {
        self.start
    }

    #[must_use]
    pub fn text(&self) -> &str {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &self.page[start..end]
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Creates an exact text run from a byte slice of this root source witness.
    pub fn exact_slice(
        &self,
        owner: StreamRange,
        local: Range<usize>,
    ) -> Result<TextRun, TextIrError> {
        if owner.len() != self.len() as u64
            || local.start > local.end
            || local.end > self.len()
            || !self.text().is_char_boundary(local.start)
            || !self.text().is_char_boundary(local.end)
        {
            let start = owner.start().as_u64().saturating_add(local.start as u64);
            let end = owner.start().as_u64().saturating_add(local.end as u64);
            let local = StreamRange::try_new(
                StreamOffset::new(start.min(owner.end().as_u64())),
                StreamOffset::new(end.min(owner.end().as_u64())),
            )
            .unwrap_or(owner);
            return Err(TextIrError::InvalidSourceSlice { owner, local });
        }
        let range = StreamRange::new(
            owner.start().saturating_add(local.start as u64),
            owner.start().saturating_add(local.end as u64),
        );
        TextRun::exact(&self.text()[local], range)
    }
}

/// The closed set of generic text projection values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextContent {
    Raw(RawText),
    Block(Block),
}

impl TextContent {
    pub fn raw(text: impl Into<Arc<str>>) -> Self {
        Self::Raw(RawText::new(text))
    }
    #[must_use]
    pub fn block(block: Block) -> Self {
        Self::Block(block)
    }
}

impl From<RawText> for TextContent {
    fn from(value: RawText) -> Self {
        Self::Raw(value)
    }
}

impl From<Block> for TextContent {
    fn from(value: Block) -> Self {
        Self::Block(value)
    }
}

impl fmt::Display for RawText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text())
    }
}
