use super::*;

fn at(text: &str, row: usize, col: usize) -> Buffer {
    let mut b = Buffer::from_text(text);
    b.set_cursor(row, col);
    b
}

#[test]
fn enter_after_an_opener_indents_one_more_level() {
    for opener in ["{", "(", "["] {
        let text = format!("  f{opener}");
        let mut b = at(&text, 0, 4);
        b.insert_newline_indent();
        assert_eq!(b.text(), format!("{text}\n      "));
        assert_eq!(b.cursor(), (1, 6));
    }
}

#[test]
fn trailing_spaces_after_the_opener_are_ignored() {
    let mut b = at("{  ", 0, 3);
    b.insert_newline_indent();
    assert_eq!(b.text(), "{  \n    ");
}

#[test]
fn a_tab_indented_line_gets_a_tab() {
    let mut b = at("\tfn x() {", 0, 9);
    b.insert_newline_indent();
    assert_eq!(b.text(), "\tfn x() {\n\t\t");
    assert_eq!(b.cursor(), (1, 2));
}

#[test]
fn between_a_pair_the_closer_moves_to_its_own_line() {
    let mut b = at("  {}", 0, 3);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  {\n      \n  }");
    assert_eq!(b.cursor(), (1, 6));
    // One undo step.
    b.undo();
    assert_eq!(b.text(), "  {}");
    assert_eq!(b.cursor(), (0, 3));
}

#[test]
fn a_mismatched_pair_is_not_split() {
    let mut b = at("{)", 0, 1);
    b.insert_newline_indent();
    assert_eq!(b.text(), "{\n    )");
}

#[test]
fn a_selection_or_a_closer_before_the_cursor_gets_plain_auto_indent() {
    let mut b = at("  a{\n", 0, 0);
    b.set_cursor(0, 4);
    b.move_left_select(true);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  a\n  \n");
    let mut b = at("  x}", 0, 4);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  x}\n  ");
}

#[test]
fn an_opener_after_the_cursor_does_not_count() {
    let mut b = at("  a {", 0, 3);
    b.insert_newline_indent();
    assert_eq!(b.text(), "  a\n   {");
}

#[test]
fn typing_a_closer_on_a_whitespace_only_row_dedents_one_level() {
    let mut b = at("    ", 0, 4);
    b.insert_closer('}');
    assert_eq!(b.text(), "}");
    assert_eq!(b.cursor(), (0, 1));
    let mut b = at("      ", 0, 6);
    b.insert_closer(')');
    assert_eq!(b.text(), "  )");
    let mut b = at("\t\t", 0, 2);
    b.insert_closer(']');
    assert_eq!(b.text(), "\t]");
}

#[test]
fn a_closer_after_text_or_without_indent_is_plain_typing() {
    let mut b = at("  a ", 0, 4);
    b.insert_closer('}');
    assert_eq!(b.text(), "  a }");
    let mut b = at("", 0, 0);
    b.insert_closer('}');
    assert_eq!(b.text(), "}");
    // Text after the cursor does not matter, only the row before it.
    let mut b = at("    x", 0, 4);
    b.insert_closer('}');
    assert_eq!(b.text(), "}x");
}

#[test]
fn a_dedenting_closer_is_one_undo_step_and_a_selection_is_replaced_plainly() {
    let mut b = at("    ", 0, 4);
    b.insert_closer('}');
    b.undo();
    assert_eq!(b.text(), "    ");
    assert_eq!(b.cursor(), (0, 4));
    let mut b = at("    abc", 0, 4);
    b.move_right_select(true);
    b.insert_closer('}');
    assert_eq!(b.text(), "    }bc");
}
