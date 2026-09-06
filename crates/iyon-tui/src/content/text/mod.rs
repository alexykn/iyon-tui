mod annotations;
mod ansi;
mod block;
mod content;
mod diff;
mod errors;
mod inline;
mod markdown;
mod markdown_options;
mod origin;
mod plain;
mod provenance;
mod render;
mod source;
mod style;
mod validate;
mod visit;

#[cfg(test)]
mod migrated_tests;

pub use annotations::{Annotations, SemanticKey, SemanticTag, SemanticValue};
pub use ansi::{AnsiOptions, AnsiProjector};
pub use block::{
    Alignment, Block, BlockKind, CodeBlock, HeadingLevel, List, ListItem, ListMarker,
    NumberDelimiter, NumberStyle, Table, TableCell, TableColumn, TableRow,
};
pub use content::{RawText, TextContent};
pub use diff::DiffProjector;
pub use errors::{TextIrError, TextProjectionError};
pub use inline::{
    BreakKind, FormatId, Image, Inline, InlineContent, InlineKind, LanguageId, LinkTarget, Mark,
    MarkSet,
};
pub use markdown::MarkdownProjector;
pub use markdown_options::MarkdownOptions;
pub use origin::TextOrigin;
pub use plain::PlainTextProjector;
pub use provenance::{LiteralText, TextProvenance, TextRun};
pub(crate) use render::TextRenderer;
pub use render::{
    CodeBlockLabelPolicy, SoftBreakPolicy, TableColumnSizing, TaskListMarkerPolicy,
    TextRenderPolicy,
};
#[allow(unused_imports)]
pub(crate) use style::{TEXT_THEME_KEY, TextFacts, text_style_ref};
pub use style::{TextListKind, TextPart, TextRole, TextSelector, TextTableSection, TextTaskState};
pub use validate::{validate_text_content, validate_text_projection};
pub use visit::{
    TextRewriter, TextVisitor, walk_block, walk_content, walk_inline, walk_inline_content,
    walk_literal, walk_rewrite_block, walk_rewrite_blocks, walk_rewrite_content,
    walk_rewrite_inline, walk_rewrite_inline_content, walk_rewrite_literal,
};
