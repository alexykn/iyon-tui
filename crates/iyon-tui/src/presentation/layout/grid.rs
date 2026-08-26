//! Span-aware two-dimensional track allocation.
//!
//! This solver is independent of Views. Row/Column continue to use
//! [`allocate_tracks`](super::tracks::allocate_tracks).

use crate::presentation::ir::{PersistentSeq, TrackSize};

use super::tracks::TrackAllocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SpanRequirement {
    pub(super) start: usize,
    pub(super) span: usize,
    pub(super) preferred: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FlexMode {
    Intrinsic,
    Fill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GrowClass {
    Content,
    Flex,
}

pub(super) trait TrackList {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<TrackSize>;
}

impl TrackList for [TrackSize] {
    fn len(&self) -> usize {
        <[TrackSize]>::len(self)
    }

    fn get(&self, index: usize) -> Option<TrackSize> {
        <[TrackSize]>::get(self, index).copied()
    }
}

impl TrackList for Vec<TrackSize> {
    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn get(&self, index: usize) -> Option<TrackSize> {
        self.as_slice().get(index).copied()
    }
}

impl<const N: usize> TrackList for [TrackSize; N] {
    fn len(&self) -> usize {
        N
    }

    fn get(&self, index: usize) -> Option<TrackSize> {
        self.as_slice().get(index).copied()
    }
}

impl TrackList for PersistentSeq<TrackSize> {
    fn len(&self) -> usize {
        PersistentSeq::len(self)
    }

    fn get(&self, index: usize) -> Option<TrackSize> {
        PersistentSeq::get(self, index).copied()
    }
}

pub(super) fn allocate_grid_tracks<T: TrackList + ?Sized>(
    available: u16,
    requested_gap: u16,
    tracks: &T,
    requirements: &[SpanRequirement],
    flex_mode: FlexMode,
) -> TrackAllocation {
    let count = tracks.len();
    if count == 0 {
        return TrackAllocation {
            tracks: Vec::new(),
            gap: 0,
        };
    }

    let gap_count = count.saturating_sub(1);
    let available_us = usize::from(available);
    let gap = usize::from(requested_gap).min(available_us.checked_div(gap_count).unwrap_or(0));
    let capacity = available_us.saturating_sub(gap.saturating_mul(gap_count));
    let mut allocation = vec![0u16; count];
    let mut used = 0usize;

    for index in 0..tracks.len() {
        let track = tracks.get(index).expect("track index is in range");
        if let TrackSize::Fixed(requested) = track {
            let amount = usize::from(requested).min(capacity.saturating_sub(used));
            allocation[index] = amount as u16;
            used += amount;
        }
    }

    for index in 0..tracks.len() {
        let track = tracks.get(index).expect("track index is in range");
        let minimum = match track {
            TrackSize::Flex { min } => usize::from(min),
            TrackSize::FlexMax { min, max } => usize::from(min).min(usize::from(max)),
            _ => continue,
        };
        let amount = minimum.min(capacity.saturating_sub(used));
        allocation[index] = amount as u16;
        used += amount;
    }

    let mut remaining = capacity.saturating_sub(used);
    let mut ordered: Vec<(usize, &SpanRequirement)> = requirements.iter().enumerate().collect();
    ordered.sort_by_key(|(index, requirement)| (requirement.span, requirement.start, *index));
    for (_, requirement) in ordered {
        if requirement.span == 0 || requirement.start >= count {
            continue;
        }
        let span = requirement
            .span
            .min(count.saturating_sub(requirement.start));
        let internal_gaps = gap.saturating_mul(span.saturating_sub(1));
        let required = usize::from(requirement.preferred).saturating_sub(internal_gaps);
        let current = span_sum(&allocation, requirement.start, span);
        let deficit = required.saturating_sub(current).min(remaining);
        if deficit == 0 {
            continue;
        }
        let mut granted = grow_class(
            &mut allocation,
            tracks,
            requirement.start,
            span,
            deficit,
            GrowClass::Content,
        );
        if granted < deficit {
            granted += grow_class(
                &mut allocation,
                tracks,
                requirement.start,
                span,
                deficit - granted,
                GrowClass::Flex,
            );
        }
        remaining = remaining.saturating_sub(granted);
    }

    if matches!(flex_mode, FlexMode::Fill) && remaining > 0 {
        grow_flex_fill(&mut allocation, tracks, remaining);
    }

    TrackAllocation {
        tracks: allocation,
        gap: gap.min(usize::from(u16::MAX)) as u16,
    }
}

pub(super) fn track_offset(allocation: &TrackAllocation, index: usize) -> u16 {
    let before = allocation.tracks.len().min(index);
    let widths = span_sum(&allocation.tracks, 0, before);
    let gaps = usize::from(allocation.gap).saturating_mul(before);
    widths.saturating_add(gaps).min(usize::from(u16::MAX)) as u16
}

pub(super) fn span_extent(allocation: &TrackAllocation, start: usize, span: usize) -> u16 {
    if span == 0 {
        return 0;
    }
    let widths = span_sum(&allocation.tracks, start, span);
    let gaps = usize::from(allocation.gap).saturating_mul(span.saturating_sub(1));
    widths.saturating_add(gaps).min(usize::from(u16::MAX)) as u16
}

fn span_sum(tracks: &[u16], start: usize, span: usize) -> usize {
    tracks
        .iter()
        .skip(start)
        .take(span)
        .map(|track| usize::from(*track))
        .sum()
}

fn grow_class<T: TrackList + ?Sized>(
    allocation: &mut [u16],
    tracks: &T,
    start: usize,
    span: usize,
    deficit: usize,
    class: GrowClass,
) -> usize {
    let mut remaining = deficit;
    let mut active: Vec<usize> = (start..start.saturating_add(span))
        .filter(|&index| {
            index < tracks.len()
                && grow_room(
                    tracks.get(index).expect("track index is in range"),
                    allocation[index],
                    class,
                ) > 0
        })
        .collect();
    let mut granted_total = 0usize;
    while remaining > 0 && !active.is_empty() {
        let each = remaining / active.len();
        let extra = remaining % active.len();
        let mut next_active = Vec::with_capacity(active.len());
        let mut round = 0usize;
        for (order, index) in active.iter().copied().enumerate() {
            let requested = each.saturating_add(usize::from(order < extra));
            let room = grow_room(
                tracks.get(index).expect("track index is in range"),
                allocation[index],
                class,
            );
            let granted = requested.min(room).min(remaining.saturating_sub(round));
            allocation[index] = allocation[index].saturating_add(granted as u16);
            round += granted;
            if grow_room(
                tracks.get(index).expect("track index is in range"),
                allocation[index],
                class,
            ) > 0
            {
                next_active.push(index);
            }
        }
        if round == 0 {
            break;
        }
        granted_total += round;
        remaining -= round;
        active = next_active;
    }
    granted_total
}

fn grow_flex_fill<T: TrackList + ?Sized>(
    allocation: &mut [u16],
    tracks: &T,
    remaining: usize,
) -> usize {
    grow_class(
        allocation,
        tracks,
        0,
        tracks.len(),
        remaining,
        GrowClass::Flex,
    )
}

fn grow_room(track: TrackSize, current: u16, class: GrowClass) -> usize {
    let cap = match (track, class) {
        (TrackSize::Content { max }, GrowClass::Content) => usize::from(max.unwrap_or(u16::MAX)),
        (TrackSize::Flex { .. }, GrowClass::Flex) => usize::from(u16::MAX),
        (TrackSize::FlexMax { max, .. }, GrowClass::Flex) => usize::from(max),
        _ => return 0,
    };
    cap.saturating_sub(usize::from(current))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> TrackSize {
        TrackSize::Content { max: None }
    }

    fn req(start: usize, span: usize, preferred: u16) -> SpanRequirement {
        SpanRequirement {
            start,
            span,
            preferred,
        }
    }

    #[test]
    fn content_single_track_takes_preferred() {
        let allocated = allocate_grid_tracks(20, 0, &[content()], &[req(0, 1, 7)], FlexMode::Fill);
        assert_eq!(allocated.tracks, [7]);
    }

    #[test]
    fn fixed_track_does_not_grow() {
        let allocated = allocate_grid_tracks(
            20,
            0,
            &[TrackSize::Fixed(5)],
            &[req(0, 1, 10)],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [5]);
    }

    #[test]
    fn content_max_caps_growth() {
        let allocated = allocate_grid_tracks(
            20,
            0,
            &[TrackSize::Content { max: Some(4) }],
            &[req(0, 1, 10)],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [4]);
    }

    #[test]
    fn flex_takes_remainder_after_fixed_and_content() {
        let allocated = allocate_grid_tracks(
            20,
            0,
            &[
                TrackSize::Fixed(3),
                TrackSize::Content { max: None },
                TrackSize::Flex { min: 1 },
            ],
            &[req(0, 1, 3), req(1, 1, 5), req(2, 1, 4)],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [3, 5, 12]);
    }

    #[test]
    fn flex_fill_consumes_remaining_after_requirements() {
        let fill = allocate_grid_tracks(
            10,
            0,
            &[TrackSize::Flex { min: 1 }],
            &[req(0, 1, 3)],
            FlexMode::Fill,
        );
        assert_eq!(fill.tracks, [10]);
        let intrinsic = allocate_grid_tracks(
            10,
            0,
            &[TrackSize::Flex { min: 1 }],
            &[req(0, 1, 3)],
            FlexMode::Intrinsic,
        );
        assert_eq!(intrinsic.tracks, [3]);
    }

    #[test]
    fn spanning_content_splits_fairly() {
        let allocated = allocate_grid_tracks(
            20,
            0,
            &[content(), content()],
            &[req(0, 2, 9)],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [5, 4]);
    }

    #[test]
    fn fixed_plus_content_span_grows_only_content() {
        let allocated = allocate_grid_tracks(
            20,
            0,
            &[TrackSize::Fixed(3), content()],
            &[req(0, 2, 10)],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [3, 7]);
    }

    #[test]
    fn gap_contributes_to_span_area() {
        let allocated = allocate_grid_tracks(
            20,
            1,
            &[TrackSize::Fixed(3), content()],
            &[req(0, 2, 10)],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [3, 6]);
        assert_eq!(allocated.gap, 1);
        assert_eq!(span_extent(&allocated, 0, 2), 10);
    }

    #[test]
    fn flex_max_redistributes_excess() {
        let allocated = allocate_grid_tracks(
            10,
            0,
            &[
                TrackSize::FlexMax { min: 1, max: 3 },
                TrackSize::Flex { min: 1 },
            ],
            &[],
            FlexMode::Fill,
        );
        assert_eq!(allocated.tracks, [3, 7]);
    }

    #[test]
    fn narrow_capacity_never_exceeds_available() {
        let allocated = allocate_grid_tracks(
            6,
            0,
            &[TrackSize::Fixed(5), TrackSize::Fixed(5)],
            &[],
            FlexMode::Fill,
        );
        assert_eq!(
            allocated
                .tracks
                .iter()
                .map(|track| usize::from(*track))
                .sum::<usize>(),
            6
        );
    }

    #[test]
    fn gaps_clamp_so_tracks_plus_gaps_fit() {
        let allocated = allocate_grid_tracks(
            4,
            10,
            &[content(), content(), content()],
            &[],
            FlexMode::Fill,
        );
        let gap_total = usize::from(allocated.gap) * 2;
        let track_total: usize = allocated
            .tracks
            .iter()
            .map(|track| usize::from(*track))
            .sum();
        assert!(gap_total + track_total <= 4);
        assert_eq!(track_offset(&allocated, 0), 0);
        let _ = track_offset(&allocated, 2);
        let _ = span_extent(&allocated, 0, 3);
    }

    #[test]
    fn conflicting_spans_stay_within_capacity() {
        let allocated = allocate_grid_tracks(
            40,
            0,
            &[content(), content(), content()],
            &[req(0, 2, 8), req(1, 2, 10)],
            FlexMode::Fill,
        );
        let total: usize = allocated
            .tracks
            .iter()
            .map(|track| usize::from(*track))
            .sum();
        assert!(total <= 40);
        assert!(span_extent(&allocated, 0, 2) >= 8);
        assert!(span_extent(&allocated, 1, 2) >= 10);
    }
}
