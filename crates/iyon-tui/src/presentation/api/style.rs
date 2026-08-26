//! Backend-neutral semantic styling and decoration vocabulary.

use std::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
};

use unicode_segmentation::UnicodeSegmentation;

/// Vertical alignment for children in a horizontal composition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
}
/// Sparse backend-neutral text-style intent. Unspecified fields inherit from
/// the preceding cascade layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleSpec {
    pub(crate) foreground: Option<ColorSpec>,
    pub(crate) background: Option<ColorSpec>,
    pub(crate) attributes: TextAttributeSpec,
}

impl StyleSpec {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an explicit plain-text attribute patch.
    ///
    /// Unlike [`StyleSpec::new`], which leaves all attributes unspecified and
    /// therefore inheritable, `plain` explicitly disables every supported
    /// text attribute. Foreground and background remain unspecified.
    pub fn plain() -> Self {
        Self::new()
            .attribute(TextAttribute::Bold, false)
            .attribute(TextAttribute::Dim, false)
            .attribute(TextAttribute::Italic, false)
            .attribute(TextAttribute::Underline, false)
            .attribute(TextAttribute::Reversed, false)
            .attribute(TextAttribute::Strikethrough, false)
    }

    pub fn foreground(mut self, color: ColorSpec) -> Self {
        self.foreground = Some(color);
        self
    }

    pub fn background(mut self, color: ColorSpec) -> Self {
        self.background = Some(color);
        self
    }

    pub fn bold(self) -> Self {
        self.attribute(TextAttribute::Bold, true)
    }

    pub fn dim(self) -> Self {
        self.attribute(TextAttribute::Dim, true)
    }

    pub fn italic(self) -> Self {
        self.attribute(TextAttribute::Italic, true)
    }

    pub fn underline(self) -> Self {
        self.attribute(TextAttribute::Underline, true)
    }

    pub fn reversed(self) -> Self {
        self.attribute(TextAttribute::Reversed, true)
    }

    pub fn strikethrough(self) -> Self {
        self.attribute(TextAttribute::Strikethrough, true)
    }

    pub fn attribute(mut self, attribute: TextAttribute, enabled: bool) -> Self {
        self.attributes.set(attribute, enabled);
        self
    }

    pub fn set_foreground(&mut self, color: ColorSpec) {
        self.foreground = Some(color);
    }

    pub fn set_background(&mut self, color: ColorSpec) {
        self.background = Some(color);
    }

    pub fn set_attribute(&mut self, attribute: TextAttribute, enabled: bool) {
        self.attributes.set(attribute, enabled);
    }

    pub fn with_attributes(mut self, attributes: TextAttributeSpec) -> Self {
        self.attributes = attributes;
        self
    }

    pub fn attribute_value(&self, attribute: TextAttribute) -> Option<bool> {
        match attribute {
            TextAttribute::Bold => self.attributes.bold,
            TextAttribute::Dim => self.attributes.dim,
            TextAttribute::Italic => self.attributes.italic,
            TextAttribute::Underline => self.attributes.underline,
            TextAttribute::Reversed => self.attributes.reversed,
            TextAttribute::Strikethrough => self.attributes.strikethrough,
        }
    }

    /// Applies the explicitly specified fields from `incoming`; it is the
    /// more-specific patch and never clears unspecified fields.
    pub(crate) fn overlay(&mut self, incoming: &Self) {
        if incoming.foreground.is_some() {
            self.foreground = incoming.foreground.clone();
        }
        if incoming.background.is_some() {
            self.background = incoming.background.clone();
        }
        self.attributes.overlay(incoming.attributes);
    }
}
/// Insets applied to a semantic view's surface. Padding belongs to the
/// decorated view, not its structural child.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Insets {
    pub(crate) top: u16,
    pub(crate) right: u16,
    pub(crate) bottom: u16,
    pub(crate) left: u16,
}

impl Insets {
    pub const ZERO: Self = Self {
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
    };

