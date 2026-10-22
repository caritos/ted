use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(editor: &mut Editor, text: &str) {
    for c in text.chars() {
        editor.handle_key(key(KeyCode::Char(c)));
    }
}

fn ed(text: &str) -> Editor {
    Editor::new(Buffer::from_text(text))
}

fn many_lines(n: usize) -> String {
    (0..n).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
}

#[test]
fn ctrl_f_opens_find_prompt_in_message_row() {
    let mut e = ed("hello");
    assert!(!e.in_prompt());
    e.handle_key(ctrl('f'));
    assert!(e.in_prompt());
    assert_eq!(e.view(40, 5).message, "Find: _");
    type_str(&mut e, "ll");
    assert_eq!(e.view(40, 5).message, "Find: ll_");
    assert_eq!(e.buffer().text(), "hello");
    assert!(!e.buffer().is_modified());
}

#[test]
fn typing_searches_incrementally_and_scrolls_to_match() {
    let mut e = ed(&many_lines(100));
    e.view(40, 12);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "line 7");
    assert_eq!(e.buffer().cursor(), (7, 0));
    type_str(&mut e, "0");
    assert_eq!(e.buffer().cursor(), (70, 0));
    let v = e.view(40, 12);
    assert!(v.lines.iter().any(|l| l == "line 70"));
}

#[test]
fn incremental_search_restarts_from_origin_not_last_match() {
    let mut e = ed("ab\nxx\nabc\nabcd");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "ab");
    assert_eq!(e.buffer().cursor(), (0, 0));
    type_str(&mut e, "c");
    assert_eq!(e.buffer().cursor(), (2, 0));
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn backspace_to_empty_restores_origin() {
    let mut e = ed("one\ntwo\nthree");
    e.buffer.set_cursor(1, 1);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "three");
    assert_eq!(e.buffer().cursor(), (2, 0));
    for _ in 0..5 {
        e.handle_key(key(KeyCode::Backspace));
    }
    assert_eq!(e.buffer().cursor(), (1, 1));
    assert!(e.in_prompt());
}

#[test]
fn esc_restores_cursor_and_scroll() {
    let mut e = ed(&many_lines(100));
    e.view(40, 12);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "line 90");
    let moved = e.view(40, 12);
    assert!(moved.lines.iter().any(|l| l == "line 90"));
    e.handle_key(key(KeyCode::Esc));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (0, 0));
    let v = e.view(40, 12);
    assert_eq!(v.lines[0], "line 0");
    assert_eq!(v.message, "");
}

#[test]
fn esc_restores_horizontal_scroll() {
    let mut e = ed(&format!("{}\n{}needle", "a".repeat(100), " ".repeat(90)));
    e.view(20, 5);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "needle");
    assert_eq!(e.buffer().cursor(), (1, 90));
    assert!(e.view(20, 5).lines[1].ends_with('n'));
    e.handle_key(key(KeyCode::Esc));
    let v = e.view(20, 5);
    assert_eq!(v.lines[0], "a".repeat(20));
    assert_eq!(v.cursor, (0, 0));
}

#[test]
fn enter_keeps_match_and_scroll() {
    let mut e = ed(&many_lines(100));
    e.view(40, 12);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "line 90");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (90, 0));
    assert!(e.view(40, 12).lines.iter().any(|l| l == "line 90"));
    assert_eq!(e.buffer().text(), many_lines(100));
    assert!(!e.buffer().is_modified());
}

#[test]
fn down_and_ctrl_f_go_to_next_up_to_previous_with_wrap() {
    let mut e = ed("a1\nb\na2\nb\na3");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "a");
    assert_eq!(e.buffer().cursor(), (0, 0));
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.buffer().cursor(), (2, 0));
    e.handle_key(ctrl('f'));
    assert_eq!(e.buffer().cursor(), (4, 0));
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.buffer().cursor(), (0, 0));
    assert_eq!(e.view(40, 6).message, "Find: a (wrapped)");
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.buffer().cursor(), (4, 0));
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.buffer().cursor(), (2, 0));
    assert_eq!(e.view(40, 6).message, "Find: a_");
}

