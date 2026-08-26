use anyhow::{Result, anyhow};
use termwiz::{
    cell::CellAttributes,
    surface::{Change, CursorVisibility, Position, Surface},
    terminal::Terminal,
};

use crate::physical::PhysicalRow;

use super::lower::row_changes;

pub(crate) struct TermwizPresenter {
    presented: Surface,
    known: bool,
    sync_output_active: bool,
}

impl TermwizPresenter {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            presented: Surface::new(width, height),
            known: false,
            sync_output_active: false,
        }
    }

    pub(crate) fn dimensions(&self) -> (usize, usize) {
        self.presented.dimensions()
    }

    #[cfg(test)]
    pub(crate) fn presented_for_test(&self) -> &Surface {
        &self.presented
    }

    pub(crate) fn resize(&mut self, width: usize, height: usize) {
        if self.dimensions() == (width, height) {
            return;
        }
        self.presented.resize(width, height);
        self.known = false;
    }

    pub(crate) fn finish_sync_output_best_effort<T: Terminal + ?Sized>(
        &mut self,
        terminal: &mut T,
    ) {
        if !self.sync_output_active {
            return;
        }

        #[cfg(unix)]
        {
            let _ = terminal.render(&[Change::Text(SYNC_END.to_owned())]);
            let _ = terminal.flush();
        }
        self.sync_output_active = false;
    }

    pub(crate) fn present<T: Terminal + ?Sized>(
        &mut self,
        terminal: &mut T,
        desired: Surface,
    ) -> Result<()> {
        if desired.dimensions() != self.dimensions() {
            self.finish_sync_output_best_effort(terminal);
            let (width, height) = desired.dimensions();
            self.resize(width, height);
        }

        if !self.known {
            let changes = full_repaint_changes(&desired);
            return self.apply(terminal, changes, desired);
        }

        let finishing_sync = self.sync_output_active;
        let mut changes = self.presented.diff_screens(&desired);
        if changes.is_empty() && !finishing_sync {
            return Ok(());
        }
        changes.extend(canonical_terminal_state(desired.dimensions().1));
        #[cfg(unix)]
        if finishing_sync {
            changes.push(Change::Text(SYNC_END.to_owned()));
        }
        self.apply(terminal, changes, desired)
    }

    pub(crate) fn insert_history<T: Terminal + ?Sized>(
        &mut self,
        terminal: &mut T,
        rows: &[PhysicalRow],
    ) -> Result<usize> {
        if rows.is_empty() {
            return Ok(0);
        }

        let (width, height) = self.dimensions();
        if width == 0 || height == 0 {
            return Ok(0);
        }
        if !self.known {
            let retained = self.presented.clone();
            let changes = full_repaint_changes(&retained);
            self.apply(terminal, changes, retained)?;
        }

        let begin_sync = !self.sync_output_active;
        let transaction = native_transaction(rows, height, begin_sync);

        match terminal.render(&transaction).and_then(|_| terminal.flush()) {
            Ok(()) => {
                apply_native_scroll_model(&mut self.presented, rows.len());
                self.known = true;
                #[cfg(unix)]
                if begin_sync {
                    self.sync_output_active = true;
                }
                Ok(rows.len())
            }
            Err(error) => {
                self.known = false;
                self.abort_sync_output_best_effort(terminal, begin_sync);
                Err(anyhow!(error))
            }
        }
    }

    pub(crate) fn position_after_final_frame<T: Terminal + ?Sized>(
        &mut self,
        terminal: &mut T,
    ) -> Result<()> {
        self.finish_sync_output_best_effort(terminal);
        let (_, height) = self.dimensions();
        let changes = canonical_terminal_state(height);
        terminal.render(&changes)?;
        terminal.flush()?;
        Ok(())
    }

    fn abort_sync_output_best_effort<T: Terminal + ?Sized>(
        &mut self,
        terminal: &mut T,
        may_be_active: bool,
    ) {
        if !may_be_active && !self.sync_output_active {
            return;
        }

        #[cfg(unix)]
        {
            let _ = terminal.render(&[Change::Text(SYNC_END.to_owned())]);
            let _ = terminal.flush();
        }
        self.sync_output_active = false;
    }

    fn apply<T: Terminal + ?Sized>(
        &mut self,
        terminal: &mut T,
        changes: Vec<Change>,
        desired: Surface,
    ) -> Result<()> {
        if let Err(error) = terminal.render(&changes).and_then(|_| terminal.flush()) {
            self.known = false;
            self.abort_sync_output_best_effort(terminal, self.sync_output_active);
            return Err(anyhow!(error));
        }
        self.presented = desired;
        self.known = true;
        self.sync_output_active = false;
        Ok(())
    }
}

#[cfg(unix)]
const SYNC_BEGIN: &str = "\x1b[?2026h";
#[cfg(unix)]
const SYNC_END: &str = "\x1b[?2026l";

