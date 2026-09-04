use std::{fmt, ops::Range, sync::Arc};

use crate::stream::{StreamOffset, StreamRange};

use super::{Block, TextIrError, TextRun};

/// Exact, unclaimed text at the root of a text projection.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RawText(Arc<str>);

impl RawText {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self(text.into())
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.0
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
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
        f.write_str(&self.0)
    }
}
