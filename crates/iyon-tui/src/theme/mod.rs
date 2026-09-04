//! Application-owned semantic paint policy.
//!
//! Themes contain named colors and sparse named text styles. They deliberately
//! do not contain layout or terminal geometry.

mod framework;

pub(crate) use framework::framework_theme;

use std::collections::HashMap;

use crate::presentation::api::{
    StyleFacts, StyleSelector, StyleSpec, StyleStates, ThemeColor, ThemeKey,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ThemeVariant<T> {
    selector: StyleSelector,
    value: T,
    declaration_order: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ThemeEntry<T> {
    base: Option<T>,
    variants: Vec<ThemeVariant<T>>,
}

impl<T> Default for ThemeEntry<T> {
    fn default() -> Self {
        Self {
            base: None,
            variants: Vec::new(),
        }
    }
}

impl<T> ThemeEntry<T> {
    fn set_variant(
        &mut self,
        selector: StyleSelector,
        value: T,
        declaration_order: u64,
    ) -> Option<T> {
        let old = if let Some(variant) = self
            .variants
            .iter_mut()
            .find(|variant| variant.selector == selector)
        {
            variant.declaration_order = declaration_order;
            Some(std::mem::replace(&mut variant.value, value))
        } else {
            self.variants.push(ThemeVariant {
                selector,
                value,
                declaration_order,
            });
            None
        };
        sort_variants(&mut self.variants);
        old
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Theme {
    colors: HashMap<ThemeKey, ThemeEntry<ThemeColor>>,
    styles: HashMap<ThemeKey, ThemeEntry<StyleSpec>>,
    next_declaration_order: u64,
}

fn sort_variants<T>(variants: &mut [ThemeVariant<T>]) {
    variants.sort_by_key(|variant| {
        (
            variant.selector.predicate_count(),
            variant.declaration_order,
        )
    });
}

impl Theme {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_color(mut self, key: impl Into<ThemeKey>, color: ThemeColor) -> Self {
        self.set_color(key, color);
        self
    }

    #[must_use]
    pub fn with_color_variant(
        mut self,
        key: impl Into<ThemeKey>,
        selector: StyleSelector,
        color: ThemeColor,
    ) -> Self {
        self.set_color_variant(key, selector, color);
        self
    }

    #[must_use]
    pub fn with_style(mut self, key: impl Into<ThemeKey>, style: StyleSpec) -> Self {
        self.set_style(key, style);
        self
    }

    #[must_use]
    pub fn with_style_variant(
        mut self,
        key: impl Into<ThemeKey>,
        selector: StyleSelector,
        style: StyleSpec,
    ) -> Self {
        self.set_style_variant(key, selector, style);
        self
    }

    pub fn set_color(&mut self, key: impl Into<ThemeKey>, color: ThemeColor) -> Option<ThemeColor> {
        self.colors
            .entry(key.into())
            .or_default()
            .base
            .replace(color)
    }

    pub fn set_color_variant(
        &mut self,
        key: impl Into<ThemeKey>,
        selector: StyleSelector,
        color: ThemeColor,
    ) -> Option<ThemeColor> {
        let order = self.next_order();
        self.colors
            .entry(key.into())
            .or_default()
            .set_variant(selector, color, order)
    }

    pub fn set_style(&mut self, key: impl Into<ThemeKey>, style: StyleSpec) -> Option<StyleSpec> {
        self.styles
            .entry(key.into())
            .or_default()
            .base
            .replace(style)
    }

    pub fn set_style_variant(
        &mut self,
        key: impl Into<ThemeKey>,
        selector: StyleSelector,
        style: StyleSpec,
    ) -> Option<StyleSpec> {
        let order = self.next_order();
        self.styles
            .entry(key.into())
            .or_default()
            .set_variant(selector, style, order)
    }

    #[must_use]
    pub fn color(&self, key: &str) -> Option<ThemeColor> {
        self.resolve_color(
            key,
            false,
            false,
            &StyleStates::default(),
            &StyleFacts::default(),
        )
    }

    #[must_use]
    pub fn style(&self, key: &str) -> Option<&StyleSpec> {
        self.styles.get(key)?.base.as_ref()
    }

    pub(crate) fn resolve_color(
        &self,
        key: &str,
        focused: bool,
        focus_within: bool,
        states: &StyleStates,
        facts: &StyleFacts,
    ) -> Option<ThemeColor> {
        let entry = self.colors.get(key)?;
        let mut resolved = entry.base;
        for variant in entry.variants.iter().filter(|variant| {
            variant
                .selector
                .matches(focused, focus_within, states, facts)
        }) {
            resolved = Some(variant.value);
        }
        resolved
    }

    pub(crate) fn resolve_style(
        &self,
        key: &str,
        focused: bool,
        focus_within: bool,
        states: &StyleStates,
        facts: &StyleFacts,
    ) -> Option<StyleSpec> {
        let entry = self.styles.get(key)?;
        let mut resolved = entry.base.clone().unwrap_or_default();
        for variant in entry.variants.iter().filter(|variant| {
            variant
                .selector
                .matches(focused, focus_within, states, facts)
        }) {
            resolved.overlay(&variant.value);
        }
        Some(resolved)
    }

    fn next_order(&mut self) -> u64 {
        let order = self.next_declaration_order;
        self.next_declaration_order = self.next_declaration_order.saturating_add(1);
        order
    }
}
