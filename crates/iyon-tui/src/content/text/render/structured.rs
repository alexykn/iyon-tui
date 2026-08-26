use std::num::NonZeroU16;

use super::TextRenderPolicy;
use super::{Renderer, TextRenderer};
use crate::content::text::{
    Alignment, Annotations, Block, BlockKind, CodeBlock, LanguageId, List, ListItem, ListMarker,
    LiteralText, MarkdownOptions, MarkdownProjector, NumberDelimiter, NumberStyle, SemanticTag,
    Table, TableCell, TableColumn, TableRow, TextContent, TextOrigin, TextPart, TextRole,
    TextSelector, TextTableSection,
};
use crate::geometry::{LayoutConstraints, Rect};
use crate::physical::PhysicalStyle;
use crate::presentation::ir::{GridView, TrackSize, ViewKind};
use crate::presentation::layout::{ViewCompiler, compile_view, compile_view_with_theme};
use crate::projection::ProjectionBuilder;
use crate::stream::{StreamOffset, StreamRange};
use crate::{
    CodeBlockLabelPolicy, Projector, StyleSpec, TableColumnSizing, TaskListMarkerPolicy, Theme,
    View, WrapMode,
};

fn style_at(view: &View, theme: &Theme, needle: &str) -> PhysicalStyle {
    style_at_width(view, theme, 80, needle)
}

fn style_at_width(view: &View, theme: &Theme, width: u16, needle: &str) -> PhysicalStyle {
    let painted = compile_view_with_theme(view, width, theme);
    for row in &painted.rows {
        if let Some(index) = row.cell_x_of(needle) {
            return row.style_at(index).expect("painted cell");
        }
    }
    panic!(
        "did not find {needle:?} in {:?}",
        painted
            .rows
            .iter()
            .map(|row| row.plain_text())
            .collect::<Vec<_>>()
    );
}

fn text_at(view: &View, width: u16, needle: &str) -> (u16, u16) {
    let block = compile_view(view, width);
    for (y, row) in block.rows.iter().enumerate() {
        if let Some(x) = row.cell_x_of(needle) {
            return (x as u16, y as u16);
        }
    }
    panic!(
        "did not find {needle:?} in {:?}",
        block
            .rows
            .iter()
            .map(|row| row.plain_text())
            .collect::<Vec<_>>()
    );
}

fn lines(view: &View, width: u16) -> Vec<String> {
    compile_view(view, width)
        .rows
        .iter()
        .map(|row| row.plain_text())
        .collect()
}

fn nz(span: u16) -> NonZeroU16 {
    NonZeroU16::new(span).unwrap()
}

fn cell(text: &str) -> TableCell {
    TableCell::text(text)
}

fn spanned(text: &str, row_span: u16, col_span: u16) -> TableCell {
    TableCell::new([Block::paragraph(text)], None, nz(row_span), nz(col_span))
}

fn aligned(text: &str, alignment: Alignment) -> TableCell {
    TableCell::new([Block::paragraph(text)], Some(alignment), nz(1), nz(1))
}

fn walk_views(view: &View, visit: &mut impl FnMut(&View)) {
    visit(view);
    match view.kind() {
        ViewKind::Column(column) => {
            for child in column.children.iter() {
                walk_views(&child.view, visit);
            }
        }
        ViewKind::Row(row) => {
            for child in row.children.iter() {
                walk_views(&child.view, visit);
            }
        }
        ViewKind::Grid(grid) => {
            for cell in grid.cells.iter() {
                walk_views(&cell.view, visit);
            }
        }
        ViewKind::Hanging(hanging) => {
            walk_views(&hanging.prefix, visit);
            walk_views(&hanging.continuation_prefix, visit);
            walk_views(&hanging.body, visit);
        }
        ViewKind::Container(container) => walk_views(&container.child, visit),
        ViewKind::ClampRows(clamp) => walk_views(&clamp.child, visit),
        ViewKind::RowViewport(viewport) => walk_views(&viewport.child, visit),
        ViewKind::Text(_) | ViewKind::Spacer { .. } | ViewKind::ComponentSlot(_) => {}
    }
}

