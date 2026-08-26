//! Type-erased semantic stream units for ordered History.

use std::{any::Any, cell::RefCell, marker::PhantomData, sync::Arc, time::Instant};

use crate::{
    View,
    stream::{
        CompiledStream, StreamModel, StreamModelError, StreamOffset, StreamRevision,
        StreamRowIndex, StreamSnapshot, StreamingSource, build_index_from, reindex_in_place,
        window_view,
    },
};

use super::{HistoryError, HistoryUnitId};

pub(crate) struct ErasedHistoryStream {
    state: Box<dyn ErasedHistoryStreamState>,
}

impl ErasedHistoryStream {
    pub(crate) fn new<S: StreamingSource>(source: S) -> Result<Self, StreamModelError> {
        let typed = TypedHistoryStream::new(source)?;
        Ok(Self {
            state: Box::new(typed),
        })
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> &StreamSnapshot {
        self.state.snapshot()
    }

    pub(crate) fn is_sealed(&self) -> bool {
        self.state.is_sealed()
    }

    pub(crate) fn next_wakeup(&self) -> Option<Instant> {
        self.state.next_wakeup()
    }

    pub(crate) fn advance(&mut self, now: Instant) -> Result<bool, HistoryError> {
        self.state.advance(now)
    }

    pub(crate) fn prepare_from(&self, start: StreamOffset, width: u16) -> Arc<StreamRowIndex> {
        self.state.prepare_from(start, width)
    }

    pub(crate) fn window_view(&self, index: &StreamRowIndex, top_row: usize, height: u16) -> View {
        self.state.window_view(index, top_row, height)
    }

    pub(crate) fn compile_from(
        &self,
        offset: StreamOffset,
        width: u16,
        theme: &crate::Theme,
    ) -> CompiledStream {
        self.state.compile_from(offset, width, theme)
    }

    pub(crate) fn release_resident_through(&mut self, offset: StreamOffset) {
        self.state.release_resident_through(offset);
    }

    pub(crate) fn semantic_base(&self) -> StreamOffset {
        self.state.semantic_base()
    }

    pub(crate) fn source_end(&self) -> StreamOffset {
        self.state.source_end()
    }

    pub(crate) fn revision(&self) -> StreamRevision {
        self.state.revision()
    }

    #[allow(dead_code)]
    pub(crate) fn refresh<S: StreamingSource>(
        &mut self,
        unit: HistoryUnitId,
    ) -> Result<(), HistoryError> {
        self.typed_mut::<S>(unit)?.refresh(unit)
    }

    pub(crate) fn seal<S: StreamingSource>(
        &mut self,
        unit: HistoryUnitId,
    ) -> Result<(), HistoryError> {
        self.typed_mut::<S>(unit)?.seal(unit)
    }

    pub(crate) fn update<S: StreamingSource, R>(
        &mut self,
        unit: HistoryUnitId,
        update: impl FnOnce(&mut S) -> R,
    ) -> Result<R, HistoryError> {
        let typed = self.typed_mut::<S>(unit)?;
        if typed.sealed {
            return Err(HistoryError::StreamAlreadySealed { unit });
        }
        let result = update(typed.model.source_mut());
        typed.model.refresh().map_err(HistoryError::Stream)?;
        Ok(result)
    }

    fn typed_mut<S: StreamingSource>(
        &mut self,
        unit: HistoryUnitId,
    ) -> Result<&mut TypedHistoryStream<S>, HistoryError> {
        self.state
            .as_any_mut()
            .downcast_mut::<TypedHistoryStream<S>>()
            .ok_or(HistoryError::StreamTypeMismatch { unit })
    }
}

trait ErasedHistoryStreamState: Any {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    #[cfg(test)]
    fn snapshot(&self) -> &StreamSnapshot;
    fn is_sealed(&self) -> bool;
    fn next_wakeup(&self) -> Option<Instant>;
    fn advance(&mut self, now: Instant) -> Result<bool, HistoryError>;
    fn prepare_from(&self, start: StreamOffset, width: u16) -> Arc<StreamRowIndex>;
    fn window_view(&self, index: &StreamRowIndex, top_row: usize, height: u16) -> View;
    fn compile_from(
        &self,
        offset: StreamOffset,
        width: u16,
        theme: &crate::Theme,
    ) -> CompiledStream;
    fn release_resident_through(&mut self, offset: StreamOffset);
    fn semantic_base(&self) -> StreamOffset;
    fn source_end(&self) -> StreamOffset;
    fn revision(&self) -> StreamRevision;
}

struct StreamLayoutCache {
    width: u16,
    semantic_base: StreamOffset,
    indexed_from: StreamOffset,
    indexed_through: StreamOffset,
    revision: StreamRevision,
    stable_through: StreamOffset,
    semantic_changed_from: StreamOffset,
    visual_restart_from: StreamOffset,
    index: Arc<StreamRowIndex>,
}

struct TypedHistoryStream<S: StreamingSource> {
    model: StreamModel<S>,
    sealed: bool,
    published: Option<StreamSnapshot>,
    layout: RefCell<Option<StreamLayoutCache>>,
}

impl<S: StreamingSource> TypedHistoryStream<S> {
    fn new(source: S) -> Result<Self, StreamModelError> {
        let model = StreamModel::new(source)?;
        let sealed = model.source().is_sealed();
        if sealed && model.snapshot().stable_through() != model.snapshot().source_end() {
            return Err(StreamModelError::UnstableAfterSeal);
        }
        let published = sealed.then(|| model.snapshot().clone());
        Ok(Self {
            model,
            sealed,
            published,
            layout: RefCell::new(None),
        })
    }
}

impl<S: StreamingSource> TypedHistoryStream<S> {
    #[allow(dead_code)]
    fn refresh(&mut self, unit: HistoryUnitId) -> Result<(), HistoryError> {
        if self.sealed {
            let observed = self.model.source().snapshot();
            if !self.model.source().is_sealed() || self.published.as_ref() != Some(&observed) {
                return Err(HistoryError::SealedStreamChanged { unit });
            }
            return Ok(());
        }
        self.model.refresh().map_err(HistoryError::Stream)
    }

