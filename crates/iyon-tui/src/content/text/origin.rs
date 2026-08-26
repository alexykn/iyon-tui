use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
};

use super::{
    Annotations, Block, BlockKind, Image, Inline, InlineContent, InlineKind, List, ListItem,
    SemanticKey, SemanticValue, Table, TableCell, TableRow, TextIrError,
};

/// Identifies the syntax/projector that claimed a semantic text value.
///
/// Origin is semantic metadata and is distinct from `TextProvenance`, which
/// maps semantic output back to source byte ranges.
///
/// Generic styling should normally ignore origin. Applications may select it
/// when source-specific presentation is desired.
#[derive(Clone, Debug)]
pub struct TextOrigin(OriginAtom);

#[derive(Clone, Debug)]
enum OriginAtom {
    Static(&'static str),
    Owned(Arc<str>),
}

impl TextOrigin {
    /// Origin claimed by [`super::MarkdownProjector`].
    pub const MARKDOWN: Self = Self(OriginAtom::Static("markdown"));
    /// Origin claimed by [`super::PlainTextProjector`].
    pub const PLAIN_TEXT: Self = Self(OriginAtom::Static("plain-text"));

    pub fn markdown() -> Self {
        Self::MARKDOWN
    }

    pub fn plain_text() -> Self {
        Self::PLAIN_TEXT
    }

    pub fn new(value: impl Into<Arc<str>>) -> Result<Self, TextIrError> {
        let value = value.into();
        super::errors::validate_name(&value)?;
        Ok(Self(OriginAtom::Owned(value)))
    }

    pub fn as_str(&self) -> &str {
        match &self.0 {
            OriginAtom::Static(value) => value,
            OriginAtom::Owned(value) => value,
        }
    }
}

impl PartialEq for TextOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for TextOrigin {}

impl PartialOrd for TextOrigin {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextOrigin {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for TextOrigin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl fmt::Display for TextOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn origin_key() -> SemanticKey {
    static KEY: OnceLock<SemanticKey> = OnceLock::new();
    KEY.get_or_init(|| {
        SemanticKey::new("iyon-tui", "origin").expect("framework origin key is valid")
    })
    .clone()
}

impl Annotations {
    pub fn with_origin(self, origin: TextOrigin) -> Self {
        self.with_property(
            origin_key(),
            SemanticValue::Text(Arc::from(origin.as_str())),
        )
    }

    pub fn origin(&self) -> Option<TextOrigin> {
        match self.property(&origin_key())? {
            SemanticValue::Text(value) => TextOrigin::new(Arc::clone(value)).ok(),
            _ => None,
        }
    }
}

impl Block {
    pub fn with_origin(&self, origin: TextOrigin) -> Self {
        self.map_annotations(|annotations| annotations.with_origin(origin))
    }

    pub fn origin(&self) -> Option<TextOrigin> {
        self.annotations().origin()
    }
}

impl Inline {
    pub fn with_origin(&self, origin: TextOrigin) -> Self {
        self.map_annotations(|annotations| annotations.with_origin(origin))
    }

    pub fn origin(&self) -> Option<TextOrigin> {
        self.annotations().origin()
    }
}

impl ListItem {
    pub fn with_origin(self, origin: TextOrigin) -> Self {
        let annotations = self.annotations().clone().with_origin(origin);
        self.with_annotations(annotations)
    }

    pub fn origin(&self) -> Option<TextOrigin> {
        self.annotations().origin()
    }
}

impl TableRow {
    pub fn with_origin(self, origin: TextOrigin) -> Self {
        let annotations = self.annotations().clone().with_origin(origin);
        self.with_annotations(annotations)
    }

    pub fn origin(&self) -> Option<TextOrigin> {
        self.annotations().origin()
    }
}

impl TableCell {
    pub fn with_origin(self, origin: TextOrigin) -> Self {
        let annotations = self.annotations().clone().with_origin(origin);
        self.with_annotations(annotations)
    }

