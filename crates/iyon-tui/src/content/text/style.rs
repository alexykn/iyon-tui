//! Typed generic structured-text styling vocabulary.
//!
//! [`TextSelector`] is a convenience facade over ordinary [`StyleSelector`]
//! matching. It does not introduce a separate text theme engine. Private fact
//! encoding is shared by selectors and the crate-private [`TextFacts`] builder
//! that the renderer emits.

use super::{Annotations, FormatId, HeadingLevel, LanguageId, SemanticTag, TextOrigin};
use crate::Theme;
use crate::presentation::api::{
    StyleFacts, StyleRef, StyleSelector, StyleSpec, StyleStateKey, StyleStateValue,
};

pub(crate) const TEXT_THEME_KEY: &str = "__iyon_tui.text";

const PRESENT: &str = "present";
const HEADING_LEVEL_KEY: &str = "__iyon_tui.text.heading.level";
const ORIGIN_KEY: &str = "__iyon_tui.text.origin";
const PART_KEY: &str = "__iyon_tui.text.part";
const LIST_KIND_KEY: &str = "__iyon_tui.text.list.kind";
const TASK_STATE_KEY: &str = "__iyon_tui.text.task.state";
const TABLE_SECTION_KEY: &str = "__iyon_tui.text.table.section";
const LANGUAGE_KEY: &str = "__iyon_tui.text.language";
const FORMAT_KEY: &str = "__iyon_tui.text.format";
const ANNOTATION_KEY_PREFIX: &str = "__iyon_tui.text.annotation|";

/// Semantic classification used by generic structured-text styling.
///
/// Roles name IR-level meaning and are source-format independent. They are
/// additive: a node may be both strong and a link.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextRole {
    Paragraph,
    Heading,
    BlockQuote,
    List,
    ListItem,
    CodeBlock,
    Table,
    TableRow,
    TableCell,
    ThematicBreak,
    RawBlock,
    Container,
    Strong,
    Emphasis,
    Strikethrough,
    Underline,
    Superscript,
    Subscript,
    SmallCaps,
    InlineCode,
    Link,
    Image,
    RawInline,
}

