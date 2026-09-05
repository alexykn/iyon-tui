use super::super::{
    Alignment, Block, BlockKind, CodeBlock, HeadingLevel, InlineContent, InlineKind, List,
    ListItem, ListMarker, NumberDelimiter, NumberStyle, Table, TableCell, TextListKind, TextPart,
    TextRole, TextTableSection, TextTaskState,
};
use super::TextRenderer;
use super::identity::{RenderContext, part_facts, semantic_view_facts, stamp_text, stamp_view};
use super::policy::{CodeBlockLabelPolicy, TableColumnSizing, TaskListMarkerPolicy};
use crate::{GridCellSpec, GridTrack, HorizontalAlign, IntoView, View};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BlockCacheKey {
    pub(crate) block_ptr: usize,
    pub(crate) context: super::identity::RenderContextKey,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockLoweringCache {
    entries: std::collections::HashMap<BlockCacheKey, BlockLoweringEntry>,
}

/// A cache hit must retain the immutable Block owner that supplied the key.
/// The address is only an efficient lookup key; without this owner a dropped
/// `Arc<BlockData>` could let the allocator reuse the address for a different
/// block while the stale View remained in the cache.
#[derive(Clone, Debug)]
pub(crate) struct BlockLoweringEntry {
    pub(crate) owner: Block,
    pub(crate) view: View,
}

impl BlockLoweringCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn get(&self, key: &BlockCacheKey) -> Option<&BlockLoweringEntry> {
        self.entries.get(key)
    }

    pub(crate) fn insert(&mut self, key: BlockCacheKey, owner: Block, view: View) {
        if self.entries.len() >= 1024 {
            self.entries.clear();
        }
        self.entries.insert(key, BlockLoweringEntry { owner, view });
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

impl TextRenderer {
    pub(super) fn lower_block(&self, block: &Block, context: &RenderContext) -> View {
        let block_ptr = block.identity_ptr();
        let key = BlockCacheKey {
            block_ptr,
            context: context.cache_key(),
        };
        if let Ok(cache) = self.cache.lock()
            && let Some(entry) = cache.get(&key)
        {
            debug_assert_eq!(entry.owner.identity_ptr(), block_ptr);
            return entry.view.clone();
        }
        let view = self.lower_block_uncached(block, context);
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, block.clone(), view.clone());
        }
        view
    }

    fn lower_block_uncached(&self, block: &Block, context: &RenderContext) -> View {
        let context = context.for_node(block.annotations());
        match block.kind() {
            BlockKind::Paragraph(content) => self.render_paragraph(block, content, &context, None),
            BlockKind::Heading { level, content } => {
                self.render_heading(block, *level, content, &context)
            }
            BlockKind::BlockQuote { blocks } => self.render_quote(block, blocks, &context),
            BlockKind::List(list) => self.render_list(block, list, &context),
            BlockKind::CodeBlock(code) => self.render_code_block(block, code, &context),
            BlockKind::Table(table) => self.render_table(block, table, &context),
            BlockKind::ThematicBreak => stamp_text(
                View::text("───").no_wrap(),
                semantic_view_facts(&context, TextRole::ThematicBreak, block.annotations())
                    .part(TextPart::ThematicRule),
            ),
            BlockKind::RawBlock { format, body } => {
                let facts = semantic_view_facts(&context, TextRole::RawBlock, block.annotations())
                    .format(format);
                let child_context = context.with_role(TextRole::RawBlock).with_format(format);
                stamp_text(self.render_literal(body, &child_context).no_wrap(), facts)
            }
            BlockKind::Container { blocks } => {
                let facts = semantic_view_facts(&context, TextRole::Container, block.annotations());
                let child_context = context.with_role(TextRole::Container);
                stamp_view(self.render_blocks(blocks, &child_context), facts)
            }
        }
    }

    pub(super) fn render_blocks(&self, blocks: &[Block], context: &RenderContext) -> View {
        self.render_blocks_with_gap(blocks, context, self.policy.block_gap())
    }

    fn render_blocks_with_gap(&self, blocks: &[Block], context: &RenderContext, gap: u16) -> View {
        // L1-04: moved children plus the direct column factory.
        View::column_from_views(
            blocks
                .iter()
                .map(|block| self.lower_block(block, context))
                .collect(),
            gap,
        )
    }

    pub(super) fn render_paragraph(
        &self,
        block: &Block,
        content: &InlineContent,
        context: &RenderContext,
        alignment: Option<HorizontalAlign>,
    ) -> View {
        let facts = semantic_view_facts(context, TextRole::Paragraph, block.annotations());
        let mut text = self.render_inline_content(content, context);
        if let Some(alignment) = alignment {
            text = text.text_align(alignment);
        }
        // Open pipe tables stay raw until they close. Word-wrap reflows them at
        // every ` | ` boundary, so the last fragment jumps by a row as cells arrive.
        if is_pipe_source_paragraph(content) {
            text = text.no_wrap();
        }
        stamp_text(text, facts)
    }

    fn render_heading(
        &self,
        block: &Block,
        level: HeadingLevel,
        content: &InlineContent,
        context: &RenderContext,
    ) -> View {
        let facts = semantic_view_facts(context, TextRole::Heading, block.annotations())
            .heading_level(level);
        stamp_text(self.render_inline_content(content, context), facts)
    }

    fn render_quote(&self, block: &Block, blocks: &[Block], context: &RenderContext) -> View {
        let facts = semantic_view_facts(context, TextRole::BlockQuote, block.annotations());
        let marker_facts = part_facts(context, TextPart::QuoteMarker, block.annotations());
        let prefix = stamp_text(View::text("> ").no_wrap(), marker_facts.clone());
        let continuation = stamp_text(View::text("> ").no_wrap(), marker_facts);
        let body = self.render_blocks(blocks, &context.with_role(TextRole::BlockQuote));
        stamp_view(View::hanging(prefix, continuation, body), facts)
    }

    fn render_list(&self, block: &Block, list: &List, context: &RenderContext) -> View {
        let kind = match list.marker() {
            ListMarker::Bullet => TextListKind::Bullet,
            ListMarker::Ordered { .. } => TextListKind::Ordered,
        };
        let facts =
            semantic_view_facts(context, TextRole::List, block.annotations()).list_kind(kind);
        let item_context = context.with_role(TextRole::List).with_list_kind(kind);
        // Gap only exists between sibling items. A one-item list has none,
        // so keep gap 0 even when the parent list was loose.
        let gap = if list.tight() || list.items().len() <= 1 {
            0
        } else {
            self.policy.block_gap()
        };
        // L1-04: moved children plus the direct column factory.
        let items = View::column_from_views(
            list.items()
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    self.render_list_item(list.marker(), index, item, list.tight(), &item_context)
                })
                .collect(),
            gap,
        );
        stamp_view(items, facts)
    }

    fn render_list_item(
        &self,
        marker: ListMarker,
        index: usize,
        item: &ListItem,
        tight: bool,
        context: &RenderContext,
    ) -> View {
        let item_context = context.for_node(item.annotations());
        let task_state = item.checked().map(|checked| {
            if checked {
                TextTaskState::Checked
            } else {
                TextTaskState::Unchecked
            }
        });
        let mut facts = semantic_view_facts(&item_context, TextRole::ListItem, item.annotations());
        if let Some(state) = task_state {
            facts = facts.task_state(state);
        }
        let mut marker_context = item_context.clone();
        marker_context.task_state = task_state;
        let prefix = self.list_item_prefix(marker, index, item, &marker_context);
        let body = self.render_blocks_with_gap(
            item.blocks(),
            &item_context
                .with_role(TextRole::ListItem)
                .with_task_state(task_state),
            if tight || item.blocks().len() <= 1 {
                0
            } else {
                self.policy.block_gap()
            },
        );
        stamp_view(View::hanging(prefix, View::spacer(0), body), facts)
    }

    fn list_item_prefix(
        &self,
        marker: ListMarker,
        index: usize,
        item: &ListItem,
        context: &RenderContext,
    ) -> View {
        let list_marker = stamp_text(
            View::text(list_marker_text(marker, index)).no_wrap(),
            part_facts(context, TextPart::ListMarker, item.annotations()),
        );
        let Some(checked) = item.checked() else {
            return list_marker;
        };
        let task_marker = stamp_text(
            View::text(if checked { "[x] " } else { "[ ] " }).no_wrap(),
            part_facts(context, TextPart::TaskMarker, item.annotations()),
        );
        match self.policy.task_list_marker() {
            TaskListMarkerPolicy::TaskOnly => task_marker,
            // L1-04: moved children plus the direct row factory.
            TaskListMarkerPolicy::TaskAndList => {
                View::row_from_views(vec![task_marker, list_marker], 0)
            }
        }
    }

    fn render_code_block(&self, block: &Block, code: &CodeBlock, context: &RenderContext) -> View {
        let mut facts = semantic_view_facts(context, TextRole::CodeBlock, block.annotations());
        if let Some(language) = code.language() {
            facts = facts.language(language);
        }
        let child_context = context
            .with_role(TextRole::CodeBlock)
            .with_language(code.language());
        let body = self
            .render_literal(code.body(), &child_context)
            .wrap(self.policy.code_wrap());
        let inner = match code_label_text(code, self.policy.code_block_label()) {
            Some(label) => {
                let label_context = context.with_language(code.language());
                let label_facts =
                    part_facts(&label_context, TextPart::CodeLabel, block.annotations());
                // L1-04: moved children plus the direct column factory.
                View::column_from_views(
                    vec![
                        stamp_text(View::text(label).no_wrap(), label_facts),
                        body.into_view(),
                    ],
                    self.policy.code_block_gap(),
                )
            }
            None => body.container(),
        };
        stamp_view(inner, facts)
    }

    fn render_table(&self, block: &Block, table: &Table, context: &RenderContext) -> View {
        let facts = semantic_view_facts(context, TextRole::Table, block.annotations());
        let table_context = context.with_role(TextRole::Table);
        let start_columns = table.cell_start_columns();
        let track = match self.policy.table_column_sizing() {
            TableColumnSizing::Content => GridTrack::content(),
            TableColumnSizing::Flex => GridTrack::flex(),
        };
        // L1-04: parsed rows move into final storage through the shared
        // placement factory. Source rows use content tracks, matching the
        // closure builder default.
        let mut rows = Vec::with_capacity(table.rows().len());
        for (row_index, row) in table.rows().iter().enumerate() {
            let section = if row_index < table.header_rows() {
                TextTableSection::Header
            } else {
                TextTableSection::Body
            };
            let row_context = table_context
                .for_node(row.annotations())
                .with_table_section(section);
            let row_facts =
                semantic_view_facts(&row_context, TextRole::TableRow, row.annotations());
            let mut cells = Vec::with_capacity(row.cells().len());
            for (cell_index, cell) in row.cells().iter().enumerate() {
                let logical_column = start_columns[row_index][cell_index];
                let cell_view = self.render_cell(
                    cell,
                    table,
                    logical_column,
                    &row_context.with_role(TextRole::TableRow),
                );
                let alignment = cell
                    .alignment()
                    .unwrap_or_else(|| table.columns()[logical_column].alignment());
                cells.push((
                    GridCellSpec::new()
                        .row_span(cell.row_span().get())
                        .column_span(cell.col_span().get())
                        .horizontal_align(to_horizontal_align(alignment)),
                    stamp_view(cell_view.container(), row_facts.clone()),
                ));
            }
            rows.push((GridTrack::content(), cells));
        }
        let mut grid = View::grid_from_parts(
            vec![track; table.columns().len()],
            self.policy.table_column_gap(),
            self.policy.table_row_gap(),
            rows,
        );
        if matches!(self.policy.table_column_sizing(), TableColumnSizing::Flex) {
            grid = grid.fill_width();
        }
        let body = if let Some(caption) = table.caption() {
            // L1-04: moved children plus the direct column factory.
            View::column_from_views(
                vec![self.render_blocks(caption, &table_context), grid],
                self.policy.block_gap(),
            )
        } else {
            grid
        };
        stamp_view(body, facts)
    }

    fn render_cell(
        &self,
        cell: &TableCell,
        table: &Table,
        logical_column: usize,
        context: &RenderContext,
    ) -> View {
        let context = context.for_node(cell.annotations());
        let facts = semantic_view_facts(&context, TextRole::TableCell, cell.annotations());
        let child_context = context.with_role(TextRole::TableCell);
        let horizontal = to_horizontal_align(
            cell.alignment()
                .unwrap_or_else(|| table.columns()[logical_column].alignment()),
        );
        let inner = if cell.blocks().len() == 1 {
            match cell.blocks()[0].kind() {
                BlockKind::Paragraph(content) => self.render_paragraph(
                    &cell.blocks()[0],
                    content,
                    &child_context,
                    Some(horizontal),
                ),
                _ => self.render_blocks(cell.blocks(), &child_context),
            }
        } else {
            self.render_blocks(cell.blocks(), &child_context)
        };
        stamp_view(inner.container(), facts)
    }
}