    pub const fn all(value: u16) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub const fn vertical(value: u16) -> Self {
        Self {
            top: value,
            bottom: value,
            ..Self::ZERO
        }
    }

    pub const fn horizontal(value: u16) -> Self {
        Self {
            right: value,
            left: value,
            ..Self::ZERO
        }
    }

    /// Creates insets in top, right, bottom, left order.
    pub const fn new(top: u16, right: u16, bottom: u16, left: u16) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn top(self) -> u16 {
        self.top
    }
    pub const fn right(self) -> u16 {
        self.right
    }
    pub const fn bottom(self) -> u16 {
        self.bottom
    }
    pub const fn left(self) -> u16 {
        self.left
    }
}

impl From<u16> for Insets {
    fn from(value: u16) -> Self {
        Self::all(value)
    }
}
/// Public named ANSI colors supported by terminal backends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnsiColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
}

/// An application-owned semantic styling dimension name.
#[derive(Clone, Debug)]
pub struct StyleStateKey(StyleAtom);

impl StyleStateKey {
    pub const fn from_static(value: &'static str) -> Self {
        Self(StyleAtom::Static(value))
    }

    pub fn new(value: impl Into<String>) -> Self {
        Self(StyleAtom::Owned(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for StyleStateKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for StyleStateKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl PartialEq for StyleStateKey {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for StyleStateKey {}

impl Hash for StyleStateKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

/// An application-owned semantic styling value.
#[derive(Clone, Debug)]
pub struct StyleStateValue(StyleAtom);

impl StyleStateValue {
    pub const fn from_static(value: &'static str) -> Self {
        Self(StyleAtom::Static(value))
    }

    pub fn new(value: impl Into<String>) -> Self {
        Self(StyleAtom::Owned(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Normalized key/value assignments used by both inheritable state and
/// self-only facts. Entries are sorted so cloning and equality remain cheap
/// and deterministic for the small context bags used during paint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StyleAssignments {
    entries: Vec<(StyleStateKey, StyleStateValue)>,
}

impl StyleAssignments {
    fn get(&self, key: &StyleStateKey) -> Option<&StyleStateValue> {
        self.entries
            .binary_search_by(|(candidate, _)| candidate.as_str().cmp(key.as_str()))
            .ok()
            .map(|index| &self.entries[index].1)
    }

    fn set(&mut self, key: StyleStateKey, value: StyleStateValue) -> Option<StyleStateValue> {
        match self
            .entries
            .binary_search_by(|(candidate, _)| candidate.as_str().cmp(key.as_str()))
        {
            Ok(index) => Some(std::mem::replace(&mut self.entries[index].1, value)),
            Err(index) => {
                self.entries.insert(index, (key, value));
                None
            }
        }
    }

    fn overlay(&mut self, incoming: &Self) {
        for (key, value) in &incoming.entries {
            self.set(key.clone(), value.clone());
        }
    }

    #[allow(dead_code)]
    fn iter(&self) -> impl Iterator<Item = (&StyleStateKey, &StyleStateValue)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }
}

/// Inheritable semantic styling state.
///
/// Style states describe runtime/application context that propagates through
/// the presentation tree. This is distinct from [`StyleFacts`], which identify
/// only the presentation node/span/run currently being resolved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StyleStates {
    assignments: StyleAssignments,
}

impl StyleStates {
    pub(crate) fn set(&mut self, key: impl Into<StyleStateKey>, value: impl Into<StyleStateValue>) {
        self.assignments.set(key.into(), value.into());
    }

    pub(crate) fn overlay(&mut self, incoming: &Self) {
        self.assignments.overlay(&incoming.assignments);
    }

    pub(crate) fn get(&self, key: &StyleStateKey) -> Option<&StyleStateValue> {
        self.assignments.get(key)
    }

    #[allow(dead_code)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&StyleStateKey, &StyleStateValue)> {
        self.assignments.iter()
    }
}

/// Self-only semantic styling identity. Facts apply to the current
/// presentation value and are cleared before descending to children; the
/// computed physical style still inherits normally.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StyleFacts {
    assignments: StyleAssignments,
}

impl StyleFacts {
    pub(crate) fn set(&mut self, key: impl Into<StyleStateKey>, value: impl Into<StyleStateValue>) {
        self.assignments.set(key.into(), value.into());
    }

    pub(crate) fn get(&self, key: &StyleStateKey) -> Option<&StyleStateValue> {
        self.assignments.get(key)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&StyleStateKey, &StyleStateValue)> {
        self.assignments.iter()
    }
}

impl From<&str> for StyleStateValue {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for StyleStateValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl PartialEq for StyleStateValue {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for StyleStateValue {}

impl Hash for StyleStateValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

#[derive(Clone, Debug)]
enum StyleAtom {
    Static(&'static str),
    Owned(String),
}

impl StyleAtom {
    fn as_str(&self) -> &str {
        match self {
            Self::Static(value) => value,
            Self::Owned(value) => value,
        }
    }
}

/// A positive conjunction of framework interaction predicates and semantic
/// application-owned key/value requirements.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StyleSelector {
    focused: bool,
    focus_within: bool,
    states: Vec<(StyleStateKey, StyleStateValue)>,
}

impl StyleSelector {
    pub fn focused() -> Self {
        Self {
            focused: true,
            ..Self::default()
        }
    }

    pub fn focus_within() -> Self {
        Self {
            focus_within: true,
            ..Self::default()
        }
    }

    pub fn state(key: impl Into<StyleStateKey>, value: impl Into<StyleStateValue>) -> Self {
        Self::default().and_state(key, value)
    }

    pub fn and_focused(mut self) -> Self {
        self.focused = true;
        self
    }

    pub fn and_focus_within(mut self) -> Self {
        self.focus_within = true;
        self
    }

    pub fn and_state(
        mut self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        let key = key.into();
        if let Some(existing) = self
            .states
            .iter_mut()
            .find(|(existing, _)| existing == &key)
        {
            existing.1 = value.into();
        } else {
            self.states.push((key, value.into()));
        }
        self.states
            .sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
        self
    }

    pub(crate) fn predicate_count(&self) -> usize {
        usize::from(self.focused) + usize::from(self.focus_within) + self.states.len()
    }

    pub(crate) fn matches(
        &self,
        focused: bool,
        focus_within: bool,
        states: &StyleStates,
        facts: &StyleFacts,
    ) -> bool {
        (!self.focused || focused)
            && (!self.focus_within || focus_within)
            && self.states.iter().all(|(key, value)| {
                facts
                    .get(key)
                    .or_else(|| states.get(key))
                    .is_some_and(|candidate| candidate == value)
            })
    }
}

impl Default for StyleSelector {
    fn default() -> Self {
        Self {
            focused: false,
            focus_within: false,
            states: Vec::new(),
        }
    }
}

/// A backend-neutral theme color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeColor {
    Default,
    Named(AnsiColor),
    Indexed(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

/// Backend-neutral theme, ANSI, or RGB color specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorSpec {
    Theme(ThemeKey),
    Named(AnsiColor),
    Ansi(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl ColorSpec {
    pub fn theme(key: impl Into<ThemeKey>) -> Self {
        Self::Theme(key.into())
    }

    pub const fn named(color: AnsiColor) -> Self {
        Self::Named(color)
    }

    pub const fn ansi(value: u8) -> Self {
        Self::Ansi(value)
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgb { r, g, b }
    }
}

/// Opaque semantic key resolved by the host theme.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThemeKey(pub(crate) String);

impl ThemeKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for ThemeKey {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for ThemeKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ThemeKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A semantic named style plus a sparse local override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleRef {
    pub(crate) theme: Option<ThemeKey>,
    pub(crate) local: StyleSpec,
}

impl StyleRef {
    pub fn direct(style: StyleSpec) -> Self {
        Self {
            theme: None,
            local: style,
        }
    }

    pub fn theme(key: impl Into<ThemeKey>) -> Self {
        Self {
            theme: Some(key.into()),
            local: StyleSpec::default(),
        }
    }

    pub fn themed(key: impl Into<ThemeKey>, overrides: StyleSpec) -> Self {
        Self {
            theme: Some(key.into()),
            local: overrides,
        }
    }

    pub fn overrides(mut self, patch: StyleSpec) -> Self {
        self.local.overlay(&patch);
        self
    }

    pub(crate) fn overlay(&mut self, patch: &StyleSpec) {
        self.local.overlay(patch);
    }
}

impl From<StyleSpec> for StyleRef {
    fn from(style: StyleSpec) -> Self {
        Self::direct(style)
    }
}

impl std::ops::Deref for StyleRef {
    type Target = StyleSpec;

    fn deref(&self) -> &Self::Target {
        &self.local
    }
}

impl std::ops::DerefMut for StyleRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.local
    }
}

impl PartialEq<StyleSpec> for StyleRef {
    fn eq(&self, other: &StyleSpec) -> bool {
        self.theme.is_none() && &self.local == other
    }
}

impl PartialEq<StyleRef> for StyleSpec {
    fn eq(&self, other: &StyleRef) -> bool {
        other == self
    }
}

/// Sparse text-attribute intent used by semantic style patches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextAttributeSpec {
    pub(crate) bold: Option<bool>,
    pub(crate) dim: Option<bool>,
    pub(crate) italic: Option<bool>,
    pub(crate) underline: Option<bool>,
    pub(crate) reversed: Option<bool>,
    pub(crate) strikethrough: Option<bool>,
}

impl TextAttributeSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attribute(mut self, attribute: TextAttribute, enabled: bool) -> Self {
        self.set(attribute, enabled);
        self
    }

    pub(in crate::presentation::api) fn set(&mut self, attribute: TextAttribute, enabled: bool) {
        match attribute {
            TextAttribute::Bold => self.bold = Some(enabled),
            TextAttribute::Dim => self.dim = Some(enabled),
            TextAttribute::Italic => self.italic = Some(enabled),
            TextAttribute::Underline => self.underline = Some(enabled),
            TextAttribute::Reversed => self.reversed = Some(enabled),
            TextAttribute::Strikethrough => self.strikethrough = Some(enabled),
        }
    }

    fn overlay(&mut self, incoming: Self) {
        if incoming.bold.is_some() {
            self.bold = incoming.bold;
        }
        if incoming.dim.is_some() {
            self.dim = incoming.dim;
        }
        if incoming.italic.is_some() {
            self.italic = incoming.italic;
        }
        if incoming.underline.is_some() {
            self.underline = incoming.underline;
        }
        if incoming.reversed.is_some() {
            self.reversed = incoming.reversed;
        }
        if incoming.strikethrough.is_some() {
            self.strikethrough = incoming.strikethrough;
        }
    }
}

/// Selects a sparse semantic text attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAttribute {
    Bold,
    Dim,
    Italic,
    Underline,
    Reversed,
    Strikethrough,
}

/// Which sides of a semantic border are painted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BorderEdges {
    pub(crate) top: bool,
    pub(crate) right: bool,
    pub(crate) bottom: bool,
    pub(crate) left: bool,
}

impl BorderEdges {
    pub const NONE: Self = Self {
        top: false,
        right: false,
        bottom: false,
        left: false,
    };

    pub const ALL: Self = Self {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };

    pub const TOP_BOTTOM: Self = Self {
        top: true,
        right: false,
        bottom: true,
        left: false,
    };

    pub const fn new(top: bool, right: bool, bottom: bool, left: bool) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

/// Failure to construct a border glyph that occupies exactly one cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BorderGlyphError {
    pub field: &'static str,
    pub width: usize,
    pub graphemes: usize,
}

impl fmt::Display for BorderGlyphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "border glyph `{}` must contain one grapheme with width one (got {} graphemes, width {})",
            self.field, self.graphemes, self.width
        )
    }
}