    pub fn origin(&self) -> Option<TextOrigin> {
        self.annotations().origin()
    }
}

pub(crate) fn stamp_block_origin(block: Block, origin: &TextOrigin) -> Block {
    let annotations = block.annotations().clone().with_origin(origin.clone());
    let kind = match block.kind().clone() {
        BlockKind::Paragraph(content) => {
            BlockKind::Paragraph(stamp_inline_content(content, origin))
        }
        BlockKind::Heading { level, content } => BlockKind::Heading {
            level,
            content: stamp_inline_content(content, origin),
        },
        BlockKind::BlockQuote { blocks } => BlockKind::BlockQuote {
            blocks: stamp_blocks(&blocks, origin),
        },
        BlockKind::Container { blocks } => BlockKind::Container {
            blocks: stamp_blocks(&blocks, origin),
        },
        BlockKind::List(list) => BlockKind::List(stamp_list(list, origin)),
        BlockKind::Table(table) => BlockKind::Table(stamp_table(table, origin)),
        BlockKind::CodeBlock(code) => BlockKind::CodeBlock(code),
        BlockKind::ThematicBreak => BlockKind::ThematicBreak,
        BlockKind::RawBlock { format, body } => BlockKind::RawBlock { format, body },
    };
    Block::from_parts(kind, annotations)
}

fn stamp_blocks(blocks: &[Block], origin: &TextOrigin) -> std::sync::Arc<[Block]> {
    blocks
        .iter()
        .cloned()
        .map(|block| stamp_block_origin(block, origin))
        .collect()
}

fn stamp_inline_content(content: InlineContent, origin: &TextOrigin) -> InlineContent {
    InlineContent::new(
        content
            .items()
            .iter()
            .cloned()
            .map(|inline| stamp_inline_origin(inline, origin)),
    )
}

fn stamp_inline_origin(inline: Inline, origin: &TextOrigin) -> Inline {
    let annotations = inline.annotations().clone().with_origin(origin.clone());
    let kind = match inline.kind().clone() {
        InlineKind::Image(image) => InlineKind::Image(Image::new(
            image.destination(),
            image.title(),
            stamp_inline_content(image.alt().clone(), origin),
        )),
        kind => kind,
    };
    Inline::from_parts(kind, inline.marks().clone(), annotations)
}

fn stamp_list(list: List, origin: &TextOrigin) -> List {
    let items = list.items().iter().map(|item| {
        let blocks = item
            .blocks()
            .iter()
            .cloned()
            .map(|block| stamp_block_origin(block, origin));
        ListItem::new(blocks)
            .with_annotations(item.annotations().clone().with_origin(origin.clone()))
            .with_checked(item.checked())
    });
    List::new(list.marker(), list.tight(), items)
}

fn stamp_table(table: Table, origin: &TextOrigin) -> Table {
    let caption = table
        .caption()
        .map(|blocks| stamp_blocks(blocks, origin).to_vec());
    let rows = table.rows().iter().map(|row| {
        let cells = row.cells().iter().map(|cell| {
            let blocks = cell
                .blocks()
                .iter()
                .cloned()
                .map(|block| stamp_block_origin(block, origin));
            TableCell::new(blocks, cell.alignment(), cell.row_span(), cell.col_span())
                .with_annotations(cell.annotations().clone().with_origin(origin.clone()))
        });
        TableRow::new(cells).with_annotations(row.annotations().clone().with_origin(origin.clone()))
    });
    Table::new(
        caption,
        table.columns().iter().copied(),
        table.header_rows(),
        rows,
    )
    .expect("stamping origin on a valid table must preserve table validity")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::text::{HeadingLevel, SemanticTag};

    #[test]
    fn origin_accepts_open_validated_names() {
        let origin = TextOrigin::new("asciidoc").unwrap();
        assert_eq!(origin.as_str(), "asciidoc");
        assert_eq!(TextOrigin::MARKDOWN.as_str(), "markdown");
        assert_eq!(TextOrigin::markdown(), TextOrigin::MARKDOWN);
        assert_eq!(TextOrigin::new("markdown").unwrap(), TextOrigin::MARKDOWN);
        assert!(TextOrigin::new("").is_err());
        assert!(TextOrigin::new("plain text").is_err());
    }

    #[test]
    fn annotations_store_a_single_origin_property() {
        let annotations = Annotations::new().with_origin(TextOrigin::MARKDOWN);
        assert_eq!(annotations.origin(), Some(TextOrigin::MARKDOWN));

        let replaced = annotations.with_origin(TextOrigin::new("asciidoc").unwrap());
        assert_eq!(replaced.origin().unwrap().as_str(), "asciidoc");
        assert_eq!(
            replaced
                .properties()
                .iter()
                .filter(|(key, _)| key == &origin_key())
                .count(),
            1
        );
    }

    #[test]
    fn origin_coexists_with_arbitrary_annotation_properties() {
        let tag = SemanticTag::new("example", "annotated").unwrap();
        let extra = SemanticKey::new("app", "foo").unwrap();
        let annotations = Annotations::new()
            .with_tag(tag.clone())
            .with_property(extra.clone(), "bar")
            .with_origin(TextOrigin::MARKDOWN);
        assert_eq!(annotations.origin(), Some(TextOrigin::MARKDOWN));
        assert!(annotations.contains_tag(&tag));
        assert_eq!(
            annotations.property(&extra),
            Some(&SemanticValue::from("bar"))
        );

        let syntax = SemanticKey::new("syntax", "bar").unwrap();
        let with_extra = annotations.with_property(syntax.clone(), true);
        assert_eq!(with_extra.origin(), Some(TextOrigin::MARKDOWN));
        assert_eq!(
            with_extra.property(&syntax),
            Some(&SemanticValue::Bool(true))
        );
    }

    #[test]
    fn block_and_inline_helpers_delegate_to_annotations() {
        let heading = Block::heading(HeadingLevel::H1, "Hello").with_origin(TextOrigin::MARKDOWN);
        assert_eq!(heading.origin(), Some(TextOrigin::MARKDOWN));
        assert_ne!(heading, Block::heading(HeadingLevel::H1, "Hello"));

        let inline = Inline::text("Hello").with_origin(TextOrigin::PLAIN_TEXT);
        assert_eq!(inline.origin(), Some(TextOrigin::PLAIN_TEXT));
    }

    #[test]
    fn recursive_stamping_covers_nested_semantic_containers() {
        let item = ListItem::paragraph("hello");
        let list = Block::list(List::bulleted([item]));
        let stamped = stamp_block_origin(list, &TextOrigin::MARKDOWN);
        assert_eq!(stamped.origin(), Some(TextOrigin::MARKDOWN));
        let item = &stamped.as_list().unwrap().items()[0];
        assert_eq!(item.origin(), Some(TextOrigin::MARKDOWN));
        assert_eq!(item.blocks()[0].origin(), Some(TextOrigin::MARKDOWN));
        let BlockKind::Paragraph(content) = item.blocks()[0].kind() else {
            panic!("expected paragraph");
        };
        assert_eq!(content.items()[0].origin(), Some(TextOrigin::MARKDOWN));
    }

    #[test]
    fn table_row_and_cell_helpers_own_origin_metadata() {
        let cell = TableCell::text("x").with_origin(TextOrigin::MARKDOWN);
        let row = TableRow::new([cell]).with_origin(TextOrigin::MARKDOWN);
        assert_eq!(row.origin(), Some(TextOrigin::MARKDOWN));
        assert_eq!(row.cells()[0].origin(), Some(TextOrigin::MARKDOWN));
    }
}
