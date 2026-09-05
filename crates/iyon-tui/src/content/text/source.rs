use std::{ops::Range, sync::Arc};

use crate::{
    projection::ProjectionSpan,
    stream::{StreamOffset, StreamRange},
};

use super::{RawText, TextContent, TextIrError, TextProjectionError, TextRun};

/// A consecutive run of Raw projection spans and the source witness they own.
///
/// Parser offsets are local to `text`; every conversion back to the semantic IR
/// goes through the original piece witnesses instead of asserting provenance.
#[derive(Clone, Debug)]
pub(crate) struct RawDomain {
    source_base: StreamOffset,
    source_end: StreamOffset,
    text: Arc<str>,
    sub_range: Range<usize>,
    pieces: Vec<RawPiece>,
}

#[derive(Clone, Debug)]
struct RawPiece {
    source: StreamRange,
    local: Range<usize>,
    raw: RawText,
}

impl RawDomain {
    pub(crate) fn from_spans(
        spans: &[ProjectionSpan<TextContent>],
    ) -> Result<Self, TextProjectionError> {
        let Some(first) = spans.first() else {
            return Err(TextProjectionError::RawByteLengthMismatch {
                source: StreamRange::new(StreamOffset::ZERO, StreamOffset::ZERO),
                text_len: 0,
            });
        };
        let source_base = first.source().start();
        let mut expected = source_base;

        if spans.len() == 1 {
            let span = &spans[0];
            if span.values().len() != 1 {
                return Err(TextProjectionError::RawMustBeSoleValue {
                    source: span.source(),
                });
            }
            let TextContent::Raw(raw) = &span.values()[0] else {
                return Err(TextProjectionError::RawMustBeSoleValue {
                    source: span.source(),
                });
            };
            if raw.len() as u64 != span.source().len() {
                return Err(TextProjectionError::RawByteLengthMismatch {
                    source: span.source(),
                    text_len: raw.len() as u64,
                });
            }
            let piece = RawPiece {
                source: span.source(),
                local: 0..raw.len(),
                raw: raw.clone(),
            };
            return Ok(Self {
                source_base,
                source_end: span.source().end(),
                text: Arc::clone(raw.page()),
                sub_range: raw.page_start() as usize..(raw.page_start() as usize + raw.len()),
                pieces: vec![piece],
            });
        }

        let mut total_len = 0usize;
        for span in spans {
            if span.source().start() != expected || span.values().len() != 1 {
                return Err(TextProjectionError::RawMustBeSoleValue {
                    source: span.source(),
                });
            }
            let TextContent::Raw(raw) = &span.values()[0] else {
                return Err(TextProjectionError::RawMustBeSoleValue {
                    source: span.source(),
                });
            };
            if raw.len() as u64 != span.source().len() {
                return Err(TextProjectionError::RawByteLengthMismatch {
                    source: span.source(),
                    text_len: raw.len() as u64,
                });
            }
            total_len = total_len.saturating_add(raw.len());
            expected = span.source().end();
        }

        let mut text = String::with_capacity(total_len);
        let mut pieces = Vec::with_capacity(spans.len());
        for span in spans {
            let TextContent::Raw(raw) = &span.values()[0] else {
                unreachable!();
            };
            let start = text.len();
            text.push_str(raw.text());
            let end = text.len();
            pieces.push(RawPiece {
                source: span.source(),
                local: start..end,
                raw: raw.clone(),
            });
        }
        let text_len = text.len();
        Ok(Self {
            source_base,
            source_end: expected,
            text: Arc::from(text),
            sub_range: 0..text_len,
            pieces,
        })
    }

    pub(crate) fn source_base(&self) -> StreamOffset {
        self.source_base
    }

    pub(crate) fn source_end(&self) -> StreamOffset {
        self.source_end
    }

    pub(crate) fn len(&self) -> usize {
        self.sub_range.len()
    }

    pub(crate) fn text(&self) -> &str {
        &self.text[self.sub_range.clone()]
    }

    #[allow(dead_code)]
    pub(crate) fn text_prefix(&self, end: StreamOffset) -> Option<&str> {
        let local = end.as_u64().checked_sub(self.source_base.as_u64())?;
        let local = usize::try_from(local).ok()?;
        self.text().get(..local)
    }

    #[allow(dead_code)]
    pub(crate) fn prefix(&self, local_end: usize) -> Result<Self, TextProjectionError> {
        let len = self.len();
        if local_end > len || !self.text().is_char_boundary(local_end) {
            return Err(TextProjectionError::Ir(TextIrError::NotCharBoundary));
        }
        let mut pieces = Vec::new();
        for piece in &self.pieces {
            let start = piece.local.start;
            let end = piece.local.end.min(local_end);
            if start >= end {
                continue;
            }
            let raw_end = end - piece.local.start;
            pieces.push(RawPiece {
                source: StreamRange::new(
                    piece.source.start(),
                    piece.source.start().saturating_add(raw_end as u64),
                ),
                local: start..end,
                raw: RawText::from_page_slice(
                    Arc::clone(piece.raw.page()),
                    piece.raw.page_start(),
                    raw_end as u32,
                ),
            });
        }
        Ok(Self {
            source_base: self.source_base,
            source_end: self.source_base.saturating_add(local_end as u64),
            text: Arc::clone(&self.text),
            sub_range: self.sub_range.start..(self.sub_range.start + local_end),
            pieces,
        })
    }

