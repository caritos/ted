//! Text helpers shared by the git view and the file tree.

use crate::screen::{Row, Span, Style};

/// First row to show so that `selected` is inside a window of `height` rows.
pub(crate) fn window_start(selected: usize, height: usize) -> usize {
    (selected + 1).saturating_sub(height.max(1))
}

/// `s` safe to print: C0 controls, DEL and C1 controls become `?` so text
/// from git, file names or messages can never send escape sequences, carriage
/// returns and the like to the terminal. A tab (diff lines expand theirs
/// before this) becomes one space so the width stays one column.
pub(crate) fn sanitize(s: &str) -> String {
    s.chars().map(sanitize_char).collect()
}

/// One char of [`sanitize`]: always exactly one char out per char in, so
/// columns and per-char styles stay aligned with the text they describe.
pub(crate) fn sanitize_char(c: char) -> char {
    match c {
        '\t' => ' ',
        c if c.is_control() => '?',
        c => c,
    }
}

/// At most `n` characters of `s`, sanitized. Every piece of text in a row,
/// the status or the message goes through here.
pub(crate) fn clip(s: &str, n: usize) -> String {
    sanitize(&s.chars().take(n).collect::<String>())
}

/// `s` clipped or space-padded to exactly `n` characters.
pub(crate) fn pad(s: &str, n: usize) -> String {
    let mut out = clip(s, n);
    let len = out.chars().count();
    out.extend(std::iter::repeat_n(' ', n - len));
    out
}

pub(crate) fn styled(s: &str, width: usize, style: Style) -> Row {
    vec![Span { text: clip(s, width), style }]
}
