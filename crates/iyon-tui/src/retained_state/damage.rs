//! Rectangle damage produced by retained-state presentation changes.

use crate::geometry::{Rect, Size};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DamageRegion {
    pub(crate) rects: Vec<Rect>,
    pub(crate) full: bool,
}

impl DamageRegion {
    pub(crate) fn full(size: Size) -> Self {
        Self {
            rects: if size.width == 0 || size.height == 0 {
                Vec::new()
            } else {
                vec![Rect::new(0, 0, size.width, size.height)]
            },
            full: true,
        }
    }

    pub(crate) fn from_rects(rects: impl IntoIterator<Item = Rect>, viewport: Size) -> Self {
        let viewport = Rect::new(0, 0, viewport.width, viewport.height);
        let mut merged = Vec::new();
        for rect in rects {
            let Some(rect) = rect.intersection(viewport) else {
                continue;
            };
            let mut current = rect;
            let mut index = 0;
            while index < merged.len() {
                if touches(current, merged[index]) {
                    current = union(current, merged.swap_remove(index));
                } else {
                    index += 1;
                }
            }
            merged.push(current);
        }
        crate::perf::add(
            crate::perf::Counter::ViewStateDamageRects,
            merged.len() as u64,
        );
        let total_area = merged
            .iter()
            .map(|rect| u32::from(rect.width) * u32::from(rect.height))
            .sum::<u32>();
        let viewport_area = u32::from(viewport.width) * u32::from(viewport.height);
        let full = merged.len() > 64
            || (viewport_area > 0 && total_area.saturating_mul(2) >= viewport_area);
        if full {
            crate::perf::inc(crate::perf::Counter::ViewStateFullDamageRepaints);
            return Self::full(viewport.size());
        }
        Self {
            rects: merged,
            full,
        }
    }
}

fn touches(left: Rect, right: Rect) -> bool {
    u32::from(left.x) <= u32::from(right.right())
        && u32::from(right.x) <= u32::from(left.right())
        && u32::from(left.y) <= u32::from(right.bottom())
        && u32::from(right.y) <= u32::from(left.bottom())
}

fn union(left: Rect, right: Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = u32::from(left.right()).max(u32::from(right.right()));
    let bottom_edge = u32::from(left.bottom()).max(u32::from(right.bottom()));
    Rect::new(
        x,
        y,
        right_edge
            .saturating_sub(u32::from(x))
            .min(u32::from(u16::MAX)) as u16,
        bottom_edge
            .saturating_sub(u32::from(y))
            .min(u32::from(u16::MAX)) as u16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_damage_is_merged_and_large_damage_is_full() {
        let damage = DamageRegion::from_rects(
            [Rect::new(1, 1, 2, 2), Rect::new(3, 2, 2, 2)],
            Size::new(20, 10),
        );
        assert_eq!(damage.rects, vec![Rect::new(1, 1, 4, 3)]);

        let full = DamageRegion::from_rects([Rect::new(0, 0, 20, 5)], Size::new(20, 10));
        assert!(full.full);
        assert_eq!(full.rects, vec![Rect::new(0, 0, 20, 10)]);
    }
}