fn find_grid_view(view: &View) -> &View {
    find_grid_view_opt(view).expect("rendered table contains a Grid")
}

fn find_grid_view_opt(view: &View) -> Option<&View> {
    if matches!(view.kind(), ViewKind::Grid(_)) {
        return Some(view);
    }
    match view.kind() {
        ViewKind::Column(column) => column
            .children
            .iter()
            .find_map(|child| find_grid_view_opt(&child.view)),
        ViewKind::Row(row) => row
            .children
            .iter()
            .find_map(|child| find_grid_view_opt(&child.view)),
        ViewKind::Grid(_) => Some(view),
        ViewKind::Hanging(hanging) => find_grid_view_opt(&hanging.prefix)
            .or_else(|| find_grid_view_opt(&hanging.continuation_prefix))
            .or_else(|| find_grid_view_opt(&hanging.body)),
        ViewKind::Container(container) => find_grid_view_opt(&container.child),
        ViewKind::ClampRows(clamp) => find_grid_view_opt(&clamp.child),
        ViewKind::RowViewport(viewport) => find_grid_view_opt(&viewport.child),
        ViewKind::Text(_) | ViewKind::Spacer { .. } | ViewKind::ComponentSlot(_) => None,
    }
}

fn grid_ir(view: &View) -> &GridView {
    let ViewKind::Grid(grid) = find_grid_view(view).kind() else {
        unreachable!("find_grid_view returns a Grid");
    };
    grid.as_ref()
}

fn child_rects(view: &View, width: u16) -> Vec<Rect> {
    let tree = ViewCompiler::default().layout_tree(view, LayoutConstraints::width_only(width));
    let root = tree.node(tree.root);
    root.children.iter().map(|id| tree.node(*id).rect).collect()
}

fn render(block: &Block) -> View {
    TextRenderer::new().render(block)
}

fn render_with(policy: TextRenderPolicy, block: &Block) -> View {
    TextRenderer::with_policy(policy).render(block)
}

fn two_by_two() -> Table {
    Table::new(
        None::<Vec<Block>>,
        [TableColumn::start(), TableColumn::start()],
        1,
        [
            TableRow::new([cell("Head"), cell("Description")]),
            TableRow::new([cell("longer"), cell("x")]),
        ],
    )
    .unwrap()
}

fn project_markdown(options: MarkdownOptions, source: &str) -> Vec<Block> {
    let end = StreamOffset::new(source.len() as u64);
    let input = ProjectionBuilder::new(StreamOffset::ZERO, end, end, true)
        .emit(
            StreamRange::new(StreamOffset::ZERO, end),
            TextContent::raw(source),
        )
        .finish()
        .unwrap();
    let output = MarkdownProjector::new(options).project(&input).unwrap();
    output
        .spans()
        .iter()
        .flat_map(|span| span.values())
        .filter_map(|value| match value {
            TextContent::Block(block) => Some(block.clone()),
            TextContent::Raw(_) => None,
        })
        .collect()
}

#[test]
fn table_primary_structure_is_grid() {
    let view = render(&Block::table(two_by_two()));
    let mut grids = 0usize;
    let mut row_geometry = 0usize;
    walk_views(&view, &mut |candidate| match candidate.kind() {
        ViewKind::Grid(_) => grids += 1,
        ViewKind::Row(_) => row_geometry += 1,
        _ => {}
    });
    assert_eq!(grids, 1, "table matrix must be a single Grid");
    assert_eq!(
        row_geometry, 0,
        "table matrix must not be independent Rows per table row"
    );
}

