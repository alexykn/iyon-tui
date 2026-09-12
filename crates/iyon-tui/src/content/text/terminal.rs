//! Direct semantic text projection for terminal content.
//!
//! This module is deliberately separate from the retained [`View`] renderer.
//! It consumes already validated text IR and produces an immutable, owned
//! product whose rows can be painted repeatedly at one measured width.  The
//! product contains no source handle, connector, terminal state, or layout
//! cache.  In particular, painting never invokes Unicode wrapping again.

use std::{collections::HashMap, fmt, ops::Range, sync::Arc};

use taffy::prelude::{
    AvailableSpace, Dimension, Display, GridTemplateComponent, Size, Style, TrackSizingFunction,
};
use taffy::style_helpers::{FromLength, auto, fr, length, line, span};
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    Theme,
    geometry::Rect,
    physical::{PhysicalCell, PhysicalRow, PhysicalStyle, Surface, grapheme_cell_width},
    presentation::paint::{StyleContext, ThemeResolver},
    presentation::wrap::{WrapToken, wrap_token_lines},
    presentation::{HorizontalAlign, WrapMode},
    stream::{StreamOffset, StreamRange},
};

use super::{
    Alignment, Annotations, Block, BlockKind, BreakKind, CodeBlock, FormatId, HeadingLevel, Inline,
    InlineContent, InlineKind, LanguageId, List, ListItem, ListMarker, LiteralText, Mark,
    NumberDelimiter, NumberStyle, RawText, Table, TableCell, TextContent, TextFacts, TextListKind,
    TextOrigin, TextPart, TextProvenance, TextRenderPolicy, TextRole, TextRun, TextTableSection,
    TextTaskState, text_style_ref,
};

type SemanticContext = TerminalSemanticContext;

/// Width request used by direct semantic content measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalWidthConstraint {
    /// Wrap at exactly this terminal-cell width.  Zero is a real constraint.
    Definite(u16),
    /// Select the longest unbreakable unit under the active wrap policy.
    MinContent,
    /// Select the maximum unwrapped hard-line extent.
    MaxContent,
}

/// Width-only constraints for semantic content.
///
/// Height intentionally is not part of this type.  A host may allocate or
/// clip a product after measurement, but that must not silently alter the
/// semantic rows or their wrapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalConstraints {
    width: TerminalWidthConstraint,
}

impl TerminalConstraints {
    #[must_use]
    pub(crate) const fn definite(width: u16) -> Self {
        Self {
            width: TerminalWidthConstraint::Definite(width),
        }
    }

    #[must_use]
    pub(crate) const fn min_content() -> Self {
        Self {
            width: TerminalWidthConstraint::MinContent,
        }
    }

    #[must_use]
    pub(crate) const fn max_content() -> Self {
        Self {
            width: TerminalWidthConstraint::MaxContent,
        }
    }

    #[must_use]
    pub(crate) const fn width(self) -> TerminalWidthConstraint {
        self.width
    }
}

/// Checked failures while constructing or painting a terminal content product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TerminalProjectionError {
    ExtentOverflow { axis: &'static str, value: usize },
    RowCountOverflow { value: usize },
    RunIndexOverflow { value: usize },
    InvalidRunRange { run: usize, range: Range<usize> },
    PaintWindowOverflow { first_row: usize, row_count: usize },
    TableLayoutFailure,
}

impl fmt::Display for TerminalProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtentOverflow { axis, value } => {
                write!(formatter, "terminal {axis} extent {value} exceeds u16")
            }
            Self::RowCountOverflow { value } => {
                write!(formatter, "terminal row count {value} exceeds u16")
            }
            Self::RunIndexOverflow { value } => {
                write!(formatter, "terminal run index {value} exceeds u32")
            }
            Self::InvalidRunRange { run, range } => {
                write!(
                    formatter,
                    "terminal run {run} has invalid byte range {range:?}"
                )
            }
            Self::PaintWindowOverflow {
                first_row,
                row_count,
            } => write!(
                formatter,
                "terminal paint window {first_row}..{} exceeds product rows",
                first_row.saturating_add(*row_count)
            ),
            Self::TableLayoutFailure => write!(formatter, "terminal table Taffy layout failed"),
        }
    }
}

/// Painting failures are kept separate from projection failures so a caller
/// can distinguish a bad captured product from an invalid target/clip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TerminalPaintError {
    Projection(TerminalProjectionError),
}

impl fmt::Display for TerminalPaintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => error.fmt(formatter),
        }
    }
}

impl From<TerminalProjectionError> for TerminalPaintError {
    fn from(error: TerminalProjectionError) -> Self {
        Self::Projection(error)
    }
}

/// A bounded row window supplied to the exact paint consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalRowWindow {
    first_row: usize,
    row_count: usize,
}

impl TerminalRowWindow {
    #[must_use]
    pub(crate) const fn new(first_row: usize, row_count: usize) -> Self {
        Self {
            first_row,
            row_count,
        }
    }

    #[must_use]
    pub(crate) const fn first_row(self) -> usize {
        self.first_row
    }

    #[must_use]
    pub(crate) const fn row_count(self) -> usize {
        self.row_count
    }
}

/// Terminal-cell rectangle in the immutable semantic product.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalRect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

/// Two-dimensional terminal-cell extent without a position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalSize {
    width: u16,
    height: u16,
}

impl TerminalSize {
    #[must_use]
    pub(crate) const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub(crate) const fn width(self) -> u16 {
        self.width
    }

    #[must_use]
    pub(crate) const fn height(self) -> u16 {
        self.height
    }
}

impl TerminalRect {
    #[must_use]
    pub(crate) const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub(crate) const fn x(self) -> u16 {
        self.x
    }

    #[must_use]
    pub(crate) const fn y(self) -> u16 {
        self.y
    }

    #[must_use]
    pub(crate) const fn width(self) -> u16 {
        self.width
    }

    #[must_use]
    pub(crate) const fn height(self) -> u16 {
        self.height
    }
}

/// Captured semantic dimensions used by selectors and paint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalSemanticContext {
    ancestor_roles: Arc<[TextRole]>,
    heading_level: Option<HeadingLevel>,
    origin: Option<TextOrigin>,
    list_kind: Option<TextListKind>,
    task_state: Option<TextTaskState>,
    table_section: Option<TextTableSection>,
    language: Option<LanguageId>,
    format: Option<FormatId>,
}

impl TerminalSemanticContext {
    #[must_use]
    pub(crate) fn ancestor_roles(&self) -> &[TextRole] {
        &self.ancestor_roles
    }

    #[must_use]
    pub(crate) fn origin(&self) -> Option<&TextOrigin> {
        self.origin.as_ref()
    }

    #[must_use]
    pub(crate) const fn heading_level(&self) -> Option<HeadingLevel> {
        self.heading_level
    }

    #[must_use]
    pub(crate) fn list_kind(&self) -> Option<TextListKind> {
        self.list_kind
    }

    #[must_use]
    pub(crate) fn task_state(&self) -> Option<TextTaskState> {
        self.task_state
    }

    #[must_use]
    pub(crate) fn table_section(&self) -> Option<TextTableSection> {
        self.table_section
    }

    #[must_use]
    pub(crate) fn language(&self) -> Option<&LanguageId> {
        self.language.as_ref()
    }

    #[must_use]
    pub(crate) fn format(&self) -> Option<&FormatId> {
        self.format.as_ref()
    }

    fn for_node(&self, annotations: &Annotations) -> Self {
        let mut next = self.clone();
        if let Some(origin) = annotations.origin() {
            next.origin = Some(origin);
        }
        next
    }

    fn with_role(&self, role: TextRole) -> Self {
        let mut next = self.clone();
        let mut roles = next.ancestor_roles.to_vec();
        roles.push(role);
        next.ancestor_roles = roles.into();
        next
    }

    fn with_list_kind(&self, kind: TextListKind) -> Self {
        let mut next = self.clone();
        next.list_kind = Some(kind);
        next
    }

    fn with_task_state(&self, state: Option<TextTaskState>) -> Self {
        let mut next = self.clone();
        next.task_state = state;
        next
    }

    fn with_table_section(&self, section: TextTableSection) -> Self {
        let mut next = self.clone();
        next.table_section = Some(section);
        next
    }

    fn with_language(&self, language: Option<&LanguageId>) -> Self {
        let mut next = self.clone();
        next.language = language.cloned();
        next
    }

    fn with_format(&self, format: &FormatId) -> Self {
        let mut next = self.clone();
        next.format = Some(format.clone());
        next
    }

    fn with_heading_level(&self, level: HeadingLevel) -> Self {
        let mut next = self.clone();
        next.heading_level = Some(level);
        next
    }
}

/// Semantic identity retained by one block or run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalSemanticIdentity {
    role: TextRole,
    part: Option<TextPart>,
    context: TerminalSemanticContext,
    annotations: Annotations,
    facts: crate::presentation::StyleFacts,
}

fn terminal_identity(
    context: &SemanticContext,
    role: TextRole,
    part: Option<TextPart>,
    annotations: &Annotations,
) -> TerminalSemanticIdentity {
    let mut facts = TextFacts::new()
        .roles(context.ancestor_roles.iter().copied())
        .role(role);
    if let Some(part) = part {
        facts = facts.part(part);
    }
    if let Some(origin) = &context.origin {
        facts = facts.origin(origin);
    }
    if let Some(level) = context.heading_level {
        facts = facts.heading_level(level);
    }
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
    TerminalSemanticIdentity {
        role,
        part,
        context: TerminalSemanticContext {
            ancestor_roles: context.ancestor_roles.clone().into(),
            heading_level: context.heading_level,
            origin: context.origin.clone(),
            list_kind: context.list_kind,
            task_state: context.task_state,
            table_section: context.table_section,
            language: context.language.clone(),
            format: context.format.clone(),
        },
        annotations: annotations.clone(),
        facts: facts.annotations(annotations).finish(),
    }
}

impl TerminalSemanticIdentity {
    #[must_use]
    pub(crate) const fn role(&self) -> TextRole {
        self.role
    }

    #[must_use]
    pub(crate) const fn part(&self) -> Option<TextPart> {
        self.part
    }

    #[must_use]
    pub(crate) fn context(&self) -> &TerminalSemanticContext {
        &self.context
    }

    #[must_use]
    pub(crate) fn annotations(&self) -> &Annotations {
        &self.annotations
    }
}

/// One owned semantic text run. Rows refer to byte ranges in this owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalRun {
    text: Arc<str>,
    provenance: TextProvenance,
    identity: TerminalSemanticIdentity,
    style: crate::StyleRef,
}

impl TerminalRun {
    #[must_use]
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub(crate) fn provenance(&self) -> &TextProvenance {
        &self.provenance
    }

    #[must_use]
    pub(crate) fn identity(&self) -> &TerminalSemanticIdentity {
        &self.identity
    }
}

/// A row paint input. It contains no duplicate text: `byte_range` indexes the
/// owning [`TerminalRun`] in the product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalPaintSpan {
    run_index: usize,
    byte_range: Range<usize>,
    source: TextProvenance,
    x: u16,
    cell_width: u16,
}

impl TerminalPaintSpan {
    #[must_use]
    pub(crate) const fn run_index(&self) -> usize {
        self.run_index
    }

    #[must_use]
    pub(crate) fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }

    #[must_use]
    pub(crate) fn source(&self) -> &TextProvenance {
        &self.source
    }

    #[must_use]
    pub(crate) const fn x(&self) -> u16 {
        self.x
    }

    #[must_use]
    pub(crate) const fn cell_width(&self) -> u16 {
        self.cell_width
    }
}

