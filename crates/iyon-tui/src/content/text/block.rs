use std::{fmt, num::NonZeroU16, sync::Arc};

use super::{Annotations, FormatId, InlineContent, LanguageId, LiteralText, TextIrError};

/// Validated heading levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingLevel(u8);

impl HeadingLevel {
    pub const H1: Self = Self(1);
    pub const H2: Self = Self(2);
    pub const H3: Self = Self(3);
    pub const H4: Self = Self(4);
    pub const H5: Self = Self(5);
    pub const H6: Self = Self(6);

    pub fn new(level: u8) -> Result<Self, TextIrError> {
        (1..=6)
            .contains(&level)
            .then_some(Self(level))
            .ok_or(TextIrError::InvalidHeadingLevel)
    }

    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Generic list marker semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListMarker {
    Bullet,
    Ordered {
        start: u64,
        style: NumberStyle,
        delimiter: NumberDelimiter,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumberStyle {
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumberDelimiter {
    Period,
    Paren,
    TwoParens,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    marker: ListMarker,
    tight: bool,
    items: Arc<[ListItem]>,
}

impl List {
    pub fn bulleted(items: impl IntoIterator<Item = ListItem>) -> Self {
        Self::new(ListMarker::Bullet, true, items)
    }

    pub fn ordered(start: u64, items: impl IntoIterator<Item = ListItem>) -> Self {
        Self::new(
            ListMarker::Ordered {
                start,
                style: NumberStyle::Decimal,
                delimiter: NumberDelimiter::Period,
            },
            true,
            items,
        )
    }

    pub fn new(marker: ListMarker, tight: bool, items: impl IntoIterator<Item = ListItem>) -> Self {
        Self {
            marker,
            tight,
            items: items.into_iter().collect(),
        }
    }
    #[must_use]
    pub fn marker(&self) -> ListMarker {
        self.marker
    }
    #[must_use]
    pub fn tight(&self) -> bool {
        self.tight
    }
    #[must_use]
    pub fn items(&self) -> &[ListItem] {
        &self.items
    }

    #[must_use]
    pub fn with_tight(mut self, tight: bool) -> Self {
        self.tight = tight;
        self
    }

    pub fn with_number_style(mut self, style: NumberStyle) -> Result<Self, TextIrError> {
        let ListMarker::Ordered {
            start, delimiter, ..
        } = self.marker
        else {
            return Err(TextIrError::InvalidListConfiguration);
        };
        self.marker = ListMarker::Ordered {
            start,
            style,
            delimiter,
        };
        Ok(self)
    }

    pub fn with_delimiter(mut self, delimiter: NumberDelimiter) -> Result<Self, TextIrError> {
        let ListMarker::Ordered { start, style, .. } = self.marker else {
            return Err(TextIrError::InvalidListConfiguration);
        };
        self.marker = ListMarker::Ordered {
            start,
            style,
            delimiter,
        };
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    annotations: Annotations,
    checked: Option<bool>,
    blocks: Arc<[Block]>,
}

impl ListItem {
    pub fn paragraph(content: impl Into<InlineContent>) -> Self {
        Self::new([Block::paragraph(content)])
    }

    pub fn task(content: impl Into<InlineContent>, checked: bool) -> Self {
        Self::paragraph(content).with_checked(Some(checked))
    }

    pub fn new(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self {
            annotations: Annotations::default(),
            checked: None,
            blocks: blocks.into_iter().collect(),
        }
    }
    #[must_use]
    pub fn annotations(&self) -> &Annotations {
        &self.annotations
    }
    #[must_use]
    pub fn checked(&self) -> Option<bool> {
        self.checked
    }
    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    #[must_use]
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }
    #[must_use]
    pub fn with_checked(mut self, checked: Option<bool>) -> Self {
        self.checked = checked;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Alignment {
    Default,
    Start,
    Center,
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    caption: Option<Arc<[Block]>>,
    columns: Arc<[TableColumn]>,
    header_rows: usize,
    rows: Arc<[TableRow]>,
}

impl Table {
    pub fn new(
        caption: Option<impl IntoIterator<Item = Block>>,
        columns: impl IntoIterator<Item = TableColumn>,
        header_rows: usize,
        rows: impl IntoIterator<Item = TableRow>,
    ) -> Result<Self, TextIrError> {
        let table = Self {
            caption: caption.map(|value| value.into_iter().collect()),
            columns: columns.into_iter().collect(),
            header_rows,
            rows: rows.into_iter().collect(),
        };
        table.validate()?;
        Ok(table)
    }

    fn compute_cell_columns(&self) -> Result<Vec<Vec<usize>>, TextIrError> {
        if self.header_rows > self.rows.len() {
            return Err(TextIrError::InvalidTableHeaderRows {
                header_rows: self.header_rows,
                row_count: self.rows.len(),
            });
        }
        let width = self.columns.len();
        let mut occupied = vec![0usize; width];
        let mut starts = Vec::with_capacity(self.rows.len());
        for (row_index, row) in self.rows.iter().enumerate() {
            let mut column = 0usize;
            let mut row_starts = Vec::with_capacity(row.cells.len());
            for (cell_index, cell) in row.cells.iter().enumerate() {
                while column < width && occupied[column] > row_index {
                    column += 1;
                }
                let end = column.saturating_add(cell.col_span.get() as usize);
                if end > width {
                    return Err(TextIrError::TableCellDoesNotFit {
                        row: row_index,
                        cell: cell_index,
                    });
                }
                if occupied[column..end]
                    .iter()
                    .any(|&occupied_until| occupied_until > row_index)
                {
                    let column = occupied[column..end]
                        .iter()
                        .position(|&occupied_until| occupied_until > row_index)
                        .map_or(column, |offset| column + offset);
                    return Err(TextIrError::TableCellOverlaps {
                        row: row_index,
                        cell: cell_index,
                        column,
                    });
                }
                if row_index.saturating_add(cell.row_span.get() as usize) > self.rows.len() {
                    return Err(TextIrError::TableSpanExceedsRows {
                        row: row_index,
                        cell: cell_index,
                    });
                }
                for slot in &mut occupied[column..end] {
                    *slot = (*slot).max(row_index + cell.row_span.get() as usize);
                }
                row_starts.push(column);
                column = end;
            }
            starts.push(row_starts);
        }
        Ok(starts)
    }

    pub fn validate(&self) -> Result<(), TextIrError> {
        self.compute_cell_columns().map(|_| ())
    }

    pub(crate) fn cell_start_columns(&self) -> Vec<Vec<usize>> {
        self.compute_cell_columns()
            .expect("Table instances are validated at construction")
    }
    #[must_use]
    pub fn caption(&self) -> Option<&[Block]> {
        self.caption.as_deref()
    }
    #[must_use]
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }
    #[must_use]
    pub fn header_rows(&self) -> usize {
        self.header_rows
    }
    #[must_use]
    pub fn rows(&self) -> &[TableRow] {
        &self.rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableColumn {
    alignment: Alignment,
}
impl TableColumn {
    #[must_use]
    pub const fn start() -> Self {
        Self::new(Alignment::Start)
    }

    #[must_use]
    pub const fn center() -> Self {
        Self::new(Alignment::Center)
    }

    #[must_use]
    pub const fn end() -> Self {
        Self::new(Alignment::End)
    }

    #[must_use]
    pub const fn new(alignment: Alignment) -> Self {
        Self { alignment }
    }
    #[must_use]
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    annotations: Annotations,
    cells: Arc<[TableCell]>,
}
impl TableRow {
    pub fn new(cells: impl IntoIterator<Item = TableCell>) -> Self {
        Self {
            annotations: Annotations::default(),
            cells: cells.into_iter().collect(),
        }
    }
    #[must_use]
    pub fn annotations(&self) -> &Annotations {
        &self.annotations
    }
    #[must_use]
    pub fn cells(&self) -> &[TableCell] {
        &self.cells
    }
    #[must_use]
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell {
    annotations: Annotations,
    alignment: Option<Alignment>,
    row_span: NonZeroU16,
    col_span: NonZeroU16,
    blocks: Arc<[Block]>,
}
impl TableCell {
    pub fn text(content: impl Into<InlineContent>) -> Self {
        Self::plain([Block::paragraph(content)])
    }

    pub fn new(
        blocks: impl IntoIterator<Item = Block>,
        alignment: Option<Alignment>,
        row_span: NonZeroU16,
        col_span: NonZeroU16,
    ) -> Self {
        Self {
            annotations: Annotations::default(),
            alignment,
            row_span,
            col_span,
            blocks: blocks.into_iter().collect(),
        }
    }
    pub fn plain(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self::new(blocks, None, NonZeroU16::MIN, NonZeroU16::MIN)
    }
    #[must_use]
    pub fn annotations(&self) -> &Annotations {
        &self.annotations
    }
    #[must_use]
    pub fn alignment(&self) -> Option<Alignment> {
        self.alignment
    }
    #[must_use]
    pub fn row_span(&self) -> NonZeroU16 {
        self.row_span
    }
    #[must_use]
    pub fn col_span(&self) -> NonZeroU16 {
        self.col_span
    }
    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    #[must_use]
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }
}

/// Generic block-level semantic kind.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph(InlineContent),
    Heading {
        level: HeadingLevel,
        content: InlineContent,
    },
    BlockQuote {
        blocks: Arc<[Block]>,
    },
    List(List),
    CodeBlock(CodeBlock),
    Table(Table),
    ThematicBreak,
    RawBlock {
        format: FormatId,
        body: LiteralText,
    },
    Container {
        blocks: Arc<[Block]>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeBlock {
    language: Option<LanguageId>,
    info: Option<Arc<str>>,
    body: LiteralText,
}
impl CodeBlock {
    pub fn new(
        language: Option<LanguageId>,
        info: Option<impl Into<Arc<str>>>,
        body: impl Into<LiteralText>,
    ) -> Self {
        Self {
            language,
            info: info.map(Into::into),
            body: body.into(),
        }
    }
    #[must_use]
    pub fn language(&self) -> Option<&LanguageId> {
        self.language.as_ref()
    }
    #[must_use]
    pub fn info(&self) -> Option<&str> {
        self.info.as_deref()
    }
    #[must_use]
    pub fn body(&self) -> &LiteralText {
        &self.body
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Block(Arc<BlockData>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct BlockData {
    kind: BlockKind,
    annotations: Annotations,
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Block").field(&self.0.kind).finish()
    }
}

impl Block {
    #[must_use]
    pub fn new(kind: BlockKind) -> Self {
        Self(Arc::new(BlockData {
            kind,
            annotations: Annotations::default(),
        }))
    }
    pub fn paragraph(content: impl Into<InlineContent>) -> Self {
        Self::new(BlockKind::Paragraph(content.into()))
    }
    pub fn heading(level: HeadingLevel, content: impl Into<InlineContent>) -> Self {
        Self::new(BlockKind::Heading {
            level,
            content: content.into(),
        })
    }
    pub fn block_quote(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self::new(BlockKind::BlockQuote {
            blocks: blocks.into_iter().collect(),
        })
    }
    #[must_use]
    pub fn list(list: List) -> Self {
        Self::new(BlockKind::List(list))
    }
    #[must_use]
    pub fn code(code: CodeBlock) -> Self {
        Self::new(BlockKind::CodeBlock(code))
    }
    #[must_use]
    pub fn table(table: Table) -> Self {
        Self::new(BlockKind::Table(table))
    }
    #[must_use]
    pub fn thematic_break() -> Self {
        Self::new(BlockKind::ThematicBreak)
    }
    #[must_use]
    pub fn raw(format: FormatId, body: LiteralText) -> Self {
        Self::new(BlockKind::RawBlock { format, body })
    }
    pub fn container(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self::new(BlockKind::Container {
            blocks: blocks.into_iter().collect(),
        })
    }
    #[must_use]
    pub fn kind(&self) -> &BlockKind {
        &self.0.kind
    }
    #[must_use]
    pub fn annotations(&self) -> &Annotations {
        &self.0.annotations
    }
    #[must_use]
    pub fn with_annotations(&self, annotations: Annotations) -> Self {
        Self(Arc::new(BlockData {
            kind: self.0.kind.clone(),
            annotations,
        }))
    }
    #[must_use]
    pub fn map_annotations(&self, map: impl FnOnce(Annotations) -> Annotations) -> Self {
        self.with_annotations(map(self.annotations().clone()))
    }
    #[must_use]
    pub fn as_code_block(&self) -> Option<&CodeBlock> {
        match &self.0.kind {
            BlockKind::CodeBlock(code) => Some(code),
            _ => None,
        }
    }
    #[must_use]
    pub fn as_list(&self) -> Option<&List> {
        match &self.0.kind {
            BlockKind::List(list) => Some(list),
            _ => None,
        }
    }
    #[must_use]
    pub fn as_container(&self) -> Option<&[Block]> {
        match &self.0.kind {
            BlockKind::Container { blocks } => Some(blocks),
            _ => None,
        }
    }
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn from_parts(kind: BlockKind, annotations: Annotations) -> Self {
        Self(Arc::new(BlockData { kind, annotations }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU16;

    fn nz(span: u16) -> NonZeroU16 {
        NonZeroU16::new(span).unwrap()
    }

    fn cell(text: &str) -> TableCell {
        TableCell::text(text)
    }

    fn spanned(text: &str, row_span: u16, col_span: u16) -> TableCell {
        TableCell::new([Block::paragraph(text)], None, nz(row_span), nz(col_span))
    }

    fn columns(count: usize) -> Vec<TableColumn> {
        vec![TableColumn::start(); count]
    }

    #[test]
    fn ordinary_rows_place_cells_in_source_order() {
        let table = Table::new(
            None::<Vec<Block>>,
            columns(2),
            1,
            [
                TableRow::new([cell("H1"), cell("H2")]),
                TableRow::new([cell("A"), cell("B")]),
            ],
        )
        .unwrap();
        assert_eq!(table.cell_start_columns(), vec![vec![0, 1], vec![0, 1]]);
    }

    #[test]
    fn column_span_advances_the_next_start_column() {
        let table = Table::new(
            None::<Vec<Block>>,
            columns(3),
            0,
            [
                TableRow::new([spanned("A", 1, 2), cell("B")]),
                TableRow::new([cell("C"), cell("D"), cell("E")]),
            ],
        )
        .unwrap();
        assert_eq!(table.cell_start_columns(), vec![vec![0, 2], vec![0, 1, 2]]);
    }

    #[test]
    fn row_span_skips_occupied_columns_on_later_rows() {
        let table = Table::new(
            None::<Vec<Block>>,
            columns(2),
            0,
            [
                TableRow::new([spanned("A", 2, 1), cell("B")]),
                TableRow::new([cell("C")]),
            ],
        )
        .unwrap();
        assert_eq!(table.cell_start_columns(), vec![vec![0, 1], vec![1]]);
    }

    #[test]
    fn combined_spans_preserve_logical_columns() {
        let table = Table::new(
            None::<Vec<Block>>,
            columns(3),
            0,
            [
                TableRow::new([spanned("A", 2, 2), cell("B")]),
                TableRow::new([cell("C")]),
                TableRow::new([cell("D"), cell("E"), cell("F")]),
            ],
        )
        .unwrap();
        assert_eq!(
            table.cell_start_columns(),
            vec![vec![0, 2], vec![2], vec![0, 1, 2]]
        );
    }

    #[test]
    fn overlapping_spans_are_rejected() {
        assert!(matches!(
            Table::new(
                None::<Vec<Block>>,
                columns(3),
                0,
                [
                    TableRow::new([cell("A"), spanned("B", 2, 1), cell("C")]),
                    TableRow::new([spanned("D", 1, 2)]),
                ],
            ),
            Err(TextIrError::TableCellOverlaps { .. })
        ));
    }
}