/// Renderer-generated presentation pieces, distinct from semantic [`TextRole`]s.
///
/// `TextRole` names an IR semantic value. `TextPart` names a chrome fragment
/// the renderer generates around that value, such as a list marker.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextPart {
    ListMarker,
    TaskMarker,
    QuoteMarker,
    CodeLabel,
    TableRule,
    ThematicRule,
    ImageFallback,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextListKind {
    Bullet,
    Ordered,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextTaskState {
    Checked,
    Unchecked,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextTableSection {
    Header,
    Body,
}

/// Typed selector for generic structured-text presentation.
///
/// `TextSelector` is a convenience facade over the framework's ordinary
/// `StyleSelector` mechanism. It does not introduce a separate text theme or
/// selector engine.
///
/// Roles such as heading, strong, and link are source-format independent.
/// Optional dimensions such as origin allow an application to specialize
/// presentation for content claimed by a particular projector.
///
/// ```
/// use iyon_tui::{
///     ColorSpec, HeadingLevel, StyleSpec, TextOrigin, TextRole, TextSelector, Theme,
/// };
///
/// let _theme = Theme::new()
///     .with_text_style(TextSelector::heading(), StyleSpec::new().bold())
///     .with_text_style(
///         TextSelector::heading().level(HeadingLevel::H1),
///         StyleSpec::new().underline(),
///     )
///     .with_text_style(
///         TextSelector::heading().origin(TextOrigin::MARKDOWN),
///         StyleSpec::new().foreground(ColorSpec::theme("accent")),
///     )
///     .with_text_style(
///         TextSelector::strong().and_role(TextRole::Link),
///         StyleSpec::new().underline(),
///     );
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSelector {
    inner: StyleSelector,
}

impl TextSelector {
    pub fn any() -> Self {
        Self {
            inner: StyleSelector::default(),
        }
    }

    pub fn role(role: TextRole) -> Self {
        Self::any().and_role(role)
    }

    pub fn part(part: TextPart) -> Self {
        Self::any().and_fact(TextFact::Part(part))
    }

    pub fn and_part(self, part: TextPart) -> Self {
        self.and_fact(TextFact::Part(part))
    }

    pub fn annotation(tag: &SemanticTag) -> Self {
        Self::any().and_annotation(tag)
    }

    pub fn paragraph() -> Self {
        Self::role(TextRole::Paragraph)
    }

    pub fn heading() -> Self {
        Self::role(TextRole::Heading)
    }

    pub fn block_quote() -> Self {
        Self::role(TextRole::BlockQuote)
    }

    pub fn list() -> Self {
        Self::role(TextRole::List)
    }

    pub fn list_item() -> Self {
        Self::role(TextRole::ListItem)
    }

    pub fn code_block() -> Self {
        Self::role(TextRole::CodeBlock)
    }

    pub fn table() -> Self {
        Self::role(TextRole::Table)
    }

    pub fn table_row() -> Self {
        Self::role(TextRole::TableRow)
    }

    pub fn table_cell() -> Self {
        Self::role(TextRole::TableCell)
    }

    pub fn thematic_break() -> Self {
        Self::role(TextRole::ThematicBreak)
    }

    pub fn strong() -> Self {
        Self::role(TextRole::Strong)
    }

    pub fn emphasis() -> Self {
        Self::role(TextRole::Emphasis)
    }

    pub fn strikethrough() -> Self {
        Self::role(TextRole::Strikethrough)
    }

    pub fn underline() -> Self {
        Self::role(TextRole::Underline)
    }

    pub fn inline_code() -> Self {
        Self::role(TextRole::InlineCode)
    }

    pub fn link() -> Self {
        Self::role(TextRole::Link)
    }

    pub fn and_role(self, role: TextRole) -> Self {
        self.and_fact(TextFact::Role(role))
    }

    pub fn level(self, level: HeadingLevel) -> Self {
        self.and_fact(TextFact::HeadingLevel(level))
    }

    pub fn origin(self, origin: TextOrigin) -> Self {
        self.and_fact(TextFact::Origin(origin))
    }

    pub fn list_kind(self, kind: TextListKind) -> Self {
        self.and_fact(TextFact::ListKind(kind))
    }

    pub fn task_state(self, state: TextTaskState) -> Self {
        self.and_fact(TextFact::TaskState(state))
    }

    pub fn table_section(self, section: TextTableSection) -> Self {
        self.and_fact(TextFact::TableSection(section))
    }

    pub fn language(self, language: &LanguageId) -> Self {
        self.and_fact(TextFact::Language(language.clone()))
    }

    pub fn format(self, format: &FormatId) -> Self {
        self.and_fact(TextFact::Format(format.clone()))
    }

    pub fn and_annotation(self, tag: &SemanticTag) -> Self {
        self.and_fact(TextFact::Annotation(tag.clone()))
    }

    pub fn and_focused(self) -> Self {
        Self {
            inner: self.inner.and_focused(),
        }
    }

    pub fn and_focus_within(self) -> Self {
        Self {
            inner: self.inner.and_focus_within(),
        }
    }

    pub fn and_state(
        self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        Self {
            inner: self.inner.and_state(key, value),
        }
    }

    fn and_fact(self, fact: TextFact) -> Self {
        let (key, value) = fact.key_value();
        Self {
            inner: self.inner.and_state(key, value),
        }
    }

    fn is_any(&self) -> bool {
        self.inner.predicate_count() == 0
    }

    pub(crate) fn into_style_selector(self) -> StyleSelector {
        self.inner
    }
}

impl Theme {
    /// Adds application styling policy for generic structured text.
    ///
    /// Text styles use the same Theme/StyleSelector cascade as ordinary Views.
    /// Application text rules resolve after framework defaults and before local
    /// `StyleRef` overrides.
    pub fn with_text_style(mut self, selector: TextSelector, style: StyleSpec) -> Self {
        self.set_text_style(selector, style);
        self
    }

    pub fn set_text_style(
        &mut self,
        selector: TextSelector,
        style: StyleSpec,
    ) -> Option<StyleSpec> {
        if selector.is_any() {
            self.set_style(TEXT_THEME_KEY, style)
        } else {
            self.set_style_variant(TEXT_THEME_KEY, selector.into_style_selector(), style)
        }
    }
}

pub(crate) fn text_style_ref() -> StyleRef {
    StyleRef::theme(TEXT_THEME_KEY)
}

/// Private encoding shared by [`TextSelector`] and renderer-emitted facts.
enum TextFact {
    Role(TextRole),
    Part(TextPart),
    HeadingLevel(HeadingLevel),
    Origin(TextOrigin),
    ListKind(TextListKind),
    TaskState(TextTaskState),
    TableSection(TextTableSection),
    Language(LanguageId),
    Format(FormatId),
    Annotation(SemanticTag),
}

impl TextFact {
    fn key_value(&self) -> (StyleStateKey, StyleStateValue) {
        match self {
            Self::Role(role) => (
                StyleStateKey::from_static(role_key(*role)),
                StyleStateValue::from_static(PRESENT),
            ),
            Self::Part(part) => (
                StyleStateKey::from_static(PART_KEY),
                StyleStateValue::from_static(part_value(*part)),
            ),
            Self::HeadingLevel(level) => (
                StyleStateKey::from_static(HEADING_LEVEL_KEY),
                StyleStateValue::from_static(heading_level_value(*level)),
            ),
            Self::Origin(origin) => (
                StyleStateKey::from_static(ORIGIN_KEY),
                StyleStateValue::new(origin.as_str()),
            ),
            Self::ListKind(kind) => (
                StyleStateKey::from_static(LIST_KIND_KEY),
                StyleStateValue::from_static(list_kind_value(*kind)),
            ),
            Self::TaskState(state) => (
                StyleStateKey::from_static(TASK_STATE_KEY),
                StyleStateValue::from_static(task_state_value(*state)),
            ),
            Self::TableSection(section) => (
                StyleStateKey::from_static(TABLE_SECTION_KEY),
                StyleStateValue::from_static(table_section_value(*section)),
            ),
            Self::Language(language) => (
                StyleStateKey::from_static(LANGUAGE_KEY),
                StyleStateValue::new(language.as_str()),
            ),
            Self::Format(format) => (
                StyleStateKey::from_static(FORMAT_KEY),
                StyleStateValue::new(format.as_str()),
            ),
            Self::Annotation(tag) => (
                annotation_fact_key(tag),
                StyleStateValue::from_static(PRESENT),
            ),
        }
    }
}

fn role_key(role: TextRole) -> &'static str {
    match role {
        TextRole::Paragraph => "__iyon_tui.text.role.paragraph",
        TextRole::Heading => "__iyon_tui.text.role.heading",
        TextRole::BlockQuote => "__iyon_tui.text.role.block-quote",
        TextRole::List => "__iyon_tui.text.role.list",
        TextRole::ListItem => "__iyon_tui.text.role.list-item",
        TextRole::CodeBlock => "__iyon_tui.text.role.code-block",
        TextRole::Table => "__iyon_tui.text.role.table",
        TextRole::TableRow => "__iyon_tui.text.role.table-row",
        TextRole::TableCell => "__iyon_tui.text.role.table-cell",
        TextRole::ThematicBreak => "__iyon_tui.text.role.thematic-break",
        TextRole::RawBlock => "__iyon_tui.text.role.raw-block",
        TextRole::Container => "__iyon_tui.text.role.container",
        TextRole::Strong => "__iyon_tui.text.role.strong",
        TextRole::Emphasis => "__iyon_tui.text.role.emphasis",
        TextRole::Strikethrough => "__iyon_tui.text.role.strikethrough",
        TextRole::Underline => "__iyon_tui.text.role.underline",
        TextRole::Superscript => "__iyon_tui.text.role.superscript",
        TextRole::Subscript => "__iyon_tui.text.role.subscript",
        TextRole::SmallCaps => "__iyon_tui.text.role.small-caps",
        TextRole::InlineCode => "__iyon_tui.text.role.inline-code",
        TextRole::Link => "__iyon_tui.text.role.link",
        TextRole::Image => "__iyon_tui.text.role.image",
        TextRole::RawInline => "__iyon_tui.text.role.raw-inline",
    }
}

fn part_value(part: TextPart) -> &'static str {
    match part {
        TextPart::ListMarker => "list-marker",
        TextPart::TaskMarker => "task-marker",
        TextPart::QuoteMarker => "quote-marker",
        TextPart::CodeLabel => "code-label",
        TextPart::TableRule => "table-rule",
        TextPart::ThematicRule => "thematic-rule",
        TextPart::ImageFallback => "image-fallback",
    }
}