/// One exact measured visual row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalRow {
    block_index: usize,
    spans: Arc<[TerminalPaintSpan]>,
    width: u16,
    fits: bool,
}

impl TerminalRow {
    #[must_use]
    pub(crate) const fn block_index(&self) -> usize {
        self.block_index
    }

    #[must_use]
    pub(crate) fn spans(&self) -> &[TerminalPaintSpan] {
        &self.spans
    }

    #[must_use]
    pub(crate) const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub(crate) const fn fits(&self) -> bool {
        self.fits
    }
}

/// Semantic block geometry flattened for efficient row-window paint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalBlock {
    kind: TerminalBlockKind,
    rect: TerminalRect,
    identity: TerminalSemanticIdentity,
    children: Arc<[usize]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalBlockKind {
    RawText,
    Paragraph,
    Heading(HeadingLevel),
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
}

impl TerminalBlock {
    #[must_use]
    pub(crate) const fn kind(&self) -> TerminalBlockKind {
        self.kind
    }

    #[must_use]
    pub(crate) const fn rect(&self) -> TerminalRect {
        self.rect
    }

    #[must_use]
    pub(crate) fn identity(&self) -> &TerminalSemanticIdentity {
        &self.identity
    }

    #[must_use]
    pub(crate) fn children(&self) -> &[usize] {
        &self.children
    }
}

/// Immutable, owned measured terminal semantic content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalTextProduct {
    size: TerminalSize,
    intrinsic_size: TerminalSize,
    constraint: TerminalWidthConstraint,
    wrap_width: u16,
    physically_complete: bool,
    runs: Arc<[TerminalRun]>,
    blocks: Arc<[TerminalBlock]>,
    rows: Arc<[TerminalRow]>,
}

impl TerminalTextProduct {
    #[must_use]
    pub(crate) const fn size(&self) -> TerminalSize {
        self.size
    }

    #[must_use]
    pub(crate) const fn intrinsic_size(&self) -> TerminalSize {
        self.intrinsic_size
    }

    #[must_use]
    pub(crate) const fn constraint(&self) -> TerminalWidthConstraint {
        self.constraint
    }

    #[must_use]
    pub(crate) const fn wrap_width(&self) -> u16 {
        self.wrap_width
    }

    #[must_use]
    pub(crate) const fn physically_complete(&self) -> bool {
        self.physically_complete
    }

    #[must_use]
    pub(crate) fn runs(&self) -> &[TerminalRun] {
        &self.runs
    }

    #[must_use]
    pub(crate) fn blocks(&self) -> &[TerminalBlock] {
        &self.blocks
    }

    #[must_use]
    pub(crate) fn rows(&self) -> &[TerminalRow] {
        &self.rows
    }

    /// Paint exactly the retained row window. Theme resolution changes cells,
    /// not geometry: this method never wraps, measures, or changes the row
    /// product. Rows outside `clip` are skipped with wide-glyph-safe writes.
    pub(crate) fn paint_window(
        &self,
        theme: &Theme,
        inherited: PhysicalStyle,
        target: &mut Surface,
        target_origin: (i32, i32),
        clip: Rect,
        window: TerminalRowWindow,
    ) -> Result<(), TerminalPaintError> {
        let end = window.first_row.checked_add(window.row_count).ok_or(
            TerminalProjectionError::PaintWindowOverflow {
                first_row: window.first_row,
                row_count: window.row_count,
            },
        )?;
        if end > self.rows.len() {
            return Err(TerminalProjectionError::PaintWindowOverflow {
                first_row: window.first_row,
                row_count: window.row_count,
            }
            .into());
        }
        let resolver = ThemeResolver::new(theme);
        let row_surface_width = self.size.width;
        for (row_index, row) in self.rows[window.first_row..end].iter().enumerate() {
            let mut row_surface = Surface::new(row_surface_width, 1);
            self.paint_row(row, &resolver, inherited, &mut row_surface)?;
            let row_offset = i32::try_from(row_index).map_err(|_| {
                TerminalPaintError::Projection(TerminalProjectionError::PaintWindowOverflow {
                    first_row: window.first_row,
                    row_count: window.row_count,
                })
            })?;
            let target_y =
                target_origin
                    .1
                    .checked_add(row_offset)
                    .ok_or(TerminalPaintError::Projection(
                        TerminalProjectionError::PaintWindowOverflow {
                            first_row: window.first_row,
                            row_count: window.row_count,
                        },
                    ))?;
            target.composite_clipped(&row_surface, target_origin.0, target_y, clip);
        }
        Ok(())
    }

    fn paint_row(
        &self,
        row: &TerminalRow,
        resolver: &ThemeResolver,
        inherited: PhysicalStyle,
        surface: &mut Surface,
    ) -> Result<(), TerminalPaintError> {
        for span in row.spans() {
            let run =
                self.runs
                    .get(span.run_index)
                    .ok_or(TerminalProjectionError::RunIndexOverflow {
                        value: span.run_index,
                    })?;
            let text = run.text.get(span.byte_range.clone()).ok_or_else(|| {
                TerminalProjectionError::InvalidRunRange {
                    run: span.run_index,
                    range: span.byte_range.clone(),
                }
            })?;
            let style = resolver.resolve_text_style(
                inherited,
                &run.style,
                &StyleContext::default().with_local_facts(&run.identity.facts),
            );
            let (painted_width, clipped) = paint_span_graphemes(span, text, style, surface)?;
            if clipped {
                surface.physically_complete = false;
            }
            if !clipped && painted_width != usize::from(span.cell_width) {
                return Err(TerminalProjectionError::ExtentOverflow {
                    axis: "paint span width",
                    value: painted_width,
                }
                .into());
            }
        }
        Ok(())
    }

    /// Materialize one retained row for tests and content-local consumers.
    pub(crate) fn paint_row_to_physical(
        &self,
        theme: &Theme,
        inherited: PhysicalStyle,
        row_index: usize,
    ) -> Result<Option<PhysicalRow>, TerminalPaintError> {
        let Some(row) = self.rows.get(row_index) else {
            return Ok(None);
        };
        let resolver = ThemeResolver::new(theme);
        let mut surface = Surface::new(self.size.width, 1);
        self.paint_row(row, &resolver, inherited, &mut surface)?;
        Ok(Some(PhysicalRow::from_cells(surface.cells)))
    }
}

fn paint_span_graphemes(
    span: &TerminalPaintSpan,
    text: &str,
    style: PhysicalStyle,
    surface: &mut Surface,
) -> Result<(usize, bool), TerminalPaintError> {
    let mut column = span.x;
    let mut painted_width = 0usize;
    for grapheme in text.graphemes(true) {
        let width = grapheme_cell_width(grapheme);
        if width == 0 {
            continue;
        }
        let end = usize::from(column).checked_add(width).ok_or(
            TerminalProjectionError::ExtentOverflow {
                axis: "paint row",
                value: usize::MAX,
            },
        )?;
        if end > usize::from(surface.width()) {
            return Ok((painted_width, true));
        }
        surface.clear_glyph_at(column, 0);
        *surface.get_mut(column, 0) = PhysicalCell {
            grapheme: Some(grapheme.to_owned()),
            style,
            painted: true,
            continuation: false,
        };
        for continuation in 1..width {
            let continuation_column = column
                .checked_add(u16::try_from(continuation).map_err(|_| {
                    TerminalPaintError::Projection(TerminalProjectionError::ExtentOverflow {
                        axis: "paint continuation",
                        value: continuation,
                    })
                })?)
                .ok_or(TerminalPaintError::Projection(
                    TerminalProjectionError::ExtentOverflow {
                        axis: "paint continuation",
                        value: usize::MAX,
                    },
                ))?;
            *surface.get_mut(continuation_column, 0) = PhysicalCell {
                grapheme: None,
                style,
                painted: true,
                continuation: true,
            };
        }
        column = column
            .checked_add(u16::try_from(width).map_err(|_| {
                TerminalPaintError::Projection(TerminalProjectionError::ExtentOverflow {
                    axis: "paint span",
                    value: width,
                })
            })?)
            .ok_or(TerminalPaintError::Projection(
                TerminalProjectionError::ExtentOverflow {
                    axis: "paint span",
                    value: usize::MAX,
                },
            ))?;
        painted_width =
            painted_width
                .checked_add(width)
                .ok_or(TerminalProjectionError::ExtentOverflow {
                    axis: "paint span width",
                    value: usize::MAX,
                })?;
    }
    Ok((painted_width, false))
}

/// Pure direct projector from semantic text IR to terminal rows and boxes.
#[derive(Clone, Debug)]
pub(crate) struct TerminalTextProjector {
    policy: TextRenderPolicy,
}

impl TerminalTextProjector {
    #[must_use]
    pub(crate) fn new(policy: TextRenderPolicy) -> Self {
        Self { policy }
    }

    #[must_use]
    pub(crate) fn policy(&self) -> &TextRenderPolicy {
        &self.policy
    }

    pub(crate) fn project(
        &self,
        content: &TextContent,
        constraints: TerminalConstraints,
    ) -> Result<TerminalTextProduct, TerminalProjectionError> {
        self.project_contents(std::slice::from_ref(content), constraints)
    }

    pub(crate) fn project_contents(
        &self,
        contents: &[TextContent],
        constraints: TerminalConstraints,
    ) -> Result<TerminalTextProduct, TerminalProjectionError> {
        let intrinsic = measure_contents(contents, &self.policy);
        let selected = match constraints.width() {
            TerminalWidthConstraint::Definite(width) => usize::from(width),
            TerminalWidthConstraint::MinContent => intrinsic.min_width,
            TerminalWidthConstraint::MaxContent => intrinsic.max_width,
        };
        let wrap_width = checked_extent("width", selected)?;
        let mut builder = ProductBuilder::new(&self.policy, wrap_width);
        builder.render_contents(contents, wrap_width)?;
        builder.finish(
            constraints.width(),
            wrap_width,
            checked_extent("intrinsic width", intrinsic.max_width)?,
            intrinsic,
        )
    }
}

/// Alias used by content owners that call the type simply `TerminalProjector`.
pub(crate) type TerminalProjector = TerminalTextProjector;

/// Alias emphasizing that this is a measured content product, not a View.
pub(crate) type TerminalContentProduct = TerminalTextProduct;

#[derive(Clone, Copy, Debug, Default)]
struct IntrinsicMetrics {
    min_width: usize,
    max_width: usize,
}

#[derive(Clone, Debug)]
struct ProductBuilder<'a> {
    policy: &'a TextRenderPolicy,
    width: u16,
    runs: Vec<TerminalRun>,
    blocks: Vec<TerminalBlock>,
    rows: Vec<TerminalRow>,
    run_lookup: HashMap<usize, usize>,
}

#[derive(Clone, Debug)]
struct TableCellInput<'a> {
    row_index: usize,
    logical_column: usize,
    cell: &'a TableCell,
    context: SemanticContext,
}

#[derive(Clone, Copy, Debug)]
struct TableCellGeometry {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Clone, Debug)]
struct TableGridLayout {
    height: usize,
    row_edges: Vec<usize>,
    cells: Vec<TableCellGeometry>,
}

fn build_cell_product<'a>(
    policy: &'a TextRenderPolicy,
    table: &Table,
    cell: &TableCellInput<'_>,
    width: u16,
) -> Result<ProductBuilder<'a>, TerminalProjectionError> {
    let mut product = ProductBuilder::new(policy, width);
    let root = product.begin_block(
        TerminalBlockKind::TableCell,
        terminal_identity(
            &cell.context,
            TextRole::TableCell,
            None,
            cell.cell.annotations(),
        ),
        0,
        width,
    )?;
    product.render_cell(
        root,
        table,
        cell.cell,
        cell.logical_column,
        &cell.context,
        0,
        width,
    )?;
    product.finish_block(root, 0, width)?;
    Ok(product)
}

