use super::super::{
    Annotations, FormatId, LanguageId, TextFacts, TextListKind, TextOrigin, TextPart, TextRole,
    TextTableSection, TextTaskState, text_style_ref,
};
use crate::View;
use crate::presentation::factory as vf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RenderContextKey {
    /// The complete inherited role path is part of the cache key.  A depth
    /// alone is not enough: selectors can distinguish two paths with the
    /// same number of ancestors.
    pub(crate) ancestor_roles: Vec<TextRole>,
    pub(crate) heading_level: Option<super::super::HeadingLevel>,
    pub(crate) origin: Option<TextOrigin>,
    pub(crate) list_kind: Option<TextListKind>,
    pub(crate) task_state: Option<TextTaskState>,
    pub(crate) table_section: Option<TextTableSection>,
    pub(crate) language: Option<LanguageId>,
    pub(crate) format: Option<FormatId>,
}

/// Semantic environment known while lowering IR into Views.
#[derive(Clone, Debug, Default)]
pub(crate) struct RenderContext {
    pub(crate) ancestor_roles: Vec<TextRole>,
    pub(crate) heading_level: Option<super::super::HeadingLevel>,
    pub(crate) origin: Option<TextOrigin>,
    pub(crate) list_kind: Option<TextListKind>,
    pub(crate) task_state: Option<TextTaskState>,
    pub(crate) table_section: Option<TextTableSection>,
    pub(crate) language: Option<LanguageId>,
    pub(crate) format: Option<FormatId>,
}

impl RenderContext {
    pub(crate) fn cache_key(&self) -> RenderContextKey {
        RenderContextKey {
            ancestor_roles: self.ancestor_roles.clone(),
            heading_level: self.heading_level,
            origin: self.origin.clone(),
            list_kind: self.list_kind,
            task_state: self.task_state,
            table_section: self.table_section,
            language: self.language.clone(),
            format: self.format.clone(),
        }
    }

    pub(crate) fn effective_origin(&self, annotations: &Annotations) -> Option<TextOrigin> {
        annotations.origin().or_else(|| self.origin.clone())
    }

    pub(crate) fn for_node(&self, annotations: &Annotations) -> Self {
        let mut next = self.clone();
        next.origin = self.effective_origin(annotations);
        next
    }

    pub(crate) fn with_role(&self, role: TextRole) -> Self {
        let mut next = self.clone();
        next.ancestor_roles.push(role);
        next
    }

    pub(crate) fn with_list_kind(&self, kind: TextListKind) -> Self {
        let mut next = self.clone();
        next.list_kind = Some(kind);
        next
    }

    pub(crate) fn with_task_state(&self, state: Option<TextTaskState>) -> Self {
        let mut next = self.clone();
        next.task_state = state;
        next
    }

    pub(crate) fn with_table_section(&self, section: TextTableSection) -> Self {
        let mut next = self.clone();
        next.table_section = Some(section);
        next
    }

    pub(crate) fn with_language(&self, language: Option<&LanguageId>) -> Self {
        let mut next = self.clone();
        next.language = language.cloned();
        next
    }

    pub(crate) fn with_format(&self, format: &FormatId) -> Self {
        let mut next = self.clone();
        next.format = Some(format.clone());
        next
    }

    pub(crate) fn with_heading_level(&self, level: super::super::HeadingLevel) -> Self {
        let mut next = self.clone();
        next.heading_level = Some(level);
        next
    }
}

pub(super) fn semantic_view_facts(
    context: &RenderContext,
    role: TextRole,
    annotations: &Annotations,
) -> TextFacts {
    apply_scalars(
        TextFacts::new()
            .roles(context.ancestor_roles.iter().copied())
            .role(role),
        context,
    )
    .annotations(annotations)
}

pub(super) fn inline_base_facts(context: &RenderContext) -> TextFacts {
    apply_scalars(TextFacts::new(), context)
}

pub(super) fn part_facts(
    context: &RenderContext,
    part: TextPart,
    annotations: &Annotations,
) -> TextFacts {
    apply_scalars(TextFacts::new().part(part), context).annotations(annotations)
}

pub(super) fn stamp_view(view: View, facts: TextFacts) -> View {
    vf::with_style_facts(vf::style(view, text_style_ref()), facts.finish())
}

pub(super) fn stamp_text(text: View, facts: TextFacts) -> View {
    vf::with_style_facts(vf::style(text, text_style_ref()), facts.finish())
}

fn apply_scalars(facts: TextFacts, context: &RenderContext) -> TextFacts {
    let mut facts = facts.origin_if(context.origin.as_ref());
    if let Some(kind) = context.list_kind {
        facts = facts.list_kind(kind);
    }
    if let Some(state) = context.task_state {
        facts = facts.task_state(state);
    }
    if let Some(section) = context.table_section {
        facts = facts.table_section(section);
    }
    if let Some(language) = &context.language {
        facts = facts.language(language);
    }
    if let Some(format) = &context.format {
        facts = facts.format(format);
    }
    facts
}