#[test]
fn table_and_grid_placement_agree_for_spans() {
    let table = Table::new(
        None::<Vec<Block>>,
        [
            TableColumn::start(),
            TableColumn::start(),
            TableColumn::start(),
        ],
        0,
        [
            TableRow::new([spanned("A", 2, 2), cell("B")]),
            TableRow::new([cell("C")]),
            TableRow::new([cell("D"), cell("E"), cell("F")]),
        ],
    )
    .unwrap();
    let expected = table.cell_start_columns();
    let view = render(&Block::table(table));
    let grid = grid_ir(&view);
    let mut actual = vec![Vec::new(); expected.len()];
    for cell in grid.cells.iter() {
        actual[cell.row].push(cell.column);
    }
    assert_eq!(actual, expected);
}

#[test]
fn shared_columns_align_header_and_body() {
    let view = render(&Block::table(two_by_two()));
    let (head_x, _) = text_at(&view, 40, "Description");
    let (body_x, _) = text_at(&view, 40, "x");
    assert_eq!(head_x, body_x);
}

#[test]
fn column_span_occupies_shared_tracks() {
    let table = Table::new(
        None::<Vec<Block>>,
        [
            TableColumn::start(),
            TableColumn::start(),
            TableColumn::start(),
        ],
        0,
        [
            TableRow::new([spanned("AA", 1, 2), cell("B")]),
            TableRow::new([cell("C"), cell("D"), cell("E")]),
        ],
    )
    .unwrap();
    let view = render(&Block::table(table));
    let grid_view = find_grid_view(&view);
    let grid = grid_ir(&view);
    assert_eq!((grid.cells[0].column, grid.cells[0].column_span), (0, 2));
    let rects = child_rects(grid_view, 40);
    assert_eq!(rects.len(), 5);
    assert_eq!(rects[0].x, rects[2].x);
    assert!(
        rects[0].width > rects[2].width,
        "colspan cell must be wider than a single track: {rects:?}"
    );
    assert_eq!(rects[1].x, rects[4].x);
}

#[test]
fn row_span_uses_logical_column_alignment() {
    let table = Table::new(
        None::<Vec<Block>>,
        [TableColumn::end(), TableColumn::start()],
        0,
        [
            TableRow::new([spanned("AAAA", 2, 1), cell("B")]),
            TableRow::new([cell("C")]),
        ],
    )
    .unwrap();
    let view = render(&Block::table(table.clone()));
    let grid = grid_ir(&view);
    assert_eq!(grid.cells[0].row_span, 2);
    assert_eq!(grid.cells[0].column, 0);
    assert_eq!(grid.cells[1].column, 1);
    assert_eq!(grid.cells[2].column, 1);
    assert_eq!(
        table.cell_start_columns()[1][0],
        1,
        "C is logically column 1, not cells()[0]"
    );

    let grid_view = find_grid_view(&view);
    let rects = child_rects(grid_view, 20);
    assert_eq!(rects.len(), 3);
    assert_eq!(rects[1].x, rects[2].x);
    assert_eq!(rects[0].y, rects[1].y);
    assert!(rects[2].y > rects[1].y);

    let (b_x, _) = text_at(&view, 20, "B");
    let (c_x, _) = text_at(&view, 20, "C");
    assert_eq!(
        b_x, c_x,
        "C must use column 1 Start alignment, not row.cells()[0] → column 0 End"
    );
}

