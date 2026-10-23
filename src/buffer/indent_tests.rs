use super::*;

/// A buffer with the cursor at (row, col).
fn at(text: &str, row: usize, col: usize) -> Buffer {
    let mut b = Buffer::from_text(text);
    b.set_cursor(row, col);
    b
}

#[test]
fn enter_at_the_end_of_an_indented_line_copies_the_indent() {
    let mut b = at("    foo", 0, 7);
    b.insert_newline_indent();
    assert_eq!(b.text(), "    foo\n    ");
    assert_eq!(b.cursor(), (1, 4));
    assert!(b.is_modified());
}

#[test]
fn tabs_and_mixed_whitespace_are_copied_as_they_are() {
    let mut b = at("\t \tx", 0, 4);
    b.insert_newline_indent();
    assert_eq!(b.text(), "\t \tx\n\t \t");
    assert_eq!(b.cursor(), (1, 3));
}

#[test]
fn no_indent_means_a_plain_newline() {
    let mut b = at("foo", 0, 3);
    b.insert_newline_indent();
    assert_eq!(b.text(), "foo\n");
    assert_eq!(b.cursor(), (1, 0));
    let mut b = Buffer::from_text("");
    b.insert_newline_indent();
    assert_eq!(b.text(), "\n");
}

#[test]
fn splitting_a_line_indents_the_rest() {
    let mut b = at("  abcd", 0, 4);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  ab\n  cd");
    assert_eq!(b.cursor(), (1, 2));
}

#[test]
fn inside_the_indent_only_the_part_before_the_cursor_is_copied() {
    let mut b = at("    foo", 0, 2);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  \n    foo");
    assert_eq!(b.cursor(), (1, 2));
}

#[test]
fn at_column_zero_nothing_is_copied() {
    let mut b = at("  foo", 0, 0);
    b.insert_newline_indent();
    assert_eq!(b.text(), "\n  foo");
}

#[test]
fn uses_the_row_the_cursor_is_on() {
    let mut b = at("    a\nb\n  c", 2, 3);
    b.insert_newline_indent();
    assert_eq!(b.text(), "    a\nb\n  c\n  ");
}

#[test]
fn a_selection_is_replaced_and_its_start_row_gives_the_indent() {
    let mut b = at("  ab cd\nxyz", 0, 3);
    b.move_right_select(true);
    b.move_right_select(true);
    b.move_right_select(true);
    assert_eq!(b.selected_text().as_deref(), Some("b c"));
    b.insert_newline_indent();
    assert_eq!(b.text(), "  a\n  d\nxyz");
    assert_eq!(b.cursor(), (1, 2));
    assert!(b.selection().is_none());

    // Selected backwards: the start is the cursor.
    let mut b = at("  ab cd", 0, 6);
    for _ in 0..3 {
        b.move_left_select(true);
    }
    b.insert_newline_indent();
    assert_eq!(b.text(), "  a\n  d");
    assert_eq!(b.cursor(), (1, 2));
}

#[test]
fn one_undo_step_restores_text_and_cursor() {
    let mut b = at("    foo", 0, 7);
    b.insert_newline_indent();
    b.insert_char('x');
    assert!(b.undo());
    assert_eq!(b.text(), "    foo\n    ", "typing after Enter is its own step");
    assert!(b.undo());
    assert_eq!(b.text(), "    foo");
    assert_eq!(b.cursor(), (0, 7));
    assert!(!b.is_modified());
    assert!(b.redo());
    assert_eq!(b.text(), "    foo\n    ");
    assert_eq!(b.cursor(), (1, 4));
}

#[test]
fn a_line_ending_in_cr_lf_and_unicode_text_work() {
    let mut b = at("  \u{e9}\u{e9}\r\nx", 0, 4);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  \u{e9}\u{e9}\n  \r\nx");
    assert_eq!(b.cursor(), (1, 2));
}