fn code_label_text(code: &CodeBlock, policy: CodeBlockLabelPolicy) -> Option<String> {
    match policy {
        CodeBlockLabelPolicy::Hidden => None,
        CodeBlockLabelPolicy::Language => {
            code.language().map(|language| language.as_str().to_owned())
        }
        CodeBlockLabelPolicy::Info => code
            .info()
            .filter(|info| !info.is_empty())
            .map(str::to_owned)
            .or_else(|| code.language().map(|language| language.as_str().to_owned())),
    }
}

fn list_marker_text(marker: ListMarker, index: usize) -> String {
    match marker {
        ListMarker::Bullet => "- ".to_owned(),
        ListMarker::Ordered {
            start,
            style,
            delimiter,
        } => format_marker(start.saturating_add(index as u64), style, delimiter),
    }
}

fn format_marker(value: u64, style: NumberStyle, delimiter: NumberDelimiter) -> String {
    let number = format_number(value, style);
    match delimiter {
        NumberDelimiter::Period => format!("{number}. "),
        NumberDelimiter::Paren => format!("{number}) "),
        NumberDelimiter::TwoParens => format!("({number}) "),
    }
}

fn format_number(value: u64, style: NumberStyle) -> String {
    match style {
        NumberStyle::Decimal => value.to_string(),
        NumberStyle::LowerAlpha => alpha_number(value, b'a'),
        NumberStyle::UpperAlpha => alpha_number(value, b'A'),
        NumberStyle::LowerRoman => roman_number(value).to_lowercase(),
        NumberStyle::UpperRoman => roman_number(value),
    }
}

