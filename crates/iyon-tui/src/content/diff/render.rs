use super::{DiffHunk, DiffLine, DiffLineKind, DiffLineTermination, DiffRange};
use crate::presentation::factory as vf;
use crate::{StyleRef, View};

/// Direct native-ingress lowering for validated static diff records.  The
/// operation-specific function keeps the formatting algorithm on the same
/// retained path without making the native boundary depend on the public
/// renderer/extension trait.
#[doc(hidden)]
pub fn lower_diff_hunks(hunks: &[DiffHunk]) -> View {
    vf::column(hunks.iter().map(lower_hunk).collect(), 0)
}

fn lower_hunk(hunk: &DiffHunk) -> View {
    // L1-04: moved children plus the direct column factory. The numeric
    // hunk/line/termination metadata below is untouched.
    let mut children = Vec::with_capacity(hunk.lines().len() + 1);
    children.push(header(hunk.old_range(), hunk.new_range()));
    for line in hunk.lines() {
        children.push(render_line(line));
        if line.termination() == DiffLineTermination::Unterminated {
            children.push(vf::text_with_style(
                "\\ No newline at end of file",
                crate::WrapMode::NoWrap,
                crate::HorizontalAlign::Start,
                StyleRef::theme("diff.meta"),
            ));
        }
    }
    vf::column(children, 0)
}

fn header(old: DiffRange, new_side: DiffRange) -> View {
    let text = format!("@@ -{} +{} @@", display_range(old), display_range(new_side));
    vf::text_with_style(
        text,
        crate::WrapMode::NoWrap,
        crate::HorizontalAlign::Start,
        StyleRef::theme("diff.header"),
    )
}

fn display_range(range: DiffRange) -> String {
    if range.is_empty() {
        return format!("{},0", range.start().as_u64());
    }
    let start = range.start().as_u64() + 1;
    if range.line_count() == 1 {
        start.to_string()
    } else {
        format!("{start},{}", range.line_count())
    }
}

