use super::*;

fn at(text: &str, row: usize, col: usize) -> Buffer {
    let mut b = Buffer::from_text(text);
    b.set_cursor(row, col);
    b
}

/// Cursor columns after each word move to the right until it stops.
fn rights(text: &str) -> Vec<(usize, usize)> {
    let mut b = at(text, 0, 0);
    let mut seen = Vec::new();
    loop {
        let before = b.cursor();
        b.move_word_right_select(false);
        if b.cursor() == before {
            return seen;
        }
        seen.push(b.cursor());
    }
}

fn lefts(text: &str, row: usize, col: usize) -> Vec<(usize, usize)> {
    let mut b = at(text, row, col);
    let mut seen = Vec::new();
    loop {
        let before = b.cursor();
        b.move_word_left_select(false);
        if b.cursor() == before {
            return seen;
        }
        seen.push(b.cursor());
    }
}

#[test]
fn right_stops_at_the_end_of_each_word() {
    assert_eq!(rights("foo bar_baz  qux"), vec![(0, 3), (0, 11), (0, 16)]);
}

#[test]
fn left_stops_at_the_start_of_each_word() {
    assert_eq!(lefts("foo bar_baz  qux", 0, 16), vec![(0, 13), (0, 4), (0, 0)]);
}

#[test]
fn punctuation_separates_words_and_is_skipped() {
    assert_eq!(rights("a.b::c"), vec![(0, 1), (0, 3), (0, 6)]);
    assert_eq!(lefts("a.b::c", 0, 6), vec![(0, 5), (0, 2), (0, 0)]);
}

#[test]
fn moves_cross_line_breaks() {
    assert_eq!(rights("foo\n  bar\n\nbaz"), vec![(0, 3), (1, 5), (3, 3)]);
    assert_eq!(lefts("foo\n  bar\n\nbaz", 3, 3), vec![(3, 0), (1, 2), (0, 0)]);
}

#[test]
fn unicode_letters_are_word_chars() {
    assert_eq!(rights("h\u{e9}llo w\u{f6}rld"), vec![(0, 5), (0, 11)]);
    assert_eq!(rights("\u{4e2d}\u{6587} x"), vec![(0, 2), (0, 4)]);
}

#[test]
fn nothing_moves_at_the_ends_and_in_an_empty_buffer() {
    let mut b = Buffer::from_text("");
    b.move_word_left_select(false);
    b.move_word_right_select(false);
    assert_eq!(b.cursor(), (0, 0));
    assert!(!b.is_modified());
    assert_eq!(rights("   "), vec![(0, 3)]);
}

#[test]
fn a_move_without_select_clears_the_selection_and_with_select_extends_it() {
    let mut b = at("foo bar baz", 0, 0);
    b.move_word_right_select(true);
    b.move_word_right_select(true);
    assert_eq!(b.selected_text().as_deref(), Some("foo bar"));
    b.move_word_left_select(true);
    assert_eq!(b.selected_text().as_deref(), Some("foo "));
    b.move_word_right_select(false);
    assert!(b.selection().is_none());
    assert_eq!(b.cursor(), (0, 7));
}

#[test]
fn up_and_down_use_the_column_the_word_move_ended_on() {
    let mut b = at("foo bar\nlonger line here", 0, 0);
    b.move_word_right_select(false);
    b.move_down();
    assert_eq!(b.cursor(), (1, 3));
}

#[test]
fn delete_word_left_is_one_undo_step() {
    let mut b = at("foo bar", 0, 7);
    b.delete_word_left();
    assert_eq!(b.text(), "foo ");
    assert_eq!(b.cursor(), (0, 4));
    b.delete_word_left();
    assert_eq!(b.text(), "");
    assert!(b.undo());
    assert_eq!(b.text(), "foo ");
    assert!(b.undo());
    assert_eq!(b.text(), "foo bar");
    assert_eq!(b.cursor(), (0, 7));
    assert!(!b.is_modified());
}

#[test]
fn delete_word_left_joins_lines_at_column_zero_and_stops_at_the_start() {
    let mut b = at("ab\ncd", 1, 0);
    b.delete_word_left();
    assert_eq!(b.text(), "cd");
    assert_eq!(b.cursor(), (0, 0));
    b.delete_word_left();
    assert_eq!(b.text(), "cd");
    assert!(b.undo());
    assert!(!b.undo(), "the second call changed nothing");
}

#[test]
fn delete_word_right_deletes_to_the_end_of_the_word() {
    let mut b = at("foo bar", 0, 0);
    b.delete_word_right();
    assert_eq!(b.text(), " bar");
    assert_eq!(b.cursor(), (0, 0));
    let mut b = at("x", 0, 1);
    b.delete_word_right();
    assert_eq!(b.text(), "x");
    assert!(!b.is_modified());
}

#[test]
fn deleting_a_word_with_a_selection_deletes_only_the_selection() {
    let mut b = at("foo bar baz", 0, 0);
    b.move_right_select(true);
    b.move_right_select(true);
    b.delete_word_left();
    assert_eq!(b.text(), "o bar baz");
    let mut b = at("foo bar baz", 0, 0);
    b.move_right_select(true);
    b.delete_word_right();
    assert_eq!(b.text(), "oo bar baz");
}
