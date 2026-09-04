use std::{error::Error, fmt, num::NonZeroU64, sync::Arc};

/// A zero-based source line boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiffLineOffset(u64);

impl DiffLineOffset {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(offset: u64) -> Self {
        Self(offset)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn checked_add(self, rhs: u64) -> Option<Self> {
        match self.0.checked_add(rhs) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn saturating_add(self, rhs: u64) -> Self {
        Self(self.0.saturating_add(rhs))
    }
}

/// A one-based concrete source line number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiffLineNumber(NonZeroU64);

impl DiffLineNumber {
    #[must_use]
    pub const fn new(line: u64) -> Option<Self> {
        match NonZeroU64::new(line) {
            Some(line) => Some(Self(line)),
            None => None,
        }
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }
}

/// A contiguous range of source lines, represented by its start boundary and
/// number of lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiffRange {
    start: DiffLineOffset,
    line_count: u64,
}

impl DiffRange {
    pub fn new(start: DiffLineOffset, line_count: u64) -> Result<Self, DiffValidationError> {
        start
            .checked_add(line_count)
            .ok_or(DiffValidationError::RangeOverflow { start, line_count })?;
        Ok(Self { start, line_count })
    }

    #[must_use]
    pub const fn start(&self) -> DiffLineOffset {
        self.start
    }

    #[must_use]
    pub const fn line_count(&self) -> u64 {
        self.line_count
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.line_count == 0
    }

    #[must_use]
    pub fn end(&self) -> DiffLineOffset {
        self.start
            .checked_add(self.line_count)
            .expect("DiffRange invariant violated")
    }
}

/// The semantic classification of a changed source line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
}

/// Whether a source line has a line terminator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DiffLineTermination {
    #[default]
    Terminated,
    Unterminated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DiffLineCoordinates {
    Context {
        old: DiffLineNumber,
        new: DiffLineNumber,
    },
    Addition {
        new: DiffLineNumber,
    },
    Deletion {
        old: DiffLineNumber,
    },
}

/// A semantic diff line. Its text is the logical payload without a marker or
/// line terminator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    coordinates: DiffLineCoordinates,
    text: Arc<str>,
    termination: DiffLineTermination,
}

impl DiffLine {
    pub fn context(
        old_line: DiffLineNumber,
        new_line: DiffLineNumber,
        text: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            coordinates: DiffLineCoordinates::Context {
                old: old_line,
                new: new_line,
            },
            text: text.into(),
            termination: DiffLineTermination::default(),
        }
    }

    pub fn addition(new_line: DiffLineNumber, text: impl Into<Arc<str>>) -> Self {
        Self {
            coordinates: DiffLineCoordinates::Addition { new: new_line },
            text: text.into(),
            termination: DiffLineTermination::default(),
        }
    }

    pub fn deletion(old_line: DiffLineNumber, text: impl Into<Arc<str>>) -> Self {
        Self {
            coordinates: DiffLineCoordinates::Deletion { old: old_line },
            text: text.into(),
            termination: DiffLineTermination::default(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> DiffLineKind {
        match &self.coordinates {
            DiffLineCoordinates::Context { .. } => DiffLineKind::Context,
            DiffLineCoordinates::Addition { .. } => DiffLineKind::Addition,
            DiffLineCoordinates::Deletion { .. } => DiffLineKind::Deletion,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn old_line(&self) -> Option<DiffLineNumber> {
        match &self.coordinates {
            DiffLineCoordinates::Context { old, .. } | DiffLineCoordinates::Deletion { old } => {
                Some(*old)
            }
            DiffLineCoordinates::Addition { .. } => None,
        }
    }

    #[must_use]
    pub const fn new_line(&self) -> Option<DiffLineNumber> {
        match &self.coordinates {
            DiffLineCoordinates::Context { new, .. } | DiffLineCoordinates::Addition { new } => {
                Some(*new)
            }
            DiffLineCoordinates::Deletion { .. } => None,
        }
    }

    #[must_use]
    pub const fn termination(&self) -> DiffLineTermination {
        self.termination
    }

    #[must_use]
    pub fn with_termination(mut self, termination: DiffLineTermination) -> Self {
        self.termination = termination;
        self
    }
}

/// Validation diagnostics for semantic diff ranges and hunks.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffValidationError {
    RangeOverflow {
        start: DiffLineOffset,
        line_count: u64,
    },
    CoordinateOverflow {
        index: usize,
        side: &'static str,
    },
    LineCoordinateMismatch {
        index: usize,
        kind: DiffLineKind,
        expected_old: Option<DiffLineNumber>,
        actual_old: Option<DiffLineNumber>,
        expected_new: Option<DiffLineNumber>,
        actual_new: Option<DiffLineNumber>,
    },
    CountMismatch {
        expected_old: u64,
        consumed_old: u64,
        expected_new: u64,
        consumed_new: u64,
    },
}

impl fmt::Display for DiffValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RangeOverflow { start, line_count } => write!(
                formatter,
                "diff range overflows at boundary {} with {} lines",
                start.as_u64(),
                line_count
            ),
            Self::CoordinateOverflow { index, side } => {
                write!(
                    formatter,
                    "diff line {index} overflows its {side} coordinate"
                )
            }
            Self::LineCoordinateMismatch {
                index,
                kind,
                expected_old,
                actual_old,
                expected_new,
                actual_new,
            } => write!(
                formatter,
                "diff line {index} ({kind:?}) has coordinates old={actual_old:?}, new={actual_new:?}; expected old={expected_old:?}, new={expected_new:?}"
            ),
            Self::CountMismatch {
                expected_old,
                consumed_old,
                expected_new,
                consumed_new,
            } => write!(
                formatter,
                "diff hunk consumed old {consumed_old}/{expected_old} and new {consumed_new}/{expected_new} lines"
            ),
        }
    }
}

