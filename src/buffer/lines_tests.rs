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

// ---- delete ----

#[test]
fn delete_line_removes_the_row_and_its_break() {
    let mut b = at("a\nb\nc\n", 1, 0);
    assert!(b.delete_lines());
    assert_eq!(b.text(), "a\nc\n");
    assert_eq!(b.cursor(), (1, 0));
}

#[test]
fn delete_line_keeps_the_column_when_it_fits() {
    let mut b = at("a\nbcdef\nxyz\n", 1, 2);
    b.delete_lines();
    assert_eq!(b.text(), "a\nxyz\n");
    assert_eq!(b.cursor(), (1, 2));
}

#[test]
fn delete_last_line_without_a_break_takes_the_break_before_it() {
    let mut b = at("a\nb\nc", 2, 1);
    b.delete_lines();
    assert_eq!(b.text(), "a\nb");
    assert_eq!(b.cursor(), (1, 1));
}

#[test]
fn delete_the_only_line_leaves_an_empty_buffer() {
    let mut b = at("only", 0, 2);
    b.delete_lines();
    assert_eq!(b.text(), "");
    assert_eq!(b.cursor(), (0, 0));
}

#[test]
fn delete_covers_every_row_a_selection_touches() {
    let mut b = at("a\nb\nc\nd\n", 0, 0);
    select(&mut b, (1, 1), (2, 1));
    b.delete_lines();
    assert_eq!(b.text(), "a\nd\n");
    assert_eq!(b.selection(), None);
}

#[test]
fn a_selection_ending_at_column_zero_leaves_that_row_alone() {
    let mut b = at("a\nb\nc\nd\n", 0, 0);
    select(&mut b, (1, 0), (3, 0));
    b.delete_lines();
    assert_eq!(b.text(), "a\nd\n");
}

#[test]
fn delete_is_one_undo_step_and_undo_restores_text_and_cursor() {
    let mut b = at("a\nb\nc\n", 1, 0);
    b.delete_lines();
    assert!(b.undo());
    assert_eq!(b.text(), "a\nb\nc\n");
    assert_eq!(b.cursor(), (1, 0));
    assert!(!b.is_modified());
}

#[test]
fn crlf_breaks_are_removed_whole() {
    let mut b = at("a\r\nb\r\nc", 1, 0);
    b.delete_lines();
    assert_eq!(b.text(), "a\r\nc");
}

// ---- duplicate ----

#[test]
fn duplicate_copies_the_row_below_and_the_cursor_follows() {
    let mut b = at("a\nbc\nd\n", 1, 1);
    assert!(b.duplicate_lines());
    assert_eq!(b.text(), "a\nbc\nbc\nd\n");
    assert_eq!(b.cursor(), (2, 1));
}

#[test]
fn duplicate_the_last_line_without_a_break_adds_one() {
    let mut b = at("a\nb", 1, 1);
    b.duplicate_lines();
    assert_eq!(b.text(), "a\nb\nb");
    assert_eq!(b.cursor(), (2, 1));
}

#[test]
fn duplicate_a_selected_block_copies_all_its_rows() {
    let mut b = at("a\nb\nc\n", 0, 0);
    select(&mut b, (0, 0), (1, 1));
    b.duplicate_lines();
    assert_eq!(b.text(), "a\nb\na\nb\nc\n");
    assert_eq!(b.cursor(), (3, 1));
    assert_eq!(b.selection_anchor(), Some((2, 0)), "the selection moves with the copy");
}

#[test]
fn duplicate_is_one_undo_step() {
    let mut b = at("a\nb\n", 0, 0);
    b.duplicate_lines();
    assert!(b.undo());
    assert_eq!(b.text(), "a\nb\n");
    assert_eq!(b.cursor(), (0, 0));
    assert!(!b.undo(), "nothing else to undo");
}

// ---- move ----

#[test]
fn move_down_swaps_with_the_next_row_and_the_cursor_follows() {
    let mut b = at("a\nbc\nd\n", 1, 1);
    assert!(b.move_lines_down());
    assert_eq!(b.text(), "a\nd\nbc\n");
    assert_eq!(b.cursor(), (2, 1));
}