#[test]
fn table_alignment_and_cell_override() {
    let table = Table::new(
        None::<Vec<Block>>,
        [
            TableColumn::start(),
            TableColumn::center(),
            TableColumn::end(),
        ],
        0,
        [TableRow::new([cell("A"), cell("B"), cell("C")])],
    )
    .unwrap();
    let view = render(&Block::table(table)).fill_width();
    let (a_x, _) = text_at(&view, 30, "A");
    let (b_x, _) = text_at(&view, 30, "B");
    let (c_x, _) = text_at(&view, 30, "C");
    assert_eq!(a_x, 0);
    assert!(b_x > a_x, "center column sits after start: {b_x}");
    assert!(c_x > b_x, "end column sits after center: {c_x}");

    let overridden = Table::new(
        None::<Vec<Block>>,
        [
            TableColumn::start(),
            TableColumn::start(),
            TableColumn::start(),
        ],
        0,
        [
            TableRow::new([cell("AAAA"), cell("M"), cell("R")]),
            TableRow::new([aligned("L", Alignment::End), cell("m"), cell("r")]),
        ],
    )
    .unwrap();
    let view = render(&Block::table(overridden)).fill_width();
    let (start_x, _) = text_at(&view, 30, "AAAA");
    let (end_x, _) = text_at(&view, 30, "L");
    assert_eq!(start_x, 0);
    assert!(
        end_x > start_x,
        "cell End override must beat column Start: L={end_x} AAAA={start_x}"
    );
}

#[test]
fn table_sizing_policy_maps_to_grid_tracks() {
    let table = two_by_two();
    let flex = render_with(
        TextRenderPolicy::new().with_table_column_sizing(TableColumnSizing::Flex),
        &Block::table(table.clone()),
    );
    let content = render_with(
        TextRenderPolicy::new().with_table_column_sizing(TableColumnSizing::Content),
        &Block::table(table),
    );
    assert!(
        grid_ir(&flex)
            .columns
            .iter()
            .all(|track| matches!(track, TrackSize::Flex { .. }))
    );
    assert!(
        grid_ir(&content)
            .columns
            .iter()
            .all(|track| matches!(track, TrackSize::Content { .. }))
    );
    let flex_width = ViewCompiler::default()
        .layout_tree(&flex, LayoutConstraints::width_only(80))
        .size
        .width;
    let content_width = ViewCompiler::default()
        .layout_tree(&content, LayoutConstraints::width_only(80))
        .size
        .width;
    assert!(
        flex_width > content_width,
        "flex={flex_width} content={content_width}"
    );
}

#[test]
fn table_gaps_are_structural_policy() {
    let table = two_by_two();
    let view = render_with(
        TextRenderPolicy::new()
            .with_table_column_gap(3)
            .with_table_row_gap(2),
        &Block::table(table),
    );
    let grid = grid_ir(&view);
    assert_eq!(grid.column_gap, 3);
    assert_eq!(grid.row_gap, 2);
}

#[test]
fn table_caption_renders_above_grid() {
    let table = Table::new(
        Some([Block::paragraph("caption")]),
        [TableColumn::start()],
        1,
        [TableRow::new([cell("cell")])],
    )
    .unwrap();
    let view = render(&Block::table(table));
    let (caption_y, _) = {
        let (x, y) = text_at(&view, 40, "caption");
        (y, x)
    };
    let (cell_y, _) = {
        let (x, y) = text_at(&view, 40, "cell");
        (y, x)
    };
    assert!(caption_y < cell_y);
    assert!(matches!(view.kind(), ViewKind::Column(_)));
    assert!(matches!(find_grid_view(&view).kind(), ViewKind::Grid(_)));
}

#[test]
fn narrow_table_does_not_panic() {
    let view = render(&Block::table(two_by_two()));
    let block = compile_view(&view, 3);
    assert!(!block.rows.is_empty());
}

#[test]
fn header_bold_survives_grid_migration() {
    let table = two_by_two();
    let view = render(&Block::table(table.clone()));
    let empty = Theme::new();
    assert!(style_at(&view, &empty, "Head").bold);
    assert!(!style_at(&view, &empty, "longer").bold);
    let override_theme = Theme::new().with_text_style(
        TextSelector::table_row().table_section(TextTableSection::Header),
        StyleSpec::plain(),
    );
    assert!(!style_at(&render(&Block::table(table)), &override_theme, "Head").bold);
}

