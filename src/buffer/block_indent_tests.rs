use super::*;

fn at(text: &str, row: usize, col: usize) -> Buffer {
    let mut b = Buffer::from_text(text);
    b.set_cursor(row, col);
    b
}

/// Selects from `from` to `to`, leaving the cursor at `to`.
fn select(b: &mut Buffer, from: (usize, usize), to: (usize, usize)) {
    b.set_cursor(to.0, to.1);
    b.restore_selection(Some(from));
}

#[test]
fn indent_adds_four_spaces_to_every_non_blank_row_of_the_block() {
    let mut b = at("a\n\nb\nc\n", 0, 0);
    select(&mut b, (0, 0), (2, 1));
    assert!(b.indent_lines());
    assert_eq!(b.text(), "    a\n\n    b\nc\n");
    assert_eq!(b.cursor(), (2, 5));
    assert_eq!(b.selection(), Some(((0, 4), (2, 5))));
}

#[test]
fn indent_without_a_selection_acts_on_the_cursor_row() {
    let mut b = at("a\nb\n", 1, 1);
    assert!(b.indent_lines());
    assert_eq!(b.text(), "a\n    b\n");
    assert_eq!(b.cursor(), (1, 5));
}

#[test]
fn indent_leaves_out_a_row_the_selection_only_touches_at_column_zero() {
    let mut b = at("a\nb\nc\n", 0, 0);
    select(&mut b, (0, 0), (2, 0));
    b.indent_lines();
    assert_eq!(b.text(), "    a\n    b\nc\n");
}

#[test]
fn indent_of_only_blank_rows_changes_nothing() {
    let mut b = at("\n\n", 0, 0);
    assert!(!b.indent_lines());
    assert_eq!(b.text(), "\n\n");
    assert!(!b.can_undo());
}

#[test]
fn indent_keeps_crlf_and_a_last_row_without_a_break() {
    let mut b = at("a\r\nb", 0, 0);
    select(&mut b, (0, 0), (1, 1));
    b.indent_lines();
    assert_eq!(b.text(), "    a\r\n    b");
}

#[test]
fn dedent_removes_up_to_four_spaces_or_one_tab() {
    let mut b = at("      a\n  b\n\tc\nd\n", 0, 0);
    select(&mut b, (0, 0), (3, 1));
    assert!(b.dedent_lines());
    assert_eq!(b.text(), "  a\nb\nc\nd\n");
}

#[test]
fn dedent_moves_the_cursor_back_but_not_before_the_text() {
    let mut b = at("    abc\n", 0, 6);
    b.dedent_lines();
    assert_eq!(b.cursor(), (0, 2));
    let mut b = at("  abc\n", 0, 1);
    b.dedent_lines();
    assert_eq!(b.text(), "abc\n");
    assert_eq!(b.cursor(), (0, 0));
}

#[test]
fn dedent_with_nothing_to_remove_changes_nothing() {
    let mut b = at("a\nb\n", 0, 0);
    select(&mut b, (0, 0), (1, 1));
    assert!(!b.dedent_lines());
    assert!(!b.can_undo());
}

#[test]
fn indent_and_dedent_are_one_undo_step_each_and_round_trip() {
    let mut b = at("a\nb\nc\n", 0, 0);
    select(&mut b, (0, 1), (2, 1));
    b.indent_lines();
    b.dedent_lines();
    assert_eq!(b.text(), "a\nb\nc\n");
    assert_eq!(b.selection(), Some(((0, 1), (2, 1))));
    b.undo();
    assert_eq!(b.text(), "    a\n    b\n    c\n");
    b.undo();
    assert_eq!(b.text(), "a\nb\nc\n");
}
