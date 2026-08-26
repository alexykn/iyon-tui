//! Root-coordinate projections with explicit source coverage.

use super::validate::validate_projection;
use crate::stream::{StreamOffset, StreamRange};

/// A validated projection of a contiguous interval in one root source space.
///
/// Every non-empty source interval is represented by exactly one or more
/// contiguous [`ProjectionSpan`] values. A span may contain no output values;
/// that is explicit elision, not an uncovered source gap.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection<T> {
    pub(crate) source_base: StreamOffset,
    pub(crate) stable_through: StreamOffset,
    pub(crate) source_end: StreamOffset,
    pub(crate) sealed: bool,
    pub(crate) spans: Vec<ProjectionSpan<T>>,
}

/// One contiguous source interval and the values projected from it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionSpan<T> {
    pub(crate) source: StreamRange,
    pub(crate) values: Vec<T>,
}

/// Validated construction boundary for a [`Projection`].
#[derive(Debug, Clone)]
pub struct ProjectionBuilder<T> {
    source_base: StreamOffset,
    stable_through: StreamOffset,
    source_end: StreamOffset,
    sealed: bool,
    spans: Vec<ProjectionSpan<T>>,
}

impl<T> ProjectionBuilder<T> {
    pub fn new(
        source_base: StreamOffset,
        stable_through: StreamOffset,
        source_end: StreamOffset,
        sealed: bool,
    ) -> Self {
        Self {
            source_base,
            stable_through,
            source_end,
            sealed,
            spans: Vec::new(),
        }
    }

    /// Adds one output value for a source interval.
    pub fn emit(mut self, source: StreamRange, value: T) -> Self {
        self.spans.push(ProjectionSpan {
            source,
            values: vec![value],
        });
        self
    }

    /// Adds any number of output values for a source interval.
    pub fn emit_many(mut self, source: StreamRange, values: impl IntoIterator<Item = T>) -> Self {
        self.spans.push(ProjectionSpan {
            source,
            values: values.into_iter().collect(),
        });
        self
    }

    /// Explicitly accounts for a source interval with no projected values.
    pub fn elide(self, source: StreamRange) -> Self {
        self.emit_many(source, [])
    }

    pub fn finish(self) -> Result<Projection<T>, super::ProjectionValidationError> {
        let projection = Projection {
            source_base: self.source_base,
            stable_through: self.stable_through,
            source_end: self.source_end,
            sealed: self.sealed,
            spans: self.spans,
        };
        validate_projection(&projection)?;
        Ok(projection)
    }
}

impl<T> Projection<T> {
    /// Appends one contiguous source span without replaying the existing
    /// projection through a builder. This is used by append-only producers
    /// whose source envelope advances monotonically.
    pub(crate) fn append_span(
        &mut self,
        source: StreamRange,
        value: T,
    ) -> Result<(), super::ProjectionValidationError> {
        self.append_span_many(source, [value])
    }

    pub(crate) fn append_span_many(
        &mut self,
        source: StreamRange,
        values: impl IntoIterator<Item = T>,
    ) -> Result<(), super::ProjectionValidationError> {
        assert_eq!(
            source.start(),
            self.source_end,
            "appended projection spans must be contiguous"
        );
        assert!(
            source.end() >= source.start(),
            "appended projection span must not reverse the source"
        );
        self.spans.push(ProjectionSpan {
            source,
            values: values.into_iter().collect(),
        });
        self.source_end = source.end();
        self.stable_through = self.source_end;
        validate_projection(self)
    }

    /// Updates only the append-only stability envelope. The source spans are
    /// unchanged, so this avoids rebuilding them when a stream is sealed.
    pub(crate) fn set_envelope(&mut self, stable_through: StreamOffset, sealed: bool) {
        assert!(stable_through >= self.source_base && stable_through <= self.source_end);
        self.stable_through = stable_through;
        self.sealed = sealed;
    }

    /// Starts a builder preserving this projection's source envelope.
    pub fn rebuild<U>(&self) -> ProjectionBuilder<U> {
        ProjectionBuilder::new(
            self.source_base,
            self.stable_through,
            self.source_end,
            self.sealed,
        )
    }

    pub fn source_base(&self) -> StreamOffset {
        self.source_base
    }

    pub fn stable_through(&self) -> StreamOffset {
        self.stable_through
    }

    pub fn source_end(&self) -> StreamOffset {
        self.source_end
    }

    pub fn is_sealed(&self) -> bool {
        self.sealed
    }

    pub fn spans(&self) -> &[ProjectionSpan<T>] {
        &self.spans
    }

    /// Returns spans that may extend past a source coordinate. Projection
    /// spans are ordered and contiguous, so the prefix can be skipped with a
    /// binary search instead of rescanning it on every append.
    pub(crate) fn spans_from(&self, offset: StreamOffset) -> &[ProjectionSpan<T>] {
        let first = self
            .spans
            .partition_point(|span| span.source.end() <= offset);
        &self.spans[first..]
    }

    /// Maps borrowed values without changing ownership or stability metadata.
    pub fn map_ref<U>(&self, mut map: impl FnMut(&T) -> U) -> Projection<U> {
        Projection {
            source_base: self.source_base,
            stable_through: self.stable_through,
            source_end: self.source_end,
            sealed: self.sealed,
            spans: self
                .spans
                .iter()
                .map(|span| ProjectionSpan {
                    source: span.source,
                    values: span.values.iter().map(&mut map).collect(),
                })
                .collect(),
        }
    }

    pub fn try_map_ref<U, E>(
        &self,
        mut map: impl FnMut(&T) -> Result<U, E>,
    ) -> Result<Projection<U>, E> {
        let mut output = self.rebuild();
        for span in &self.spans {
            let values = span
                .values
                .iter()
                .map(&mut map)
                .collect::<Result<Vec<_>, _>>()?;
            output = output.emit_many(span.source, values);
        }
        Ok(output
            .finish()
            .expect("rebuilding a valid projection must remain valid"))
    }

    /// Maps each source span to zero or more values while retaining one span.
    pub fn map_spans<U>(&self, mut map: impl FnMut(&ProjectionSpan<T>) -> Vec<U>) -> Projection<U> {
        Projection {
            source_base: self.source_base,
            stable_through: self.stable_through,
            source_end: self.source_end,
            sealed: self.sealed,
            spans: self
                .spans
                .iter()
                .map(|span| ProjectionSpan {
                    source: span.source,
                    values: map(span),
                })
                .collect(),
        }
    }

    pub fn try_map_spans<U, E>(
        &self,
        mut map: impl FnMut(&ProjectionSpan<T>) -> Result<Vec<U>, E>,
    ) -> Result<Projection<U>, E> {
        let mut output = self.rebuild();
        for span in &self.spans {
            output = output.emit_many(span.source, map(span)?);
        }
        Ok(output
            .finish()
            .expect("rebuilding a valid projection must remain valid"))
    }

    /// Changes the value type without changing ownership or stability metadata.
    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> Projection<U> {
        Projection {
            source_base: self.source_base,
            stable_through: self.stable_through,
            source_end: self.source_end,
            sealed: self.sealed,
            spans: self
                .spans
                .into_iter()
                .map(|span| ProjectionSpan {
                    source: span.source,
                    values: span.values.into_iter().map(&mut map).collect(),
                })
                .collect(),
        }
    }
}

impl<T> ProjectionSpan<T> {
    pub fn source(&self) -> StreamRange {
        self.source
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }
}