impl Error for DiffValidationError {}

/// A validated pair of old/new source ranges and its semantic diff lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    old: DiffRange,
    new_range: DiffRange,
    lines: Arc<[DiffLine]>,
}

impl DiffHunk {
    pub fn new(
        old: DiffRange,
        new_range: DiffRange,
        lines: impl IntoIterator<Item = DiffLine>,
    ) -> Result<Self, DiffValidationError> {
        let hunk = Self {
            old,
            new_range,
            lines: lines.into_iter().collect::<Vec<_>>().into(),
        };
        hunk.validate()?;
        Ok(hunk)
    }

    #[must_use]
    pub const fn old_range(&self) -> DiffRange {
        self.old
    }

    #[must_use]
    pub const fn new_range(&self) -> DiffRange {
        self.new_range
    }

    #[must_use]
    pub fn lines(&self) -> &[DiffLine] {
        &self.lines
    }

    pub fn validate(&self) -> Result<(), DiffValidationError> {
        let mut old_consumed = 0;
        let mut new_consumed = 0;
        let mut next_old = first_line(self.old)?;
        let mut next_new = first_line(self.new_range)?;

        for (index, line) in self.lines.iter().enumerate() {
            let (expected_old, expected_new) = match line.kind() {
                DiffLineKind::Context => (next_old, next_new),
                DiffLineKind::Addition => (None, next_new),
                DiffLineKind::Deletion => (next_old, None),
            };
            let actual_old = line.old_line();
            let actual_new = line.new_line();
            if actual_old != expected_old || actual_new != expected_new {
                return Err(DiffValidationError::LineCoordinateMismatch {
                    index,
                    kind: line.kind(),
                    expected_old,
                    actual_old,
                    expected_new,
                    actual_new,
                });
            }

            if expected_old.is_some() {
                old_consumed += 1;
                if old_consumed < self.old.line_count() {
                    next_old = next_coordinate(next_old, index, "old")?;
                }
            }
            if expected_new.is_some() {
                new_consumed += 1;
                if new_consumed < self.new_range.line_count() {
                    next_new = next_coordinate(next_new, index, "new")?;
                }
            }
        }

        if old_consumed != self.old.line_count() || new_consumed != self.new_range.line_count() {
            return Err(DiffValidationError::CountMismatch {
                expected_old: self.old.line_count(),
                consumed_old: old_consumed,
                expected_new: self.new_range.line_count(),
                consumed_new: new_consumed,
            });
        }
        Ok(())
    }
}

fn first_line(range: DiffRange) -> Result<Option<DiffLineNumber>, DiffValidationError> {
    if range.is_empty() {
        return Ok(None);
    }
    range
        .start()
        .checked_add(1)
        .and_then(|offset| DiffLineNumber::new(offset.as_u64()))
        .ok_or(DiffValidationError::CoordinateOverflow {
            index: 0,
            side: "range start",
        })
        .map(Some)
}