impl Error for BorderGlyphError {}

/// Custom one-cell border glyphs. Applications can use ASCII, Unicode box
/// drawing, or another backend-supported pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BorderGlyphs {
    pub(crate) top: String,
    pub(crate) right: String,
    pub(crate) bottom: String,
    pub(crate) left: String,
    pub(crate) top_left: String,
    pub(crate) top_right: String,
    pub(crate) bottom_left: String,
    pub(crate) bottom_right: String,
}

impl BorderGlyphs {
    pub fn new(
        top: impl Into<String>,
        right: impl Into<String>,
        bottom: impl Into<String>,
        left: impl Into<String>,
        top_left: impl Into<String>,
        top_right: impl Into<String>,
        bottom_left: impl Into<String>,
        bottom_right: impl Into<String>,
    ) -> Result<Self, BorderGlyphError> {
        let top = top.into();
        let right = right.into();
        let bottom = bottom.into();
        let left = left.into();
        let top_left = top_left.into();
        let top_right = top_right.into();
        let bottom_left = bottom_left.into();
        let bottom_right = bottom_right.into();
        for (field, glyph) in [
            ("top", &top),
            ("right", &right),
            ("bottom", &bottom),
            ("left", &left),
            ("top_left", &top_left),
            ("top_right", &top_right),
            ("bottom_left", &bottom_left),
            ("bottom_right", &bottom_right),
        ] {
            let graphemes = glyph.graphemes(true).count();
            let width = crate::physical::text_cell_width(glyph.as_str());
            if graphemes != 1 || width != 1 {
                return Err(BorderGlyphError {
                    field,
                    width,
                    graphemes,
                });
            }
        }
        Ok(Self {
            top,
            right,
            bottom,
            left,
            top_left,
            top_right,
            bottom_left,
            bottom_right,
        })
    }

