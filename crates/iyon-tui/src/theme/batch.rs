//! Batched theme construction for native ingress.
//!
//! The interactive `Theme::set_*` setters re-sort an entry's variants after
//! every insertion. A theme payload carries all of its variants at once, so
//! the batch accumulates entries with declaration orders assigned and sorts
//! each entry exactly once at `finish`. Duplicate-selector replacement and
//! declaration ordering match the sequential setters exactly: the last write
//! wins and takes the newest order.

use std::collections::HashMap;

use crate::content::text::TEXT_THEME_KEY;
use crate::presentation::api::{StyleSelector, StyleSpec, ThemeColor, ThemeKey};

use super::{Theme, ThemeEntry, sort_variants};
use crate::content::text::TextSelector;

#[derive(Debug, Default)]
pub(crate) struct ThemeBatch {
    colors: HashMap<ThemeKey, ThemeEntry<ThemeColor>>,
    styles: HashMap<ThemeKey, ThemeEntry<StyleSpec>>,
    next_order: u64,
}

impl ThemeBatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set_color_base(&mut self, key: ThemeKey, color: ThemeColor) {
        self.colors.entry(key).or_default().base.replace(color);
    }

    pub(crate) fn push_color_variant(
        &mut self,
        key: ThemeKey,
        selector: StyleSelector,
        color: ThemeColor,
    ) {
        let order = self.next_order();
        self.colors
            .entry(key)
            .or_default()
            .push_variant(selector, color, order);
    }

    pub(crate) fn set_style_base(&mut self, key: ThemeKey, style: StyleSpec) {
        self.styles.entry(key).or_default().base.replace(style);
    }

    pub(crate) fn push_style_variant(
        &mut self,
        key: ThemeKey,
        selector: StyleSelector,
        style: StyleSpec,
    ) {
        let order = self.next_order();
        self.styles
            .entry(key)
            .or_default()
            .push_variant(selector, style, order);
    }

    pub(crate) fn push_text_style(&mut self, selector: TextSelector, style: StyleSpec) {
        if selector.is_any() {
            self.set_style_base(ThemeKey::from(TEXT_THEME_KEY), style);
        } else {
            self.push_style_variant(
                ThemeKey::from(TEXT_THEME_KEY),
                selector.into_style_selector(),
                style,
            );
        }
    }

    /// Builds a theme from payload-ordered parts with one sort per entry.
    /// This backs the single public batched assembler so native ingress
    /// never pays per-insertion sorting.
    pub(crate) fn build_from_parts(
        colors: Vec<(
            ThemeKey,
            Option<ThemeColor>,
            Vec<(StyleSelector, ThemeColor)>,
        )>,
        styles: Vec<(ThemeKey, Option<StyleSpec>, Vec<(StyleSelector, StyleSpec)>)>,
        text_styles: Vec<(TextSelector, StyleSpec)>,
    ) -> Theme {
        let mut batch = Self::new();
        for (key, base, variants) in colors {
            if let Some(base) = base {
                batch.set_color_base(key.clone(), base);
            }
            for (selector, value) in variants {
                batch.push_color_variant(key.clone(), selector, value);
            }
        }
        for (key, base, variants) in styles {
            if let Some(base) = base {
                batch.set_style_base(key.clone(), base);
            }
            for (selector, value) in variants {
                batch.push_style_variant(key.clone(), selector, value);
            }
        }
        for (selector, style) in text_styles {
            batch.push_text_style(selector, style);
        }
        batch.finish()
    }

    pub(crate) fn finish(mut self) -> Theme {
        for entry in self.colors.values_mut() {
            sort_variants(&mut entry.variants);
        }
        for entry in self.styles.values_mut() {
            sort_variants(&mut entry.variants);
        }
        Theme {
            colors: self.colors,
            styles: self.styles,
            next_declaration_order: self.next_order,
        }
    }

    fn next_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::api::{StyleStateKey, StyleStateValue};

    fn focused_selector() -> StyleSelector {
        StyleSelector::focused()
    }

    fn state_selector() -> StyleSelector {
        StyleSelector::state(
            StyleStateKey::from_static("mode"),
            StyleStateValue::from_static("error"),
        )
    }

    #[test]
    fn batch_matches_sequential_construction_including_duplicates() {
        // Sequential reference: duplicate replacement takes the newest
        // declaration order, mixed predicate counts sort by count then order.
        let mut sequential = Theme::new();
        sequential.set_color("a", ThemeColor::Indexed(1));
        sequential.set_color_variant("a", focused_selector(), ThemeColor::Indexed(2));
        sequential.set_color_variant("a", state_selector(), ThemeColor::Indexed(3));
        sequential.set_color_variant("a", focused_selector(), ThemeColor::Indexed(4));
        sequential.set_style("s", StyleSpec::new().bold());
        sequential.set_style_variant("s", state_selector(), StyleSpec::new().italic());
        sequential.set_style_variant("s", state_selector(), StyleSpec::new().dim());

        let mut batch = ThemeBatch::new();
        batch.set_color_base(ThemeKey::from("a"), ThemeColor::Indexed(1));
        batch.push_color_variant(
            ThemeKey::from("a"),
            focused_selector(),
            ThemeColor::Indexed(2),
        );
        batch.push_color_variant(
            ThemeKey::from("a"),
            state_selector(),
            ThemeColor::Indexed(3),
        );
        batch.push_color_variant(
            ThemeKey::from("a"),
            focused_selector(),
            ThemeColor::Indexed(4),
        );
        batch.set_style_base(ThemeKey::from("s"), StyleSpec::new().bold());
        batch.push_style_variant(
            ThemeKey::from("s"),
            state_selector(),
            StyleSpec::new().italic(),
        );
        batch.push_style_variant(
            ThemeKey::from("s"),
            state_selector(),
            StyleSpec::new().dim(),
        );

        assert_eq!(batch.finish(), sequential);
    }

    #[test]
    fn batch_text_styles_match_set_text_style() {
        use crate::content::text::{TextRole, TextSelector as ContentTextSelector};

        let mut sequential = Theme::new();
        sequential.set_text_style(
            ContentTextSelector::role(TextRole::Link),
            StyleSpec::new().underline(),
        );
        sequential.set_text_style(ContentTextSelector::any(), StyleSpec::new().dim());

        let mut batch = ThemeBatch::new();
        batch.push_text_style(
            ContentTextSelector::role(TextRole::Link),
            StyleSpec::new().underline(),
        );
        batch.push_text_style(ContentTextSelector::any(), StyleSpec::new().dim());

        assert_eq!(batch.finish(), sequential);
    }
}
