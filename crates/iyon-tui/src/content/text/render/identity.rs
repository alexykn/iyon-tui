use super::super::{
    Annotations, FormatId, LanguageId, TextFacts, TextListKind, TextOrigin, TextPart, TextRole,
    TextTableSection, TextTaskState, text_style_ref,
};
use crate::View;
use crate::presentation::factory as vf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct RenderContextKey {
    /// The complete inherited role path is part of the cache key.  A depth
    /// alone is not enough: selectors can distinguish two paths with the
    /// same number of ancestors.
    pub(super) ancestor_roles: Vec<TextRole>,
    pub(super) origin: Option<TextOrigin>,
    pub(super) list_kind: Option<TextListKind>,
    pub(super) task_state: Option<TextTaskState>,
    pub(super) table_section: Option<TextTableSection>,
    pub(super) language: Option<LanguageId>,
    pub(super) format: Option<FormatId>,
}

/// Semantic environment known while lowering IR into Views.
#[derive(Clone, Debug, Default)]
pub(super) struct RenderContext {
    pub(super) ancestor_roles: Vec<TextRole>,
    pub(super) origin: Option<TextOrigin>,
    pub(super) list_kind: Option<TextListKind>,
    pub(super) task_state: Option<TextTaskState>,
    pub(super) table_section: Option<TextTableSection>,
    pub(super) language: Option<LanguageId>,
    pub(super) format: Option<FormatId>,
}

impl RenderContext {
    pub(super) fn cache_key(&self) -> RenderContextKey {
        RenderContextKey {
            ancestor_roles: self.ancestor_roles.clone(),
            origin: self.origin.clone(),
            list_kind: self.list_kind,
            task_state: self.task_state,
            table_section: self.table_section,
            language: self.language.clone(),
            format: self.format.clone(),
        }
    }

    pub(super) fn effective_origin(&self, annotations: &Annotations) -> Option<TextOrigin> {
        annotations.origin().or_else(|| self.origin.clone())
    }

    pub(super) fn for_node(&self, annotations: &Annotations) -> Self {
        let mut next = self.clone();
        next.origin = self.effective_origin(annotations);
        next
    }

    pub(super) fn with_role(&self, role: TextRole) -> Self {
        let mut next = self.clone();
        next.ancestor_roles.push(role);
        next
    }

    pub(super) fn with_list_kind(&self, kind: TextListKind) -> Self {
        let mut next = self.clone();
        next.list_kind = Some(kind);
        next
    }

    pub(super) fn with_task_state(&self, state: Option<TextTaskState>) -> Self {
        let mut next = self.clone();
        next.task_state = state;
        next
    }

    pub(super) fn with_table_section(&self, section: TextTableSection) -> Self {
        let mut next = self.clone();
        next.table_section = Some(section);
        next
    }

    pub(super) fn with_language(&self, language: Option<&LanguageId>) -> Self {
        let mut next = self.clone();
        next.language = language.cloned();
        next
    }

    pub(super) fn with_format(&self, format: &FormatId) -> Self {
        let mut next = self.clone();
        next.format = Some(format.clone());
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
