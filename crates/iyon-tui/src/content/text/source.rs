use std::{
    ops::Range,
    sync::{Arc, OnceLock},
};

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
    /// A single retained page can be borrowed directly. Multi-page domains
    /// keep only their piece witnesses until a parser actually asks for a
    /// contiguous working region.
    text: Option<Arc<str>>,
    assembled: Option<Arc<OnceLock<Arc<str>>>>,
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
                text: Some(Arc::clone(raw.page())),
                assembled: None,
                sub_range: raw.page_start() as usize..(raw.page_start() as usize + raw.len()),
                pieces: vec![piece],
            });
        }

        let mut total_len = 0usize;
        let mut pieces = Vec::with_capacity(spans.len());
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
            let start = total_len.saturating_sub(raw.len());
            pieces.push(RawPiece {
                source: span.source(),
                local: start..total_len,
                raw: raw.clone(),
            });
            expected = span.source().end();
        }
        Ok(Self {
            source_base,
            source_end: expected,
            text: None,
            assembled: Some(Arc::new(OnceLock::new())),
            sub_range: 0..total_len,
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
        if let Some(text) = &self.text {
            return &text[self.sub_range.clone()];
        }
        let assembled = self
            .assembled
            .as_ref()
            .expect("piece-backed RawDomain has a working buffer")
            .get_or_init(|| {
                let mut text = String::with_capacity(self.len());
                for piece in &self.pieces {
                    text.push_str(piece.raw.text());
                }
                Arc::<str>::from(text)
            });
        &assembled[self.sub_range.clone()]
    }

    /// Reports the final line's open reference-definition shape without
    /// materializing the preceding source. The parser only needs this small
    /// suffix after its dependency-aware restart decision has run.
    pub(crate) fn has_open_reference_definition_prefix(&self) -> Result<bool, TextProjectionError> {
        let mut line_start = 0;
        for piece in self.pieces.iter().rev() {
            if let Some(offset) = piece
                .raw
                .text()
                .as_bytes()
                .iter()
                .rposition(|byte| *byte == b'\n')
            {
                line_start = piece.local.start.saturating_add(offset + 1);
                break;
            }
        }
        let suffix = self.suffix(line_start)?;
        let line = suffix.text();
        Ok(!line.ends_with('\n')
            && line
                .rsplit('\n')
                .next()
                .unwrap_or(line)
                .trim_start()
                .starts_with('['))
    }

    /// Visits newline offsets in source order without assembling a
    /// multi-page domain. Callers that only need line boundaries can retain
    /// the page-backed proof and avoid a parser working buffer entirely.
    pub(crate) fn newline_offsets(&self) -> Vec<usize> {
        let mut offsets = Vec::new();
        for piece in &self.pieces {
            offsets.extend(
                piece
                    .raw
                    .text()
                    .bytes()
                    .enumerate()
                    .filter_map(|(offset, byte)| {
                        (byte == b'\n').then_some(piece.local.start + offset)
                    }),
            );
        }
        offsets
    }

    #[allow(dead_code)]
    pub(crate) fn prefix(&self, local_end: usize) -> Result<Self, TextProjectionError> {
        let len = self.len();
        if local_end > len || !self.is_char_boundary(local_end) {
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
        let text = self.materialized_text();
        let assembled = text.is_none().then(|| Arc::new(OnceLock::new()));
        Ok(Self {
            source_base: self.source_base,
            source_end: self.source_base.saturating_add(local_end as u64),
            text,
            assembled,
            sub_range: self.sub_range.start..(self.sub_range.start + local_end),
            pieces,
        })
    }

    pub(crate) fn suffix(&self, local_start: usize) -> Result<Self, TextProjectionError> {
        let len = self.len();
        if local_start > len || !self.is_char_boundary(local_start) {
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
        let text = self.materialized_text();
        let sub_range = if text.is_some() {
            (self.sub_range.start + local_start)..self.sub_range.end
        } else {
            0..(self.len() - local_start)
        };
        let assembled = text.is_none().then(|| Arc::new(OnceLock::new()));
        Ok(Self {
            source_base: self.source_base.saturating_add(local_start as u64),
            source_end: self.source_end,
            text,
            assembled,
            sub_range,
            pieces,
        })
    }

    fn materialized_text(&self) -> Option<Arc<str>> {
        if let Some(text) = &self.text {
            return Some(Arc::clone(text));
        }
        self.assembled
            .as_ref()
            .and_then(|assembled| assembled.get().cloned())
    }

    pub(crate) fn source_slice(&self, local: Range<usize>) -> Result<&str, TextProjectionError> {
        self.validate_local_range(&local)?;
        let text = self.text();
        Ok(&text[local])
    }

    fn validate_local_range(&self, local: &Range<usize>) -> Result<(), TextProjectionError> {
        if local.start > local.end
            || local.end > self.len()
            || !self.is_char_boundary(local.start)
            || !self.is_char_boundary(local.end)
        {
            return Err(TextProjectionError::Ir(TextIrError::InvalidSourceSlice {
                owner: StreamRange::new(self.source_base, self.source_end),
                local: StreamRange::new(
                    self.source_base.saturating_add(local.start as u64),
                    self.source_base.saturating_add(local.end as u64),
                ),
            }));
        }
        Ok(())
    }

    fn is_char_boundary(&self, local: usize) -> bool {
        if local == 0 || local == self.len() {
            return true;
        }
        self.pieces
            .iter()
            .find(|piece| piece.local.start <= local && local < piece.local.end)
            .is_some_and(|piece| {
                piece
                    .raw
                    .text()
                    .is_char_boundary(local.saturating_sub(piece.local.start))
            })
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
        self.validate_local_range(&local)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::ProjectionBuilder;

    #[test]
    fn piece_domain_materializes_only_the_selected_working_region() {
        let chunks = ["stable\nprefix", "tail"];
        let source_end = chunks.iter().map(|chunk| chunk.len()).sum::<usize>() as u64;
        let mut builder = ProjectionBuilder::new(
            StreamOffset::ZERO,
            StreamOffset::new(source_end),
            StreamOffset::new(source_end),
            true,
        );
        let mut start = 0;
        for chunk in chunks {
            let end = start + chunk.len() as u64;
            builder = builder.emit(
                StreamRange::new(StreamOffset::new(start), StreamOffset::new(end)),
                TextContent::raw(chunk),
            );
            start = end;
        }
        let projection = builder.finish().expect("raw projection");
        let domain = RawDomain::from_spans(projection.spans()).expect("piece domain");
        let domain_buffer = domain.assembled.as_ref().expect("lazy domain buffer");
        assert!(domain_buffer.get().is_none());

        let suffix = domain.suffix("stable\n".len()).expect("suffix boundary");
        assert_eq!(suffix.text(), "prefixtail");
        assert_eq!(
            suffix
                .assembled
                .as_ref()
                .expect("suffix working buffer")
                .get()
                .expect("assembled suffix")
                .as_ref(),
            "prefixtail"
        );
        assert!(domain_buffer.get().is_none());

        let runs = suffix
            .exact_runs(0..suffix.len())
            .expect("witnessed suffix");
        assert_eq!(
            runs.iter().map(|run| run.text()).collect::<String>(),
            "prefixtail"
        );
        assert_eq!(runs.len(), 2);
    }
}