#[test]
fn table_cell_annotation_is_local() {
    let warning = SemanticTag::new("app", "warning").unwrap();
    let themed = Theme::new().with_text_style(
        TextSelector::table_cell().and_annotation(&warning),
        StyleSpec::new().reversed(),
    );
    let table = Table::new(
        None::<Vec<Block>>,
        [TableColumn::start(), TableColumn::start()],
        0,
        [TableRow::new([
            cell("warn").with_annotations(Annotations::new().with_tag(warning)),
            cell("plain"),
        ])],
    )
    .unwrap();
    let view = render(&Block::table(table));
    assert!(style_at(&view, &themed, "warn").reversed);
    assert!(!style_at(&view, &themed, "plain").reversed);
}

#[test]
fn table_row_annotation_does_not_become_a_cell_fact() {
    let warning = SemanticTag::new("app", "warning").unwrap();
    let row_theme = Theme::new().with_text_style(
        TextSelector::table_row().and_annotation(&warning),
        StyleSpec::new().dim(),
    );
    let cell_theme = Theme::new().with_text_style(
        TextSelector::table_cell().and_annotation(&warning),
        StyleSpec::new().reversed(),
    );
    let table = Table::new(
        None::<Vec<Block>>,
        [TableColumn::start(), TableColumn::start()],
        0,
        [TableRow::new([cell("one"), cell("two")])
            .with_annotations(Annotations::new().with_tag(warning))],
    )
    .unwrap();
    let view = render(&Block::table(table));
    assert!(style_at(&view, &row_theme, "one").dim);
    assert!(style_at(&view, &row_theme, "two").dim);
    assert!(
        !style_at(&view, &cell_theme, "one").reversed,
        "row annotation must not match table_cell selectors"
    );
}

#[test]
fn task_and_list_markers_are_independent_parts() {
    let theme = Theme::new()
        .with_text_style(
            TextSelector::part(TextPart::TaskMarker),
            StyleSpec::new().bold(),
        )
        .with_text_style(
            TextSelector::part(TextPart::ListMarker),
            StyleSpec::new().underline(),
        );
    let view = render(&Block::list(List::bulleted([ListItem::task("item", true)])));
    let task = style_at(&view, &theme, "[");
    let marker = style_at(&view, &theme, "-");
    let item = style_at(&view, &theme, "item");
    assert!(task.bold);
    assert!(!task.underline);
    assert!(marker.underline);
    assert!(!marker.bold);
    assert!(!item.bold);
    assert!(!item.underline);
}

#[test]
fn task_only_hides_list_marker_but_keeps_list_semantics() {
    let view = render_with(
        TextRenderPolicy::new().with_task_list_marker(TaskListMarkerPolicy::TaskOnly),
        &Block::list(List::bulleted([ListItem::task("item", true)])),
    );
    let painted = lines(&view, 40).join("\n");
    assert!(painted.contains("[x] item"));
    assert!(!painted.contains("-"));
    let theme = Theme::new().with_text_style(
        TextSelector::list_item().list_kind(crate::content::text::TextListKind::Bullet),
        StyleSpec::new().dim(),
    );
    assert!(style_at(&view, &theme, "item").dim);
}

#[test]
fn wrapped_task_continues_at_measured_prefix_width() {
    let view = render(&Block::list(List::bulleted([ListItem::task(
        "alpha beta gamma delta",
        true,
    )])));
    let block = compile_view(&view, 16);
    let first = block.rows[0].plain_text();
    let prefix = first.find("alpha").expect("body on first line");
    for row in block.rows.iter().skip(1) {
        let text = row.plain_text();
        if let Some(index) = text.find(|ch: char| !ch.is_whitespace()) {
            assert_eq!(index, prefix, "continuation indent must match prefix width");
        }
    }
}

