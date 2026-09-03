//! Text normalization and shell-editor word runs.

pub(super) const WORD_SEPARATORS: &str = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";

pub(super) fn canonicalize(input: &str, multiline: bool) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        match character {
            '\t' => output.push_str("    "),
            '\n' if multiline => output.push('\n'),
            '\n' => output.push(' '),
            character => output.push(character),
        }
    }
    output
}

pub(super) fn is_separator(character: char) -> bool {
    character.is_whitespace() || WORD_SEPARATORS.contains(character)
}
