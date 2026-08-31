//! Exhaustive retained-state capability tables.
//!
//! These tables intentionally live with the retained-state plane rather than
//! in structural transport. A new native ViewKind must choose every state
//! capability explicitly before it can accept a ViewState attachment.

use crate::presentation::ir::ViewKind;

use super::geometry::GeometryOverrides;

/// Concrete physical/structural classes used for retained-state validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StateNodeKind {
    Text,
    Spacer,
    Row,
    Column,
    Grid,
    Hanging,
    Container,
    ClampRows,
    RowViewport,
    ComponentSlot,
}

/// Exhaustive mapping from the native semantic node vocabulary to a state
/// capability class.
pub(crate) fn state_node_kind(kind: &ViewKind) -> StateNodeKind {
    match kind {
        ViewKind::Text(_) => StateNodeKind::Text,
        ViewKind::Spacer { .. } => StateNodeKind::Spacer,
        ViewKind::Row(_) => StateNodeKind::Row,
        ViewKind::Column(_) => StateNodeKind::Column,
        ViewKind::Grid(_) => StateNodeKind::Grid,
        ViewKind::Hanging(_) => StateNodeKind::Hanging,
        ViewKind::Container(_) => StateNodeKind::Container,
        ViewKind::ClampRows(_) => StateNodeKind::ClampRows,
        ViewKind::RowViewport(_) => StateNodeKind::RowViewport,
        ViewKind::ComponentSlot(_) => StateNodeKind::ComponentSlot,
    }
}

/// Returns whether a native semantic kind owns an addressable presentation box.
///
/// Component indirections are structural references, not physical boxes; the
/// concrete component View owns its own retained presentation state.
pub(crate) fn presentation_state_capable(kind: &ViewKind) -> bool {
    match state_node_kind(kind) {
        StateNodeKind::Text
        | StateNodeKind::Spacer
        | StateNodeKind::Row
        | StateNodeKind::Column
        | StateNodeKind::Grid
        | StateNodeKind::Hanging
        | StateNodeKind::Container
        | StateNodeKind::ClampRows
        | StateNodeKind::RowViewport => true,
        StateNodeKind::ComponentSlot => false,
    }
}

/// Validates all geometry overrides already stored on a ViewState against the
/// target occurrence. This is called at the H3 desired-binding boundary and
/// again by mutation validation through the desired binding.
pub(crate) fn validate_geometry_for_kind(
    kind: StateNodeKind,
    geometry: &GeometryOverrides,
) -> anyhow::Result<()> {
    if kind == StateNodeKind::ComponentSlot {
        return Err(unsupported("geometry", kind));
    }
    if geometry.gap.is_some()
        && !matches!(
            kind,
            StateNodeKind::Row | StateNodeKind::Column | StateNodeKind::Grid
        )
    {
        return Err(unsupported("gap", kind));
    }
    if let Some(alignment) = geometry.alignment {
        if alignment.horizontal.is_some() && kind != StateNodeKind::Text {
            return Err(unsupported("alignment.horizontal", kind));
        }
        if alignment.vertical.is_some() && kind != StateNodeKind::Row {
            return Err(unsupported("alignment.vertical", kind));
        }
    }
    Ok(())
}

fn unsupported(property: &str, kind: StateNodeKind) -> anyhow::Error {
    anyhow::anyhow!("UNSUPPORTED_STATE_PROPERTY: {property} is not supported on {kind:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{IntoView, View};

    #[test]
    fn concrete_layout_kinds_have_presentation_boxes() {
        let kinds = [
            View::text("text").into_view(),
            View::spacer(1),
            View::vertical(|_| {}),
            View::horizontal(|_| {}),
            View::text("hanging").into_view(),
        ];
        assert!(
            kinds
                .iter()
                .all(|view| presentation_state_capable(view.kind()))
        );
    }
}
