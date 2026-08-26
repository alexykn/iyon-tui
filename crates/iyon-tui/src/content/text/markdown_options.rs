use pulldown_cmark::Options;

/// Explicitly selected Markdown extensions supported by [`super::MarkdownProjector`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownOptions {
    tables: bool,
    strikethrough: bool,
    task_lists: bool,
    /// Keep an unsealed GFM table as raw pipe source
    /// until a closer. This is **not** GFM. Spec Example 202 allows a pipe-less
    /// line to remain a short table row; a table ends on a blank line or a new
    /// block. Enable this only for a live streaming pipeline.
    live_table_stabilization: bool,
}

impl MarkdownOptions {
    /// Strict CommonMark parsing with all optional extensions disabled.
    pub const fn commonmark() -> Self {
        Self {
            tables: false,
            strikethrough: false,
            task_lists: false,
            live_table_stabilization: false,
        }
    }

    /// GitHub-Flavored-Markdown-oriented extension preset supported by
    /// `MarkdownProjector`.
    ///
    /// Enables the GFM extensions represented by the generic text IR:
    /// tables, strikethrough, and task-list markers.
    ///
    /// `Default` remains strict CommonMark.
    pub const fn gfm() -> Self {
        Self {
            tables: true,
            strikethrough: true,
            task_lists: true,
            live_table_stabilization: false,
        }
    }

    pub const fn with_tables(mut self, enabled: bool) -> Self {
        self.tables = enabled;
        self
    }

    pub const fn with_strikethrough(mut self, enabled: bool) -> Self {
        self.strikethrough = enabled;
        self
    }

    pub const fn with_task_lists(mut self, enabled: bool) -> Self {
        self.task_lists = enabled;
        self
    }

    /// Keep unsealed tables as raw pipe paragraphs until a caller-defined closer.
    ///
    /// Not GFM grammar. Used by a live streaming pipeline so a live table aligns
    /// once, after the following line proves it cannot grow more `|` rows.
    pub const fn with_live_table_stabilization(mut self, enabled: bool) -> Self {
        self.live_table_stabilization = enabled;
        self
    }

    pub const fn tables(self) -> bool {
        self.tables
    }

    pub const fn strikethrough(self) -> bool {
        self.strikethrough
    }

    pub const fn task_lists(self) -> bool {
        self.task_lists
    }

    pub const fn live_table_stabilization(self) -> bool {
        self.live_table_stabilization
    }

    pub(crate) fn pulldown(self) -> Options {
        let mut options = Options::empty();
        if self.tables {
            options.insert(Options::ENABLE_TABLES);
        }
        if self.strikethrough {
            options.insert(Options::ENABLE_STRIKETHROUGH);
        }
        if self.task_lists {
            options.insert(Options::ENABLE_TASKLISTS);
        }
        options
    }
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self::commonmark()
    }
}