#[test]
fn single_match_next_lands_on_same_match() {
    let mut e = ed("x\nneedle\ny");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "needle");
    e.handle_key(key(KeyCode::Down));
    assert_eq!(e.buffer().cursor(), (1, 0));
    assert_eq!(e.view(40, 6).message, "Find: needle (wrapped)");
}

#[test]
fn typing_more_after_not_found_does_not_search_again() {
    let mut e = ed(&many_lines(50));
    e.handle_key(ctrl('f'));
    type_str(&mut e, "zzz");
    assert_eq!(e.view(40, 6).message, "Find: zzz (not found)");
    let before = crate::buffer::FINDS.with(|c| c.get());
    type_str(&mut e, "abc");
    assert_eq!(crate::buffer::FINDS.with(|c| c.get()), before);
    assert_eq!(e.view(40, 6).message, "Find: zzzabc (not found)");
    assert_eq!(e.buffer().cursor(), (0, 0));
    // Backspace can create a match again, so it does search.
    for _ in 0..6 {
        e.handle_key(key(KeyCode::Backspace));
    }
    type_str(&mut e, "line 4");
    assert_eq!(e.buffer().cursor(), (4, 0));
    assert_eq!(e.view(40, 6).message, "Find: line 4_");
}

#[test]
fn not_found_keeps_cursor_and_says_so() {
    let mut e = ed("abc");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "zzz");
    assert_eq!(e.buffer().cursor(), (0, 0));
    assert_eq!(e.view(40, 6).message, "Find: zzz (not found)");
    e.handle_key(key(KeyCode::Down));
    e.handle_key(key(KeyCode::Up));
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn smart_case_through_the_prompt() {
    let mut e = ed("foo\nFoo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "Foo");
    assert_eq!(e.buffer().cursor(), (1, 0));
}

#[test]
fn remembered_term_prefills_and_searches() {
    let mut e = ed("x\nfoo\ny\nfoo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().cursor(), (1, 0));
    e.handle_key(ctrl('f'));
    assert_eq!(e.view(40, 6).message, "Find: foo_");
    assert_eq!(e.buffer().cursor(), (1, 0));
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().cursor(), (1, 0));
}

#[test]
fn empty_enter_does_not_forget_term() {
    let mut e = ed("foo\nbar\nfoo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(ctrl('f'));
    for _ in 0..3 {
        e.handle_key(key(KeyCode::Backspace));
    }
    e.handle_key(key(KeyCode::Enter));
    e.handle_key(key(KeyCode::F(3)));
    assert_eq!(e.buffer().cursor(), (2, 0));
}

#[test]
fn f3_finds_next_and_reports_missing() {
    let mut e = ed("foo\nbar\nfoo");
    e.handle_key(key(KeyCode::F(3)));
    assert_eq!(e.message(), "No previous search");
    assert!(!e.in_prompt());
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().cursor(), (0, 0));
    e.handle_key(key(KeyCode::F(3)));
    assert_eq!(e.buffer().cursor(), (2, 0));
    e.handle_key(key(KeyCode::F(3)));
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn f3_not_found_message() {
    let mut e = ed("foo");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    e.handle_key(key(KeyCode::Enter));
    for _ in 0..3 {
        e.handle_key(key(KeyCode::Delete));
    }
    type_str(&mut e, "bar");
    e.handle_key(key(KeyCode::F(3)));
    assert_eq!(e.message(), "Not found: foo");
}

#[test]
fn goto_line_opens_accepts_digits_only_and_jumps() {
    let mut e = ed(&many_lines(50));
    e.handle_key(ctrl('l'));
    assert!(e.in_prompt());
    assert_eq!(e.view(40, 6).message, "Go to line: _");
    type_str(&mut e, "4x2-");
    assert_eq!(e.view(40, 6).message, "Go to line: 42_");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (41, 0));
    assert_eq!(e.buffer().text(), many_lines(50));
    assert!(!e.buffer().is_modified());
    assert!(e.view(40, 6).lines.iter().any(|l| l == "line 41"));
}

#[test]
fn goto_line_clamps_and_zero_or_empty_is_noop() {
    let mut e = ed("a\nb\nc");
    e.handle_key(ctrl('l'));
    type_str(&mut e, "999999999999999999999999");
    e.handle_key(key(KeyCode::Enter));
    assert_eq!(e.buffer().cursor(), (2, 0));
    e.buffer.set_cursor(1, 0);
    e.handle_key(ctrl('l'));
    type_str(&mut e, "0");
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (1, 0));
    e.handle_key(ctrl('l'));
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (1, 0));
}

