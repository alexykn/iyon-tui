//! Simple append-only UTF-8 text streaming source.

use super::{
    StreamOffset, StreamRange, StreamRevision, StreamSnapshot, StreamSnapshotBuilder,
    StreamingSource, append::append_only_text_stable_frontier,
};
use crate::TextSpan;

/// A small append-only UTF-8 source for ordinary History text streams.
#[derive(Clone, Debug, Default)]
pub struct TextStream {
    source_base: StreamOffset,
    text: String,
    revision: StreamRevision,
    sealed: bool,
}

impl TextStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let revision = if text.is_empty() {
            StreamRevision::ZERO
        } else {
            StreamRevision::ZERO.next()
        };
        Self {
            source_base: StreamOffset::ZERO,
            text,
            revision,
            sealed: false,
        }
    }

    /// Appends UTF-8 text and advances the source revision.
    ///
    /// # Panics
    ///
    /// Panics if the source has been sealed.
    pub fn push(&mut self, text: impl AsRef<str>) {
        assert!(!self.sealed, "cannot append to a sealed TextStream");
        let text = text.as_ref();
        if text.is_empty() {
            return;
        }
        self.text.push_str(text);
        self.revision = self.revision.next();
    }

    /// Returns the currently retained suffix; compacted text is not included.
    pub fn retained_text(&self) -> &str {
        &self.text
    }

    pub fn source_base(&self) -> StreamOffset {
        self.source_base
    }

    pub fn source_end(&self) -> StreamOffset {
        self.source_base.saturating_add(self.text.len() as u64)
    }

    pub fn revision(&self) -> StreamRevision {
        self.revision
    }
}

impl From<&str> for TextStream {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

impl From<String> for TextStream {
    fn from(value: String) -> Self {
        Self::from_text(value)
    }
}

impl StreamingSource for TextStream {
    fn snapshot(&self) -> StreamSnapshot {
        let end = self.source_end();
        let stable = append_only_text_stable_frontier(&self.text, self.source_base, self.sealed);
        let stable_len = usize::try_from(stable.as_u64() - self.source_base.as_u64())
            .expect("TextStream coordinate fits usize");
        let builder = StreamSnapshotBuilder::new(self.revision, self.source_base, stable, end);
        let builder = if self.text.is_empty() {
            builder.continuous_exact_text(StreamRange::new(self.source_base, end), [])
        } else if stable_len == 0 {
            builder.continuous_exact_text(
                StreamRange::new(stable, end),
                [TextSpan::plain(self.text.clone())],
            )
        } else if stable_len == self.text.len() {
            builder.continuous_exact_text(
                StreamRange::new(self.source_base, stable),
                [TextSpan::plain(self.text.clone())],
            )
        } else {
            let stable_range = StreamRange::new(self.source_base, stable);
            let tail_range = StreamRange::new(stable, end);
            builder
                .continuous_exact_text(
                    stable_range,
                    [TextSpan::plain(self.text[..stable_len].to_owned())],
                )
                .continuous_exact_text(
                    tail_range,
                    [TextSpan::plain(self.text[stable_len..].to_owned())],
                )
        };
        builder.finish().expect("TextStream snapshot must be valid")
    }

    fn compact_before(&mut self, offset: StreamOffset) {
        let stable = append_only_text_stable_frontier(&self.text, self.source_base, self.sealed);
        let target = offset.min(stable).max(self.source_base);
        if target == self.source_base {
            return;
        }
        let local = usize::try_from(target.as_u64() - self.source_base.as_u64())
            .expect("TextStream coordinate fits usize");
        assert!(
            self.text.is_char_boundary(local),
            "TextStream compaction must be UTF-8 aligned"
        );
        self.text.drain(..local);
        self.source_base = target;
        self.revision = self.revision.next();
    }

    fn seal(&mut self) {
        if !self.sealed {
            self.sealed = true;
            self.revision = self.revision.next();
        }
    }

    fn is_sealed(&self) -> bool {
        self.sealed
    }
}