#[test]
fn ordered_task_continuation_uses_measured_prefix() {
    let list = List::ordered(12, [ListItem::task("alpha beta gamma", true)]);
    let view = render(&Block::list(list));
    let block = compile_view(&view, 18);
    let first = block.rows[0].plain_text();
    assert!(first.contains("[x] 12. "));
    let prefix = first.find("alpha").expect("body");
    for row in block.rows.iter().skip(1) {
        let text = row.plain_text();
        if let Some(index) = text.find(|ch: char| !ch.is_whitespace()) {
            assert_eq!(index, prefix);
        }
    }
}

#[test]
fn nested_lists_keep_hanging_and_marker_identity() {
    let nested = Block::list(List::bulleted([ListItem::new([
        Block::paragraph("bullet"),
        Block::list(List::ordered(
            1,
            [ListItem::new([
                Block::paragraph("ordered"),
                Block::list(List::bulleted([ListItem::task("task", true)])),
            ])],
        )),
    ])]));
    let view = render(&nested);
    let mut hanging = 0usize;
    walk_views(&view, &mut |candidate| {
        if matches!(candidate.kind(), ViewKind::Hanging(_)) {
            hanging += 1;
        }
    });
    assert!(hanging >= 3, "nested hanging={hanging}");
    let theme = Theme::new()
        .with_text_style(
            TextSelector::part(TextPart::TaskMarker),
            StyleSpec::new().bold(),
        )
        .with_text_style(
            TextSelector::part(TextPart::ListMarker),
            StyleSpec::new().underline(),
        );
    assert!(style_at(&view, &theme, "[").bold);
    assert!(style_at(&view, &theme, "-").underline);
    assert!(style_at(&view, &theme, "1.").underline);
}

#[test]
fn code_label_policies() {
    let rust = LanguageId::new("rust").unwrap();
    let block = Block::code(CodeBlock::new(
        Some(rust.clone()),
        Some("rust linenums"),
        LiteralText::from("fn main() {}"),
    ));
    let hidden = lines(&render(&block), 40).join("\n");
    assert!(hidden.contains("fn main() {}"));
    assert!(!hidden.contains("rust"));

    let language = render_with(
        TextRenderPolicy::new().with_code_block_label(CodeBlockLabelPolicy::Language),
        &block,
    );
    let language_lines = lines(&language, 40);
    assert_eq!(language_lines[0].trim(), "rust");
    assert!(
        language_lines
            .iter()
            .any(|line| line.contains("fn main() {}"))
    );

    let info = render_with(
        TextRenderPolicy::new().with_code_block_label(CodeBlockLabelPolicy::Info),
        &block,
    );
    assert!(lines(&info, 40)[0].contains("rust linenums"));
}

#[test]
fn code_label_styling_does_not_select_body() {
    let rust = LanguageId::new("rust").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::CodeLabel),
        StyleSpec::new().dim(),
    );
    let view = render_with(
        TextRenderPolicy::new().with_code_block_label(CodeBlockLabelPolicy::Language),
        &Block::code(CodeBlock::new(
            Some(rust),
            Some("rust"),
            LiteralText::from("fn"),
        )),
    );
    assert!(style_at(&view, &theme, "rust").dim);
    assert!(!style_at(&view, &theme, "fn").dim);
}

#[test]
fn code_label_receives_block_annotations() {
    let tag = SemanticTag::new("app", "meta").unwrap();
    let rust = LanguageId::new("rust").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::CodeLabel).and_annotation(&tag),
        StyleSpec::new().reversed(),
    );
    let view = render_with(
        TextRenderPolicy::new().with_code_block_label(CodeBlockLabelPolicy::Language),
        &Block::code(CodeBlock::new(
            Some(rust),
            Some("rust"),
            LiteralText::from("fn"),
        ))
        .with_annotations(Annotations::new().with_tag(tag)),
    );
    assert!(style_at(&view, &theme, "rust").reversed);
    assert!(
        !style_at(&view, &theme, "fn").reversed,
        "block tag must not become a body-run annotation fact"
    );
}

