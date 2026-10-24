use super::*;

fn at(text: &str, row: usize, col: usize) -> Buffer {
    let mut b = Buffer::from_text(text);
    b.set_cursor(row, col);
    b
}

#[test]
fn jumps_from_an_opening_bracket_to_its_partner() {
    let mut b = at("f(a, (b))", 0, 1);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 8));
}

#[test]
fn jumps_from_a_closing_bracket_back() {
    let mut b = at("f(a, (b))", 0, 8);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn nesting_is_respected() {
    let mut b = at("f(a, (b))", 0, 5);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 7));
}

#[test]
fn works_for_all_three_kinds_across_lines() {
    let mut b = at("fn x() {\n    [1,\n     2]\n}", 0, 7);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (3, 0));
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 7));
    b.set_cursor(1, 4);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (2, 6));
}

#[test]
fn other_kinds_inside_are_ignored() {
    let mut b = at("(a ] b } c)", 0, 0);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 10));
}

#[test]
fn a_bracket_just_before_the_cursor_counts() {
    // After typing `)` the cursor sits behind it.
    let mut b = at("(ab)", 0, 4);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 0));
}

#[test]
fn the_bracket_under_the_cursor_wins_over_the_one_before() {
    let mut b = at("()", 0, 1);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 0));
    let mut b = at(")(", 0, 1);
    assert!(!b.move_to_matching_bracket(false)); // `(` has no partner
    assert_eq!(b.cursor(), (0, 1));
}

#[test]
fn no_bracket_or_no_partner_does_nothing() {
    let mut b = at("abc", 0, 1);
    assert!(!b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 1));
    let mut b = at("(abc", 0, 0);
    assert!(!b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 0));
    let mut b = at("abc)", 0, 3);
    assert!(!b.move_to_matching_bracket(false));
    let mut b = Buffer::from_text("");
    assert!(!b.move_to_matching_bracket(false));
}

#[test]
fn select_extends_from_the_start_and_plain_clears() {
    let mut b = at("x(ab)y", 0, 1);
    assert!(b.move_to_matching_bracket(true));
    assert_eq!(b.cursor(), (0, 4));
    assert_eq!(b.selection(), Some(((0, 1), (0, 4))));
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.selection(), None);
}

#[test]
fn a_failed_jump_changes_nothing() {
    let mut b = at("(ab", 0, 0);
    assert!(!b.move_to_matching_bracket(true));
    assert_eq!(b.cursor(), (0, 0));
    assert_eq!(b.selection(), None);
}

#[test]
fn the_goal_column_follows_and_no_undo_step_is_made() {
    let mut b = at("(a)\nlonger line\nx", 0, 0);
    assert!(b.move_to_matching_bracket(false));
    assert_eq!(b.cursor(), (0, 2));
    b.move_down();
    assert_eq!(b.cursor(), (1, 2));
    assert!(!b.can_undo());
}