fn layout_table_grid(
    table: &Table,
    cells: &[TableCellInput<'_>],
    policy: &TextRenderPolicy,
    width: u16,
) -> Result<TableGridLayout, TerminalProjectionError> {
    let row_count = table.rows().len();
    if row_count == 0 {
        return Ok(TableGridLayout {
            height: 0,
            row_edges: vec![0],
            cells: Vec::new(),
        });
    }
    let mut grid = taffy::TaffyTree::<()>::with_capacity(cells.len() + row_count + 1);
    grid.disable_rounding();
    let mut cell_nodes = Vec::with_capacity(cells.len());
    for input in cells {
        let row_end = input
            .row_index
            .checked_add(usize::from(input.cell.row_span().get()))
            .ok_or(TerminalProjectionError::TableLayoutFailure)?;
        let column_end = input
            .logical_column
            .checked_add(usize::from(input.cell.col_span().get()))
            .ok_or(TerminalProjectionError::TableLayoutFailure)?;
        if row_end > row_count || column_end > table.columns().len() {
            return Err(TerminalProjectionError::TableLayoutFailure);
        }
        let row_line = i16::try_from(input.row_index + 1)
            .map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
        let column_line = i16::try_from(input.logical_column + 1)
            .map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
        let mut style = Style::default();
        style.display = Display::Grid;
        style.grid_row = taffy::geometry::Line {
            start: line(row_line),
            end: span(input.cell.row_span().get()),
        };
        style.grid_column = taffy::geometry::Line {
            start: line(column_line),
            end: span(input.cell.col_span().get()),
        };
        let node = grid
            .new_leaf(style)
            .map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
        cell_nodes.push(node);
    }
    let mut row_markers = Vec::with_capacity(row_count);
    for row in 0..row_count {
        let row_line =
            i16::try_from(row + 1).map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
        let mut style = Style::default();
        style.position = taffy::prelude::Position::Absolute;
        style.size = taffy::prelude::Size {
            width: Dimension::length(0.0_f32),
            height: Dimension::length(0.0_f32),
        };
        style.grid_row = line(row_line);
        style.grid_column = line(1);
        row_markers.push(
            grid.new_leaf(style)
                .map_err(|_| TerminalProjectionError::TableLayoutFailure)?,
        );
    }
    let mut table_style = Style::default();
    table_style.display = Display::Grid;
    table_style.size.width = Dimension::length(f32::from(width));
    let content_widths = matches!(
        policy.table_column_sizing(),
        super::TableColumnSizing::Content
    )
    .then(|| table_column_widths(table, policy, width))
    .transpose()?;
    table_style.grid_template_columns = (0..table.columns().len())
        .map(|column| {
            let track = if let Some(widths) = content_widths.as_ref() {
                TrackSizingFunction::from_length(f32::from(widths[column]))
            } else {
                fr(1.0_f32)
            };
            GridTemplateComponent::Single(track)
        })
        .collect();
    table_style.grid_template_rows = (0..row_count)
        .map(|_| GridTemplateComponent::Single(auto()))
        .collect();
    table_style.gap = Size {
        width: length(f32::from(policy.table_column_gap())),
        height: length(f32::from(policy.table_row_gap())),
    };
    let mut children = cell_nodes.clone();
    children.extend(row_markers.iter().copied());
    let root = grid
        .new_with_children(table_style, &children)
        .map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
    let mut measure_error = None;
    grid.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::Definite(f32::from(width)),
            height: AvailableSpace::MaxContent,
        },
        |known, available, node, _, _| {
            let Some(cell_index) = cell_nodes.iter().position(|candidate| *candidate == node)
            else {
                return Size::ZERO;
            };
            let input = &cells[cell_index];
            let metrics = measure_block_slice(input.cell.blocks(), policy);
            let min_width = match u16::try_from(metrics.min_width) {
                Ok(width) => f32::from(width),
                Err(_) => {
                    measure_error = Some(TerminalProjectionError::ExtentOverflow {
                        axis: "table cell min-content width",
                        value: metrics.min_width,
                    });
                    return Size::ZERO;
                }
            };
            let max_width = match u16::try_from(metrics.max_width) {
                Ok(width) => f32::from(width),
                Err(_) => {
                    measure_error = Some(TerminalProjectionError::ExtentOverflow {
                        axis: "table cell max-content width",
                        value: metrics.max_width,
                    });
                    return Size::ZERO;
                }
            };
            let requested_width = known.width.or(match available.width {
                AvailableSpace::Definite(value) => Some(value),
                AvailableSpace::MinContent => Some(min_width),
                AvailableSpace::MaxContent => Some(max_width),
            });
            let requested_width = requested_width.expect("table grid always supplies a width");
            let width = match checked_table_value("table cell width", requested_width) {
                Ok(value) => value,
                Err(error) => {
                    measure_error = Some(error);
                    return Size::ZERO;
                }
            };
            let width = match u16::try_from(width) {
                Ok(width) => width,
                Err(_) => {
                    measure_error = Some(TerminalProjectionError::ExtentOverflow {
                        axis: "table cell width",
                        value: usize::try_from(width).expect("checked table width is nonnegative"),
                    });
                    return Size::ZERO;
                }
            };
            let height = match build_cell_product(policy, table, input, width).and_then(|product| {
                u16::try_from(product.rows.len()).map_err(|_| {
                    TerminalProjectionError::RowCountOverflow {
                        value: product.rows.len(),
                    }
                })
            }) {
                Ok(value) => f32::from(value),
                Err(error) => {
                    measure_error = Some(error);
                    return Size::ZERO;
                }
            };
            Size {
                width: known.width.unwrap_or(requested_width),
                height: known.height.unwrap_or(height),
            }
        },
    )
    .map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
    if let Some(error) = measure_error {
        return Err(error);
    }
    let root_layout = grid.unrounded_layout(root).to_owned();
    let height = usize::from(checked_extent(
        "table height",
        usize::try_from(checked_table_value(
            "table height",
            root_layout.size.height,
        )?)
        .map_err(|_| TerminalProjectionError::TableLayoutFailure)?,
    )?);
    let mut row_edges = Vec::with_capacity(row_count + 1);
    for marker in row_markers {
        let edge = usize::try_from(checked_table_value(
            "table row edge",
            grid.unrounded_layout(marker).location.y,
        )?)
        .map_err(|_| TerminalProjectionError::TableLayoutFailure)?;
        row_edges.push(usize::from(checked_extent("table row edge", edge)?));
    }
    row_edges.push(height);
    for pair in row_edges.windows(2) {
        if pair[1] < pair[0] {
            return Err(TerminalProjectionError::TableLayoutFailure);
        }
    }
    let mut geometries = Vec::with_capacity(cell_nodes.len());
    for node in cell_nodes {
        let layout = grid.unrounded_layout(node);
        let x = checked_table_value("table cell x", layout.location.x)?;
        let y = checked_table_value("table cell y", layout.location.y)?;
        let right = checked_table_value("table cell right", layout.location.x + layout.size.width)?;
        let bottom =
            checked_table_value("table cell bottom", layout.location.y + layout.size.height)?;
        let width = right
            .checked_sub(x)
            .ok_or(TerminalProjectionError::TableLayoutFailure)?;
        let height = bottom
            .checked_sub(y)
            .ok_or(TerminalProjectionError::TableLayoutFailure)?;
        geometries.push(TableCellGeometry {
            x: checked_extent(
                "table cell x",
                usize::try_from(x).map_err(|_| TerminalProjectionError::TableLayoutFailure)?,
            )?,
            y: checked_extent(
                "table cell y",
                usize::try_from(y).map_err(|_| TerminalProjectionError::TableLayoutFailure)?,
            )?,
            width: checked_extent(
                "table cell width",
                usize::try_from(width).map_err(|_| TerminalProjectionError::TableLayoutFailure)?,
            )?,
            height: checked_extent(
                "table cell height",
                usize::try_from(height).map_err(|_| TerminalProjectionError::TableLayoutFailure)?,
            )?,
        });
    }
    Ok(TableGridLayout {
        height,
        row_edges,
        cells: geometries,
    })
}

fn checked_table_value(axis: &'static str, value: f32) -> Result<i32, TerminalProjectionError> {
    let rounded = f64::from(value).round();
    if !rounded.is_finite() || rounded < 0.0 || rounded > f64::from(i32::MAX) {
        return Err(TerminalProjectionError::ExtentOverflow {
            axis,
            value: if value.is_sign_negative() {
                0
            } else {
                usize::MAX
            },
        });
    }
    let rounded = rounded as i64;
    i32::try_from(rounded).map_err(|_| TerminalProjectionError::ExtentOverflow {
        axis,
        value: usize::MAX,
    })
}

impl<'a> ProductBuilder<'a> {
    fn new(policy: &'a TextRenderPolicy, width: u16) -> Self {
        Self {
            policy,
            width,
            runs: Vec::new(),
            blocks: Vec::new(),
            rows: Vec::new(),
            run_lookup: HashMap::new(),
        }
    }

    fn finish(
        self,
        constraint: TerminalWidthConstraint,
        wrap_width: u16,
        intrinsic_width: u16,
        _intrinsic: IntrinsicMetrics,
    ) -> Result<TerminalTextProduct, TerminalProjectionError> {
        let height = checked_rows(self.rows.len())?;
        let intrinsic_height = height;
        let physically_complete = self.rows.iter().all(TerminalRow::fits);
        Ok(TerminalTextProduct {
            size: TerminalSize::new(wrap_width, height),
            intrinsic_size: TerminalSize::new(intrinsic_width, intrinsic_height),
            constraint,
            wrap_width,
            physically_complete,
            runs: self.runs.into(),
            blocks: self.blocks.into(),
            rows: self.rows.into(),
        })
    }

    fn render_contents(
        &mut self,
        contents: &[TextContent],
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let mut previous_list = None;
        for (index, content) in contents.iter().enumerate() {
            if index > 0 {
                let gap = block_gap_between(previous_list, content, self.policy);
                self.push_blank_rows(gap);
            }
            let (kind, annotations) = match content {
                TextContent::Raw(_) => (TerminalBlockKind::RawText, Annotations::default()),
                TextContent::Block(block) => (block_kind(block), block.annotations().clone()),
            };
            let context = SemanticContext::default();
            let index = self.begin_block(
                kind,
                terminal_identity(&context, block_role(content), None, &annotations),
                0,
                width,
            )?;
            match content {
                TextContent::Raw(raw) => {
                    let pieces = self.raw_pieces(raw, &context);
                    self.render_text_rows(
                        index,
                        pieces,
                        0,
                        width,
                        self.policy.text_wrap(),
                        HorizontalAlign::Start,
                    )?;
                }
                TextContent::Block(block) => {
                    self.render_block_into(index, block, &context, 0, width)?
                }
            }
            self.finish_block(index, 0, width)?;
            previous_list = list_marker(content);
        }
        Ok(())
    }

    fn begin_block(
        &mut self,
        kind: TerminalBlockKind,
        identity: TerminalSemanticIdentity,
        x: u16,
        width: u16,
    ) -> Result<usize, TerminalProjectionError> {
        let y = checked_extent("block y", self.rows.len())?;
        let index = self.blocks.len();
        self.blocks.push(TerminalBlock {
            kind,
            rect: TerminalRect::new(x, y, width, 0),
            identity,
            children: Arc::new([]),
        });
        Ok(index)
    }