#[test]
fn move_up_swaps_with_the_previous_row() {
    let mut b = at("a\nbc\nd\n", 1, 1);
    assert!(b.move_lines_up());
    assert_eq!(b.text(), "bc\na\nd\n");
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn moving_past_either_end_does_nothing() {
    let mut b = at("a\nb", 0, 0);
    assert!(!b.move_lines_up());
    assert_eq!(b.text(), "a\nb");
    b.set_cursor(1, 0);
    assert!(!b.move_lines_down());
    assert!(!b.is_modified());
}

#[test]
fn the_last_line_without_a_break_trades_places_cleanly() {
    let mut b = at("a\nb\nc", 2, 1);
    assert!(b.move_lines_up());
    assert_eq!(b.text(), "a\nc\nb");
    assert!(b.move_lines_down());
    assert_eq!(b.text(), "a\nb\nc");
    assert_eq!(b.cursor(), (2, 1));
}

#[test]
fn a_block_moves_together_and_keeps_its_selection() {
    let mut b = at("a\nb\nc\nd\n", 0, 0);
    select(&mut b, (1, 0), (2, 1));
    assert!(b.move_lines_up());
    assert_eq!(b.text(), "b\nc\na\nd\n");
    assert_eq!(b.selection_anchor(), Some((0, 0)));
    assert_eq!(b.cursor(), (1, 1));
    assert!(b.move_lines_down());
    assert_eq!(b.text(), "a\nb\nc\nd\n");
}

#[test]
fn moves_keep_each_rows_own_line_break() {
    let mut b = at("a\r\nb\nc", 0, 0);
    b.move_lines_down();
    assert_eq!(b.text(), "b\r\na\nc");
}

#[test]
fn a_move_is_one_undo_step_and_redo_puts_the_cursor_back() {
    let mut b = at("a\nbc\nd\n", 1, 1);
    b.move_lines_down();
    assert!(b.undo());
    assert_eq!(b.text(), "a\nbc\nd\n");
    assert_eq!(b.cursor(), (1, 1));
    assert!(b.redo());
    assert_eq!(b.text(), "a\nd\nbc\n");
    assert_eq!(b.cursor(), (2, 1));
}

// ---- comment ----

#[test]
fn comment_adds_the_prefix_at_the_cursor_row_indent() {
    let mut b = at("fn a() {\n    x();\n}\n", 1, 6);
    assert!(b.toggle_comment("//"));
    assert_eq!(b.text(), "fn a() {\n    // x();\n}\n");
    assert_eq!(b.cursor(), (1, 9));
}

#[test]
fn comment_again_removes_it_and_the_space() {
    let mut b = at("    // x();\n", 0, 9);
    assert!(b.toggle_comment("//"));
    assert_eq!(b.text(), "    x();\n");
    assert_eq!(b.cursor(), (0, 6));
}

#[test]
fn uncomment_also_accepts_a_prefix_without_a_space() {
    let mut b = at("//x\n", 0, 0);
    b.toggle_comment("//");
    assert_eq!(b.text(), "x\n");
}

#[test]
fn a_block_is_commented_at_its_shallowest_indent_and_blanks_are_skipped() {
    let mut b = at("    a\n\n  b\n      c\n", 0, 0);
    select(&mut b, (0, 0), (3, 3));
    b.toggle_comment("#");
    assert_eq!(b.text(), "  #   a\n\n  # b\n  #     c\n");
}

#[test]
fn a_block_is_uncommented_only_when_every_non_blank_row_is_commented() {
    let mut b = at("// a\n\n// b\n", 0, 0);
    select(&mut b, (0, 0), (2, 2));
    b.toggle_comment("//");
    assert_eq!(b.text(), "a\n\nb\n");
    let mut mixed = at("// a\nb\n", 0, 0);
    select(&mut mixed, (0, 0), (1, 1));
    mixed.toggle_comment("//");
    assert_eq!(mixed.text(), "// // a\n// b\n");
}

#[test]
fn only_blank_rows_change_nothing() {
    let mut b = at("\n  \n", 0, 0);
    select(&mut b, (0, 0), (1, 1));
    assert!(!b.toggle_comment("//"));
    assert_eq!(b.text(), "\n  \n");
    assert!(!b.is_modified());
}

#[test]
fn a_comment_toggle_is_one_undo_step() {
    let mut b = at("a\nb\n", 0, 0);
    select(&mut b, (0, 0), (1, 1));
    b.toggle_comment("//");
    assert!(b.undo());
    assert_eq!(b.text(), "a\nb\n");
    assert!(!b.is_modified());
}

#[test]
fn a_cursor_left_of_the_prefix_does_not_move() {
    let mut b = at("    x\n", 0, 1);
    b.toggle_comment("//");
    assert_eq!(b.text(), "    // x\n");
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn every_operation_survives_every_cursor_position_and_empty_text() {
    for text in ["", "\n", "a", "a\n", "\n\n", "a\r\nb", "é\tx\n  \n"] {
        let rows = Buffer::from_text(text).line_count();
        for row in 0..rows {
            for col in 0..4 {
                for op in 0..5 {
                    let mut b = at(text, row, col);
                    match op {
                        0 => drop(b.delete_lines()),
                        1 => drop(b.duplicate_lines()),
                        2 => drop(b.move_lines_up()),
                        3 => drop(b.move_lines_down()),
                        _ => drop(b.toggle_comment("//")),
                    }
                    let (r, c) = b.cursor();
                    assert!(r < b.line_count() && c <= b.line(r).chars().count(), "{text:?} {row},{col} op {op}");
                    b.undo();
                    assert_eq!(b.text(), text, "{text:?} {row},{col} op {op}");
                }
            }
        }
    }
}
