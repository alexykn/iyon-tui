//! Invariant-preserving errors for physical History ownership.

use super::HistoryUnitId;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    DuplicateUnit { unit: HistoryUnitId },
    UnitNotFound { unit: HistoryUnitId },
    UnitNotLive { unit: HistoryUnitId },
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateUnit { unit } => {
                write!(formatter, "History unit {unit:?} already exists")
            }
            Self::UnitNotFound { unit } => write!(formatter, "History unit {unit:?} was not found"),
            Self::UnitNotLive { unit } => write!(formatter, "History unit {unit:?} is not live"),
        }
    }
}

impl std::error::Error for HistoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