#[test]
fn code_block_language_selector_survives_container() {
    let keyword = SemanticTag::new("syntax", "keyword").unwrap();
    let rust = LanguageId::new("rust").unwrap();
    let theme = Theme::new().with_text_style(
        TextSelector::annotation(&keyword).language(&rust),
        StyleSpec::new().reversed(),
    );
    let tagged =
        LiteralText::new([crate::TextRun::from("let")
            .map_annotations(|annotations| annotations.with_tag(keyword))]);
    let view = render(&Block::code(CodeBlock::new(
        Some(rust),
        Some("rust"),
        tagged,
    )));
    assert!(style_at(&view, &theme, "let").reversed);
}

#[test]
fn code_block_role_still_styles_the_body() {
    let theme = Theme::new().with_text_style(TextSelector::code_block(), StyleSpec::new().dim());
    let view = render(&Block::code(CodeBlock::new(
        Some(LanguageId::new("rust").unwrap()),
        Some("rust"),
        LiteralText::from("fn"),
    )));
    assert!(style_at(&view, &theme, "fn").dim);
}

#[test]
fn code_wrap_is_ordinary_view_wrapping() {
    let source = "abcdefghijklmnopqrstuvwxyz";
    let block = Block::code(CodeBlock::new(
        None,
        None::<&str>,
        LiteralText::from(source),
    ));
    let nowrap = compile_view(&render(&block), 8);
    assert_eq!(nowrap.rows.len(), 1);
    let wrapped = compile_view(
        &render_with(
            TextRenderPolicy::new().with_code_wrap(WrapMode::Grapheme),
            &block,
        ),
        8,
    );
    assert!(wrapped.rows.len() > 1);
}

#[test]
fn wrapped_quote_markers_keep_part_identity() {
    let theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::QuoteMarker),
        StyleSpec::new().reversed(),
    );
    let view = render(&Block::block_quote([Block::paragraph(
        "alpha beta gamma delta epsilon",
    )]));
    let painted = compile_view_with_theme(&view, 16, &theme);
    let mut saw_marker = false;
    for row in &painted.rows {
        let text = row.plain_text();
        if let Some(index) = text.find("> ") {
            saw_marker = true;
            assert!(row.style_at(index).expect("marker").reversed);
            if let Some(body) = text[index + 2..].find(|ch: char| !ch.is_whitespace()) {
                assert!(!row.style_at(index + 2 + body).expect("body").reversed);
            }
        }
    }
    assert!(saw_marker);
    assert!(painted.rows.len() > 1);
}

#[test]
fn nested_quote_and_quote_containing_list() {
    let theme = Theme::new().with_text_style(
        TextSelector::paragraph().and_role(TextRole::BlockQuote),
        StyleSpec::new().italic(),
    );
    let nested = render(&Block::block_quote([Block::block_quote([
        Block::paragraph("inner"),
    ])]));
    assert!(style_at(&nested, &theme, "inner").italic);
    let listed = render(&Block::block_quote([Block::list(List::bulleted([
        ListItem::paragraph("item"),
    ]))]));
    assert!(
        style_at(
            &listed,
            &Theme::new().with_text_style(
                TextSelector::part(TextPart::QuoteMarker),
                StyleSpec::new().reversed(),
            ),
            ">"
        )
        .reversed
    );
    assert!(
        !style_at(
            &listed,
            &Theme::new().with_text_style(
                TextSelector::part(TextPart::QuoteMarker),
                StyleSpec::new().reversed(),
            ),
            "item"
        )
        .reversed
    );
}

#[test]
fn thematic_rule_keeps_identity() {
    let theme = Theme::new().with_text_style(
        TextSelector::part(TextPart::ThematicRule),
        StyleSpec::new().dim(),
    );
    let view = render(&Block::thematic_break());
    assert!(style_at(&view, &theme, "─").dim);
    assert_eq!(lines(&view, 40)[0].trim(), "───");
}