fn next_coordinate(
    coordinate: Option<DiffLineNumber>,
    index: usize,
    side: &'static str,
) -> Result<Option<DiffLineNumber>, DiffValidationError> {
    let coordinate = coordinate.ok_or(DiffValidationError::CoordinateOverflow { index, side })?;
    coordinate
        .as_u64()
        .checked_add(1)
        .and_then(DiffLineNumber::new)
        .ok_or(DiffValidationError::CoordinateOverflow { index, side })
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(value: u64) -> DiffLineNumber {
        DiffLineNumber::new(value).unwrap()
    }

    fn range(start: u64, count: u64) -> DiffRange {
        DiffRange::new(DiffLineOffset::new(start), count).unwrap()
    }

    #[test]
    fn offsets_and_numbers_preserve_boundary_and_line_semantics() {
        assert_eq!(DiffLineOffset::ZERO.as_u64(), 0);
        assert_eq!(
            DiffLineOffset::new(4).checked_add(3),
            Some(DiffLineOffset::new(7))
        );
        assert_eq!(DiffLineOffset::new(u64::MAX).checked_add(1), None);
        assert_eq!(
            DiffLineOffset::new(u64::MAX).saturating_add(1),
            DiffLineOffset::new(u64::MAX)
        );
        assert_eq!(DiffLineNumber::new(0), None);
        assert_eq!(DiffLineNumber::new(1).unwrap().as_u64(), 1);
        assert_eq!(DiffLineNumber::new(u64::MAX).unwrap().as_u64(), u64::MAX);
    }

    #[test]
    fn ranges_validate_boundaries_and_empty_ranges() {
        assert!(range(0, 0).is_empty());
        assert_eq!(range(10, 0).end(), DiffLineOffset::new(10));
        assert_eq!(range(12, 3).end(), DiffLineOffset::new(15));
        assert!(DiffRange::new(DiffLineOffset::new(u64::MAX), 1).is_err());
    }

    #[test]
    fn lines_preserve_semantics_and_payload() {
        let context = DiffLine::context(number(2), number(3), "+ payload");
        assert_eq!(context.kind(), DiffLineKind::Context);
        assert_eq!(context.old_line(), Some(number(2)));
        assert_eq!(context.new_line(), Some(number(3)));
        assert_eq!(context.text(), "+ payload");
        assert_eq!(context.termination(), DiffLineTermination::Terminated);

        let addition = DiffLine::addition(number(4), "@@ payload")
            .with_termination(DiffLineTermination::Unterminated);
        assert_eq!(addition.kind(), DiffLineKind::Addition);
        assert_eq!(addition.old_line(), None);
        assert_eq!(addition.new_line(), Some(number(4)));
        assert_eq!(addition.text(), "@@ payload");
        assert_eq!(addition.termination(), DiffLineTermination::Unterminated);

        let deletion = DiffLine::deletion(number(5), "--- payload");
        assert_eq!(deletion.kind(), DiffLineKind::Deletion);
        assert_eq!(deletion.old_line(), Some(number(5)));
        assert_eq!(deletion.new_line(), None);
    }

    #[test]
    fn hunks_accept_all_semantic_line_sequences() {
        let context = DiffLine::context(number(11), number(21), "same");
        let deletion = DiffLine::deletion(number(12), "old");
        let addition = DiffLine::addition(number(22), "new")
            .with_termination(DiffLineTermination::Unterminated);
        let hunk =
            DiffHunk::new(range(10, 2), range(20, 2), [context, deletion, addition]).unwrap();
        assert_eq!(hunk.lines().len(), 3);
        assert_eq!(hunk.old_range(), range(10, 2));
        assert_eq!(hunk.new_range(), range(20, 2));

        DiffHunk::new(
            range(8, 0),
            range(8, 2),
            [
                DiffLine::addition(number(9), "a"),
                DiffLine::addition(number(10), "b"),
            ],
        )
        .unwrap();
        DiffHunk::new(
            range(8, 2),
            range(8, 0),
            [
                DiffLine::deletion(number(9), "a"),
                DiffLine::deletion(number(10), "b"),
            ],
        )
        .unwrap();
    }

    #[test]
    fn hunks_accept_final_maximum_coordinates_without_successors() {
        let final_range = range(u64::MAX - 1, 1);
        let empty_range = range(u64::MAX, 0);
        let final_number = number(u64::MAX);

        let deletion = DiffHunk::new(
            final_range,
            empty_range,
            [DiffLine::deletion(final_number, "deleted")],
        )
        .unwrap();
        deletion.validate().unwrap();

        let addition = DiffHunk::new(
            empty_range,
            final_range,
            [DiffLine::addition(final_number, "added")],
        )
        .unwrap();
        addition.validate().unwrap();

        let context = DiffHunk::new(
            final_range,
            final_range,
            [DiffLine::context(final_number, final_number, "same")],
        )
        .unwrap();
        context.validate().unwrap();
    }

    #[test]
    fn hunks_reject_count_and_coordinate_mismatches() {
        assert!(DiffHunk::new(range(0, 1), range(0, 0), []).is_err());
        assert!(
            DiffHunk::new(
                range(0, 0),
                range(0, 1),
                [DiffLine::addition(number(2), "x")]
            )
            .is_err()
        );
        assert!(
            DiffHunk::new(
                range(0, 1),
                range(0, 1),
                [DiffLine::deletion(number(1), "x")]
            )
            .is_err()
        );
        assert!(
            DiffHunk::new(
                range(0, 2),
                range(0, 2),
                [
                    DiffLine::context(number(1), number(1), "x"),
                    DiffLine::context(number(1), number(2), "y")
                ],
            )
            .is_err()
        );
        assert!(
            DiffHunk::new(
                range(0, 1),
                range(0, 1),
                [DiffLine::context(number(2), number(1), "x")],
            )
            .is_err()
        );
    }
}