#[test]
fn goto_line_backspace_and_esc() {
    let mut e = ed(&many_lines(50));
    e.view(40, 12);
    e.handle_key(ctrl('l'));
    type_str(&mut e, "30");
    e.handle_key(key(KeyCode::Backspace));
    assert_eq!(e.view(40, 6).message, "Go to line: 3_");
    e.handle_key(key(KeyCode::Esc));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (0, 0));
}

#[test]
fn prompt_swallows_ctrl_keys_and_never_edits() {
    let dir = std::env::temp_dir().join(format!("ted-prompt-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("file.txt");
    fs::write(&path, "abc\n").unwrap();
    let mut e = Editor::new(Buffer::open(&path).unwrap());
    e.handle_key(ctrl('f'));
    e.handle_key(ctrl('s'));
    e.handle_key(ctrl('x'));
    e.handle_key(key(KeyCode::Tab));
    e.handle_key(key(KeyCode::Delete));
    assert!(e.in_prompt());
    assert_eq!(e.message(), "");
    assert_eq!(e.buffer().text(), "abc\n");
    assert!(!e.buffer().is_modified());
    assert_eq!(e.view(40, 5).message, "Find: _");
    e.handle_key(ctrl('l'));
    assert_eq!(e.view(40, 5).message, "Find: _");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn prompt_does_not_clear_modified_or_set_it() {
    let mut e = ed("abc");
    e.handle_key(key(KeyCode::Char('x')));
    assert!(e.buffer().is_modified());
    e.handle_key(ctrl('f'));
    type_str(&mut e, "b");
    e.handle_key(key(KeyCode::Esc));
    assert!(e.buffer().is_modified());
}

#[test]
fn cancel_prompt_restores_like_esc() {
    let mut e = ed("a\nb\nneedle");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "needle");
    assert_eq!(e.buffer().cursor(), (2, 0));
    e.cancel_prompt();
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (0, 0));
    e.cancel_prompt();
}

#[test]
fn hostile_query_is_sanitized_in_message() {
    let mut e = ed("x");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "\x1b[31mred\u{9b}\t");
    let v = e.view(60, 5);
    assert!(!v.message.chars().any(|c| c.is_control()), "{:?}", v.message);
    assert!(v.message.starts_with("Find: ?[31mred?"));
}

#[test]
fn utf8_columns_are_char_indexes() {
    let mut e = ed("héllo wörld");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "wö");
    assert_eq!(e.buffer().cursor(), (0, 6));
}

/// 200 lines, cursor mid-viewport with scroll_row > 0; returns the editor.
fn scrolled_editor() -> Editor {
    let mut e = ed(&many_lines(200));
    e.view(40, 12);
    for _ in 0..3 {
        e.handle_key(key(KeyCode::PageDown));
    }
    e.view(40, 12);
    for _ in 0..4 {
        e.handle_key(key(KeyCode::Up));
    }
    e
}

/// One long line, cursor mid-line scrolled right, then a short line "needle".
fn scrolled_right_editor() -> Editor {
    let long = "abcdefghij".repeat(10);
    let mut e = ed(&format!("{long}\nneedle"));
    e.handle_key(key(KeyCode::End));
    e.view(20, 5);
    for _ in 0..10 {
        e.handle_key(key(KeyCode::Left));
    }
    e
}

#[test]
fn esc_restores_mid_viewport_origin_vertical_scroll() {
    let mut e = scrolled_editor();
    let before = e.view(40, 12);
    assert_eq!(e.buffer().cursor(), (23, 0));
    assert!(e.scroll_row > 0);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "line 150");
    assert_eq!(e.buffer().cursor(), (150, 0));
    e.view(40, 12);
    e.handle_key(key(KeyCode::Esc));
    let after = e.view(40, 12);
    assert_eq!(e.buffer().cursor(), (23, 0));
    assert_eq!(after.lines, before.lines);
    assert_eq!(after.cursor, before.cursor);
}