fn native_transaction(rows: &[PhysicalRow], height: usize, begin_sync: bool) -> Vec<Change> {
    let mut transaction = Vec::new();
    #[cfg(not(unix))]
    let _ = begin_sync;
    #[cfg(unix)]
    if begin_sync {
        transaction.push(Change::Text(SYNC_BEGIN.to_owned()));
    }

    for chunk in rows.chunks(height) {
        for (row_index, row) in chunk.iter().enumerate() {
            transaction.extend(row_changes(row, row_index, true));
        }
        transaction.push(Change::AllAttributes(CellAttributes::default()));
        transaction.push(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(height - 1),
        });
        transaction.push(Change::Text("\r\n".repeat(chunk.len())));
        transaction.push(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(height - chunk.len()),
        });
        transaction.push(Change::AllAttributes(CellAttributes::default()));
        transaction.push(Change::ClearToEndOfScreen(Default::default()));
    }

    transaction.extend(canonical_terminal_state(height));
    transaction
}

fn apply_native_scroll_model(presented: &mut Surface, rows: usize) {
    let (_, height) = presented.dimensions();
    if height == 0 {
        return;
    }

    let mut remaining = rows;
    while remaining > 0 {
        let count = remaining.min(height);
        // MODEL-ONLY.
        //
        // This Change is applied exclusively to an in-memory Termwiz Surface.
        // Native terminal scrollback is created only by ordinary full-screen
        // CRLF. Never emit this ScrollRegionUp to the real terminal.
        presented.add_change(Change::ScrollRegionUp {
            first_row: 0,
            region_size: height,
            scroll_count: count,
        });
        remaining -= count;
    }
}

#[cfg(test)]
fn model_after_native_scroll(presented: &Surface, rows: usize) -> Surface {
    let mut next = presented.clone();
    apply_native_scroll_model(&mut next, rows);
    next
}

fn full_repaint_changes(surface: &Surface) -> Vec<Change> {
    let (width, height) = surface.dimensions();
    let mut changes = vec![Change::CursorVisibility(CursorVisibility::Hidden)];
    for (y, line) in surface.screen_lines().into_iter().enumerate() {
        changes.push(Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(y),
        });
        changes.push(Change::AllAttributes(CellAttributes::default()));
        changes.push(Change::ClearToEndOfLine(Default::default()));
        for cell in line.visible_cells() {
            let x = cell.cell_index();
            if x >= width {
                continue;
            }
            changes.push(Change::CursorPosition {
                x: Position::Absolute(x),
                y: Position::Absolute(y),
            });
            changes.push(Change::AllAttributes(cell.attrs().clone()));
            changes.push(Change::Text(cell.str().to_string()));
        }
    }
    changes.extend(canonical_terminal_state(height));
    changes
}