    fn finish_block(
        &mut self,
        index: usize,
        _start_row: usize,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let block = &mut self.blocks[index];
        let height = self
            .rows
            .len()
            .checked_sub(usize::from(block.rect.y))
            .ok_or(TerminalProjectionError::TableLayoutFailure)?;
        block.rect = TerminalRect::new(
            block.rect.x,
            block.rect.y,
            width,
            checked_extent("block height", height)?,
        );
        Ok(())
    }

    fn push_blank_rows(&mut self, count: u16) {
        for _ in 0..count {
            self.rows.push(TerminalRow {
                block_index: usize::MAX,
                spans: Arc::new([]),
                width: 0,
                fits: true,
            });
        }
    }

    fn add_run(
        &mut self,
        text: impl Into<Arc<str>>,
        provenance: TextProvenance,
        identity: TerminalSemanticIdentity,
        style: crate::StyleRef,
    ) -> usize {
        let text = text.into();
        let key = Arc::as_ptr(&text) as *const () as usize;
        if let Some(index) = self.run_lookup.get(&key).copied()
            && self.runs[index].text == text
            && self.runs[index].provenance == provenance
            && self.runs[index].identity == identity
        {
            return index;
        }
        let index = self.runs.len();
        self.runs.push(TerminalRun {
            text,
            provenance,
            identity,
            style,
        });
        self.run_lookup.insert(key, index);
        index
    }

    fn add_text_run(&mut self, run: &TextRun, context: &SemanticContext, role: TextRole) -> Piece {
        let identity = terminal_identity(context, role, None, run.annotations());
        let index = self.add_run(
            Arc::from(run.text()),
            run.provenance().clone(),
            identity,
            run.style().cloned().unwrap_or_else(text_style_ref),
        );
        Piece {
            run_index: index,
            range: 0..run.text().len(),
        }
    }

    fn add_synthetic_piece(
        &mut self,
        text: &str,
        context: &SemanticContext,
        role: TextRole,
        part: Option<TextPart>,
        annotations: &Annotations,
    ) -> Piece {
        let identity = terminal_identity(context, role, part, annotations);
        let index = self.add_run(
            Arc::from(text),
            TextProvenance::Synthetic,
            identity,
            text_style_ref(),
        );
        Piece {
            run_index: index,
            range: 0..text.len(),
        }
    }

    fn raw_pieces(&mut self, raw: &RawText, context: &SemanticContext) -> Vec<Piece> {
        let identity =
            terminal_identity(context, TextRole::Paragraph, None, &Annotations::default());
        let index = self.add_run(
            Arc::from(raw.text()),
            TextProvenance::Synthetic,
            identity,
            text_style_ref(),
        );
        vec![Piece {
            run_index: index,
            range: 0..raw.text().len(),
        }]
    }

    fn render_block_into(
        &mut self,
        index: usize,
        block: &Block,
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let context = context.for_node(block.annotations());
        match block.kind() {
            BlockKind::Paragraph(content) => {
                let pieces = self.inline_pieces(content, &context, TextRole::Paragraph);
                let mode = if is_pipe_source_paragraph(content) {
                    WrapMode::NoWrap
                } else {
                    self.policy.text_wrap()
                };
                self.render_text_rows(index, pieces, x, width, mode, HorizontalAlign::Start)?;
            }
            BlockKind::Heading { level, content } => {
                let heading_context = context.with_heading_level(*level);
                let pieces = self.inline_pieces(content, &heading_context, TextRole::Heading);
                self.render_text_rows(
                    index,
                    pieces,
                    x,
                    width,
                    self.policy.text_wrap(),
                    HorizontalAlign::Start,
                )?;
                self.blocks[index].identity = terminal_identity(
                    &heading_context,
                    TextRole::Heading,
                    None,
                    block.annotations(),
                );
                self.blocks[index].kind = TerminalBlockKind::Heading(*level);
            }
            BlockKind::BlockQuote { blocks } => {
                self.render_quote(index, block, blocks, &context, x, width)?
            }
            BlockKind::List(list) => self.render_list(index, block, list, &context, x, width)?,
            BlockKind::CodeBlock(code) => {
                self.render_code(index, block, code, &context, x, width)?
            }
            BlockKind::Table(table) => {
                self.render_table(index, block, table, &context, x, width)?
            }
            BlockKind::ThematicBreak => {
                let piece = self.add_synthetic_piece(
                    "───",
                    &context,
                    TextRole::ThematicBreak,
                    Some(TextPart::ThematicRule),
                    block.annotations(),
                );
                self.render_text_rows(
                    index,
                    vec![piece],
                    x,
                    width,
                    WrapMode::NoWrap,
                    HorizontalAlign::Start,
                )?;
            }
            BlockKind::RawBlock { format, body } => {
                let child = context.with_role(TextRole::RawBlock).with_format(format);
                let pieces = self.literal_pieces(
                    body,
                    &child,
                    TextRole::RawBlock,
                    None,
                    block.annotations(),
                );
                self.render_text_rows(
                    index,
                    pieces,
                    x,
                    width,
                    WrapMode::NoWrap,
                    HorizontalAlign::Start,
                )?;
            }
            BlockKind::Container { blocks } => {
                self.render_block_children(
                    index,
                    blocks,
                    &context.with_role(TextRole::Container),
                    x,
                    width,
                )?;
            }
        }
        Ok(())
    }

    fn render_block_children(
        &mut self,
        parent: usize,
        blocks: &[Block],
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        self.render_block_children_with_gap(
            parent,
            blocks,
            context,
            x,
            width,
            self.policy.block_gap(),
        )
    }

    fn render_block_children_with_gap(
        &mut self,
        parent: usize,
        blocks: &[Block],
        context: &SemanticContext,
        x: u16,
        width: u16,
        gap: u16,
    ) -> Result<(), TerminalProjectionError> {
        let mut children = Vec::with_capacity(blocks.len());
        for (position, child) in blocks.iter().enumerate() {
            if position > 0 {
                self.push_blank_rows(gap);
            }
            let child_index = self.begin_block(
                block_kind(child),
                terminal_identity(
                    &context.for_node(child.annotations()),
                    block_role_for_kind(child.kind()),
                    None,
                    child.annotations(),
                ),
                x,
                width,
            )?;
            self.render_block_into(child_index, child, context, x, width)?;
            self.finish_block(child_index, 0, width)?;
            children.push(child_index);
        }
        self.blocks[parent].children = children.into();
        Ok(())
    }

    fn render_quote(
        &mut self,
        index: usize,
        block: &Block,
        blocks: &[Block],
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let marker = self.add_synthetic_piece(
            "> ",
            context,
            TextRole::BlockQuote,
            Some(TextPart::QuoteMarker),
            block.annotations(),
        );
        let body_x = x.saturating_add(2).min(self.width);
        let body_width = width.saturating_sub(2);
        let start_row = self.rows.len();
        self.render_block_children(
            index,
            blocks,
            &context.with_role(TextRole::BlockQuote),
            body_x,
            body_width,
        )?;
        for row in &mut self.rows[start_row..] {
            let mut spans = Vec::with_capacity(row.spans.len() + 1);
            spans.push(span_for_piece(&marker, 0, marker.range.end, x, &self.runs)?);
            spans.extend(row.spans.iter().cloned());
            row.spans = spans.into();
            row.width = row.width.max(x.saturating_add(2));
            row.block_index = index;
        }
        Ok(())
    }