    fn seal(&mut self, unit: HistoryUnitId) -> Result<(), HistoryError> {
        if self.sealed {
            let observed = self.model.source().snapshot();
            if !self.model.source().is_sealed() || self.published.as_ref() != Some(&observed) {
                return Err(HistoryError::SealedStreamChanged { unit });
            }
            return Ok(());
        }
        self.model.seal().map_err(HistoryError::Stream)?;
        self.sealed = true;
        self.published = Some(self.model.snapshot().clone());
        Ok(())
    }
}

impl<S: StreamingSource> ErasedHistoryStreamState for TypedHistoryStream<S> {
    #[cfg(test)]
    fn snapshot(&self) -> &StreamSnapshot {
        self.model.snapshot()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_sealed(&self) -> bool {
        self.sealed
    }

    fn next_wakeup(&self) -> Option<Instant> {
        (!self.sealed)
            .then(|| self.model.source().next_wakeup())
            .flatten()
    }

    fn advance(&mut self, now: Instant) -> Result<bool, HistoryError> {
        if self.sealed {
            return Ok(false);
        }
        let before_deadline = self.model.source().next_wakeup();
        let due = before_deadline.is_some_and(|deadline| deadline <= now);
        let source_changed = self.model.source_mut().advance(now);
        let after_advance_deadline = self.model.source().next_wakeup();
        if !source_changed {
            debug_assert!(
                !due || after_advance_deadline.is_none_or(|deadline| deadline > now),
                "temporal stream must progress or move its deadline"
            );
            return Ok(false);
        }
        let before_revision = self.model.snapshot().revision();
        self.model.refresh().map_err(HistoryError::Stream)?;
        let after_deadline = self.model.source().next_wakeup();
        debug_assert!(
            !due || self.model.snapshot().revision() > before_revision
                || after_deadline.is_none_or(|deadline| deadline > now),
            "temporal stream must progress or move its deadline"
        );
        Ok(true)
    }

    fn prepare_from(&self, start: StreamOffset, width: u16) -> Arc<StreamRowIndex> {
        let snapshot = self.model.snapshot();
        let mut cache = self.layout.borrow_mut();
        let Some(previous) = cache.as_ref() else {
            let index = build_index_from(&self.model, start, width);
            let index = Arc::new(index);
            *cache = Some(StreamLayoutCache {
                width,
                semantic_base: self.model.semantic_base(),
                indexed_from: start,
                indexed_through: index.indexed_through,
                revision: snapshot.revision,
                stable_through: snapshot.stable_through,
                semantic_changed_from: index.semantic_changed_from,
                visual_restart_from: index.visual_restart_from,
                index: Arc::clone(&index),
            });
            return index;
        };
        if previous.width == width
            && previous.indexed_from == start
            && previous.revision == snapshot.revision
            && previous.indexed_through == snapshot.source_end
            && previous.stable_through <= snapshot.stable_through
            && previous.semantic_base == self.model.semantic_base()
        {
            return Arc::clone(&previous.index);
        }

        // The damage begins at the previous semantic stable frontier, not the
        // previous source end. This matters for Markdown/smoother streams: a
        // partially published block may have shifted while its source end was
        // already known, so only content before `stable_through` is immutable.
        let semantic_changed_from = if previous.width == width && previous.indexed_from == start {
            self.model.semantic_changed_from().max(start)
        } else {
            start
        };
        let index = {
            let entry = cache.as_mut().expect("stream layout cache exists");
            reindex_in_place(
                &self.model,
                Arc::make_mut(&mut entry.index),
                start,
                semantic_changed_from,
                width,
            );
            Arc::clone(&entry.index)
        };
        *cache = Some(StreamLayoutCache {
            width,
            semantic_base: self.model.semantic_base(),
            indexed_from: start,
            indexed_through: index.indexed_through,
            revision: snapshot.revision,
            stable_through: snapshot.stable_through,
            semantic_changed_from: index.semantic_changed_from,
            visual_restart_from: index.visual_restart_from,
            index: Arc::clone(&index),
        });
        index
    }

    fn window_view(&self, index: &StreamRowIndex, top_row: usize, height: u16) -> View {
        window_view(&self.model, index, top_row, height)
    }

    fn compile_from(
        &self,
        offset: StreamOffset,
        width: u16,
        theme: &crate::Theme,
    ) -> CompiledStream {
        self.model.compile_from(offset, width, theme)
    }

    fn release_resident_through(&mut self, offset: StreamOffset) {
        self.model.release_resident_through(offset);
        if let Some(cache) = self.layout.get_mut().as_mut() {
            Arc::make_mut(&mut cache.index).retain_from(offset);
            cache.semantic_base = cache.semantic_base.max(offset);
            cache.indexed_from = cache.indexed_from.max(offset);
            cache.semantic_changed_from = cache.semantic_changed_from.max(offset);
            cache.visual_restart_from = cache.visual_restart_from.max(offset);
        }
    }

    fn semantic_base(&self) -> StreamOffset {
        self.model.semantic_base()
    }

    fn source_end(&self) -> StreamOffset {
        self.model.snapshot().source_end()
    }

    fn revision(&self) -> StreamRevision {
        self.model.snapshot().revision()
    }
}

/// Typed, non-owning identity for a History stream unit.
///
/// The source type remains attached to the handle so update, refresh, and
/// seal operations never require an application-facing erased stream type.
pub struct HistoryStreamHandle<S: StreamingSource> {
    unit: HistoryUnitId,
    marker: PhantomData<fn() -> S>,
}

impl<S: StreamingSource> Copy for HistoryStreamHandle<S> {}

impl<S: StreamingSource> Clone for HistoryStreamHandle<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: StreamingSource> PartialEq for HistoryStreamHandle<S> {
    fn eq(&self, other: &Self) -> bool {
        self.unit == other.unit
    }
}

impl<S: StreamingSource> Eq for HistoryStreamHandle<S> {}

impl<S: StreamingSource> std::fmt::Debug for HistoryStreamHandle<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("HistoryStreamHandle")
    }
}

impl<S: StreamingSource> HistoryStreamHandle<S> {
    pub(crate) const fn new(unit: HistoryUnitId) -> Self {
        Self {
            unit,
            marker: PhantomData,
        }
    }

    pub const fn unit(self) -> HistoryUnitId {
        self.unit
    }
}