fn canonical_terminal_state(height: usize) -> Vec<Change> {
    let y = height.saturating_sub(1);
    vec![
        Change::AllAttributes(CellAttributes::default()),
        Change::CursorPosition {
            x: Position::Absolute(0),
            y: Position::Absolute(y),
        },
        Change::CursorVisibility(CursorVisibility::Hidden),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::shadow::ShadowTerminal;
    use super::*;
    use crate::physical::{
        PhysicalCell, PhysicalColor, PhysicalRow, PhysicalStyle, grapheme_cell_width,
    };
    use termwiz::surface::Change;
    use unicode_segmentation::UnicodeSegmentation;

    #[cfg(unix)]
    struct RecordingTerminal {
        renders: Vec<Vec<Change>>,
        fail_next_render: bool,
    }

    #[cfg(unix)]
    impl RecordingTerminal {
        fn new() -> Self {
            Self {
                renders: Vec::new(),
                fail_next_render: false,
            }
        }

        fn fail_next_render(&mut self) {
            self.fail_next_render = true;
        }

        fn text(&self) -> String {
            self.renders
                .iter()
                .flat_map(|changes| changes.iter())
                .filter_map(|change| match change {
                    Change::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        }
    }

    #[cfg(unix)]
    impl termwiz::terminal::Terminal for RecordingTerminal {
        fn set_raw_mode(&mut self) -> termwiz::Result<()> {
            Ok(())
        }

        fn set_cooked_mode(&mut self) -> termwiz::Result<()> {
            Ok(())
        }

        fn enter_alternate_screen(&mut self) -> termwiz::Result<()> {
            Ok(())
        }

        fn exit_alternate_screen(&mut self) -> termwiz::Result<()> {
            Ok(())
        }

        fn get_screen_size(&mut self) -> termwiz::Result<termwiz::terminal::ScreenSize> {
            Ok(termwiz::terminal::ScreenSize {
                rows: 2,
                cols: 4,
                xpixel: 0,
                ypixel: 0,
            })
        }

        fn set_screen_size(&mut self, _size: termwiz::terminal::ScreenSize) -> termwiz::Result<()> {
            Ok(())
        }

        fn render(&mut self, changes: &[Change]) -> termwiz::Result<()> {
            if self.fail_next_render {
                self.fail_next_render = false;
                return Err(std::io::Error::other("recording render failure").into());
            }
            self.renders.push(changes.to_vec());
            Ok(())
        }

        fn flush(&mut self) -> termwiz::Result<()> {
            Ok(())
        }

        fn poll_input(
            &mut self,
            _wait: Option<std::time::Duration>,
        ) -> termwiz::Result<Option<termwiz::input::InputEvent>> {
            Ok(None)
        }

        fn waker(&self) -> termwiz::terminal::TerminalWaker {
            panic!("recording terminal does not wake")
        }
    }

    fn surface(text: &str, width: usize, height: usize) -> Surface {
        let mut surface = Surface::new(width, height);
        surface.add_changes(vec![
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::Text(text.to_string()),
        ]);
        surface
    }

    #[cfg(unix)]
    #[test]
    fn native_sync_wraps_multiple_inserts_and_empty_diff_present() {
        let initial = physical_surface(
            vec![
                painted_row("one", 4, PhysicalStyle::default()),
                painted_row("two", 4, PhysicalStyle::default()),
            ],
            4,
        );
        let row = PhysicalRow::from_cells(vec![PhysicalCell {
            grapheme: Some("n".into()),
            style: PhysicalStyle::default(),
            painted: true,
            continuation: false,
        }]);
        let mut terminal = RecordingTerminal::new();
        let mut presenter = TermwizPresenter::new(4, 2);
        presenter.present(&mut terminal, initial.clone()).unwrap();
        terminal.renders.clear();

        presenter
            .insert_history(&mut terminal, std::slice::from_ref(&row))
            .unwrap();
        presenter
            .insert_history(&mut terminal, std::slice::from_ref(&row))
            .unwrap();
        let shifted = model_after_native_scroll(&initial, 2);
        presenter.present(&mut terminal, shifted).unwrap();

        let text = terminal.text();
        assert_eq!(text.matches(SYNC_BEGIN).count(), 1);
        assert_eq!(text.matches(SYNC_END).count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn native_insert_failure_ends_sync_output() {
        let initial = physical_surface(
            vec![
                painted_row("one", 4, PhysicalStyle::default()),
                painted_row("two", 4, PhysicalStyle::default()),
            ],
            4,
        );
        let row = PhysicalRow::from_cells(vec![PhysicalCell {
            grapheme: Some("n".into()),
            style: PhysicalStyle::default(),
            painted: true,
            continuation: false,
        }]);
        let mut terminal = RecordingTerminal::new();
        let mut presenter = TermwizPresenter::new(4, 2);
        presenter.present(&mut terminal, initial).unwrap();
        presenter
            .insert_history(&mut terminal, std::slice::from_ref(&row))
            .unwrap();
        terminal.renders.clear();
        terminal.fail_next_render();

        assert!(
            presenter
                .insert_history(&mut terminal, std::slice::from_ref(&row))
                .is_err()
        );
        assert_eq!(terminal.text().matches(SYNC_END).count(), 1);

        terminal.renders.clear();
        presenter
            .present(&mut terminal, Surface::new(4, 2))
            .unwrap();
        assert_eq!(terminal.text().matches(SYNC_END).count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn present_failure_ends_sync_output() {
        let mut terminal = RecordingTerminal::new();
        let mut presenter = TermwizPresenter::new(4, 2);
        presenter
            .present(&mut terminal, Surface::new(4, 2))
            .unwrap();
        presenter
            .insert_history(
                &mut terminal,
                &[PhysicalRow::from_cells(vec![PhysicalCell {
                    grapheme: Some("x".into()),
                    style: PhysicalStyle::default(),
                    painted: true,
                    continuation: false,
                }])],
            )
            .unwrap();
        terminal.renders.clear();
        terminal.fail_next_render();

        assert!(
            presenter
                .present(&mut terminal, Surface::new(4, 2))
                .is_err()
        );
        assert_eq!(terminal.text().matches(SYNC_END).count(), 1);

        terminal.renders.clear();
        presenter
            .present(&mut terminal, Surface::new(4, 2))
            .unwrap();
        assert_eq!(terminal.text().matches(SYNC_END).count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn resize_cleanup_ends_sync_output_before_geometry_changes() {
        let mut terminal = RecordingTerminal::new();
        let mut presenter = TermwizPresenter::new(4, 2);
        presenter
            .present(&mut terminal, Surface::new(4, 2))
            .unwrap();
        presenter
            .insert_history(
                &mut terminal,
                &[PhysicalRow::from_cells(vec![PhysicalCell {
                    grapheme: Some("x".into()),
                    style: PhysicalStyle::default(),
                    painted: true,
                    continuation: false,
                }])],
            )
            .unwrap();
        terminal.renders.clear();

        presenter
            .present(&mut terminal, Surface::new(8, 4))
            .unwrap();

        assert_eq!(terminal.text().matches(SYNC_END).count(), 1);
        assert_eq!(presenter.dimensions(), (8, 4));
    }

    #[cfg(unix)]
    #[test]
    fn finish_sync_is_idempotent() {
        let mut terminal = RecordingTerminal::new();
        let mut presenter = TermwizPresenter::new(4, 2);
        presenter
            .present(&mut terminal, Surface::new(4, 2))
            .unwrap();
        presenter
            .insert_history(
                &mut terminal,
                &[PhysicalRow::from_cells(vec![PhysicalCell {
                    grapheme: Some("x".into()),
                    style: PhysicalStyle::default(),
                    painted: true,
                    continuation: false,
                }])],
            )
            .unwrap();
        terminal.renders.clear();

        presenter.finish_sync_output_best_effort(&mut terminal);
        presenter.finish_sync_output_best_effort(&mut terminal);
        assert_eq!(terminal.text().matches(SYNC_END).count(), 1);
    }

    #[test]
    fn identical_surfaces_have_no_diff() {
        let a = surface("hello", 5, 1);
        assert!(a.diff_screens(&a).is_empty());
    }

    #[test]
    fn applying_a_diff_reaches_the_desired_surface() {
        let a = surface("hello", 5, 1);
        let b = surface("x    ", 5, 1);
        let changes = a.diff_screens(&b);
        let mut actual = a.clone();
        actual.add_changes(changes);
        assert_eq!(actual.screen_chars_to_string(), b.screen_chars_to_string());
    }

    #[test]
    fn native_transaction_uses_full_screen_crlf_without_repainting() {
        let row = PhysicalRow::from_cells(vec![
            PhysicalCell {
                grapheme: Some("a".to_string()),
                style: Default::default(),
                painted: true,
                continuation: false,
            },
            PhysicalCell {
                grapheme: Some("b".to_string()),
                style: Default::default(),
                painted: true,
                continuation: false,
            },
            PhysicalCell {
                grapheme: Some("c".to_string()),
                style: Default::default(),
                painted: true,
                continuation: false,
            },
        ]);
        let transaction = native_transaction(&[row], 2, false);
        assert!(!transaction.iter().any(|change| matches!(
            change,
            Change::ClearScreen(_)
                | Change::ScrollRegionUp { .. }
                | Change::ScrollRegionDown { .. }
        )));
        assert!(transaction.iter().any(|change| matches!(
            change,
            Change::AllAttributes(attrs) if *attrs == CellAttributes::default()
        )));
        assert_eq!(
            transaction
                .iter()
                .filter(|change| matches!(change, Change::ClearToEndOfLine(_)))
                .count(),
            1
        );
        assert!(transaction.iter().any(|change| matches!(
            change,
            Change::Text(text) if text == "\r\n"
        )));
    }

    fn physical_surface(rows: Vec<Vec<PhysicalCell>>, width: u16) -> Surface {
        let mut surface = Surface::new(usize::from(width), rows.len());
        for (y, cells) in rows.into_iter().enumerate() {
            surface.add_changes(row_changes(&PhysicalRow::from_cells(cells), y, true));
        }
        surface
    }

    fn painted_row(text: &str, width: usize, style: PhysicalStyle) -> Vec<PhysicalCell> {
        let mut cells = text
            .chars()
            .take(width)
            .map(|character| PhysicalCell {
                grapheme: Some(character.to_string()),
                style,
                painted: true,
                continuation: false,
            })
            .collect::<Vec<_>>();
        while cells.len() < width {
            cells.push(PhysicalCell {
                grapheme: Some(" ".into()),
                style,
                painted: true,
                continuation: false,
            });
        }
        cells
    }

    fn assert_virtual_screen_matches_surface(model: &VirtualTerminal, expected: &Surface) {
        assert_eq!((model.width, model.height), expected.dimensions());
        for (y, line) in expected.screen_lines().iter().enumerate() {
            for x in 0..model.width {
                let expected_cell = line.get_cell(x).expect("expected cell");
                let actual = &model.screen[y][x];
                assert_eq!(
                    actual.text,
                    expected_cell.str(),
                    "text mismatch at ({x}, {y})"
                );
                assert_eq!(
                    actual.attrs,
                    *expected_cell.attrs(),
                    "attrs mismatch at ({x}, {y})"
                );
            }
        }
    }

    fn assert_surface_state_equal(actual: &Surface, expected: &Surface) {
        assert_eq!(actual.dimensions(), expected.dimensions());
        let actual_lines = actual.screen_lines();
        let expected_lines = expected.screen_lines();
        for (actual_line, expected_line) in actual_lines.iter().zip(expected_lines.iter()) {
            for x in 0..actual.dimensions().0 {
                let actual_cell = actual_line.get_cell(x).expect("actual cell");
                let expected_cell = expected_line.get_cell(x).expect("expected cell");
                assert_eq!(actual_cell.str(), expected_cell.str());
                assert_eq!(actual_cell.attrs(), expected_cell.attrs());
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct VirtualCell {
        text: String,
        attrs: CellAttributes,
    }

    struct VirtualTerminal {
        width: usize,
        height: usize,
        screen: Vec<Vec<VirtualCell>>,
        scrollback: Vec<Vec<VirtualCell>>,
        x: usize,
        y: usize,
        attrs: CellAttributes,
    }

    impl VirtualTerminal {
        fn new(width: usize, height: usize) -> Self {
            let blank = || VirtualCell {
                text: " ".into(),
                attrs: CellAttributes::default(),
            };
            Self {
                width,
                height,
                screen: (0..height)
                    .map(|_| (0..width).map(|_| blank()).collect())
                    .collect(),
                scrollback: Vec::new(),
                x: 0,
                y: 0,
                attrs: CellAttributes::default(),
            }
        }

        fn load_surface(&mut self, surface: &Surface) {
            assert_eq!((self.width, self.height), surface.dimensions());
            self.screen = surface
                .screen_lines()
                .iter()
                .map(|line| {
                    (0..self.width)
                        .map(|x| {
                            let cell = line.get_cell(x).expect("surface cell");
                            VirtualCell {
                                text: cell.str().to_owned(),
                                attrs: cell.attrs().clone(),
                            }
                        })
                        .collect()
                })
                .collect();
            self.x = 0;
            self.y = 0;
            self.attrs = CellAttributes::default();
        }

        fn scroll_up(&mut self) {
            if self.height == 0 {
                return;
            }
            self.scrollback.push(self.screen.remove(0));
            self.screen.push(
                (0..self.width)
                    .map(|_| VirtualCell {
                        text: " ".into(),
                        attrs: self.attrs.clone(),
                    })
                    .collect(),
            );
            self.y = self.height - 1;
        }

        fn apply(&mut self, changes: &[Change]) {
            for change in changes {
                match change {
                    Change::CursorPosition { x, y } => {
                        self.x = match x {
                            Position::Absolute(value) => *value,
                            _ => panic!("test model only supports absolute x"),
                        };
                        self.y = match y {
                            Position::Absolute(value) => *value,
                            _ => panic!("test model only supports absolute y"),
                        };
                    }
                    Change::AllAttributes(attrs) => self.attrs = attrs.clone(),
                    Change::ClearToEndOfLine(_) => {
                        for cell in self.screen[self.y][self.x..].iter_mut() {
                            *cell = VirtualCell {
                                text: " ".into(),
                                attrs: self.attrs.clone(),
                            };
                        }
                    }
                    Change::ClearToEndOfScreen(_) => {
                        for row in self.screen.iter_mut().skip(self.y) {
                            for cell in row.iter_mut() {
                                *cell = VirtualCell {
                                    text: " ".into(),
                                    attrs: self.attrs.clone(),
                                };
                            }
                        }
                    }
                    Change::Text(text) => {
                        for character in text.chars() {
                            match character {
                                '\r' => self.x = 0,
                                '\n' => {
                                    if self.y + 1 >= self.height {
                                        self.scroll_up();
                                    } else {
                                        self.y += 1;
                                    }
                                }
                                character => {
                                    if self.x < self.width && self.y < self.height {
                                        self.screen[self.y][self.x] = VirtualCell {
                                            text: character.to_string(),
                                            attrs: self.attrs.clone(),
                                        };
                                    }
                                    self.x += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        fn row_text(row: &[VirtualCell]) -> String {
            row.iter().map(|cell| cell.text.as_str()).collect()
        }
    }

    #[test]
    fn native_scroll_matches_the_independent_shifted_model_and_following_diff() {
        let width = 8;
        let bubble_style = PhysicalStyle {
            background: Some(PhysicalColor::Indexed(4)),
            ..PhysicalStyle::default()
        };
        let presented = physical_surface(
            vec![
                painted_row("header", width, bubble_style),
                painted_row("header", width, bubble_style),
                painted_row("content", width, PhysicalStyle::default()),
                vec![PhysicalCell::transparent(); width],
                painted_row("--------", width, PhysicalStyle::default()),
                painted_row("input", width, PhysicalStyle::default()),
                painted_row("--------", width, PhysicalStyle::default()),
                painted_row("status", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        let rows = [
            PhysicalRow::from_cells(vec![PhysicalCell {
                grapheme: Some("a".into()),
                style: PhysicalStyle::default(),
                painted: true,
                continuation: false,
            }]),
            PhysicalRow::from_cells(Vec::new()),
            PhysicalRow::from_cells(vec![PhysicalCell {
                grapheme: Some("b".into()),
                style: PhysicalStyle::default(),
                painted: true,
                continuation: false,
            }]),
        ];
        let transaction = native_transaction(&rows, 8, false);
        let mut model = VirtualTerminal::new(width, 8);
        model.load_surface(&presented);
        model.apply(&transaction);
        let shifted = model_after_native_scroll(&presented, rows.len());

        assert_eq!(
            model
                .scrollback
                .iter()
                .map(|row| VirtualTerminal::row_text(row))
                .collect::<Vec<_>>(),
            ["a       ", "        ", "b       "]
        );
        assert_virtual_screen_matches_surface(&model, &shifted);

        let next = physical_surface(
            vec![
                painted_row("model", width, PhysicalStyle::default()),
                vec![PhysicalCell::transparent(); width],
                painted_row("--------", width, PhysicalStyle::default()),
                painted_row("input", width, PhysicalStyle::default()),
                painted_row("--------", width, PhysicalStyle::default()),
                painted_row("status", width, PhysicalStyle::default()),
                vec![PhysicalCell::transparent(); width],
                vec![PhysicalCell::transparent(); width],
            ],
            width as u16,
        );
        let mut actual = shifted.clone();
        actual.add_changes(actual.diff_screens(&next));
        assert_surface_state_equal(&actual, &next);
    }

    #[test]
    fn differential_present_clears_styled_bubble_cells_and_attributes() {
        let bubble_style = PhysicalStyle {
            foreground: Some(PhysicalColor::Indexed(4)),
            background: Some(PhysicalColor::Indexed(7)),
            bold: true,
            italic: true,
            underline: true,
            ..PhysicalStyle::default()
        };
        let a = physical_surface(
            vec![vec![
                PhysicalCell {
                    grapheme: Some("A".into()),
                    style: bubble_style,
                    painted: true,
                    continuation: false,
                },
                PhysicalCell {
                    grapheme: Some("B".into()),
                    style: bubble_style,
                    painted: true,
                    continuation: false,
                },
                PhysicalCell::transparent(),
                PhysicalCell::transparent(),
            ]],
            4,
        );
        let b = physical_surface(
            vec![vec![
                PhysicalCell {
                    grapheme: Some("x".into()),
                    style: PhysicalStyle::default(),
                    painted: true,
                    continuation: false,
                },
                PhysicalCell::transparent(),
                PhysicalCell::transparent(),
                PhysicalCell::transparent(),
            ]],
            4,
        );
        let mut actual = a.clone();
        let changes = actual.diff_screens(&b);
        actual.add_changes(changes);
        assert_surface_state_equal(&actual, &b);
    }

    #[test]
    fn native_shift_then_changed_frame_reaches_exact_styled_state() {
        let a = physical_surface(
            vec![vec![
                PhysicalCell {
                    grapheme: Some("A".into()),
                    style: PhysicalStyle {
                        background: Some(PhysicalColor::Indexed(2)),
                        ..PhysicalStyle::default()
                    },
                    painted: true,
                    continuation: false,
                },
                PhysicalCell {
                    grapheme: Some("B".into()),
                    style: PhysicalStyle::default(),
                    painted: true,
                    continuation: false,
                },
                PhysicalCell::transparent(),
                PhysicalCell::transparent(),
            ]],
            4,
        );
        let mut actual = model_after_native_scroll(&a, 1);

        let b = physical_surface(
            vec![vec![
                PhysicalCell {
                    grapheme: Some("z".into()),
                    style: PhysicalStyle::default(),
                    painted: true,
                    continuation: false,
                },
                PhysicalCell::transparent(),
                PhysicalCell::transparent(),
                PhysicalCell::transparent(),
            ]],
            4,
        );
        let changes = actual.diff_screens(&b);
        actual.add_changes(changes);
        assert_surface_state_equal(&actual, &b);
    }

    #[test]
    fn native_transaction_handles_batches_larger_than_the_screen() {
        let presented = physical_surface(
            vec![
                painted_row("zero", 4, PhysicalStyle::default()),
                painted_row("one ", 4, PhysicalStyle::default()),
                painted_row("two ", 4, PhysicalStyle::default()),
                painted_row("three", 4, PhysicalStyle::default()),
            ],
            4,
        );

        for count in [1, 2, 3, 4, 5, 8] {
            let rows = (0..count)
                .map(|index| {
                    PhysicalRow::from_cells(vec![PhysicalCell {
                        grapheme: Some(char::from(b'a' + (index % 26) as u8).to_string()),
                        style: Default::default(),
                        painted: true,
                        continuation: false,
                    }])
                })
                .collect::<Vec<_>>();
            let transaction = native_transaction(&rows, 4, false);
            let mut model = VirtualTerminal::new(4, 4);
            model.load_surface(&presented);
            model.apply(&transaction);
            let shifted = model_after_native_scroll(&presented, rows.len());
            assert_virtual_screen_matches_surface(&model, &shifted);
        }
    }

    #[test]
    fn native_scroll_resets_attributes_before_exposing_new_rows() {
        let width = 4;
        let blue = PhysicalStyle {
            background: Some(PhysicalColor::Indexed(4)),
            ..PhysicalStyle::default()
        };
        let row = PhysicalRow::from_cells(painted_row("blue", width, blue));
        let mut broken = VirtualTerminal::new(width, 2);
        broken.apply(&row_changes(&row, 0, true));
        broken.apply(&[
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(1),
            },
            Change::Text("\r\n".into()),
        ]);
        assert!(broken.screen[1][0].attrs != CellAttributes::default());

        let transaction = native_transaction(&[row], 2, false);
        let mut correct = VirtualTerminal::new(width, 2);
        correct.apply(&transaction);
        assert!(
            correct.screen[1]
                .iter()
                .all(|cell| cell.attrs == CellAttributes::default())
        );
    }

    fn wide_row(text: &str, width: usize, style: PhysicalStyle) -> PhysicalRow {
        let mut cells = Vec::new();
        for grapheme in text.graphemes(true) {
            let w = grapheme_cell_width(grapheme);
            if w == 0 {
                continue;
            }
            cells.push(PhysicalCell {
                grapheme: Some(grapheme.to_string()),
                style,
                painted: true,
                continuation: false,
            });
            for _ in 1..w {
                cells.push(PhysicalCell {
                    grapheme: None,
                    style,
                    painted: true,
                    continuation: true,
                });
            }
        }
        while cells.len() < width {
            cells.push(PhysicalCell {
                grapheme: Some(" ".into()),
                style,
                painted: true,
                continuation: false,
            });
        }
        PhysicalRow::from_cells(cells)
    }

    #[test]
    fn native_history_uses_full_screen_crlf_not_explicit_scroll_commands() {
        let row = |i: usize| {
            let character = char::from(b'a' + (i % 26) as u8).to_string();
            PhysicalRow::from_cells(vec![PhysicalCell {
                grapheme: Some(character),
                style: Default::default(),
                painted: true,
                continuation: false,
            }])
        };

        for row_count in [1, 2, 3, 5, 8, 10] {
            let rows = (0..row_count).map(row).collect::<Vec<_>>();
            let height = 4;
            let transaction = native_transaction(&rows, height, false);

            assert!(
                !transaction.iter().any(|change| matches!(
                    change,
                    Change::ScrollRegionUp { .. } | Change::ScrollRegionDown { .. }
                )),
                "native_transaction must never emit explicit scroll region commands"
            );

            let crlf_count = transaction
                .iter()
                .filter_map(|change| match change {
                    Change::Text(text) => Some(text.matches("\r\n").count()),
                    _ => None,
                })
                .sum::<usize>();

            assert_eq!(
                crlf_count, row_count,
                "number of emitted CRLFs must equal the promoted physical row count"
            );
        }
    }

    #[test]
    fn presenter_native_insert_matches_shadow_screen() {
        let width = 4;
        let height = 3;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("A", width, PhysicalStyle::default()),
                painted_row("B", width, PhysicalStyle::default()),
                painted_row("C", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let row_x = PhysicalRow::from_cells(painted_row("X", width, PhysicalStyle::default()));
        let inserted = presenter.insert_history(&mut shadow, &[row_x]).unwrap();
        assert_eq!(inserted, 1);

        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["X"]);
        assert_eq!(shadow.screen_trimmed_texts(), vec!["B", "C", ""]);
        shadow.assert_screen_matches_surface(presenter.presented_for_test());
        assert_eq!(shadow.implicit_wraps, 0);
    }

    #[test]
    fn presenter_multiple_native_inserts_match_shadow_tape() {
        let width = 4;
        let height = 4;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("A", width, PhysicalStyle::default()),
                painted_row("B", width, PhysicalStyle::default()),
                painted_row("C", width, PhysicalStyle::default()),
                painted_row("D", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let row_x = PhysicalRow::from_cells(painted_row("X", width, PhysicalStyle::default()));
        let row_y = PhysicalRow::from_cells(painted_row("Y", width, PhysicalStyle::default()));
        let inserted = presenter
            .insert_history(&mut shadow, &[row_x, row_y])
            .unwrap();
        assert_eq!(inserted, 2);

        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["X", "Y"]);
        assert_eq!(shadow.screen_trimmed_texts(), vec!["C", "D", "", ""]);
        shadow.assert_screen_matches_surface(presenter.presented_for_test());
        assert_eq!(shadow.implicit_wraps, 0);
    }

    #[test]
    fn presenter_native_inserts_larger_than_viewport_chunk_correctly() {
        let width = 4;
        let height = 4;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("A", width, PhysicalStyle::default()),
                painted_row("B", width, PhysicalStyle::default()),
                painted_row("C", width, PhysicalStyle::default()),
                painted_row("D", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let rows = ["1", "2", "3", "4", "5", "6"]
            .into_iter()
            .map(|s| PhysicalRow::from_cells(painted_row(s, width, PhysicalStyle::default())))
            .collect::<Vec<_>>();

        let inserted = presenter.insert_history(&mut shadow, &rows).unwrap();
        assert_eq!(inserted, 6);

        assert_eq!(
            shadow.scrollback_trimmed_texts(),
            vec!["1", "2", "3", "4", "5", "6"]
        );
        assert_eq!(shadow.screen_trimmed_texts(), vec!["", "", "", ""]);
        shadow.assert_screen_matches_surface(presenter.presented_for_test());
        assert_eq!(shadow.implicit_wraps, 0);
    }

    #[test]
    fn repeated_inserts_before_present_preserve_tape_and_sync() {
        let width = 6;
        let height = 4;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("init0", width, PhysicalStyle::default()),
                painted_row("init1", width, PhysicalStyle::default()),
                painted_row("init2", width, PhysicalStyle::default()),
                painted_row("init3", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let row_x = PhysicalRow::from_cells(painted_row("X", width, PhysicalStyle::default()));
        let row_y = PhysicalRow::from_cells(painted_row("Y", width, PhysicalStyle::default()));
        let row_z = PhysicalRow::from_cells(painted_row("Z", width, PhysicalStyle::default()));

        presenter.insert_history(&mut shadow, &[row_x]).unwrap();
        #[cfg(unix)]
        assert!(shadow.sync_output_active);

        presenter.insert_history(&mut shadow, &[row_y]).unwrap();
        #[cfg(unix)]
        assert!(shadow.sync_output_active);

        presenter.insert_history(&mut shadow, &[row_z]).unwrap();
        #[cfg(unix)]
        assert!(shadow.sync_output_active);

        let desired = physical_surface(
            vec![
                painted_row("init3", width, PhysicalStyle::default()),
                painted_row("next1", width, PhysicalStyle::default()),
                painted_row("next2", width, PhysicalStyle::default()),
                painted_row("next3", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, desired.clone()).unwrap();

        #[cfg(unix)]
        assert!(!shadow.sync_output_active);

        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["X", "Y", "Z"]);
        shadow.assert_screen_matches_surface(presenter.presented_for_test());
        shadow.assert_screen_matches_surface(&desired);
        assert_eq!(shadow.implicit_wraps, 0);
    }

    #[test]
    fn shadow_native_rows_preserve_styles_and_wide_glyphs() {
        let width = 12;
        let height = 4;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("row 0", width, PhysicalStyle::default()),
                painted_row("row 1", width, PhysicalStyle::default()),
                painted_row("row 2", width, PhysicalStyle::default()),
                painted_row("row 3", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let glyph_samples = ["A🐕🦺B", "🇮🇩", "👩‍🔬", "⭐"];
        for sample in glyph_samples {
            let row = wide_row(sample, width, PhysicalStyle::default());
            presenter.insert_history(&mut shadow, &[row]).unwrap();

            assert_eq!(shadow.implicit_wraps, 0);
            assert_eq!(shadow.scrollback_trimmed_texts().last().unwrap(), sample);
            shadow.assert_screen_matches_surface(presenter.presented_for_test());
        }
    }

    #[test]
    fn shadow_native_rows_preserve_styles_and_attributes() {
        let width = 10;
        let height = 3;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("A", width, PhysicalStyle::default()),
                painted_row("B", width, PhysicalStyle::default()),
                painted_row("C", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let rich_style = PhysicalStyle {
            foreground: Some(PhysicalColor::Indexed(2)),
            background: Some(PhysicalColor::Indexed(4)),
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            ..PhysicalStyle::default()
        };
        let row = wide_row("Styled", width, rich_style);
        presenter.insert_history(&mut shadow, &[row]).unwrap();

        assert_eq!(shadow.implicit_wraps, 0);
        let scrollback_row = shadow.scrollback.last().expect("scrollback row");
        assert_eq!(scrollback_row.trimmed_text(), "Styled");

        for x in 0..6 {
            let cell = &scrollback_row.cells[x];
            assert_eq!(
                cell.attrs.background(),
                termwiz::color::ColorAttribute::PaletteIndex(4)
            );
            assert_eq!(
                cell.attrs.foreground(),
                termwiz::color::ColorAttribute::PaletteIndex(2)
            );
            assert_eq!(cell.attrs.intensity(), termwiz::cell::Intensity::Bold);
            assert!(cell.attrs.italic());
            assert_eq!(cell.attrs.underline(), termwiz::cell::Underline::Single);
            assert!(cell.attrs.strikethrough());
        }

        shadow.assert_screen_matches_surface(presenter.presented_for_test());
    }

    #[test]
    fn shadow_scrollback_survives_full_repaint() {
        let width = 8;
        let height = 3;
        let mut shadow = ShadowTerminal::new(width, height);
        let mut presenter = TermwizPresenter::new(width, height);

        let initial = physical_surface(
            vec![
                painted_row("init0", width, PhysicalStyle::default()),
                painted_row("init1", width, PhysicalStyle::default()),
                painted_row("init2", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, initial).unwrap();

        let row_x =
            PhysicalRow::from_cells(painted_row("NativeX", width, PhysicalStyle::default()));
        presenter.insert_history(&mut shadow, &[row_x]).unwrap();

        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["NativeX"]);
        let saved_scrollback = shadow.scrollback.clone();

        // Force presenter unknown / full repaint path
        presenter.resize(width + 2, height + 1);
        presenter.resize(width, height);

        let new_frame = physical_surface(
            vec![
                painted_row("newA", width, PhysicalStyle::default()),
                painted_row("newB", width, PhysicalStyle::default()),
                painted_row("newC", width, PhysicalStyle::default()),
            ],
            width as u16,
        );
        presenter.present(&mut shadow, new_frame.clone()).unwrap();

        // Scrollback must remain completely untouched by the full repaint.
        assert_eq!(shadow.scrollback, saved_scrollback);
        assert_eq!(shadow.scrollback_trimmed_texts(), vec!["NativeX"]);
        shadow.assert_screen_matches_surface(presenter.presented_for_test());
        shadow.assert_screen_matches_surface(&new_frame);
        assert_eq!(shadow.implicit_wraps, 0);
    }
}
