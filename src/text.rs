//! Text helpers shared by the git view and the file tree.

use crate::screen::{Row, Span, Style};

/// Columns between tab stops; the Tab key inserts this many spaces too.
pub const TAB_STOP: usize = 4;

/// Display column where char index `char_col` of `line` starts: a tab
/// advances to the next tab stop, every other char takes one column. Chars
/// missing past the end of the line count one column each.
pub fn display_col(line: &str, char_col: usize) -> usize {
    let mut col = 0;
    let mut seen = 0;
    for c in line.chars().take(char_col) {
        col += if c == '\t' { TAB_STOP - col % TAB_STOP } else { 1 };
        seen += 1;
    }
    col + (char_col - seen)
}

/// For display columns `skip..skip + width` (stopping at the end of the
/// line) the char index of `line` drawn in each cell. A tab yields its own
/// index once per cell it covers, so a tab cut by the left edge shows only
/// its remaining cells. Walks no further than the last needed column.
pub fn layout(line: &str, skip: usize, width: usize) -> Vec<usize> {
    let end = skip.saturating_add(width);
    let mut cells = Vec::with_capacity(width.min(line.len()));
    let mut col = 0;
    for (i, c) in line.chars().enumerate() {
        if col >= end {
            break;
        }
        let w = if c == '\t' { TAB_STOP - col % TAB_STOP } else { 1 };
        let from = col.max(skip);
        let to = (col + w).min(end);
        cells.extend(std::iter::repeat_n(i, to.saturating_sub(from)));
        col += w;
    }
    cells
}

/// First row to show so that `selected` is inside a window of `height` rows.
pub(crate) fn window_start(selected: usize, height: usize) -> usize {
    (selected + 1).saturating_sub(height.max(1))
}

/// `s` safe to print: C0 controls, DEL and C1 controls become `?` so text
/// from git, file names or messages can never send escape sequences, carriage
/// returns and the like to the terminal. A tab (diff lines expand theirs
/// before this) becomes one space so the width stays one column.
pub fn sanitize(s: &str) -> String {
    s.chars().map(sanitize_char).collect()
}

/// One char of [`sanitize`]: always exactly one char out per char in, so
/// columns and per-char styles stay aligned with the text they describe.
pub fn sanitize_char(c: char) -> char {
    match c {
        '\t' => ' ',
        c if c.is_control() || is_invisible_format(c) => '?',
        c => c,
    }
}

/// Zero-width and bidirectional-control characters: they draw nothing, or
/// reorder the text around them, so what is shown would not match the file.
fn is_invisible_format(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidi_and_zero_width_chars_become_question_marks() {
        for c in ['\u{200B}', '\u{200E}', '\u{202E}', '\u{2066}', '\u{2069}', '\u{FEFF}'] {
            assert_eq!(sanitize_char(c), '?', "U+{:04X}", c as u32);
        }
        assert_eq!(sanitize("a\u{202E}b"), "a?b");
    }

    #[test]
    fn display_col_advances_tabs_to_the_next_stop() {
        assert_eq!([0, 1, 2, 3].map(|c| display_col("a\tb", c)), [0, 1, 4, 5]);
        assert_eq!(display_col("\t\tx", 2), 8);
        assert_eq!(display_col("abc", 5), 5);
        assert_eq!(display_col("a\t", 4), 6);
    }

    #[test]
    fn layout_maps_cells_to_source_chars() {
        assert_eq!(layout("a\tb", 0, 10), vec![0, 1, 1, 1, 2]);
        assert_eq!(layout("\tx", 2, 3), vec![0, 0, 1]);
        assert_eq!(layout("abc", 1, 1), vec![1]);
        assert_eq!(layout("abc", 0, 0), Vec::<usize>::new());
        assert_eq!(layout("", 0, 5), Vec::<usize>::new());
        assert_eq!(layout("abc", 5, 5), Vec::<usize>::new());
        assert_eq!(layout("\t\tx", 3, 3), vec![0, 1, 1]);
    }

    #[test]
    fn layout_of_a_huge_line_stops_at_the_width() {
        let line = "x".repeat(1_000_000);
        assert_eq!(layout(&line, 0, 10).len(), 10);
    }

    #[test]
    fn ordinary_text_is_untouched() {
        assert_eq!(sanitize("héllo 日本"), "héllo 日本");
        assert_eq!(sanitize("a\tb\x1b[2J"), "a b?[2J");
    }
}
