//! Exhaustive retained-presentation capability table.
//!
//! This table intentionally lives with the retained-state plane rather than
//! in the structural transport. A new native ViewKind must choose a state
//! capability explicitly before it can accept a ViewState attachment.

use crate::presentation::ir::ViewKind;

/// Returns whether a native semantic kind owns an addressable presentation box.
///
/// Component indirections are structural references, not physical boxes; the
/// concrete component View owns its own retained presentation state.
pub(crate) fn presentation_state_capable(kind: &ViewKind) -> bool {
    match kind {
        ViewKind::Text(_)
        | ViewKind::Spacer { .. }
        | ViewKind::Column(_)
        | ViewKind::Row(_)
        | ViewKind::Grid(_)
        | ViewKind::Hanging(_)
        | ViewKind::Container(_)
        | ViewKind::ClampRows(_)
        | ViewKind::RowViewport(_) => true,
        ViewKind::ComponentSlot(_) => false,
    }
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