#[test]
fn roman_ordered_task_renders_without_manual_width_math() {
    let list = List::new(
        ListMarker::Ordered {
            start: 4,
            style: NumberStyle::LowerRoman,
            delimiter: NumberDelimiter::Period,
        },
        true,
        [ListItem::task("item", true)],
    );
    let painted = lines(&render(&Block::list(list)), 40).join("\n");
    assert!(painted.contains("[x] iv. item"));
}

#[test]
fn hand_built_and_markdown_tables_match_until_origin_specialization() {
    let source = "| Head | Description |\n| --- | --- |\n| longer | x |\n";
    let markdown = project_markdown(MarkdownOptions::gfm(), source);
    let markdown_table = markdown
        .iter()
        .find(|block| matches!(block.kind(), BlockKind::Table(_)))
        .cloned()
        .unwrap();
    let BlockKind::Table(parsed) = markdown_table.kind() else {
        panic!("expected table");
    };
    let hand = Block::table(
        Table::new(
            None::<Vec<Block>>,
            parsed.columns().iter().copied(),
            parsed.header_rows(),
            parsed.rows().iter().map(|row| {
                TableRow::new(row.cells().iter().map(|cell| {
                    let BlockKind::Paragraph(content) = cell.blocks()[0].kind() else {
                        panic!("expected paragraph cell");
                    };
                    TableCell::plain([Block::paragraph(content.clone())])
                }))
            }),
        )
        .unwrap(),
    );
    let empty = Theme::new();
    assert_eq!(
        style_at(&render(&markdown_table), &empty, "Head"),
        style_at(&render(&hand), &empty, "Head")
    );
    let origin_theme = Theme::new().with_text_style(
        TextSelector::table_cell().origin(TextOrigin::MARKDOWN),
        StyleSpec::new().italic(),
    );
    assert!(style_at(&render(&markdown_table), &origin_theme, "Head").italic);
    assert!(!style_at(&render(&hand), &origin_theme, "Head").italic);
}

#[test]
fn gfm_end_to_end_render_fixture() {
    let source =
        "| Item | State |\n| --- | --- |\n| ~~old~~ | active |\n\n- [x] complete\n- [ ] pending\n";
    let blocks = project_markdown(MarkdownOptions::gfm(), source);
    let view = View::vertical(|column| {
        column.gap(1);
        for block in &blocks {
            column.child(render(block));
        }
    });
    let theme = Theme::new();
    assert!(style_at(&view, &theme, "old").strikethrough);
    assert!(style_at(&view, &theme, "Item").bold);
    assert!(!style_at(&view, &theme, "active").bold);
    let (item_x, _) = text_at(&view, 40, "Item");
    let (old_x, _) = text_at(&view, 40, "old");
    assert_eq!(item_x, old_x);
    let task_theme = Theme::new()
        .with_text_style(
            TextSelector::part(TextPart::TaskMarker),
            StyleSpec::new().reversed(),
        )
        .with_text_style(
            TextSelector::part(TextPart::ListMarker),
            StyleSpec::new().underline(),
        );
    assert!(style_at(&view, &task_theme, "[").reversed);
    assert!(style_at(&view, &task_theme, "-").underline);
    assert!(matches!(
        find_grid_view(&render(
            blocks
                .iter()
                .find(|block| matches!(block.kind(), BlockKind::Table(_)))
                .unwrap()
        ))
        .kind(),
        ViewKind::Grid(_)
    ));
}

#[test]
fn gfm_preset_enables_the_three_extensions() {
    assert_eq!(
        MarkdownOptions::gfm(),
        MarkdownOptions::commonmark()
            .with_tables(true)
            .with_strikethrough(true)
            .with_task_lists(true)
    );
    assert_eq!(MarkdownOptions::default(), MarkdownOptions::commonmark());
}