    fn render_list(
        &mut self,
        index: usize,
        block: &Block,
        list: &List,
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let kind = match list.marker() {
            ListMarker::Bullet => TextListKind::Bullet,
            ListMarker::Ordered { .. } => TextListKind::Ordered,
        };
        let list_context = context.with_role(TextRole::List).with_list_kind(kind);
        let mut children = Vec::with_capacity(list.items().len());
        for (item_index, item) in list.items().iter().enumerate() {
            if item_index > 0 && !list.tight() {
                self.push_blank_rows(self.policy.block_gap());
            }
            let item_context = list_context.for_node(item.annotations());
            let task_state = item.checked().map(|checked| {
                if checked {
                    TextTaskState::Checked
                } else {
                    TextTaskState::Unchecked
                }
            });
            let item_context = item_context.with_task_state(task_state);
            let marker_pieces =
                list_item_marker_pieces(self, &item_context, list.marker(), item_index, item);
            let marker_width = marker_pieces
                .iter()
                .map(|piece| {
                    checked_extent(
                        "list marker",
                        text_cell_extent(&self.runs[piece.run_index].text[piece.range.clone()]),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(usize::from)
                .sum::<usize>();
            let marker_width = checked_extent("list marker", marker_width)?;
            let item_x =
                x.checked_add(marker_width)
                    .ok_or(TerminalProjectionError::ExtentOverflow {
                        axis: "list body x",
                        value: usize::from(x).saturating_add(usize::from(marker_width)),
                    })?;
            let item_width = width.saturating_sub(marker_width);
            let item_index_in_product = self.begin_block(
                TerminalBlockKind::ListItem,
                terminal_identity(&item_context, TextRole::ListItem, None, item.annotations()),
                x,
                width,
            )?;
            let start_row = self.rows.len();
            self.render_block_children_with_gap(
                item_index_in_product,
                item.blocks(),
                &item_context.with_role(TextRole::ListItem),
                item_x,
                item_width,
                if list.tight() || item.blocks().len() <= 1 {
                    0
                } else {
                    self.policy.block_gap()
                },
            )?;
            if self.rows.len() == start_row {
                self.rows.push(empty_row(item_index_in_product));
            }
            if let Some(row) = self.rows.get_mut(start_row) {
                let mut spans = Vec::with_capacity(row.spans.len() + marker_pieces.len());
                let mut marker_x = x;
                for piece in &marker_pieces {
                    let marker_text = &self.runs[piece.run_index].text[piece.range.clone()];
                    let piece_width = checked_extent("list marker", text_cell_extent(marker_text))?;
                    spans.push(span_for_piece(
                        piece,
                        0,
                        piece.range.len(),
                        marker_x,
                        &self.runs,
                    )?);
                    marker_x = marker_x.saturating_add(piece_width);
                }
                spans.extend(row.spans.iter().cloned());
                row.spans = spans.into();
                row.width = row.width.max(x.saturating_add(marker_width));
                row.block_index = item_index_in_product;
            }
            self.finish_block(item_index_in_product, start_row, width)?;
            children.push(item_index_in_product);
        }
        self.blocks[index].children = children.into();
        if list.items().is_empty() {
            self.rows.push(empty_row(index));
        }
        let _ = block;
        Ok(())
    }

    fn render_code(
        &mut self,
        index: usize,
        block: &Block,
        code: &CodeBlock,
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let code_context = context
            .with_role(TextRole::CodeBlock)
            .with_language(code.language());
        let start = self.rows.len();
        if let Some(label) = code_label_text(code, self.policy.code_block_label()) {
            let piece = self.add_synthetic_piece(
                &label,
                &code_context,
                TextRole::CodeBlock,
                Some(TextPart::CodeLabel),
                block.annotations(),
            );
            self.render_text_rows(
                index,
                vec![piece],
                x,
                width,
                WrapMode::NoWrap,
                HorizontalAlign::Start,
            )?;
            self.push_blank_rows(self.policy.code_block_gap());
        }
        let pieces = self.literal_pieces(
            code.body(),
            &code_context,
            TextRole::CodeBlock,
            code.language(),
            block.annotations(),
        );
        self.render_text_rows(
            index,
            pieces,
            x,
            width,
            self.policy.code_wrap(),
            HorizontalAlign::Start,
        )?;
        if self.rows.len() == start {
            self.rows.push(empty_row(index));
        }
        Ok(())
    }

    fn render_table(
        &mut self,
        index: usize,
        block: &Block,
        table: &Table,
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        let table_context = context.with_role(TextRole::Table);
        let columns = table.columns().len();
        if columns == 0 {
            self.rows.push(empty_row(index));
            return Ok(());
        }
        let starts = table.cell_start_columns();
        let mut children = Vec::new();
        if let Some(caption) = table.caption() {
            let caption_start = self.rows.len();
            self.render_block_children(index, caption, &table_context, x, width)?;
            children.extend(self.blocks[index].children.iter().copied());
            if self.rows.len() > caption_start {
                self.push_blank_rows(self.policy.block_gap());
            }
        }
        let table_start = self.rows.len();
        let mut cells = Vec::new();
        for (row_index, row) in table.rows().iter().enumerate() {
            let section = if row_index < table.header_rows() {
                TextTableSection::Header
            } else {
                TextTableSection::Body
            };
            let row_context = table_context
                .for_node(row.annotations())
                .with_table_section(section);
            for (cell_index, cell) in row.cells().iter().enumerate() {
                let logical_column = starts[row_index][cell_index];
                cells.push(TableCellInput {
                    row_index,
                    logical_column,
                    cell,
                    context: row_context
                        .for_node(cell.annotations())
                        .with_role(TextRole::TableRow)
                        .with_role(TextRole::TableCell),
                });
            }
        }
        let grid = layout_table_grid(table, &cells, &self.policy, width)?;
        let row_blocks = table
            .rows()
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let row_y = table_start.checked_add(grid.row_edges[row_index]).ok_or(
                    TerminalProjectionError::ExtentOverflow {
                        axis: "table row y",
                        value: table_start.saturating_add(grid.row_edges[row_index]),
                    },
                )?;
                let row_end = grid.row_edges[row_index + 1];
                let row_height = row_end
                    .checked_sub(grid.row_edges[row_index])
                    .ok_or(TerminalProjectionError::TableLayoutFailure)?;
                let row_context = table_context
                    .for_node(row.annotations())
                    .with_table_section(if row_index < table.header_rows() {
                        TextTableSection::Header
                    } else {
                        TextTableSection::Body
                    });
                let row_index_in_product = self.blocks.len();
                self.blocks.push(TerminalBlock {
                    kind: TerminalBlockKind::TableRow,
                    rect: TerminalRect::new(
                        x,
                        checked_extent("table row y", row_y)?,
                        width,
                        checked_extent("table row height", row_height)?,
                    ),
                    identity: terminal_identity(
                        &row_context,
                        TextRole::TableRow,
                        None,
                        row.annotations(),
                    ),
                    children: Arc::new([]),
                });
                Ok(row_index_in_product)
            })
            .collect::<Result<Vec<_>, TerminalProjectionError>>()?;
        self.rows.resize(
            table_start.checked_add(grid.height).ok_or(
                TerminalProjectionError::ExtentOverflow {
                    axis: "table rows",
                    value: table_start.saturating_add(grid.height),
                },
            )?,
            empty_row(index),
        );
        let mut row_children = vec![Vec::new(); table.rows().len()];
        for (cell, geometry) in cells.iter().zip(grid.cells) {
            let cell_product = build_cell_product(&self.policy, table, cell, geometry.width)?;
            let cell_index = self.append_cell_product(
                cell_product,
                table_start,
                geometry,
                row_blocks[cell.row_index],
            )?;
            row_children[cell.row_index].push(cell_index);
        }
        for (row_index, row_block) in row_blocks.iter().copied().enumerate() {
            self.blocks[row_block].children = row_children[row_index].clone().into();
        }
        children.extend(row_blocks);
        self.blocks[index].children = children.into();
        let _ = block;
        Ok(())
    }

    fn append_cell_product(
        &mut self,
        local: ProductBuilder<'_>,
        table_start: usize,
        geometry: TableCellGeometry,
        row_block: usize,
    ) -> Result<usize, TerminalProjectionError> {
        let run_map = local
            .runs
            .iter()
            .map(|run| {
                self.add_run(
                    Arc::clone(&run.text),
                    run.provenance.clone(),
                    run.identity.clone(),
                    run.style.clone(),
                )
            })
            .collect::<Vec<_>>();
        let block_offset = self.blocks.len();
        for mut child in local.blocks {
            child.rect = TerminalRect::new(
                checked_extent(
                    "table cell x",
                    usize::from(geometry.x).saturating_add(usize::from(child.rect.x)),
                )?,
                checked_extent(
                    "table cell y",
                    table_start
                        .saturating_add(usize::from(geometry.y))
                        .saturating_add(usize::from(child.rect.y)),
                )?,
                child.rect.width,
                child.rect.height,
            );
            child.children = child
                .children
                .iter()
                .copied()
                .map(|index| {
                    if index == usize::MAX {
                        Ok(index)
                    } else {
                        index.checked_add(block_offset).ok_or(
                            TerminalProjectionError::ExtentOverflow {
                                axis: "table block index",
                                value: index.saturating_add(block_offset),
                            },
                        )
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
                .into();
            self.blocks.push(child);
        }
        let root = block_offset;
        self.blocks[root].rect = TerminalRect::new(
            geometry.x,
            checked_extent(
                "table cell y",
                table_start.saturating_add(usize::from(geometry.y)),
            )?,
            geometry.width,
            geometry.height,
        );
        let base_row = table_start.saturating_add(usize::from(geometry.y));
        for (local_row_index, local_row) in local.rows.iter().enumerate() {
            let row_index = base_row.checked_add(local_row_index).ok_or(
                TerminalProjectionError::ExtentOverflow {
                    axis: "table cell row",
                    value: base_row.saturating_add(local_row_index),
                },
            )?;
            let row = self
                .rows
                .get_mut(row_index)
                .ok_or(TerminalProjectionError::TableLayoutFailure)?;
            let mut spans = Vec::with_capacity(local_row.spans.len());
            for span in local_row.spans.iter() {
                let run_index = *run_map.get(span.run_index).ok_or(
                    TerminalProjectionError::RunIndexOverflow {
                        value: span.run_index,
                    },
                )?;
                let span_x = usize::from(geometry.x)
                    .checked_add(usize::from(span.x))
                    .ok_or(TerminalProjectionError::ExtentOverflow {
                        axis: "table span x",
                        value: usize::from(geometry.x).saturating_add(usize::from(span.x)),
                    })?;
                spans.push(TerminalPaintSpan {
                    run_index,
                    byte_range: span.byte_range.clone(),
                    source: span.source.clone(),
                    x: checked_extent("table span x", span_x)?,
                    cell_width: span.cell_width,
                });
            }
            row.spans = row
                .spans
                .iter()
                .cloned()
                .chain(spans)
                .collect::<Vec<_>>()
                .into();
            row.width = row.width.max(checked_extent(
                "table row width",
                usize::from(geometry.x).saturating_add(usize::from(local_row.width)),
            )?);
            row.fits &= local_row.fits;
            if row.block_index == usize::MAX {
                row.block_index = row_block;
            }
        }
        Ok(root)
    }

    fn render_cell(
        &mut self,
        cell_index: usize,
        table: &Table,
        cell: &TableCell,
        logical_column: usize,
        context: &SemanticContext,
        x: u16,
        width: u16,
    ) -> Result<(), TerminalProjectionError> {
        if cell.blocks().len() == 1
            && let BlockKind::Paragraph(content) = cell.blocks()[0].kind()
        {
            let alignment = cell
                .alignment()
                .unwrap_or_else(|| table.columns()[logical_column].alignment());
            let pieces = self.inline_pieces(content, context, TextRole::Paragraph);
            self.render_text_rows(
                cell_index,
                pieces,
                x,
                width,
                self.policy.text_wrap(),
                to_horizontal_align(alignment),
            )?;
        } else {
            self.render_block_children(cell_index, cell.blocks(), context, x, width)?;
        }
        Ok(())
    }

    fn inline_pieces(
        &mut self,
        content: &InlineContent,
        context: &SemanticContext,
        role: TextRole,
    ) -> Vec<Piece> {
        let mut pieces = Vec::new();
        for inline in content.iter() {
            self.push_inline(&mut pieces, inline, context, role);
        }
        pieces
    }

    fn literal_pieces(
        &mut self,
        literal: &LiteralText,
        context: &SemanticContext,
        role: TextRole,
        language: Option<&LanguageId>,
        annotations: &Annotations,
    ) -> Vec<Piece> {
        let context = context.with_language(language);
        literal
            .runs()
            .iter()
            .map(|run| {
                let identity = terminal_identity(&context, role, None, annotations);
                let index = self.add_run(
                    Arc::from(run.text()),
                    run.provenance().clone(),
                    identity,
                    run.style().cloned().unwrap_or_else(text_style_ref),
                );
                Piece {
                    run_index: index,
                    range: 0..run.text().len(),
                }
            })
            .collect()
    }

    fn push_inline(
        &mut self,
        pieces: &mut Vec<Piece>,
        inline: &Inline,
        context: &SemanticContext,
        role: TextRole,
    ) {
        let mut inline_context = context.clone();
        if let Some(origin) = inline.origin() {
            inline_context.origin = Some(origin);
        }
        let inline_role = role;
        for mark in inline.marks().marks() {
            let mut roles = inline_context.ancestor_roles.to_vec();
            roles.push(mark_role(mark));
            inline_context.ancestor_roles = roles.into();
        }
        match inline.kind() {
            InlineKind::Text(run) => {
                pieces.push(self.add_text_run(run, &inline_context, inline_role))
            }
            InlineKind::Break(BreakKind::Soft) => {
                let text = match self.policy.soft_break() {
                    super::SoftBreakPolicy::Space => " ",
                    super::SoftBreakPolicy::LineBreak => "\n",
                };
                pieces.push(self.add_synthetic_piece(
                    text,
                    &inline_context,
                    inline_role,
                    None,
                    inline.annotations(),
                ));
            }
            InlineKind::Break(BreakKind::Hard) => pieces.push(self.add_synthetic_piece(
                "\n",
                &inline_context,
                inline_role,
                None,
                inline.annotations(),
            )),
            InlineKind::Image(image) => {
                let image_context = inline_context.clone();
                for child in image.alt().iter() {
                    self.push_inline_with_role(
                        pieces,
                        child,
                        &image_context,
                        TextRole::Image,
                        Some(TextPart::ImageFallback),
                    );
                }
            }
            InlineKind::RawInline { format, body } => {
                let raw_context = inline_context
                    .with_role(TextRole::RawInline)
                    .with_format(format);
                pieces.extend(self.literal_pieces(
                    body,
                    &raw_context,
                    TextRole::RawInline,
                    None,
                    inline.annotations(),
                ));
            }
        }
    }

    fn push_inline_with_role(
        &mut self,
        pieces: &mut Vec<Piece>,
        inline: &Inline,
        context: &SemanticContext,
        role: TextRole,
        part: Option<TextPart>,
    ) {
        match inline.kind() {
            InlineKind::Text(run) => {
                let identity = terminal_identity(context, role, part, run.annotations());
                let index = self.add_run(
                    Arc::from(run.text()),
                    run.provenance().clone(),
                    identity,
                    run.style().cloned().unwrap_or_else(text_style_ref),
                );
                pieces.push(Piece {
                    run_index: index,
                    range: 0..run.text().len(),
                });
            }
            _ => self.push_inline(pieces, inline, context, role),
        }
    }

    fn render_text_rows(
        &mut self,
        block_index: usize,
        pieces: Vec<Piece>,
        x: u16,
        width: u16,
        mode: WrapMode,
        align: HorizontalAlign,
    ) -> Result<(), TerminalProjectionError> {
        let hard_lines = self.tokenize_hard_lines(&pieces)?;
        let rows = {
            let runs = &self.runs;
            wrap_token_lines(&hard_lines, width, mode, |token| {
                &runs[token.run_index].text[token.range.clone()]
            })
        };
        for tokens in &rows {
            self.push_token_row(block_index, tokens, x, width, align)?;
        }
        if rows.is_empty() {
            self.rows.push(empty_row(block_index));
        }
        Ok(())
    }

    fn tokenize_hard_lines(
        &mut self,
        pieces: &[Piece],
    ) -> Result<Vec<Vec<Token>>, TerminalProjectionError> {
        let mut lines: Vec<Vec<Piece>> = vec![Vec::new()];
        for piece in pieces {
            let text = &self.runs[piece.run_index].text[piece.range.clone()];
            let mut cursor = 0;
            for (part_index, part) in text.split('\n').enumerate() {
                if part_index > 0 {
                    lines.push(Vec::new());
                    cursor += 1;
                }
                if !part.is_empty() {
                    lines.last_mut().expect("hard line exists").push(Piece {
                        run_index: piece.run_index,
                        range: (piece.range.start + cursor)
                            ..(piece.range.start + cursor + part.len()),
                    });
                }
                cursor += part.len();
            }
        }
        lines.iter().map(|line| self.tokenize_line(line)).collect()
    }

    fn tokenize_line(&mut self, pieces: &[Piece]) -> Result<Vec<Token>, TerminalProjectionError> {
        if pieces.is_empty() {
            return Ok(Vec::new());
        }
        let mut full = String::new();
        let mut ranges = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let text = &self.runs[piece.run_index].text[piece.range.clone()];
            let start = full.len();
            full.push_str(text);
            ranges.push((start..full.len(), piece));
        }
        let mut output = Vec::new();
        for (start, grapheme) in full.grapheme_indices(true) {
            let end = start + grapheme.len();
            let start_piece = ranges
                .iter()
                .find(|(range, _)| range.contains(&start))
                .expect("grapheme starts in a text piece");
            let end_piece = ranges
                .iter()
                .find(|(range, _)| range.contains(&(end - 1)))
                .expect("grapheme ends in a text piece");
            let (run_index, range) = if std::ptr::eq(start_piece.1, end_piece.1) {
                let piece = start_piece.1;
                (
                    piece.run_index,
                    (piece.range.start + start - start_piece.0.start)
                        ..(piece.range.start + end - start_piece.0.start),
                )
            } else {
                let identity = self.runs[start_piece.1.run_index].identity.clone();
                let provenance = merged_provenance(
                    &self.runs[start_piece.1.run_index].provenance,
                    &self.runs[end_piece.1.run_index].provenance,
                );
                let run_index = self.add_run(
                    Arc::from(grapheme),
                    provenance,
                    identity,
                    self.runs[start_piece.1.run_index].style.clone(),
                );
                (run_index, 0..grapheme.len())
            };
            let width = checked_extent("grapheme", grapheme_cell_width(grapheme))?;
            let source = token_source(&self.runs[run_index].provenance, range.clone());
            output.push(Token {
                run_index,
                range,
                source,
                width,
            });
        }
        Ok(output)
    }

    fn push_token_row(
        &mut self,
        block_index: usize,
        tokens: &[Token],
        x: u16,
        width: u16,
        align: HorizontalAlign,
    ) -> Result<(), TerminalProjectionError> {
        let line_width: usize = tokens.iter().map(|token| usize::from(token.width)).sum();
        let available = usize::from(width);
        let offset = match align {
            HorizontalAlign::Start => 0,
            HorizontalAlign::Center => available.saturating_sub(line_width) / 2,
            HorizontalAlign::End => available.saturating_sub(line_width),
        };
        let mut cursor = usize::from(x).saturating_add(offset);
        let mut spans = Vec::with_capacity(tokens.len());
        for token in tokens {
            let span = TerminalPaintSpan {
                run_index: token.run_index,
                byte_range: token.range.clone(),
                source: token.source.clone(),
                x: checked_extent("span x", cursor)?,
                cell_width: token.width,
            };
            spans.push(span);
            cursor = cursor.saturating_add(usize::from(token.width));
        }
        self.rows.push(TerminalRow {
            block_index,
            spans: spans.into(),
            width: checked_extent("row width", cursor.saturating_sub(usize::from(x)))?,
            fits: line_width <= available,
        });
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Piece {
    run_index: usize,
    range: Range<usize>,
}

#[derive(Clone, Debug)]
struct Token {
    run_index: usize,
    range: Range<usize>,
    source: TextProvenance,
    width: u16,
}

impl WrapToken for Token {
    fn wrap_width(&self) -> usize {
        usize::from(self.width)
    }
}

fn empty_row(block_index: usize) -> TerminalRow {
    TerminalRow {
        block_index,
        spans: Arc::new([]),
        width: 0,
        fits: true,
    }
}

fn checked_extent(axis: &'static str, value: usize) -> Result<u16, TerminalProjectionError> {
    u16::try_from(value).map_err(|_| TerminalProjectionError::ExtentOverflow { axis, value })
}

fn checked_rows(value: usize) -> Result<u16, TerminalProjectionError> {
    u16::try_from(value).map_err(|_| TerminalProjectionError::RowCountOverflow { value })
}

fn token_source(provenance: &TextProvenance, range: Range<usize>) -> TextProvenance {
    match provenance {
        TextProvenance::Exact(source) => TextProvenance::Exact(StreamRange::new(
            StreamOffset::new(source.start().as_u64().saturating_add(range.start as u64)),
            StreamOffset::new(source.start().as_u64().saturating_add(range.end as u64)),
        )),
        TextProvenance::Derived(source) => TextProvenance::Derived(*source),
        TextProvenance::Synthetic => TextProvenance::Synthetic,
    }
}

fn merged_provenance(left: &TextProvenance, right: &TextProvenance) -> TextProvenance {
    match (left, right) {
        (TextProvenance::Exact(left), TextProvenance::Exact(right))
            if left.end() == right.start() =>
        {
            TextProvenance::Exact(StreamRange::new(left.start(), right.end()))
        }
        (TextProvenance::Derived(left), _) => TextProvenance::Derived(*left),
        (_, TextProvenance::Derived(right)) => TextProvenance::Derived(*right),
        _ => TextProvenance::Synthetic,
    }
}

fn block_kind(block: &Block) -> TerminalBlockKind {
    match block.kind() {
        BlockKind::Paragraph(_) => TerminalBlockKind::Paragraph,
        BlockKind::Heading { level, .. } => TerminalBlockKind::Heading(*level),
        BlockKind::BlockQuote { .. } => TerminalBlockKind::BlockQuote,
        BlockKind::List(_) => TerminalBlockKind::List,
        BlockKind::CodeBlock(_) => TerminalBlockKind::CodeBlock,
        BlockKind::Table(_) => TerminalBlockKind::Table,
        BlockKind::ThematicBreak => TerminalBlockKind::ThematicBreak,
        BlockKind::RawBlock { .. } => TerminalBlockKind::RawBlock,
        BlockKind::Container { .. } => TerminalBlockKind::Container,
    }
}

fn block_role(content: &TextContent) -> TextRole {
    match content {
        TextContent::Raw(_) => TextRole::Paragraph,
        TextContent::Block(block) => block_role_for_kind(block.kind()),
    }
}

fn block_role_for_kind(kind: &BlockKind) -> TextRole {
    match kind {
        BlockKind::Paragraph(_) => TextRole::Paragraph,
        BlockKind::Heading { .. } => TextRole::Heading,
        BlockKind::BlockQuote { .. } => TextRole::BlockQuote,
        BlockKind::List(_) => TextRole::List,
        BlockKind::CodeBlock(_) => TextRole::CodeBlock,
        BlockKind::Table(_) => TextRole::Table,
        BlockKind::ThematicBreak => TextRole::ThematicBreak,
        BlockKind::RawBlock { .. } => TextRole::RawBlock,
        BlockKind::Container { .. } => TextRole::Container,
    }
}

fn list_marker(content: &TextContent) -> Option<(ListMarker, bool)> {
    match content {
        TextContent::Block(block) => match block.kind() {
            BlockKind::List(list) => Some((list.marker(), list.tight())),
            _ => None,
        },
        TextContent::Raw(_) => None,
    }
}

fn block_gap_between(
    previous: Option<(ListMarker, bool)>,
    current: &TextContent,
    policy: &TextRenderPolicy,
) -> u16 {
    let Some((previous_marker, previous_tight)) = previous else {
        return policy.block_gap();
    };
    let Some((current_marker, current_tight)) = list_marker(current) else {
        return policy.block_gap();
    };
    if previous_tight && current_tight && same_list_kind(previous_marker, current_marker) {
        0
    } else {
        policy.block_gap()
    }
}

fn same_list_kind(left: ListMarker, right: ListMarker) -> bool {
    match (left, right) {
        (ListMarker::Bullet, ListMarker::Bullet) => true,
        (
            ListMarker::Ordered {
                style: left_style,
                delimiter: left_delimiter,
                ..
            },
            ListMarker::Ordered {
                style: right_style,
                delimiter: right_delimiter,
                ..
            },
        ) => left_style == right_style && left_delimiter == right_delimiter,
        _ => false,
    }
}

fn list_item_marker(
    policy: &TextRenderPolicy,
    marker: ListMarker,
    index: usize,
    item: &ListItem,
) -> String {
    let list_marker = list_marker_text(marker, index);
    let Some(checked) = item.checked() else {
        return list_marker;
    };
    let task_marker = if checked { "[x] " } else { "[ ] " };
    match policy.task_list_marker() {
        super::TaskListMarkerPolicy::TaskOnly => task_marker.to_owned(),
        super::TaskListMarkerPolicy::TaskAndList => format!("{task_marker}{list_marker}"),
    }
}

fn list_item_marker_pieces(
    builder: &mut ProductBuilder<'_>,
    context: &SemanticContext,
    marker: ListMarker,
    index: usize,
    item: &ListItem,
) -> Vec<Piece> {
    let mut pieces = Vec::with_capacity(2);
    if let Some(checked) = item.checked() {
        let task_marker = if checked { "[x] " } else { "[ ] " };
        pieces.push(builder.add_synthetic_piece(
            task_marker,
            context,
            TextRole::ListItem,
            Some(TextPart::TaskMarker),
            item.annotations(),
        ));
        if matches!(
            builder.policy.task_list_marker(),
            super::TaskListMarkerPolicy::TaskOnly
        ) {
            return pieces;
        }
    }
    pieces.push(builder.add_synthetic_piece(
        &list_marker_text(marker, index),
        context,
        TextRole::ListItem,
        Some(TextPart::ListMarker),
        item.annotations(),
    ));
    pieces
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

fn code_label_text(code: &CodeBlock, policy: super::CodeBlockLabelPolicy) -> Option<String> {
    match policy {
        super::CodeBlockLabelPolicy::Hidden => None,
        super::CodeBlockLabelPolicy::Language => {
            code.language().map(|language| language.as_str().to_owned())
        }
        super::CodeBlockLabelPolicy::Info => code
            .info()
            .filter(|info| !info.is_empty())
            .map(str::to_owned)
            .or_else(|| code.language().map(|language| language.as_str().to_owned())),
    }
}

fn text_cell_extent(text: &str) -> usize {
    text.graphemes(true).map(grapheme_cell_width).sum()
}

fn span_for_piece(
    piece: &Piece,
    start: usize,
    end: usize,
    x: u16,
    runs: &[TerminalRun],
) -> Result<TerminalPaintSpan, TerminalProjectionError> {
    let run = &runs[piece.run_index];
    let range = (piece.range.start + start)..(piece.range.start + end);
    Ok(TerminalPaintSpan {
        run_index: piece.run_index,
        byte_range: range.clone(),
        source: token_source(&run.provenance, range.clone()),
        x,
        cell_width: checked_extent("paint span", text_cell_extent(&run.text[range]))?,
    })
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

fn to_horizontal_align(alignment: Alignment) -> HorizontalAlign {
    match alignment {
        Alignment::Default | Alignment::Start => HorizontalAlign::Start,
        Alignment::Center => HorizontalAlign::Center,
        Alignment::End => HorizontalAlign::End,
    }
}

fn is_pipe_source_paragraph(content: &InlineContent) -> bool {
    let text = inline_plain_text(content, &TextRenderPolicy::default());
    let mut saw_pipe = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.trim_start().starts_with('|') {
            return false;
        }
        saw_pipe = true;
    }
    saw_pipe
}

fn inline_plain_text(content: &InlineContent, policy: &TextRenderPolicy) -> String {
    let mut text = String::new();
    for inline in content.iter() {
        match inline.kind() {
            InlineKind::Text(run) => text.push_str(run.text()),
            InlineKind::Break(BreakKind::Soft) => match policy.soft_break() {
                super::SoftBreakPolicy::Space => text.push(' '),
                super::SoftBreakPolicy::LineBreak => text.push('\n'),
            },
            InlineKind::Break(BreakKind::Hard) => text.push('\n'),
            InlineKind::Image(image) => text.push_str(&inline_plain_text(image.alt(), policy)),
            InlineKind::RawInline { body, .. } => text.push_str(&body.text()),
        }
    }
    text
}

fn intrinsic_text(text: &str, mode: WrapMode) -> IntrinsicMetrics {
    let lines: Vec<&str> = text.split('\n').collect();
    let max_width = lines
        .iter()
        .map(|line| line.graphemes(true).map(grapheme_cell_width).sum::<usize>())
        .max()
        .unwrap_or(0);
    let min_width = match mode {
        WrapMode::NoWrap => max_width,
        WrapMode::Grapheme => text
            .graphemes(true)
            .map(grapheme_cell_width)
            .max()
            .unwrap_or(0),
        WrapMode::WordThenGrapheme => lines
            .iter()
            .map(|line| word_min_width(line))
            .max()
            .unwrap_or(0),
    };
    IntrinsicMetrics {
        min_width,
        max_width,
    }
}

fn word_min_width(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let starts: Vec<usize> = graphemes
        .iter()
        .scan(0, |offset, grapheme| {
            let start = *offset;
            *offset += grapheme.len();
            Some(start)
        })
        .collect();
    let ends: Vec<usize> = graphemes
        .iter()
        .scan(0, |offset, grapheme| {
            *offset += grapheme.len();
            Some(*offset)
        })
        .collect();
    let mut chunk_start = 0;
    let mut largest = 0;
    for (offset, opportunity) in linebreaks(text) {
        if matches!(
            opportunity,
            BreakOpportunity::Allowed | BreakOpportunity::Mandatory
        ) && let Ok(index) = ends.binary_search(&offset)
        {
            let chunk = &text[starts[chunk_start]..ends[index]];
            let chunk_width = chunk
                .trim_matches(char::is_whitespace)
                .graphemes(true)
                .map(grapheme_cell_width)
                .sum::<usize>();
            largest = largest.max(chunk_width);
            chunk_start = index + 1;
        }
    }
    let trailing = starts.get(chunk_start).map_or(0, |start| {
        text[*start..]
            .trim_matches(char::is_whitespace)
            .graphemes(true)
            .map(grapheme_cell_width)
            .sum::<usize>()
    });
    largest.max(trailing)
}

fn measure_contents(contents: &[TextContent], policy: &TextRenderPolicy) -> IntrinsicMetrics {
    contents
        .iter()
        .fold(IntrinsicMetrics::default(), |metrics, content| {
            let child = match content {
                TextContent::Raw(raw) => intrinsic_text(raw.text(), policy.text_wrap()),
                TextContent::Block(block) => measure_block(block, policy),
            };
            IntrinsicMetrics {
                min_width: metrics.min_width.max(child.min_width),
                max_width: metrics.max_width.max(child.max_width),
            }
        })
}

fn measure_block(block: &Block, policy: &TextRenderPolicy) -> IntrinsicMetrics {
    match block.kind() {
        BlockKind::Paragraph(content) => {
            let mode = if is_pipe_source_paragraph(content) {
                WrapMode::NoWrap
            } else {
                policy.text_wrap()
            };
            intrinsic_text(&inline_plain_text(content, policy), mode)
        }
        BlockKind::Heading { content, .. } => {
            intrinsic_text(&inline_plain_text(content, policy), policy.text_wrap())
        }
        BlockKind::BlockQuote { blocks } | BlockKind::Container { blocks } => {
            let metrics = measure_block_slice(blocks, policy);
            if matches!(block.kind(), BlockKind::BlockQuote { .. }) {
                IntrinsicMetrics {
                    min_width: metrics.min_width.saturating_add(2),
                    max_width: metrics.max_width.saturating_add(2),
                }
            } else {
                metrics
            }
        }
        BlockKind::List(list) => list.items().iter().enumerate().fold(
            IntrinsicMetrics::default(),
            |metrics, (index, item)| {
                let body = measure_block_slice(item.blocks(), policy);
                let marker = list_item_marker(policy, list.marker(), index, item);
                let marker_width = text_cell_extent(&marker);
                IntrinsicMetrics {
                    min_width: metrics
                        .min_width
                        .max(body.min_width.saturating_add(marker_width)),
                    max_width: metrics
                        .max_width
                        .max(body.max_width.saturating_add(marker_width)),
                }
            },
        ),
        BlockKind::CodeBlock(code) => {
            let body = intrinsic_text(&code.body().text(), policy.code_wrap());
            let label = code_label_text(code, policy.code_block_label())
                .map(|label| text_cell_extent(&label))
                .unwrap_or(0);
            IntrinsicMetrics {
                min_width: body.min_width.max(label),
                max_width: body.max_width.max(label),
            }
        }
        BlockKind::Table(table) => measure_table(table, policy),
        BlockKind::ThematicBreak => IntrinsicMetrics {
            min_width: 3,
            max_width: 3,
        },
        BlockKind::RawBlock { body, .. } => intrinsic_text(&body.text(), WrapMode::NoWrap),
    }
}

fn measure_block_slice(blocks: &[Block], policy: &TextRenderPolicy) -> IntrinsicMetrics {
    blocks
        .iter()
        .fold(IntrinsicMetrics::default(), |metrics, block| {
            let child = measure_block(block, policy);
            IntrinsicMetrics {
                min_width: metrics.min_width.max(child.min_width),
                max_width: metrics.max_width.max(child.max_width),
            }
        })
}

fn measure_table(table: &Table, policy: &TextRenderPolicy) -> IntrinsicMetrics {
    let columns = table_column_metrics(table, policy);
    let gap = usize::from(policy.table_column_gap());
    let total_gap = gap.saturating_mul(columns.len().saturating_sub(1));
    let mut metrics = IntrinsicMetrics {
        min_width: columns
            .iter()
            .map(|column| column.min_width)
            .sum::<usize>()
            .saturating_add(total_gap),
        max_width: columns
            .iter()
            .map(|column| column.max_width)
            .sum::<usize>()
            .saturating_add(total_gap),
    };
    if let Some(caption) = table.caption() {
        let caption = measure_block_slice(caption, policy);
        metrics.min_width = metrics.min_width.max(caption.min_width);
        metrics.max_width = metrics.max_width.max(caption.max_width);
    }
    metrics
}

fn table_column_metrics(table: &Table, policy: &TextRenderPolicy) -> Vec<IntrinsicMetrics> {
    let mut columns = vec![IntrinsicMetrics::default(); table.columns().len()];
    let starts = table.cell_start_columns();
    for (row_index, row) in table.rows().iter().enumerate() {
        for (cell_index, cell) in row.cells().iter().enumerate() {
            let metrics = measure_block_slice(cell.blocks(), policy);
            let start = starts[row_index][cell_index];
            let span = usize::from(cell.col_span().get());
            if span == 1 {
                columns[start].min_width = columns[start].min_width.max(metrics.min_width);
                columns[start].max_width = columns[start].max_width.max(metrics.max_width);
            } else {
                let min_each = metrics.min_width.div_ceil(span);
                let max_each = metrics.max_width.div_ceil(span);
                for column in &mut columns[start..start + span] {
                    column.min_width = column.min_width.max(min_each);
                    column.max_width = column.max_width.max(max_each);
                }
            }
        }
    }
    columns
}

fn table_column_widths(
    table: &Table,
    policy: &TextRenderPolicy,
    width: u16,
) -> Result<Vec<u16>, TerminalProjectionError> {
    let preferred = table_column_metrics(table, policy)
        .into_iter()
        .map(|column| {
            u16::try_from(column.max_width).map_err(|_| TerminalProjectionError::ExtentOverflow {
                axis: "table column",
                value: column.max_width,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let gaps = usize::from(policy.table_column_gap())
        .checked_mul(preferred.len().saturating_sub(1))
        .ok_or(TerminalProjectionError::ExtentOverflow {
            axis: "table column gaps",
            value: usize::MAX,
        })?;
    let mut remaining = usize::from(width).saturating_sub(gaps);
    preferred
        .into_iter()
        .map(|preferred| {
            let allocated = usize::from(preferred).min(remaining);
            remaining = remaining.saturating_sub(allocated);
            checked_extent("table column", allocated)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        StyleRef, StyleSpec,
        content::text::{Inline, SemanticTag, TableColumn, TableRow},
        stream::StreamOffset,
    };

    fn row_text(product: &TerminalTextProduct, row: usize) -> String {
        product
            .paint_row_to_physical(&Theme::new(), PhysicalStyle::default(), row)
            .expect("row paints")
            .expect("row exists")
            .plain_text()
            .trim_end()
            .to_owned()
    }

    #[test]
    fn definite_width_retains_exact_wrapped_rows_and_geometry() {
        let projector = TerminalTextProjector::new(TextRenderPolicy::new());
        let product = projector
            .project(
                &TextContent::block(Block::paragraph("one two")),
                TerminalConstraints::definite(4),
            )
            .expect("projection");
        assert_eq!(product.size(), TerminalSize::new(4, 2));
        assert_eq!(product.wrap_width(), 4);
        assert_eq!(product.rows().len(), 2);
        assert_eq!(row_text(&product, 0), "one");
        assert_eq!(row_text(&product, 1), "two");
        assert!(product.rows().iter().all(TerminalRow::fits));
        assert_eq!(product.blocks()[0].kind(), TerminalBlockKind::Paragraph);
    }

    #[test]
    fn min_and_max_content_follow_wrap_policy_without_sentinels() {
        let projector = TerminalTextProjector::new(TextRenderPolicy::new());
        let content = TextContent::block(Block::paragraph("longword x"));
        let min = projector
            .project(&content, TerminalConstraints::min_content())
            .expect("min projection");
        let max = projector
            .project(&content, TerminalConstraints::max_content())
            .expect("max projection");
        assert_eq!(min.wrap_width(), 8);
        assert_eq!(min.rows().len(), 2);
        assert_eq!(max.wrap_width(), 10);
        assert_eq!(max.rows().len(), 1);
    }

    #[test]
    fn explicit_grapheme_and_no_wrap_policies_control_rows() {
        let grapheme =
            TerminalTextProjector::new(TextRenderPolicy::new().with_text_wrap(WrapMode::Grapheme))
                .project(
                    &TextContent::raw("abcdef"),
                    TerminalConstraints::definite(4),
                )
                .expect("grapheme projection");
        assert_eq!(
            (0..grapheme.rows().len())
                .map(|row| row_text(&grapheme, row))
                .collect::<Vec<_>>(),
            ["abcd", "ef"]
        );
        let wide_graphemes =
            TerminalTextProjector::new(TextRenderPolicy::new().with_text_wrap(WrapMode::Grapheme))
                .project(
                    &TextContent::raw("abcdef界ghi 👩‍💻 é\nsecond line"),
                    TerminalConstraints::definite(5),
                )
                .expect("wide grapheme projection");
        assert_eq!(
            (0..wide_graphemes.rows().len())
                .map(|row| row_text(&wide_graphemes, row))
                .collect::<Vec<_>>(),
            ["abcde", "f界gh", "i 👩‍💻", "é", "secon", "d lin", "e"]
        );
        let no_wrap =
            TerminalTextProjector::new(TextRenderPolicy::new().with_text_wrap(WrapMode::NoWrap))
                .project(
                    &TextContent::raw("abcdef"),
                    TerminalConstraints::definite(4),
                )
                .expect("no-wrap projection");
        assert_eq!(no_wrap.rows().len(), 1);
        assert!(!no_wrap.rows()[0].fits());
        let clipped_wide =
            TerminalTextProjector::new(TextRenderPolicy::new().with_text_wrap(WrapMode::NoWrap))
                .project(
                    &TextContent::raw("abcd界"),
                    TerminalConstraints::definite(5),
                )
                .expect("no-wrap wide projection");
        let clipped_row = clipped_wide
            .paint_row_to_physical(&Theme::new(), PhysicalStyle::default(), 0)
            .expect("no-wrap wide paint")
            .expect("no-wrap wide row");
        assert_eq!(clipped_row.cells()[4].grapheme, None);
        assert!(!clipped_row.cells()[4].continuation);
    }

    #[test]
    fn multi_character_markers_emit_one_grapheme_per_physical_cell() {
        let list = Block::list(List::ordered(10, [ListItem::paragraph("body")]));
        let product = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(&TextContent::block(list), TerminalConstraints::definite(8))
            .expect("ordered list projection");
        let row = product
            .paint_row_to_physical(&Theme::new(), PhysicalStyle::default(), 0)
            .expect("ordered list paint")
            .expect("ordered list row");
        let cells = row.cells();
        assert_eq!(cells[0].grapheme.as_deref(), Some("1"));
        assert_eq!(cells[1].grapheme.as_deref(), Some("0"));
        assert_eq!(cells[2].grapheme.as_deref(), Some("."));
        assert_eq!(cells[3].grapheme.as_deref(), Some(" "));
        assert_eq!(cells[4].grapheme.as_deref(), Some("b"));
        assert!(cells[..5].iter().all(|cell| !cell.continuation));

        let clipped = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(
                &TextContent::block(Block::list(List::ordered(
                    10,
                    [ListItem::task("body", false)],
                ))),
                TerminalConstraints::definite(3),
            )
            .expect("clipped task marker projection");
        let clipped_row = clipped
            .paint_row_to_physical(&Theme::new(), PhysicalStyle::default(), 0)
            .expect("clipped task marker paint")
            .expect("clipped task marker row");
        assert!(clipped_row.cells().iter().all(|cell| !cell.continuation));
    }

    #[test]
    fn zero_width_is_a_real_constraint_and_never_replaced_by_one() {
        let projector = TerminalTextProjector::new(TextRenderPolicy::new());
        let product = projector
            .project(&TextContent::raw("wide"), TerminalConstraints::definite(0))
            .expect("zero width remains valid");
        assert_eq!(product.size().width(), 0);
        assert_eq!(product.wrap_width(), 0);
        assert_eq!(product.rows().len(), 1);
        assert!(!product.physically_complete());
    }

    #[test]
    fn wrapped_fragments_keep_exact_source_ranges_and_owned_text() {
        let run = TextRun::exact(
            "héllo",
            StreamRange::new(StreamOffset::new(40), StreamOffset::new(46)),
        )
        .expect("exact run");
        let block = Block::paragraph(Inline::text(run));
        let product = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(&TextContent::block(block), TerminalConstraints::definite(2))
            .expect("projection");
        let spans: Vec<_> = product.rows().iter().flat_map(TerminalRow::spans).collect();
        assert_eq!(spans.len(), 5);
        assert_eq!(
            spans[0].source(),
            &TextProvenance::Exact(StreamRange::new(
                StreamOffset::new(40),
                StreamOffset::new(41)
            ))
        );
        assert_eq!(
            spans[1].source(),
            &TextProvenance::Exact(StreamRange::new(
                StreamOffset::new(41),
                StreamOffset::new(43)
            ))
        );
        assert_eq!(
            spans[2].source(),
            &TextProvenance::Exact(StreamRange::new(
                StreamOffset::new(43),
                StreamOffset::new(44)
            ))
        );
        assert_eq!(
            spans[3].source(),
            &TextProvenance::Exact(StreamRange::new(
                StreamOffset::new(44),
                StreamOffset::new(45)
            ))
        );
        assert_eq!(
            spans[4].source(),
            &TextProvenance::Exact(StreamRange::new(
                StreamOffset::new(45),
                StreamOffset::new(46)
            ))
        );
        for span in spans {
            assert!(span.byte_range().end <= product.runs()[span.run_index()].text().len());
        }
    }

    #[test]
    fn semantic_roles_annotations_and_theme_style_survive_direct_projection() {
        let annotation = SemanticTag::new("test", "important").expect("tag");
        let heading = Block::heading(
            HeadingLevel::H2,
            Inline::text(
                TextRun::synthetic("title")
                    .with_annotations(Annotations::new().with_tag(annotation.clone()))
                    .with_style(StyleRef::direct(StyleSpec::new().bold())),
            ),
        );
        let product = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(
                &TextContent::block(heading),
                TerminalConstraints::definite(10),
            )
            .expect("projection");
        let block = &product.blocks()[0];
        assert_eq!(block.identity().role(), TextRole::Heading);
        assert_eq!(
            block.identity().context().heading_level(),
            Some(HeadingLevel::H2)
        );
        let run = &product.runs()[0];
        assert_eq!(run.identity().role(), TextRole::Heading);
        assert!(run.identity().annotations().contains_tag(&annotation));
        let row = product
            .paint_row_to_physical(&Theme::new(), PhysicalStyle::default(), 0)
            .expect("paint")
            .expect("row");
        assert!(row.style_at(0).expect("cell").bold);
    }

    #[test]
    fn representative_structured_families_produce_semantic_boxes() {
        let list = Block::list(List::bulleted([
            ListItem::task("task", true),
            ListItem::paragraph("continuation"),
        ]));
        let quote = Block::block_quote([Block::paragraph("quoted")]);
        let code = Block::code(CodeBlock::new(
            Some(LanguageId::new("rust").expect("language")),
            None::<&str>,
            LiteralText::from("let x = 1;"),
        ));
        let table = Table::new(
            None::<Vec<Block>>,
            [TableColumn::start(), TableColumn::end()],
            1,
            [TableRow::new([
                TableCell::text("head"),
                TableCell::text("value"),
            ])],
        )
        .expect("table");
        let contents = [
            TextContent::block(quote),
            TextContent::block(list),
            TextContent::block(code),
            TextContent::block(Block::table(table)),
            TextContent::block(Block::thematic_break()),
        ];
        let product = TerminalTextProjector::new(
            TextRenderPolicy::new()
                .with_code_block_label(super::super::CodeBlockLabelPolicy::Language),
        )
        .project_contents(&contents, TerminalConstraints::definite(30))
        .expect("structured projection");
        let kinds: Vec<_> = product.blocks().iter().map(TerminalBlock::kind).collect();
        assert!(kinds.contains(&TerminalBlockKind::BlockQuote));
        assert!(kinds.contains(&TerminalBlockKind::List));
        assert!(kinds.contains(&TerminalBlockKind::ListItem));
        assert!(kinds.contains(&TerminalBlockKind::CodeBlock));
        assert!(kinds.contains(&TerminalBlockKind::Table));
        assert!(kinds.contains(&TerminalBlockKind::TableCell));
        assert!(kinds.contains(&TerminalBlockKind::ThematicBreak));
        assert!(product.rows().iter().any(|row| {
            row.spans()
                .iter()
                .any(|span| product.runs()[span.run_index()].text() == "> ")
        }));
    }

    #[test]
    fn signed_clip_origin_paints_without_relayout_or_panics() {
        let product = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(&TextContent::raw("wide"), TerminalConstraints::definite(4))
            .expect("projection");
        let mut surface = Surface::new(4, 1);
        product
            .paint_window(
                &Theme::new(),
                PhysicalStyle::default(),
                &mut surface,
                (-2, 0),
                Rect::new(0, 0, 4, 1),
                TerminalRowWindow::new(0, 1),
            )
            .expect("signed clipped paint");
        assert_eq!(surface.row_cells(0).len(), 4);
    }

    #[test]
    fn table_grid_places_cells_on_shared_rows_and_preserves_spans() {
        let table = Table::new(
            None::<Vec<Block>>,
            [TableColumn::start(), TableColumn::start()],
            1,
            [
                TableRow::new([TableCell::text("a\nb"), TableCell::text("x\ny")]),
                TableRow::new([TableCell::new(
                    [Block::paragraph("wide")],
                    None,
                    std::num::NonZeroU16::new(1).expect("span"),
                    std::num::NonZeroU16::new(2).expect("span"),
                )]),
            ],
        )
        .expect("valid table");
        let product = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(
                &TextContent::block(Block::table(table)),
                TerminalConstraints::definite(10),
            )
            .expect("table projection");
        let rows = (0..product.rows().len())
            .map(|row| row_text(&product, row))
            .collect::<Vec<_>>();
        assert_eq!(rows, ["a     x", "b     y", "wide"]);
        let table_cell_blocks = product
            .blocks()
            .iter()
            .filter(|block| block.kind() == TerminalBlockKind::TableCell)
            .collect::<Vec<_>>();
        assert_eq!(table_cell_blocks.len(), 3);
        assert_eq!(table_cell_blocks[2].rect().width(), 10);
        assert!(
            product
                .paint_row_to_physical(&Theme::new(), PhysicalStyle::default(), 0)
                .expect("header row paint")
                .expect("header row")
                .style_at(0)
                .expect("header style")
                .bold
        );
    }

    #[test]
    fn table_grid_handles_rows_with_no_starting_cells() {
        let table = Table::new(
            None::<Vec<Block>>,
            [TableColumn::start(), TableColumn::start()],
            0,
            [
                TableRow::new([TableCell::new(
                    [Block::paragraph("a")],
                    None,
                    std::num::NonZeroU16::new(2).expect("span"),
                    std::num::NonZeroU16::new(2).expect("span"),
                )]),
                TableRow::new([]),
            ],
        )
        .expect("valid table with a covered row");
        let product = TerminalTextProjector::new(TextRenderPolicy::new())
            .project(
                &TextContent::block(Block::table(table)),
                TerminalConstraints::definite(10),
            )
            .expect("covered-row table projection");
        assert_eq!(product.rows().len(), 1);
        assert_eq!(product.blocks().len(), 4);
        assert_eq!(product.blocks()[3].rect().height(), 1);
    }
}