    pub(crate) fn plain() -> Self {
        Self::new("─", "│", "─", "│", "┌", "┐", "└", "┘")
            .expect("built-in border glyphs are one-cell")
    }

    pub(crate) fn rounded() -> Self {
        Self::new("─", "│", "─", "│", "╭", "╮", "╰", "╯")
            .expect("built-in border glyphs are one-cell")
    }

    pub(crate) fn double() -> Self {
        Self::new("═", "║", "═", "║", "╔", "╗", "╚", "╝")
            .expect("built-in border glyphs are one-cell")
    }
}

/// Backend-neutral border description. Edges are independently optional;
/// corners are painted only when both adjacent edges are enabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BorderSpec {
    pub(crate) style: BorderStyle,
    pub(crate) color: Option<ColorSpec>,
    pub(crate) edges: BorderEdges,
    pub(crate) glyphs: BorderGlyphs,
    pub(crate) top_label: Option<String>,
}

impl BorderSpec {
    pub fn plain() -> Self {
        Self {
            style: BorderStyle::Plain,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::plain(),
            top_label: None,
        }
    }

    pub fn rounded() -> Self {
        Self {
            style: BorderStyle::Rounded,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::rounded(),
            top_label: None,
        }
    }

    pub fn double() -> Self {
        Self {
            style: BorderStyle::Double,
            color: None,
            edges: BorderEdges::ALL,
            glyphs: BorderGlyphs::double(),
            top_label: None,
        }
    }

