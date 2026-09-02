//! Invariant-preserving errors for the public History model.

use super::HistoryUnitId;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    UnitNotFound { unit: HistoryUnitId },
    UnitNotLive { unit: HistoryUnitId },
    LiveMustRemainTail { unit: HistoryUnitId },
    FinalViewContainsComponent { unit: HistoryUnitId },
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnitNotFound { unit } => write!(formatter, "History unit {unit:?} was not found"),
            Self::UnitNotLive { unit } => write!(formatter, "History unit {unit:?} is not live"),
            Self::LiveMustRemainTail { unit } => write!(
                formatter,
                "live History unit {unit:?} must remain the History tail"
            ),
            Self::FinalViewContainsComponent { unit } => write!(
                formatter,
                "final view for History unit {unit:?} contains a component"
            ),
        }
    }
}

impl std::error::Error for HistoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