fn alpha_number(mut value: u64, first: u8) -> String {
    if value == 0 {
        return String::new();
    }
    let mut result = Vec::new();
    while value != 0 {
        value -= 1;
        result.push((first + (value % 26) as u8) as char);
        value /= 26;
    }
    result.into_iter().rev().collect()
}

fn roman_number(mut value: u64) -> String {
    const VALUES: &[(u64, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut result = String::new();
    for &(unit, text) in VALUES {
        while value >= unit {
            value -= unit;
            result.push_str(text);
        }
    }
    result
}

fn to_horizontal_align(alignment: Alignment) -> HorizontalAlign {
    match alignment {
        Alignment::Default | Alignment::Start => HorizontalAlign::Start,
        Alignment::Center => HorizontalAlign::Center,
        Alignment::End => HorizontalAlign::End,
    }
}

fn is_pipe_source_paragraph(content: &InlineContent) -> bool {
    let mut text = String::new();
    for inline in content.iter() {
        match inline.kind() {
            InlineKind::Text(run) => text.push_str(run.text()),
            InlineKind::Break(_) => text.push('\n'),
            _ => return false,
        }
    }
    let mut saw_pipe_line = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.trim_start().starts_with('|') {
            return false;
        }
        saw_pipe_line = true;
    }
    saw_pipe_line
}