    pub(crate) fn suffix(&self, local_start: usize) -> Result<Self, TextProjectionError> {
        let len = self.len();
        if local_start > len || !self.text().is_char_boundary(local_start) {
            return Err(TextProjectionError::Ir(TextIrError::NotCharBoundary));
        }
        let mut pieces = Vec::new();
        for piece in &self.pieces {
            let start = piece.local.start.max(local_start);
            let end = piece.local.end;
            if start >= end {
                continue;
            }
            let raw_start = start - piece.local.start;
            let raw_end = end - piece.local.start;
            let source_start = piece.source.start().saturating_add(raw_start as u64);
            let source_end = piece.source.start().saturating_add(raw_end as u64);
            pieces.push(RawPiece {
                source: StreamRange::new(source_start, source_end),
                local: (start - local_start)..(end - local_start),
                raw: RawText::from_page_slice(
                    Arc::clone(piece.raw.page()),
                    piece.raw.page_start() + raw_start as u32,
                    (raw_end - raw_start) as u32,
                ),
            });
        }
        Ok(Self {
            source_base: self.source_base.saturating_add(local_start as u64),
            source_end: self.source_end,
            text: Arc::clone(&self.text),
            sub_range: (self.sub_range.start + local_start)..self.sub_range.end,
            pieces,
        })
    }

    pub(crate) fn source_slice(&self, local: Range<usize>) -> Result<&str, TextProjectionError> {
        let text = self.text();
        if local.start > local.end
            || local.end > text.len()
            || !text.is_char_boundary(local.start)
            || !text.is_char_boundary(local.end)
        {
            return Err(TextProjectionError::Ir(TextIrError::InvalidSourceSlice {
                owner: StreamRange::new(self.source_base, self.source_end),
                local: StreamRange::new(
                    self.source_base.saturating_add(local.start as u64),
                    self.source_base.saturating_add(local.end as u64),
                ),
            }));
        }
        Ok(&text[local])
    }

    pub(crate) fn root_range(
        &self,
        local: Range<usize>,
    ) -> Result<StreamRange, TextProjectionError> {
        if local.start > local.end || local.end > self.len() {
            return Err(TextProjectionError::Ir(TextIrError::InvalidSourceSlice {
                owner: StreamRange::new(self.source_base, self.source_end),
                local: StreamRange::new(
                    self.source_base.saturating_add(local.start as u64),
                    self.source_base.saturating_add(local.end as u64),
                ),
            }));
        }
        let start = self.root_offset(local.start)?;
        let end = self.root_offset(local.end)?;
        Ok(StreamRange::new(start, end))
    }

    fn root_offset(&self, local: usize) -> Result<StreamOffset, TextProjectionError> {
        let len = self.len();
        if local > len {
            return Err(TextProjectionError::Ir(TextIrError::InvalidSourceSlice {
                owner: StreamRange::new(self.source_base, self.source_end),
                local: StreamRange::new(
                    self.source_base.saturating_add(local as u64),
                    self.source_base.saturating_add(local as u64),
                ),
            }));
        }
        if local == len {
            return Ok(self.source_end);
        }
        let index = self.pieces.partition_point(|piece| piece.local.end < local);
        let piece = self.pieces.get(index).ok_or_else(|| {
            TextProjectionError::Ir(TextIrError::InvalidSourceSlice {
                owner: StreamRange::new(self.source_base, self.source_end),
                local: StreamRange::new(
                    self.source_base.saturating_add(local as u64),
                    self.source_base.saturating_add(local as u64),
                ),
            })
        })?;
        Ok(piece
            .source
            .start()
            .saturating_add(local.saturating_sub(piece.local.start) as u64))
    }

    /// Builds witnessed Exact runs, splitting at every retained Raw boundary.
    pub(crate) fn exact_runs(
        &self,
        local: Range<usize>,
    ) -> Result<Vec<TextRun>, TextProjectionError> {
        let _ = self.source_slice(local.clone())?;
        let mut runs = Vec::new();
        for piece in &self.pieces {
            let start = piece.local.start.max(local.start);
            let end = piece.local.end.min(local.end);
            if start >= end {
                continue;
            }
            let piece_local = (start - piece.local.start)..(end - piece.local.start);
            runs.push(
                piece
                    .raw
                    .exact_slice(piece.source, piece_local)
                    .map_err(TextProjectionError::Ir)?,
            );
        }
        Ok(runs)
    }

    pub(crate) fn derived_run(
        &self,
        text: impl Into<std::sync::Arc<str>>,
        local: Range<usize>,
    ) -> Result<TextRun, TextProjectionError> {
        Ok(TextRun::derived(text, self.root_range(local)?))
    }
}
