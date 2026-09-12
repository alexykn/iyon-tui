use crate::WrapMode;

/// How a soft line break is presented as ordinary semantic text.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SoftBreakPolicy {
    #[default]
    Space,
    LineBreak,
}

/// Shared-column sizing for generic tables.
///
/// Semantic [`super::super::TableColumn`] currently carries alignment only, so
/// the renderer maps every column through one structural policy.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableColumnSizing {
    Content,

    #[default]
    Flex,
}

/// How task-list items present checkbox chrome relative to the list marker.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskListMarkerPolicy {
    TaskOnly,

    #[default]
    TaskAndList,
}

/// Optional code-block label presentation.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodeBlockLabelPolicy {
    #[default]
    Hidden,

    Language,
    Info,
}

/// Structural-only policy for generic text-to-View lowering.
///
/// Semantic paint belongs to Theme. This type only controls document
/// structure such as gaps, wrapping, and generated chrome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRenderPolicy {
    block_gap: u16,
    soft_break: SoftBreakPolicy,
    table_column_gap: u16,
    table_row_gap: u16,
    table_column_sizing: TableColumnSizing,
    task_list_marker: TaskListMarkerPolicy,
    code_block_label: CodeBlockLabelPolicy,
    code_block_gap: u16,
    code_wrap: WrapMode,
    text_wrap: WrapMode,
}

impl Default for TextRenderPolicy {
    fn default() -> Self {
        Self {
            block_gap: 1,
            soft_break: SoftBreakPolicy::default(),
            table_column_gap: 1,
            table_row_gap: 0,
            table_column_sizing: TableColumnSizing::default(),
            task_list_marker: TaskListMarkerPolicy::default(),
            code_block_label: CodeBlockLabelPolicy::default(),
            code_block_gap: 0,
            code_wrap: WrapMode::NoWrap,
            text_wrap: WrapMode::WordThenGrapheme,
        }
    }
}

impl TextRenderPolicy {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn block_gap(&self) -> u16 {
        self.block_gap
    }

    #[must_use]
    pub fn with_block_gap(mut self, gap: u16) -> Self {
        self.block_gap = gap;
        self
    }

    #[must_use]
    pub fn soft_break(&self) -> SoftBreakPolicy {
        self.soft_break
    }

    #[must_use]
    pub fn with_soft_break(mut self, policy: SoftBreakPolicy) -> Self {
        self.soft_break = policy;
        self
    }

    #[must_use]
    pub fn table_column_gap(&self) -> u16 {
        self.table_column_gap
    }

    #[must_use]
    pub fn with_table_column_gap(mut self, gap: u16) -> Self {
        self.table_column_gap = gap;
        self
    }

    #[must_use]
    pub fn table_row_gap(&self) -> u16 {
        self.table_row_gap
    }

    #[must_use]
    pub fn with_table_row_gap(mut self, gap: u16) -> Self {
        self.table_row_gap = gap;
        self
    }

    #[must_use]
    pub fn table_column_sizing(&self) -> TableColumnSizing {
        self.table_column_sizing
    }

    #[must_use]
    pub fn with_table_column_sizing(mut self, sizing: TableColumnSizing) -> Self {
        self.table_column_sizing = sizing;
        self
    }

    #[must_use]
    pub fn task_list_marker(&self) -> TaskListMarkerPolicy {
        self.task_list_marker
    }

    #[must_use]
    pub fn with_task_list_marker(mut self, policy: TaskListMarkerPolicy) -> Self {
        self.task_list_marker = policy;
        self
    }

    #[must_use]
    pub fn code_block_label(&self) -> CodeBlockLabelPolicy {
        self.code_block_label
    }

    #[must_use]
    pub fn with_code_block_label(mut self, policy: CodeBlockLabelPolicy) -> Self {
        self.code_block_label = policy;
        self
    }

    #[must_use]
    pub fn code_block_gap(&self) -> u16 {
        self.code_block_gap
    }

    #[must_use]
    pub fn with_code_block_gap(mut self, gap: u16) -> Self {
        self.code_block_gap = gap;
        self
    }

    #[must_use]
    pub fn code_wrap(&self) -> WrapMode {
        self.code_wrap
    }

    #[must_use]
    pub fn with_code_wrap(mut self, wrap: WrapMode) -> Self {
        self.code_wrap = wrap;
        self
    }

    #[must_use]
    pub fn text_wrap(&self) -> WrapMode {
        self.text_wrap
    }

    #[must_use]
    pub fn with_text_wrap(mut self, wrap: WrapMode) -> Self {
        self.text_wrap = wrap;
        self
    }
}