#[test]
fn esc_restores_mid_line_origin_horizontal_scroll() {
    let mut e = scrolled_right_editor();
    let before = e.view(20, 5);
    assert_eq!(e.buffer().cursor(), (0, 90));
    assert!(e.scroll_col > 0);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "needle");
    assert_eq!(e.buffer().cursor(), (1, 0));
    e.view(20, 5);
    e.handle_key(key(KeyCode::Esc));
    let after = e.view(20, 5);
    assert_eq!(e.buffer().cursor(), (0, 90));
    assert_eq!(after.lines, before.lines);
    assert_eq!(after.cursor, before.cursor);
}

#[test]
fn cancel_prompt_restores_mid_viewport_scroll() {
    let mut e = scrolled_editor();
    let before = e.view(40, 12);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "line 150");
    e.view(40, 12);
    e.cancel_prompt();
    let after = e.view(40, 12);
    assert_eq!(after.lines, before.lines);
    assert_eq!(after.cursor, before.cursor);

    let mut e = scrolled_right_editor();
    let before = e.view(20, 5);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "needle");
    e.view(20, 5);
    e.cancel_prompt();
    let after = e.view(20, 5);
    assert_eq!(after.lines, before.lines);
    assert_eq!(after.cursor, before.cursor);
}

#[test]
fn goto_esc_restores_mid_viewport_scroll() {
    let mut e = scrolled_editor();
    let before = e.view(40, 12);
    e.handle_key(ctrl('l'));
    type_str(&mut e, "150");
    e.handle_key(key(KeyCode::Esc));
    let after = e.view(40, 12);
    assert_eq!(after.lines, before.lines);
    assert_eq!(after.cursor, before.cursor);
}

#[test]
fn enter_on_not_found_restores_origin_but_remembers_term() {
    let mut e = ed("zzz\nxx\nab\nend");
    e.buffer.set_cursor(1, 0);
    e.handle_key(ctrl('f'));
    type_str(&mut e, "ab");
    assert_eq!(e.buffer().cursor(), (2, 0));
    type_str(&mut e, "z");
    assert_eq!(e.buffer().cursor(), (2, 0));
    e.handle_key(key(KeyCode::Enter));
    assert!(!e.in_prompt());
    assert_eq!(e.buffer().cursor(), (1, 0));
    e.handle_key(key(KeyCode::F(3)));
    assert_eq!(e.message(), "Not found: abz");
}

#[test]
fn find_on_lone_cr_buffer_has_valid_cursor_and_match_row() {
    let mut e = ed("a\rfoo\rbar");
    e.handle_key(ctrl('f'));
    type_str(&mut e, "bar");
    assert_eq!(e.buffer().cursor(), (2, 0));
    let v = e.view(20, 6);
    assert_eq!(v.lines[2], "bar");
    assert_eq!(v.cursor, (2, 0));
    type_str(&mut e, "x");
    assert_eq!(e.view(30, 6).message, "Find: barx (not found)");
    e.handle_key(key(KeyCode::Esc));
    e.handle_key(ctrl('f'));
    type_str(&mut e, "foo");
    assert_eq!(e.buffer().cursor(), (1, 0));
}

#[test]
fn prompt_sweeps_tiny_sizes_without_panic() {
    for w in 0..=6 {
        for h in 0..=6 {
            let mut e = ed("abc\nabd\r\nxyz");
            e.view(w, h);
            e.handle_key(ctrl('f'));
            type_str(&mut e, "ab");
            e.view(w, h);
            e.handle_key(key(KeyCode::Down));
            e.view(w, h);
            e.handle_key(key(KeyCode::Esc));
            e.view(w, h);
            e.handle_key(ctrl('l'));
            type_str(&mut e, "2");
            e.view(w, h);
            e.handle_key(key(KeyCode::Enter));
            e.view(w, h);
        }
    }
}