fn render_line(line: &DiffLine) -> View {
    let style_key = match line.kind() {
        DiffLineKind::Context => "diff.context",
        DiffLineKind::Addition => "diff.addition",
        DiffLineKind::Deletion => "diff.deletion",
    };
    let style = StyleRef::theme(style_key);
    let marker = match line.kind() {
        DiffLineKind::Context => " ",
        DiffLineKind::Addition => "+",
        DiffLineKind::Deletion => "-",
    };
    vf::hanging(
        vf::text_with_style(
            marker,
            crate::WrapMode::NoWrap,
            crate::HorizontalAlign::Start,
            style.clone(),
        ),
        vf::text_with_style(
            " ",
            crate::WrapMode::NoWrap,
            crate::HorizontalAlign::Start,
            style.clone(),
        ),
        vf::text_with_style_fill(
            line.text(),
            crate::WrapMode::WordThenGrapheme,
            crate::HorizontalAlign::Start,
            style,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::{AnsiColor, PhysicalColor};
    use crate::presentation::layout::compile_view_with_theme;
    use crate::{ColorSpec, StyleSpec, Theme, ThemeColor};

    fn number(value: u64) -> crate::DiffLineNumber {
        crate::DiffLineNumber::new(value).unwrap()
    }

    fn range(start: u64, count: u64) -> crate::DiffRange {
        crate::DiffRange::new(crate::DiffLineOffset::new(start), count).unwrap()
    }

    fn hunk(
        old: crate::DiffRange,
        new_side: crate::DiffRange,
        lines: impl IntoIterator<Item = DiffLine>,
    ) -> DiffHunk {
        DiffHunk::new(old, new_side, lines).unwrap()
    }

    fn rows(view: &View, width: u16, theme: &Theme) -> Vec<String> {
        compile_view_with_theme(view, width, theme)
            .rows
            .iter()
            .map(|row| row.plain_text())
            .collect()
    }

    fn foreground(view: &View, needle: &str, theme: &Theme) -> Option<PhysicalColor> {
        let block = compile_view_with_theme(view, 80, theme);
        block.rows.iter().find_map(|row| {
            row.cell_x_of(needle)
                .and_then(|index| row.style_at(index))
                .and_then(|style| style.foreground)
        })
    }

    #[test]
    fn renderer_generates_markers_and_headers_without_reclassifying_payload() {
        let hunk = hunk(
            range(0, 2),
            range(0, 2),
            [
                DiffLine::context(number(1), number(1), "+ context"),
                DiffLine::deletion(number(2), "@@ deletion"),
                DiffLine::addition(number(2), "--- addition"),
            ],
        );
        assert_eq!(
            rows(&lower_hunk(&hunk), 80, &Theme::new()),
            [
                "@@ -1,2 +1,2 @@",
                " + context",
                "-@@ deletion",
                "+--- addition"
            ]
        );
    }

    #[test]
    fn renderer_formats_empty_ranges_and_unterminated_metadata() {
        let hunk = hunk(
            range(10, 0),
            range(10, 1),
            [DiffLine::addition(number(11), "added")
                .with_termination(DiffLineTermination::Unterminated)],
        );
        assert_eq!(
            rows(&lower_hunk(&hunk), 80, &Theme::new()),
            ["@@ -10,0 +11 @@", "+added", "\\ No newline at end of file"]
        );
    }

    #[test]
    fn narrow_width_wraps_payload_under_one_marker_and_continuation_prefix() {
        let hunk = hunk(
            range(0, 0),
            range(0, 1),
            [DiffLine::addition(number(1), "one two three four")],
        );
        let rendered = lower_hunk(&hunk);
        let result = compile_view_with_theme(&rendered, 10, &Theme::new());
        let texts: Vec<_> = result.rows.iter().map(|row| row.plain_text()).collect();
        assert_eq!(texts[0], "@@ -0,0 +1");
        assert_eq!(texts[1], "+one two ");
        assert!(texts[2].starts_with(" "));
        assert!(texts.len() > 3);
        assert_eq!(
            texts.iter().skip(1).filter(|row| row.contains('+')).count(),
            1
        );
    }

    #[test]
    fn framework_defaults_and_application_overrides_resolve_at_paint_time() {
        let hunk = hunk(
            range(0, 2),
            range(0, 2),
            [
                DiffLine::context(number(1), number(1), "context"),
                DiffLine::deletion(number(2), "deleted")
                    .with_termination(DiffLineTermination::Unterminated),
                DiffLine::addition(number(2), "added"),
            ],
        );
        let view = lower_hunk(&hunk);
        let framework = Theme::new();
        assert_eq!(
            foreground(&view, "context", &framework),
            Some(PhysicalColor::Named(AnsiColor::Gray))
        );
        assert_eq!(
            foreground(&view, "deleted", &framework),
            Some(PhysicalColor::Named(AnsiColor::Red))
        );
        assert_eq!(
            foreground(&view, "added", &framework),
            Some(PhysicalColor::Named(AnsiColor::Green))
        );
        assert_eq!(
            foreground(&view, "@@", &framework),
            Some(PhysicalColor::Named(AnsiColor::Cyan))
        );
        assert_eq!(
            foreground(&view, "\\ No newline", &framework),
            Some(PhysicalColor::Named(AnsiColor::Gray))
        );

        let colored = Theme::new()
            .with_color("diff.addition", ThemeColor::Rgb { r: 1, g: 2, b: 3 })
            .with_style(
                "diff.addition",
                StyleSpec::new().foreground(ColorSpec::theme("diff.addition")),
            );
        assert_eq!(
            foreground(&view, "added", &colored),
            Some(PhysicalColor::Rgb { r: 1, g: 2, b: 3 })
        );

        let styled = Theme::new().with_style(
            "diff.addition",
            StyleSpec::new().bold().background(ColorSpec::rgb(4, 5, 6)),
        );
        let a = compile_view_with_theme(&view, 40, &colored);
        let b = compile_view_with_theme(&view, 40, &styled);
        assert_eq!(
            a.rows
                .iter()
                .map(|row| row.plain_text())
                .collect::<Vec<_>>(),
            b.rows
                .iter()
                .map(|row| row.plain_text())
                .collect::<Vec<_>>()
        );
        assert_eq!(a.width, b.width);
        assert_ne!(
            foreground(&view, "added", &colored),
            foreground(&view, "added", &styled)
        );
        assert!(b.rows.iter().any(|row| {
            row.cell_x_of("added")
                .and_then(|index| row.style_at(index))
                .is_some_and(|style| {
                    style.bold && style.background == Some(PhysicalColor::Rgb { r: 4, g: 5, b: 6 })
                })
        }));
    }

    #[test]
    fn multiple_hunks_preserve_order_without_gap() {
        let first = hunk(
            range(0, 0),
            range(0, 1),
            [DiffLine::addition(number(1), "first")],
        );
        let second = hunk(
            range(2, 0),
            range(3, 1),
            [DiffLine::addition(number(4), "second")],
        );
        assert_eq!(
            rows(&lower_diff_hunks(&[first, second][..]), 80, &Theme::new()),
            ["@@ -0,0 +1 @@", "+first", "@@ -2,0 +4 @@", "+second"]
        );
    }
}
