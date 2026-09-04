use crate::output::{EventCx, Output};

use super::buffer::TextBuffer;

/// A borrowed snapshot of a `TextInput` after a user text mutation.
#[derive(Clone, Copy, Debug)]
pub struct TextChange<'a> {
    text: &'a str,
    cursor_bytes: usize,
}

impl<'a> TextChange<'a> {
    pub fn text(self) -> &'a str {
        self.text
    }

    pub fn cursor_bytes(self) -> usize {
        self.cursor_bytes
    }

    pub fn is_empty(self) -> bool {
        self.text.is_empty()
    }
}

pub(super) trait ChangeProjector {
    fn emit(&self, buffer: &TextBuffer, cx: &mut EventCx<'_>);
}

struct TypedProjector<R: 'static, F> {
    output: Output<R>,
    project: F,
}

impl<R, F> ChangeProjector for TypedProjector<R, F>
where
    R: 'static,
    F: for<'change> Fn(TextChange<'change>) -> R + 'static,
{
    fn emit(&self, buffer: &TextBuffer, cx: &mut EventCx<'_>) {
        let change = TextChange {
            text: buffer.text(),
            cursor_bytes: buffer.cursor_bytes(),
        };
        cx.emit(self.output, (self.project)(change));
    }
}

#[derive(Default)]
pub(super) struct ChangeOutputs {
    projectors: Vec<Box<dyn ChangeProjector>>,
}

impl ChangeOutputs {
    pub(super) fn register<R, F>(&mut self, project: F) -> Output<R>
    where
        R: 'static,
        F: for<'change> Fn(TextChange<'change>) -> R + 'static,
    {
        let output = Output::new();
        self.projectors
            .push(Box::new(TypedProjector { output, project }));
        output
    }

    pub(super) fn emit(&self, buffer: &TextBuffer, cx: &mut EventCx<'_>) {
        for projector in &self.projectors {
            projector.emit(buffer, cx);
        }
    }
}