fn heading_level_value(level: HeadingLevel) -> &'static str {
    match level.get() {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        6 => "h6",
        _ => unreachable!("HeadingLevel is validated to 1..=6"),
    }
}

fn list_kind_value(kind: TextListKind) -> &'static str {
    match kind {
        TextListKind::Bullet => "bullet",
        TextListKind::Ordered => "ordered",
    }
}

fn task_state_value(state: TextTaskState) -> &'static str {
    match state {
        TextTaskState::Checked => "checked",
        TextTaskState::Unchecked => "unchecked",
    }
}

fn table_section_value(section: TextTableSection) -> &'static str {
    match section {
        TextTableSection::Header => "header",
        TextTableSection::Body => "body",
    }
}

fn annotation_fact_key(tag: &SemanticTag) -> StyleStateKey {
    StyleStateKey::new(format!(
        "{ANNOTATION_KEY_PREFIX}{}:{}|{}:{}",
        tag.namespace().len(),
        tag.namespace(),
        tag.name().len(),
        tag.name()
    ))
}

/// Crate-private builder for renderer-emitted generic text facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TextFacts {
    inner: StyleFacts,
}

impl TextFacts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn role(self, role: TextRole) -> Self {
        self.and_fact(TextFact::Role(role))
    }

    pub(crate) fn roles(self, roles: impl IntoIterator<Item = TextRole>) -> Self {
        roles.into_iter().fold(self, TextFacts::role)
    }

    pub(crate) fn part(self, part: TextPart) -> Self {
        self.and_fact(TextFact::Part(part))
    }

    pub(crate) fn heading_level(self, level: HeadingLevel) -> Self {
        self.and_fact(TextFact::HeadingLevel(level))
    }

    pub(crate) fn origin(self, origin: &TextOrigin) -> Self {
        self.and_fact(TextFact::Origin(origin.clone()))
    }

    pub(crate) fn origin_if(self, origin: Option<&TextOrigin>) -> Self {
        match origin {
            Some(origin) => self.origin(origin),
            None => self,
        }
    }

    pub(crate) fn list_kind(self, kind: TextListKind) -> Self {
        self.and_fact(TextFact::ListKind(kind))
    }

    pub(crate) fn task_state(self, state: TextTaskState) -> Self {
        self.and_fact(TextFact::TaskState(state))
    }

    pub(crate) fn table_section(self, section: TextTableSection) -> Self {
        self.and_fact(TextFact::TableSection(section))
    }

    pub(crate) fn language(self, language: &LanguageId) -> Self {
        self.and_fact(TextFact::Language(language.clone()))
    }

    pub(crate) fn format(self, format: &FormatId) -> Self {
        self.and_fact(TextFact::Format(format.clone()))
    }

    pub(crate) fn annotation(self, tag: &SemanticTag) -> Self {
        self.and_fact(TextFact::Annotation(tag.clone()))
    }

    pub(crate) fn annotations(self, annotations: &Annotations) -> Self {
        annotations
            .tags()
            .iter()
            .fold(self, |facts, tag| facts.annotation(tag))
    }

    pub(crate) fn finish(self) -> StyleFacts {
        self.inner
    }

    fn and_fact(mut self, fact: TextFact) -> Self {
        let (key, value) = fact.key_value();
        self.inner.set(key, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::text::{FormatId, LanguageId, SemanticKey, SemanticTag};
    use crate::physical::PhysicalStyle;
    use crate::presentation::api::{StyleStates, TextAttribute};
    use crate::presentation::paint::{StyleContext, ThemeResolver};

    fn context(facts: StyleFacts) -> StyleContext {
        StyleContext {
            local_facts: facts,
            ..StyleContext::default()
        }
    }

    fn resolve(application: &Theme, facts: StyleFacts) -> PhysicalStyle {
        ThemeResolver::new(application).resolve_text_style(
            PhysicalStyle::default(),
            &text_style_ref(),
            &context(facts),
        )
    }

    fn framework_resolve(facts: StyleFacts) -> PhysicalStyle {
        resolve(&Theme::new(), facts)
    }

    #[test]
    fn selectors_normalize_predicate_order() {
        let left = TextSelector::heading()
            .level(HeadingLevel::H1)
            .origin(TextOrigin::MARKDOWN);
        let right = TextSelector::heading()
            .origin(TextOrigin::MARKDOWN)
            .level(HeadingLevel::H1);
        assert_eq!(left, right);
    }

    #[test]
    fn scalar_predicates_last_write_wins() {
        assert_eq!(
            TextSelector::heading()
                .level(HeadingLevel::H1)
                .level(HeadingLevel::H2),
            TextSelector::heading().level(HeadingLevel::H2)
        );
        assert_eq!(
            TextSelector::heading()
                .origin(TextOrigin::MARKDOWN)
                .origin(TextOrigin::PLAIN_TEXT),
            TextSelector::heading().origin(TextOrigin::PLAIN_TEXT)
        );
        let theme = Theme::new().with_text_style(
            TextSelector::part(TextPart::QuoteMarker),
            StyleSpec::new().dim(),
        );
        assert!(
            resolve(
                &theme,
                TextFacts::new()
                    .part(TextPart::ListMarker)
                    .part(TextPart::QuoteMarker)
                    .finish()
            )
            .dim
        );
        assert_eq!(
            TextSelector::list_item()
                .task_state(TextTaskState::Unchecked)
                .task_state(TextTaskState::Checked),
            TextSelector::list_item().task_state(TextTaskState::Checked)
        );
        assert_eq!(
            TextSelector::table_row()
                .table_section(TextTableSection::Body)
                .table_section(TextTableSection::Header),
            TextSelector::table_row().table_section(TextTableSection::Header)
        );
        let rust = LanguageId::new("rust").unwrap();
        let python = LanguageId::new("python").unwrap();
        assert_eq!(
            TextSelector::code_block().language(&rust).language(&python),
            TextSelector::code_block().language(&python)
        );
        let html = FormatId::new("html").unwrap();
        let xml = FormatId::new("xml").unwrap();
        assert_eq!(
            TextSelector::any().format(&html).format(&xml),
            TextSelector::any().format(&xml)
        );
    }

    #[test]
    fn roles_accumulate_instead_of_overwriting() {
        let selector = TextSelector::role(TextRole::Strong)
            .and_role(TextRole::Link)
            .and_role(TextRole::Emphasis);
        let facts = TextFacts::new()
            .role(TextRole::Strong)
            .role(TextRole::Link)
            .role(TextRole::Emphasis)
            .finish();
        let theme = Theme::new().with_text_style(selector, StyleSpec::new().reversed());
        assert!(resolve(&theme, facts).reversed);

        let strong_only = TextFacts::new().role(TextRole::Strong).finish();
        assert!(!resolve(&theme, strong_only).reversed);
    }

    #[test]
    fn annotation_tag_encoding_is_collision_resistant() {
        let dotted_ns = SemanticTag::new("a.b", "c").unwrap();
        let dotted_name = SemanticTag::new("a", "b.c").unwrap();
        assert_ne!(
            TextSelector::annotation(&dotted_ns),
            TextSelector::annotation(&dotted_name)
        );

        let slash_ns = SemanticTag::new("app/foo", "bar").unwrap();
        let slash_name = SemanticTag::new("app", "foo/bar").unwrap();
        assert_ne!(
            TextSelector::annotation(&slash_ns),
            TextSelector::annotation(&slash_name)
        );

        let theme = Theme::new().with_text_style(
            TextSelector::annotation(&dotted_ns),
            StyleSpec::new().bold(),
        );
        assert!(resolve(&theme, TextFacts::new().annotation(&dotted_ns).finish()).bold);
        assert!(!resolve(&theme, TextFacts::new().annotation(&dotted_name).finish()).bold);
    }

    #[test]
    fn annotation_selectors_compose_with_roles() {
        let annotation = SemanticTag::new("example", "annotated").unwrap();
        let theme = Theme::new().with_text_style(
            TextSelector::strong().and_annotation(&annotation),
            StyleSpec::new().dim(),
        );
        let matching = TextFacts::new()
            .role(TextRole::Strong)
            .annotation(&annotation)
            .finish();
        assert!(resolve(&theme, matching).dim);
        assert!(!resolve(&theme, TextFacts::new().role(TextRole::Strong).finish()).dim);
    }

    #[test]
    fn arbitrary_annotation_properties_are_not_selector_facts() {
        let extra = SemanticKey::new("app", "foo").unwrap();
        let annotations = crate::content::text::Annotations::new()
            .with_property(extra, "bar")
            .with_origin(TextOrigin::MARKDOWN);
        assert!(annotations.origin().is_some());
        let theme = Theme::new().with_text_style(
            TextSelector::heading().origin(TextOrigin::MARKDOWN),
            StyleSpec::new().italic(),
        );
        let without_origin = TextFacts::new().role(TextRole::Heading).finish();
        assert!(!resolve(&theme, without_origin).italic);
    }

    #[test]
    fn any_selector_maps_to_the_text_base_style() {
        let mut theme = Theme::new();
        theme.set_text_style(TextSelector::any(), StyleSpec::new().dim());
        assert_eq!(theme.style(TEXT_THEME_KEY), Some(&StyleSpec::new().dim()));
        assert!(resolve(&theme, TextFacts::new().role(TextRole::Paragraph).finish()).dim);
    }

    #[test]
    fn application_theme_does_not_contain_framework_defaults() {
        assert!(Theme::new().style(TEXT_THEME_KEY).is_none());
        let resolved = ThemeResolver::new(&Theme::new()).resolve_text_style(
            PhysicalStyle::default(),
            &StyleRef::theme("unrelated"),
            &context(TextFacts::new().role(TextRole::Strong).finish()),
        );
        assert!(!resolved.bold);
    }

    #[test]
    fn framework_default_matrix() {
        let heading = framework_resolve(TextFacts::new().role(TextRole::Heading).finish());
        assert!(heading.bold);
        assert!(!heading.underline);
        assert!(heading.foreground.is_none());

        let h1 = framework_resolve(
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .finish(),
        );
        assert!(h1.bold);
        assert!(h1.underline);

        assert!(framework_resolve(TextFacts::new().role(TextRole::Strong).finish()).bold);
        assert!(framework_resolve(TextFacts::new().role(TextRole::Emphasis).finish()).italic);
        assert!(framework_resolve(TextFacts::new().role(TextRole::Underline).finish()).underline);
        assert!(framework_resolve(TextFacts::new().role(TextRole::Link).finish()).underline);
        assert!(
            framework_resolve(TextFacts::new().role(TextRole::Strikethrough).finish())
                .strikethrough
        );
        assert!(
            framework_resolve(
                TextFacts::new()
                    .role(TextRole::TableRow)
                    .table_section(TextTableSection::Header)
                    .finish()
            )
            .bold
        );

        for role in [
            TextRole::Paragraph,
            TextRole::List,
            TextRole::BlockQuote,
            TextRole::CodeBlock,
        ] {
            let resolved = framework_resolve(TextFacts::new().role(role).finish());
            assert!(!resolved.bold, "{role:?} should not default to bold");
            assert!(!resolved.italic, "{role:?} should not default to italic");
            assert!(
                !resolved.underline,
                "{role:?} should not default to underline"
            );
            assert!(resolved.foreground.is_none());
        }
    }

    #[test]
    fn framework_defaults_are_source_invariant() {
        let generic = framework_resolve(
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .finish(),
        );
        let markdown = framework_resolve(
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .origin(&TextOrigin::MARKDOWN)
                .finish(),
        );
        let plain = framework_resolve(
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .origin(&TextOrigin::PLAIN_TEXT)
                .finish(),
        );
        assert_eq!(generic, markdown);
        assert_eq!(generic, plain);
        assert!(!generic.italic);
    }

    #[test]
    fn application_strong_override_beats_framework_default() {
        let application = Theme::new().with_text_style(
            TextSelector::strong(),
            StyleSpec::new().attribute(TextAttribute::Bold, false),
        );
        let resolved = resolve(
            &application,
            TextFacts::new().role(TextRole::Strong).finish(),
        );
        assert!(!resolved.bold);
    }

    #[test]
    fn application_generic_heading_plain_clears_framework_h1_underline() {
        let application = Theme::new().with_text_style(TextSelector::heading(), StyleSpec::plain());
        let resolved = resolve(
            &application,
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .finish(),
        );
        assert!(!resolved.bold);
        assert!(!resolved.underline);
    }

    #[test]
    fn origin_specific_heading_rule_does_not_match_generic_facts() {
        let application = Theme::new().with_text_style(
            TextSelector::heading().origin(TextOrigin::MARKDOWN),
            StyleSpec::new().italic(),
        );
        let generic = resolve(
            &application,
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .finish(),
        );
        assert!(!generic.italic);

        let markdown = resolve(
            &application,
            TextFacts::new()
                .role(TextRole::Heading)
                .heading_level(HeadingLevel::H1)
                .origin(&TextOrigin::MARKDOWN)
                .finish(),
        );
        assert!(markdown.italic);
    }

    #[test]
    fn strong_and_link_conjunction_requires_both_roles() {
        let application = Theme::new().with_text_style(
            TextSelector::strong().and_role(TextRole::Link),
            StyleSpec::new().reversed(),
        );
        let both = TextFacts::new()
            .role(TextRole::Strong)
            .role(TextRole::Link)
            .finish();
        assert!(resolve(&application, both).reversed);
        assert!(
            !resolve(
                &application,
                TextFacts::new().role(TextRole::Strong).finish()
            )
            .reversed
        );
    }

    #[test]
    fn typed_selectors_compose_with_generic_style_state() {
        let application = Theme::new().with_text_style(
            TextSelector::heading().and_state("app.mode", "compact"),
            StyleSpec::new().dim(),
        );
        let mut inherited = StyleStates::default();
        inherited.set("app.mode", "compact");
        let resolved = ThemeResolver::new(&application).resolve_text_style(
            PhysicalStyle::default(),
            &text_style_ref(),
            &StyleContext {
                inherited_states: inherited,
                local_facts: TextFacts::new().role(TextRole::Heading).finish(),
                ..StyleContext::default()
            },
        );
        assert!(resolved.dim);
    }

    #[test]
    fn typed_selectors_compose_with_focus() {
        let application = Theme::new().with_text_style(
            TextSelector::link().and_focused(),
            StyleSpec::new().reversed(),
        );
        let focused = ThemeResolver::new(&application).resolve_text_style(
            PhysicalStyle::default(),
            &text_style_ref(),
            &StyleContext {
                local_facts: TextFacts::new().role(TextRole::Link).finish(),
                focused: true,
                ..StyleContext::default()
            },
        );
        assert!(focused.reversed);

        let unfocused = resolve(&application, TextFacts::new().role(TextRole::Link).finish());
        assert!(!unfocused.reversed);
    }

    #[test]
    fn list_kind_and_part_facts_round_trip_through_the_shared_encoder() {
        let application = Theme::new()
            .with_text_style(
                TextSelector::list().list_kind(TextListKind::Ordered),
                StyleSpec::new().italic(),
            )
            .with_text_style(
                TextSelector::part(TextPart::ListMarker),
                StyleSpec::new().dim(),
            );
        assert!(
            resolve(
                &application,
                TextFacts::new()
                    .role(TextRole::List)
                    .list_kind(TextListKind::Ordered)
                    .finish()
            )
            .italic
        );
        assert!(
            resolve(
                &application,
                TextFacts::new().part(TextPart::ListMarker).finish()
            )
            .dim
        );
        assert!(
            !resolve(
                &application,
                TextFacts::new()
                    .role(TextRole::List)
                    .list_kind(TextListKind::Bullet)
                    .finish()
            )
            .italic
        );

        let rust = LanguageId::new("rust").unwrap();
        let html = FormatId::new("html").unwrap();
        let typed = Theme::new()
            .with_text_style(
                TextSelector::code_block().language(&rust),
                StyleSpec::new().dim(),
            )
            .with_text_style(TextSelector::any().format(&html), StyleSpec::new().italic())
            .with_text_style(
                TextSelector::list_item().task_state(TextTaskState::Checked),
                StyleSpec::new().reversed(),
            );
        assert!(
            resolve(
                &typed,
                TextFacts::new()
                    .role(TextRole::CodeBlock)
                    .language(&rust)
                    .finish()
            )
            .dim
        );
        assert!(resolve(&typed, TextFacts::new().format(&html).finish()).italic);
        assert!(
            resolve(
                &typed,
                TextFacts::new()
                    .role(TextRole::ListItem)
                    .task_state(TextTaskState::Checked)
                    .finish()
            )
            .reversed
        );
    }

    #[test]
    fn origin_if_omits_missing_origin() {
        let with = TextFacts::new()
            .role(TextRole::Heading)
            .origin_if(Some(&TextOrigin::MARKDOWN))
            .finish();
        let without = TextFacts::new()
            .role(TextRole::Heading)
            .origin_if(None)
            .finish();
        let application = Theme::new().with_text_style(
            TextSelector::heading().origin(TextOrigin::MARKDOWN),
            StyleSpec::new().italic(),
        );
        assert!(resolve(&application, with).italic);
        assert!(!resolve(&application, without).italic);
    }
}