    pub fn custom(glyphs: BorderGlyphs) -> Self {
        Self {
            style: BorderStyle::Plain,
            color: None,
            edges: BorderEdges::ALL,
            glyphs,
            top_label: None,
        }
    }

    pub fn edges(mut self, edges: BorderEdges) -> Self {
        self.edges = edges;
        self
    }

    pub fn color(mut self, color: ColorSpec) -> Self {
        self.color = Some(color);
        self
    }

    /// Places a semantic label over the top edge without changing geometry.
    pub fn top_label(mut self, label: impl Into<String>) -> Self {
        self.top_label = Some(label.into());
        self
    }

    pub(crate) fn left_width(&self) -> u16 {
        u16::from(self.edges.left)
    }

    pub(crate) fn right_width(&self) -> u16 {
        u16::from(self.edges.right)
    }

    pub(crate) fn top_height(&self) -> u16 {
        u16::from(self.edges.top)
    }

    pub(crate) fn bottom_height(&self) -> u16 {
        u16::from(self.edges.bottom)
    }
}

/// Terminal-independent border family used by the convenience constructors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderStyle {
    #[default]
    Plain,
    Rounded,
    Double,
}
/// Overflow treatment for a structurally clamped view.
#[derive(Clone, Debug, PartialEq)]
pub enum OverflowIndicator {
    None,
    Ellipsis { style: StyleRef },
    Footer { prefix: String, style: StyleRef },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_is_explicit_for_attributes_but_not_colors() {
        let plain = StyleSpec::plain();
        assert_eq!(plain.foreground, None);
        assert_eq!(plain.background, None);
        assert_eq!(plain.attribute_value(TextAttribute::Bold), Some(false));
        assert_eq!(plain.attribute_value(TextAttribute::Dim), Some(false));
        assert_eq!(plain.attribute_value(TextAttribute::Italic), Some(false));
        assert_eq!(plain.attribute_value(TextAttribute::Underline), Some(false));
        assert_eq!(plain.attribute_value(TextAttribute::Reversed), Some(false));
        assert_eq!(
            plain.attribute_value(TextAttribute::Strikethrough),
            Some(false)
        );

        let unspecified = StyleSpec::new();
        assert_eq!(unspecified.attribute_value(TextAttribute::Bold), None);
        assert_eq!(
            unspecified.attribute_value(TextAttribute::Strikethrough),
            None
        );
    }

