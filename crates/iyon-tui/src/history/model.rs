//! Ordered semantic History model.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::atomic::AtomicU64,
    time::Instant,
};

use crate::{
    id::next_nonzero_id,
    perf::{self, Counter},
    presentation::{IntoView, View},
    stream::StreamingSource,
};

use super::{
    ErasedHistoryStream, FlowBoundary, HistoryError, HistoryLayout, HistoryStreamHandle,
    HistoryUnit, HistoryUnitContent, HistoryUnitId,
    unit::{HistoryUnitLayout, HistoryUnitLayoutKey},
};

static NEXT_HISTORY_ID: AtomicU64 = AtomicU64::new(1);

/// An ordered root-level historical, live, or streaming semantic flow.
///
/// History owns unit order, semantic lifetime, and semantic layout. Native
/// durability remains private behind the host-owned native sink seam.
///
/// ```no_run
/// use iyon_tui::{Component, ComponentHandle, History, HistoryError, View};
///
/// fn build<C: Component>(handle: ComponentHandle<C>) -> Result<History, HistoryError> {
///     let mut history = History::new();
///     history.push("completed output")?;
///     let live = history.push(View::component(handle))?;
///     history.freeze(live, "final output")?;
///     Ok(history)
/// }
/// ```
pub struct History {
    pub(super) units: VecDeque<HistoryUnit>,
    /// Stable identity for detecting replacement of the History object in a
    /// retained Scene, even when its semantic/native revisions coincide.
    identity: u64,
    layout: HistoryLayout,
    cached_total_height: Cell<Option<usize>>,
    stale_cached_heights: Cell<usize>,
    revision: Cell<u64>,
    /// Display-frontier revision, separate from semantic History revision.
    /// Native scrollback promotion mutates this frontier without changing the
    /// ordered semantic units, so SceneHost can refresh its retained History
    /// branch after a transfer without rebuilding the body branch.
    native_revision: Cell<u64>,
    pub(super) native: super::native::NativeFrontier,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            units: VecDeque::new(),
            identity: next_nonzero_id(&NEXT_HISTORY_ID, "history identity exhausted").get(),
            layout: HistoryLayout::default(),
            cached_total_height: Cell::new(None),
            stale_cached_heights: Cell::new(0),
            revision: Cell::new(0),
            native_revision: Cell::new(0),
            native: super::native::NativeFrontier::default(),
        }
    }

    pub fn push(&mut self, view: impl IntoView) -> Result<HistoryUnitId, HistoryError> {
        self.push_with_boundary(view, FlowBoundary::Default)
    }

    pub fn push_with_boundary(
        &mut self,
        view: impl IntoView,
        boundary: FlowBoundary,
    ) -> Result<HistoryUnitId, HistoryError> {
        self.ensure_append_allowed()?;
        let view = view.into_view();
        let content = if view.contains_component_identity() {
            HistoryUnitContent::Live(view)
        } else {
            HistoryUnitContent::Static(view)
        };
        let id = HistoryUnitId::allocate();
        self.cached_total_height.set(None);
        self.stale_cached_heights.set(0);
        self.units.push_back(HistoryUnit {
            id,
            boundary,
            content,
            layout: RefCell::new(HistoryUnitLayout::default()),
        });
        self.bump_revision();
        Ok(id)
    }

    /// Replaces a tail Live unit with a typed open Stream without changing its
    /// identity or flow boundary.
    pub fn replace_live_with_stream<S: StreamingSource>(
        &mut self,
        unit: HistoryUnitId,
        source: S,
    ) -> Result<HistoryStreamHandle<S>, HistoryError> {
        let index = self.index_of(unit)?;
        if index + 1 != self.units.len() {
            return Err(HistoryError::LiveMustRemainTail { unit });
        }
        if !matches!(self.units[index].content, HistoryUnitContent::Live(_)) {
            return Err(HistoryError::UnitNotLive { unit });
        }
        let stream = ErasedHistoryStream::new(source).map_err(HistoryError::Stream)?;
        self.units[index].content = HistoryUnitContent::Stream(stream);
        self.invalidate_unit_layout(index);
        self.bump_revision();
        Ok(HistoryStreamHandle::new(unit))
    }

    /// Discards a transient tail Live unit without creating spacing or native
    /// history rows.
    pub fn discard_live(&mut self, unit: HistoryUnitId) -> Result<(), HistoryError> {
        let index = self.index_of(unit)?;
        if index + 1 != self.units.len() {
            return Err(HistoryError::LiveMustRemainTail { unit });
        }
        if !matches!(self.units[index].content, HistoryUnitContent::Live(_)) {
            return Err(HistoryError::UnitNotLive { unit });
        }
        self.units.remove(index);
        self.cached_total_height.set(None);
        self.stale_cached_heights.set(0);
        self.bump_revision();
        Ok(())
    }

    pub fn freeze(
        &mut self,
        unit: HistoryUnitId,
        final_view: impl IntoView,
    ) -> Result<(), HistoryError> {
        let index = self.index_of(unit)?;
        if !matches!(self.units[index].content, HistoryUnitContent::Live(_)) {
            return Err(HistoryError::UnitNotLive { unit });
        }
        let final_view = final_view.into_view();
        if final_view.contains_component_identity() {
            return Err(HistoryError::FinalViewContainsComponent { unit });
        }
        self.units[index].content = HistoryUnitContent::Static(final_view);
        self.invalidate_unit_layout(index);
        self.bump_revision();
        Ok(())
    }

    /// Attaches one typed semantic Stream as the History tail.
    ///
    /// ```text
    /// let stream = history.push_stream(source)?;
    /// history.update_stream(stream, |source| { /* mutate source */ })?;
    /// history.seal_stream(stream)?;
    /// history.push("next unit")?;
    /// ```
    pub fn push_stream<S: StreamingSource>(
        &mut self,
        source: S,
    ) -> Result<HistoryStreamHandle<S>, HistoryError> {
        self.push_stream_with_boundary(source, FlowBoundary::Default)
    }

    pub fn push_stream_with_boundary<S: StreamingSource>(
        &mut self,
        source: S,
        boundary: FlowBoundary,
    ) -> Result<HistoryStreamHandle<S>, HistoryError> {
        self.ensure_append_allowed()?;
        let stream = ErasedHistoryStream::new(source).map_err(HistoryError::Stream)?;
        let id = HistoryUnitId::allocate();
        self.cached_total_height.set(None);
        self.stale_cached_heights.set(0);
        self.units.push_back(HistoryUnit {
            id,
            boundary,
            content: HistoryUnitContent::Stream(stream),
            layout: RefCell::new(HistoryUnitLayout::default()),
        });
        self.bump_revision();
        Ok(HistoryStreamHandle::new(id))
    }

    pub fn update_stream<S: StreamingSource, R>(
        &mut self,
        handle: HistoryStreamHandle<S>,
        update: impl FnOnce(&mut S) -> R,
    ) -> Result<R, HistoryError> {
        let index = self.index_of(handle.unit())?;
        let history_unit = &mut self.units[index];
        let HistoryUnitContent::Stream(stream) = &mut history_unit.content else {
            return Err(HistoryError::UnitNotStream {
                unit: handle.unit(),
            });
        };
        let result = stream.update(handle.unit(), update);
        if result.is_ok() {
            self.bump_revision();
        }
        result
    }

    #[allow(dead_code)]
    pub(crate) fn refresh_stream<S: StreamingSource>(
        &mut self,
        handle: HistoryStreamHandle<S>,
    ) -> Result<(), HistoryError> {
        let index = self.index_of(handle.unit())?;
        let history_unit = &mut self.units[index];
        let HistoryUnitContent::Stream(stream) = &mut history_unit.content else {
            return Err(HistoryError::UnitNotStream {
                unit: handle.unit(),
            });
        };
        let result = stream.refresh::<S>(handle.unit());
        if result.is_ok() {
            self.bump_revision();
        }
        result
    }

    pub(crate) fn next_stream_wakeup(&self) -> Option<Instant> {
        self.units
            .iter()
            .filter_map(|unit| match &unit.content {
                HistoryUnitContent::Stream(stream) => stream.next_wakeup(),
                _ => None,
            })
            .min()
    }

    pub(crate) fn advance_streams(&mut self, now: Instant) -> Result<bool, HistoryError> {
        let mut changed = false;
        for unit in &mut self.units {
            if let HistoryUnitContent::Stream(stream) = &mut unit.content {
                changed |= stream.advance(now)?;
            }
        }
        if changed {
            self.bump_revision();
        }
        Ok(changed)
    }

    pub fn seal_stream<S: StreamingSource>(
        &mut self,
        handle: HistoryStreamHandle<S>,
    ) -> Result<(), HistoryError> {
        let index = self.index_of(handle.unit())?;
        let history_unit = &mut self.units[index];
        let HistoryUnitContent::Stream(stream) = &mut history_unit.content else {
            return Err(HistoryError::UnitNotStream {
                unit: handle.unit(),
            });
        };
        let result = stream.seal::<S>(handle.unit());
        if result.is_ok() {
            self.bump_revision();
        }
        result
    }

    pub(super) fn units(&self) -> impl Iterator<Item = &HistoryUnit> {
        self.units.iter()
    }

    /// Returns semantic History views that can carry retained state. Stream
    /// units produce their own projection views and never carry a host-owned
    /// ViewState attachment.
    pub(crate) fn state_views(&self) -> Vec<View> {
        self.units
            .iter()
            .filter_map(|unit| match &unit.content {
                HistoryUnitContent::Static(view) | HistoryUnitContent::Live(view)
                    if view.flags().contains_state_attachment()
                        || view.contains_component_identity() =>
                {
                    Some(view.clone())
                }
                HistoryUnitContent::Static(_) | HistoryUnitContent::Live(_) => None,
                HistoryUnitContent::Stream(_) => None,
            })
            .collect()
    }

    /// Returns semantic History views that can carry a retained ContentPort.
    /// Stream units remain on the legacy stream path until the content-plane
    /// projection tranche; static/live units are still validated at H3.
    pub(crate) fn content_views(&self) -> Vec<View> {
        self.units
            .iter()
            .filter_map(|unit| match &unit.content {
                HistoryUnitContent::Static(view) | HistoryUnitContent::Live(view)
                    if view.contains_content_identity() || view.contains_component_identity() =>
                {
                    Some(view.clone())
                }
                HistoryUnitContent::Static(_) | HistoryUnitContent::Live(_) => None,
                HistoryUnitContent::Stream(_) => None,
            })
            .collect()
    }

    /// Returns ContentPort-bearing History views after replacing one live
    /// unit, allowing host attachment validation before mutation.
    pub(crate) fn content_views_with_replacement(
        &self,
        unit: HistoryUnitId,
        replacement: &View,
    ) -> Result<Vec<View>, HistoryError> {
        let index = self.index_of(unit)?;
        if !matches!(self.units[index].content, HistoryUnitContent::Live(_)) {
            return Err(HistoryError::UnitNotLive { unit });
        }
        Ok(self
            .units
            .iter()
            .enumerate()
            .filter_map(|(current, entry)| {
                if current == index {
                    (replacement.contains_content_identity()
                        || replacement.contains_component_identity())
                    .then(|| replacement.clone())
                } else {
                    match &entry.content {
                        HistoryUnitContent::Static(view) | HistoryUnitContent::Live(view)
                            if view.contains_content_identity()
                                || view.contains_component_identity() =>
                        {
                            Some(view.clone())
                        }
                        HistoryUnitContent::Static(_) | HistoryUnitContent::Live(_) => None,
                        HistoryUnitContent::Stream(_) => None,
                    }
                }
            })
            .collect())
    }

    /// Returns the state-bearing History views after replacing one live unit
    /// with its prospective final view. The replacement is validated using the
    /// same unit rules as `freeze` before any History mutation occurs.
    pub(crate) fn state_views_with_replacement(
        &self,
        unit: HistoryUnitId,
        replacement: &View,
    ) -> Result<Vec<View>, HistoryError> {
        let index = self.index_of(unit)?;
        if !matches!(self.units[index].content, HistoryUnitContent::Live(_)) {
            return Err(HistoryError::UnitNotLive { unit });
        }
        if replacement.contains_component_identity() {
            return Err(HistoryError::FinalViewContainsComponent { unit });
        }
        Ok(self
            .units
            .iter()
            .enumerate()
            .filter_map(|(current, entry)| {
                if current == index {
                    (replacement.flags().contains_state_attachment()
                        || replacement.contains_component_identity())
                    .then(|| replacement.clone())
                } else {
                    match &entry.content {
                        HistoryUnitContent::Static(view) | HistoryUnitContent::Live(view)
                            if view.flags().contains_state_attachment()
                                || view.contains_component_identity() =>
                        {
                            Some(view.clone())
                        }
                        HistoryUnitContent::Static(_) | HistoryUnitContent::Live(_) => None,
                        HistoryUnitContent::Stream(_) => None,
                    }
                }
            })
            .collect())
    }

    pub fn layout(&self) -> HistoryLayout {
        self.layout
    }

    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub(crate) fn native_revision(&self) -> u64 {
        self.native_revision.get()
    }

    pub(crate) fn bump_native_revision(&self) {
        self.native_revision
            .set(self.native_revision.get().wrapping_add(1));
    }

    fn bump_revision(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    pub(crate) fn physical_rows_inserted(&self) -> u64 {
        self.native.physical_rows_inserted
    }

    pub fn set_layout(&mut self, layout: HistoryLayout) {
        if self.layout == layout {
            return;
        }
        self.layout = layout;
        self.invalidate_all_layout();
        self.bump_revision();
    }

    pub fn with_layout(mut self, layout: HistoryLayout) -> Self {
        self.set_layout(layout);
        self
    }

    pub(super) fn prepare_unit_layout(
        &self,
        index: usize,
        width: u16,
        key: HistoryUnitLayoutKey,
    ) -> Option<usize> {
        let mut cached = self.units[index].layout.borrow_mut();
        if cached.width == Some(width) && cached.key.as_ref() == Some(&key) {
            if let Some(height) = cached.height {
                perf::inc(Counter::HistoryCachedHeightHits);
                return Some(height);
            }
            return None;
        }
        if cached.height.is_some() && self.cached_total_height.get().is_some() {
            let total = self
                .cached_total_height
                .get()
                .expect("checked cached total")
                .saturating_sub(cached.height.expect("checked cached height"));
            self.cached_total_height.set(Some(total));
            self.stale_cached_heights
                .set(self.stale_cached_heights.get().saturating_add(1));
        }
        cached.width = Some(width);
        cached.key = Some(key);
        cached.height = None;
        None
    }

    pub(super) fn record_unit_height(&self, index: usize, height: usize) {
        if let Some(cached) = self.units.get(index) {
            let mut layout = cached.layout.borrow_mut();
            if layout.height.is_none() {
                if let Some(total) = self.cached_total_height.get() {
                    if self.stale_cached_heights.get() == 0 {
                        self.cached_total_height.set(None);
                    } else {
                        self.cached_total_height
                            .set(Some(total.saturating_add(height)));
                        self.stale_cached_heights
                            .set(self.stale_cached_heights.get().saturating_sub(1));
                    }
                }
            }
            layout.height = Some(height);
        }
    }

    pub(super) fn unit_height(&self, index: usize) -> Option<usize> {
        self.units
            .get(index)
            .and_then(|unit| unit.layout.borrow().height)
    }

    pub(super) fn cached_total_flow_height(&self) -> Option<usize> {
        if self.stale_cached_heights.get() == 0 {
            if let Some(total) = self.cached_total_height.get() {
                return Some(total);
            }
        }
        let mut total = 0usize;
        for unit in &self.units {
            let height = unit.layout.borrow().height?;
            total = total.saturating_add(height);
        }
        self.cached_total_height.set(Some(total));
        self.stale_cached_heights.set(0);
        Some(total)
    }

    pub(super) fn invalidate_unit_layout(&self, index: usize) {
        if let Some(unit) = self.units.get(index) {
            let mut layout = unit.layout.borrow_mut();
            if layout.height.is_some() && self.cached_total_height.get().is_some() {
                let total = self
                    .cached_total_height
                    .get()
                    .expect("checked cached total")
                    .saturating_sub(layout.height.expect("checked cached height"));
                self.cached_total_height.set(Some(total));
                self.stale_cached_heights
                    .set(self.stale_cached_heights.get().saturating_add(1));
            }
            *layout = HistoryUnitLayout::default();
        }
    }

    pub(super) fn invalidate_all_layout(&self) {
        self.cached_total_height.set(None);
        self.stale_cached_heights.set(0);
        for unit in &self.units {
            *unit.layout.borrow_mut() = HistoryUnitLayout::default();
        }
    }

    fn ensure_append_allowed(&self) -> Result<(), HistoryError> {
        let Some(last) = self.units.back() else {
            return Ok(());
        };
        if let HistoryUnitContent::Stream(stream) = &last.content {
            if !stream.is_sealed() {
                return Err(HistoryError::OpenStreamMustRemainTail { stream: last.id });
            }
        }
        Ok(())
    }

    fn index_of(&self, id: HistoryUnitId) -> Result<usize, HistoryError> {
        self.units
            .iter()
            .position(|unit| unit.id == id)
            .ok_or(HistoryError::UnitNotFound { unit: id })
    }
}
