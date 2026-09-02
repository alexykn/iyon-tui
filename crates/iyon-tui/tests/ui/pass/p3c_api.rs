use iyon_tui::{
    Block, CodeBlockLabelPolicy, Component, GridCellSpec, GridTrack, HeadingLevel, History, Inline,
    MarkdownOptions, MarkdownProjector, Projector, Renderer, Smooth, SoftBreakPolicy,
    TableColumnSizing, TaskListMarkerPolicy, TextContent, TextOrigin, TextPart, TextRenderPolicy,
    TextRenderer, TextRole, TextSelector, View, WrapMode,
};
use iyon_tui::projection::{ProjectionBuilder, ProjectionTransitionError, validate_projection_transition};
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{BlockKind, LiteralText, Mark, TextRun, TextVisitor};

fn root_vocabulary() {
    let _: fn() -> History = History::new;
    let _: fn() -> MarkdownProjector = MarkdownProjector::default;
    let _: fn() -> Smooth = Smooth::default;
    let _ = TextRenderer::default().render(&TextContent::raw("x"));
    let _ = TextRenderer::with_policy(
        TextRenderPolicy::new().with_soft_break(SoftBreakPolicy::LineBreak),
    );
    let _ = TextRenderPolicy::new()
        .with_table_column_sizing(TableColumnSizing::Content)
        .with_table_column_gap(2)
        .with_task_list_marker(TaskListMarkerPolicy::TaskOnly)
        .with_code_block_label(CodeBlockLabelPolicy::Language)
        .with_code_wrap(WrapMode::Grapheme);
    let _ = MarkdownOptions::gfm();
    let _ = TextSelector::heading();
    let _ = TextSelector::part(TextPart::CodeLabel);
    let _ = TextSelector::heading().origin(TextOrigin::MARKDOWN);
    let _ = TextRole::Heading;
    let _ = Block::heading(HeadingLevel::H1, "Heading");
    let _ = Inline::text("text");
    let _ = View::grid(|grid| {
        grid.columns([GridTrack::content(), GridTrack::flex()]);
        grid.row(|row| {
            row.cell("Name");
            row.cell_with(GridCellSpec::new(), "Value");
        });
    });
}

fn advanced_namespaces() {
    let _ = ProjectionBuilder::<u8>::new;
    let _ = ProjectionTransitionError::SourceEndRegressed;
    let _ = validate_projection_transition::<u8>;
    let _ = StreamOffset::ZERO;
    let _ = StreamRange::default();
    let _ = BlockKind::ThematicBreak;
    let _ = LiteralText::from("x");
    let _ = TextRun::from("x");
    let _ = Mark::Strong;
}

fn main() {
    let _ = root_vocabulary as fn();
    let _ = advanced_namespaces as fn();
}

fn _trait_names<C: Component, P: Projector<TextContent>, R: Renderer<TextContent>, V: TextVisitor>() {}
