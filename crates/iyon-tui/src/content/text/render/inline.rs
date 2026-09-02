use super::super::{
    BreakKind, Inline, InlineContent, InlineKind, LiteralText, Mark, TextFacts, TextPart, TextRole,
    text_style_ref,
};
use super::identity::{RenderContext, inline_base_facts};
use super::{SoftBreakPolicy, TextRenderer};
use crate::{Text, TextSpan, View};

impl TextRenderer {
    pub(super) fn render_inline_content(
        &self,
        content: &InlineContent,
        context: &RenderContext,
    ) -> Text {
        let mut spans = Vec::new();
        let base = inline_base_facts(context);
        for inline in content.iter() {
            self.push_inline(&mut spans, inline, context, base.clone());
        }
        View::styled_text(spans)
    }

    pub(super) fn render_literal(&self, literal: &LiteralText, context: &RenderContext) -> Text {
        let mut spans = Vec::new();
        let base = inline_base_facts(context);
        for run in literal.runs() {
            spans.push(run_span(run, context, base.clone()));
        }
        View::styled_text(spans)
    }

    fn push_inline(
        &self,
        spans: &mut Vec<TextSpan>,
        inline: &Inline,
        context: &RenderContext,
        inherited: TextFacts,
    ) {
        let mut facts = inherited;
        if let Some(origin) = inline.origin() {
            facts = facts.origin(&origin);
        }
        for mark in inline.marks().marks() {
            facts = facts.role(mark_role(mark));
        }
        facts = facts.annotations(inline.annotations());
        match inline.kind() {
            InlineKind::Text(run) => spans.push(run_span(run, context, facts)),
            InlineKind::Break(BreakKind::Soft) => {
                let text = match self.policy.soft_break() {
                    SoftBreakPolicy::Space => " ",
                    SoftBreakPolicy::LineBreak => "\n",
                };
                spans.push(styled_span(text, facts));
            }
            InlineKind::Break(BreakKind::Hard) => spans.push(styled_span("\n", facts)),
            InlineKind::Image(image) => {
                let facts = facts.role(TextRole::Image).part(TextPart::ImageFallback);
                for child in image.alt().iter() {
                    self.push_inline(spans, child, context, facts.clone());
                }
            }
            InlineKind::RawInline { format, body } => {
                let facts = facts.role(TextRole::RawInline).format(format);
                let mut run_context = context.clone();
                run_context = run_context.with_format(format);
                if let Some(origin) = inline.origin() {
                    run_context.origin = Some(origin);
                }
                for run in body.runs() {
                    spans.push(run_span(run, &run_context, facts.clone()));
                }
            }
        }
    }
}

fn mark_role(mark: &Mark) -> TextRole {
    match mark {
        Mark::Strong => TextRole::Strong,
        Mark::Emphasis => TextRole::Emphasis,
        Mark::Strikethrough => TextRole::Strikethrough,
        Mark::Underline => TextRole::Underline,
        Mark::Superscript => TextRole::Superscript,
        Mark::Subscript => TextRole::Subscript,
        Mark::SmallCaps => TextRole::SmallCaps,
        Mark::Code => TextRole::InlineCode,
        Mark::Link(_) => TextRole::Link,
    }
}

fn styled_span(text: impl Into<String>, facts: TextFacts) -> TextSpan {
    TextSpan::styled(text, text_style_ref()).with_style_facts(facts.finish())
}

fn run_span(
    run: &super::super::TextRun,
    context: &RenderContext,
    mut facts: TextFacts,
) -> TextSpan {
    if let Some(origin) = run.annotations().origin() {
        facts = facts.origin(&origin);
    }
    if let Some(language) = &context.language {
        facts = facts.language(language);
    }
    if let Some(format) = &context.format {
        facts = facts.format(format);
    }
    facts = facts.annotations(run.annotations());
    let style = run.style().cloned().unwrap_or_else(text_style_ref);
    TextSpan::styled(run.text(), style).with_style_facts(facts.finish())
}