    #[test]
    fn sparse_style_overlay_preserves_unspecified_fields_and_allows_false() {
        let mut existing = StyleSpec::new().foreground(ColorSpec::Ansi(1)).bold();
        existing.overlay(&StyleSpec::new().italic());
        assert_eq!(existing.foreground, Some(ColorSpec::Ansi(1)));
        assert_eq!(existing.attributes.bold, Some(true));
        assert_eq!(existing.attributes.italic, Some(true));

        existing.overlay(&StyleSpec::new().attribute(TextAttribute::Bold, false));
        assert_eq!(existing.attributes.bold, Some(false));

        let strike = StyleSpec::new().strikethrough();
        assert_eq!(
            strike.attribute_value(TextAttribute::Strikethrough),
            Some(true)
        );
        assert_eq!(
            StyleSpec::new()
                .attribute(TextAttribute::Strikethrough, false)
                .attribute_value(TextAttribute::Strikethrough),
            Some(false)
        );
    }

    #[test]
    fn semantic_style_primitive_constructors_lower_to_existing_values() {
        assert_eq!(Insets::horizontal(2), Insets::new(0, 2, 0, 2));
        assert_eq!(Insets::from(3), Insets::all(3));
        assert_eq!(ColorSpec::ansi(3), ColorSpec::Ansi(3));
        assert_eq!(ColorSpec::rgb(1, 2, 3), ColorSpec::Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(
            ColorSpec::theme("muted"),
            ColorSpec::Theme(ThemeKey::from("muted"))
        );
        assert_eq!(
            ColorSpec::theme(String::from("muted")),
            ColorSpec::Theme(ThemeKey::from("muted")),
        );
    }

    #[test]
    fn border_constructors_set_style_and_replaceable_color() {
        assert_eq!(
            BorderSpec::plain(),
            BorderSpec {
                style: BorderStyle::Plain,
                color: None,
                edges: BorderEdges::ALL,
                glyphs: BorderGlyphs::plain(),
                top_label: None,
            }
        );
        assert_eq!(BorderSpec::rounded().style, BorderStyle::Rounded);
        assert_eq!(BorderSpec::double().style, BorderStyle::Double);
        assert_eq!(
            BorderSpec::rounded()
                .color(ColorSpec::ansi(2))
                .color(ColorSpec::ansi(3)),
            BorderSpec {
                style: BorderStyle::Rounded,
                color: Some(ColorSpec::ansi(3)),
                edges: BorderEdges::ALL,
                glyphs: BorderGlyphs::rounded(),
                top_label: None,
            },
        );
    }
}
